use axum::body::Body;
use axum::http::{Request, StatusCode};
use dlna_rs::api::{router, AppState};
use dlna_rs::library::Library;
use http_body_util::BodyExt;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn make_state(lib: Arc<RwLock<Library>>) -> AppState {
	AppState {
		lib,
		cfg_path: Arc::new(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
	}
}

#[tokio::test]
async fn list_folders_empty() {
	let lib = Arc::new(RwLock::new(Library::new()));
	let app = router(make_state(lib));
	let res = app
		.oneshot(
			Request::builder()
				.uri("/api/folders")
				.body(Body::empty())
				.unwrap(),
		)
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let body = res.into_body().collect().await.unwrap().to_bytes();
	assert_eq!(&body[..], b"[]");
}

#[tokio::test]
async fn add_then_list_folder() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	let app = router(make_state(lib.clone()));

	let body = serde_json::json!({ "path": dir.path().to_str().unwrap(), "recursive": true });
	let req = Request::builder()
		.method("POST")
		.uri("/api/folders")
		.header("content-type", "application/json")
		.body(Body::from(body.to_string()))
		.unwrap();
	let res = app.clone().oneshot(req).await.unwrap();
	assert_eq!(res.status(), StatusCode::CREATED);

	let res = app
		.oneshot(
			Request::builder()
				.uri("/api/folders")
				.body(Body::empty())
				.unwrap(),
		)
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
	assert_eq!(v.as_array().unwrap().len(), 1);
	assert_eq!(v[0]["file_count"], 1);
	assert_eq!(v[0]["recursive"], true);
}

#[tokio::test]
async fn delete_folder() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	lib.write()
		.await
		.add_folder(dir.path().to_path_buf(), true)
		.unwrap();
	let app = router(make_state(lib.clone()));
	let body = serde_json::json!({ "path": dir.path().to_str().unwrap() });
	let req = Request::builder()
		.method("DELETE")
		.uri("/api/folders")
		.header("content-type", "application/json")
		.body(Body::from(body.to_string()))
		.unwrap();
	let res = app.oneshot(req).await.unwrap();
	assert_eq!(res.status(), StatusCode::NO_CONTENT);
	assert_eq!(lib.read().await.folders().len(), 0);
}

#[tokio::test]
async fn status_endpoint_lists_folders() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	lib.write()
		.await
		.add_folder(dir.path().to_path_buf(), true)
		.unwrap();
	let app = router(make_state(lib));
	let res = app
		.oneshot(
			Request::builder()
				.uri("/status")
				.body(Body::empty())
				.unwrap(),
		)
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let s = std::str::from_utf8(&bytes).unwrap();
	assert!(s.contains("1 files"));
}

#[tokio::test]
async fn list_folders_does_not_include_file_list() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	std::fs::write(dir.path().join("b.mp4"), b"x").unwrap();

	let lib = Arc::new(RwLock::new(Library::new()));
	lib.write()
		.await
		.add_folder(dir.path().to_path_buf(), true)
		.unwrap();
	let app = router(make_state(lib));

	let res = app
		.oneshot(
			Request::builder()
				.uri("/api/folders")
				.body(Body::empty())
				.unwrap(),
		)
		.await
		.unwrap();
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
	assert!(
		v[0].get("files").is_none(),
		"folders list must not include file array: got {v}"
	);
}

#[tokio::test]
async fn list_files_for_configured_folder() {
	let dir = tempdir().unwrap();
	std::fs::create_dir(dir.path().join("sub")).unwrap();
	std::fs::write(dir.path().join(".DS_Store"), b"junk").unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"hello").unwrap();
	std::fs::write(dir.path().join("cover.jpg"), b"junk").unwrap();
	std::fs::write(dir.path().join("ca.strings"), b"junk").unwrap();
	std::fs::write(dir.path().join("sub/b.mp4"), b"world!").unwrap();
	std::fs::write(dir.path().join("sub/poster.jpg"), b"junk").unwrap();
	std::fs::write(dir.path().join("sub/metadata.tvdb"), b"junk").unwrap();

	let lib = Arc::new(RwLock::new(Library::new()));
	let canonical = lib
		.write()
		.await
		.add_folder(dir.path().to_path_buf(), true)
		.unwrap()
		.path;
	let app = router(make_state(lib));

	let uri = format!(
		"/api/files?path={}",
		urlencoding_simple(canonical.to_str().unwrap())
	);
	let res = app
		.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
	assert_eq!(v["folder"], canonical.to_string_lossy().as_ref());
	assert_eq!(v["recursive"], true);
	let files = v["files"].as_array().unwrap();
	assert_eq!(files.len(), 2);
	assert_eq!(files[0]["rel"], "a.mp4");
	assert_eq!(files[0]["size"], 5);
	assert_eq!(files[1]["rel"], "sub/b.mp4");
}

#[tokio::test]
async fn list_files_404_for_unknown_folder() {
	let lib = Arc::new(RwLock::new(Library::new()));
	let app = router(make_state(lib));

	let res = app
		.oneshot(
			Request::builder()
				.uri("/api/files?path=/nope/nada")
				.body(Body::empty())
				.unwrap(),
		)
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_files_400_without_path_param() {
	let lib = Arc::new(RwLock::new(Library::new()));
	let app = router(make_state(lib));

	let res = app
		.oneshot(
			Request::builder()
				.uri("/api/files")
				.body(Body::empty())
				.unwrap(),
		)
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

fn urlencoding_simple(s: &str) -> String {
	let mut out = String::new();
	for b in s.bytes() {
		match b {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
				out.push(b as char)
			}
			_ => out.push_str(&format!("%{:02X}", b)),
		}
	}
	out
}
