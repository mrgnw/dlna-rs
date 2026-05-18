use crate::library::Library;
use axum::{
	extract::{Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
	routing::{get, post},
	Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
	pub lib: Arc<RwLock<Library>>,
	pub cfg_path: Arc<std::path::PathBuf>,
}

#[derive(Serialize)]
pub struct FolderView {
	pub path: String,
	pub recursive: bool,
	pub file_count: usize,
}

#[derive(Deserialize)]
pub struct AddFolderReq {
	pub path: String,
	pub recursive: bool,
}

#[derive(Deserialize)]
pub struct RemoveFolderReq {
	pub path: String,
}

#[derive(Deserialize)]
pub struct FilesQuery {
	pub path: Option<String>,
}

#[derive(Serialize)]
pub struct FileMetaView {
	pub rel: String,
	pub size: u64,
}

#[derive(Serialize)]
pub struct FilesResp {
	pub folder: String,
	pub recursive: bool,
	pub files: Vec<FileMetaView>,
}

#[derive(Deserialize)]
pub struct BrowseQuery {
	pub path: Option<String>,
}

#[derive(Serialize)]
pub struct BrowseEntry {
	pub name: String,
	pub is_dir: bool,
}

#[derive(Serialize)]
pub struct BrowseResp {
	pub path: String,
	pub parent: Option<String>,
	pub entries: Vec<BrowseEntry>,
}

pub fn router(state: AppState) -> Router {
	Router::new()
		.route("/api/folders", get(list_folders).post(add_folder).delete(remove_folder))
		.route("/api/folders/refresh", post(refresh_folder))
		.route("/api/files", get(list_files))
		.route("/api/browse", get(browse))
		.route("/api/status", get(api_status))
		.route("/status", get(status_text))
		.with_state(state)
}

async fn persist(state: &AppState) {
	let folders = {
		let g = state.lib.read().await;
		g.to_config_folders()
	};
	let mut cfg = crate::config::Config::load_or_default(&state.cfg_path)
		.unwrap_or_else(|_| crate::config::Config::with_defaults());
	cfg.folders = folders;
	if let Err(e) = cfg.save_atomic(&state.cfg_path) {
		tracing::warn!("save config: {e}");
	}
}

async fn list_folders(State(state): State<AppState>) -> Json<Vec<FolderView>> {
	Json(snapshot(&*state.lib.read().await))
}

async fn add_folder(State(state): State<AppState>, Json(req): Json<AddFolderReq>) -> Response {
	let mut g = state.lib.write().await;
	match g.add_folder(PathBuf::from(&req.path), req.recursive) {
		Ok(f) => {
			let view = FolderView {
				path: f.path.to_string_lossy().into_owned(),
				recursive: f.recursive,
				file_count: f.file_count,
			};
			drop(g);
			persist(&state).await;
			(StatusCode::CREATED, Json(view)).into_response()
		}
		Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
	}
}

async fn remove_folder(State(state): State<AppState>, Json(req): Json<RemoveFolderReq>) -> StatusCode {
	if state.lib.write().await.remove_folder(&PathBuf::from(&req.path)) {
		persist(&state).await;
		StatusCode::NO_CONTENT
	} else {
		StatusCode::NOT_FOUND
	}
}

async fn refresh_folder(State(state): State<AppState>, Json(req): Json<RemoveFolderReq>) -> Response {
	let mut g = state.lib.write().await;
	match g.refresh_count(&PathBuf::from(&req.path)) {
		Some(n) => {
			drop(g);
			persist(&state).await;
			Json(serde_json::json!({ "file_count": n })).into_response()
		}
		None => (StatusCode::NOT_FOUND, "folder not found").into_response(),
	}
}

async fn list_files(State(state): State<AppState>, Query(q): Query<FilesQuery>) -> Response {
	let raw = match q.path {
		Some(p) if !p.is_empty() => p,
		_ => return (StatusCode::BAD_REQUEST, "missing path query parameter").into_response(),
	};
	let target = match std::path::PathBuf::from(&raw).canonicalize() {
		Ok(p) => p,
		Err(_) => return (StatusCode::NOT_FOUND, "folder not configured").into_response(),
	};
	let g = state.lib.read().await;
	let folder = match g.folders().iter().find(|f| f.path == target) {
		Some(f) => f,
		None => return (StatusCode::NOT_FOUND, "folder not configured").into_response(),
	};
	let files: Vec<FileMetaView> = folder.files.iter()
		.map(|m| FileMetaView {
			rel: m.rel.to_string_lossy().into_owned(),
			size: m.size,
		})
		.collect();
	Json(FilesResp {
		folder: folder.path.to_string_lossy().into_owned(),
		recursive: folder.recursive,
		files,
	}).into_response()
}

async fn browse(Query(q): Query<BrowseQuery>) -> Response {
	let path = q.path
		.map(PathBuf::from)
		.unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/")));
	let path = match path.canonicalize() {
		Ok(p) => p,
		Err(e) => return (StatusCode::BAD_REQUEST, format!("canonicalize: {e}")).into_response(),
	};
	let mut entries: Vec<BrowseEntry> = match std::fs::read_dir(&path) {
		Ok(rd) => rd.filter_map(|e| e.ok())
			.filter_map(|e| {
				let ft = e.file_type().ok()?;
				let name = e.file_name().to_string_lossy().into_owned();
				if name.starts_with('.') { return None; }
				Some(BrowseEntry { name, is_dir: ft.is_dir() })
			})
			.collect(),
		Err(e) => return (StatusCode::BAD_REQUEST, format!("read_dir: {e}")).into_response(),
	};
	entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
	let parent = path.parent().map(|p| p.to_string_lossy().into_owned());
	Json(BrowseResp { path: path.to_string_lossy().into_owned(), parent, entries }).into_response()
}

async fn api_status(State(state): State<AppState>) -> Json<serde_json::Value> {
	let g = state.lib.read().await;
	let folders = snapshot(&g);
	Json(serde_json::json!({ "folders": folders }))
}

async fn status_text(State(state): State<AppState>) -> String {
	let g = state.lib.read().await;
	let folders = snapshot(&g);
	let mut s = String::new();
	for f in folders {
		s.push_str(&format!("{}  {} files\n", f.path, f.file_count));
	}
	if s.is_empty() { s.push_str("(no folders configured)\n"); }
	s
}

fn snapshot(lib: &Library) -> Vec<FolderView> {
	lib.folders().iter().map(|f| FolderView {
		path: f.path.to_string_lossy().into_owned(),
		recursive: f.recursive,
		file_count: f.file_count,
	}).collect()
}
