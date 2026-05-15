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

pub type LibState = Arc<RwLock<Library>>;

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

pub fn router(lib: LibState) -> Router {
	Router::new()
		.route("/api/folders", get(list_folders).post(add_folder).delete(remove_folder))
		.route("/api/folders/refresh", post(refresh_folder))
		.route("/api/browse", get(browse))
		.route("/api/status", get(api_status))
		.route("/status", get(status_text))
		.with_state(lib)
}

async fn list_folders(State(lib): State<LibState>) -> Json<Vec<FolderView>> {
	Json(snapshot(&*lib.read().await))
}

async fn add_folder(State(lib): State<LibState>, Json(req): Json<AddFolderReq>) -> Response {
	let mut g = lib.write().await;
	match g.add_folder(PathBuf::from(&req.path), req.recursive) {
		Ok(f) => (StatusCode::CREATED, Json(FolderView {
			path: f.path.to_string_lossy().into_owned(),
			recursive: f.recursive,
			file_count: f.file_count,
		})).into_response(),
		Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
	}
}

async fn remove_folder(State(lib): State<LibState>, Json(req): Json<RemoveFolderReq>) -> StatusCode {
	if lib.write().await.remove_folder(&PathBuf::from(&req.path)) {
		StatusCode::NO_CONTENT
	} else {
		StatusCode::NOT_FOUND
	}
}

async fn refresh_folder(State(lib): State<LibState>, Json(req): Json<RemoveFolderReq>) -> Response {
	let mut g = lib.write().await;
	match g.refresh_count(&PathBuf::from(&req.path)) {
		Some(n) => Json(serde_json::json!({ "file_count": n })).into_response(),
		None => (StatusCode::NOT_FOUND, "folder not found").into_response(),
	}
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

async fn api_status(State(lib): State<LibState>) -> Json<serde_json::Value> {
	let g = lib.read().await;
	let folders = snapshot(&*g);
	Json(serde_json::json!({ "folders": folders }))
}

async fn status_text(State(lib): State<LibState>) -> String {
	let g = lib.read().await;
	let folders = snapshot(&*g);
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
