use axum::body::Body;
use axum::http::{Request, StatusCode};
use dlna_rs::api::router;
use dlna_rs::library::Library;
use http_body_util::BodyExt;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;
use tower::ServiceExt;

#[tokio::test]
async fn list_folders_empty() {
	let lib = Arc::new(RwLock::new(Library::new()));
	let app = router(lib);
	let res = app.oneshot(Request::builder().uri("/api/folders").body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let body = res.into_body().collect().await.unwrap().to_bytes();
	assert_eq!(&body[..], b"[]");
}

#[tokio::test]
async fn add_then_list_folder() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	let app = router(lib.clone());

	let body = serde_json::json!({ "path": dir.path().to_str().unwrap(), "recursive": true });
	let req = Request::builder()
		.method("POST")
		.uri("/api/folders")
		.header("content-type", "application/json")
		.body(Body::from(body.to_string())).unwrap();
	let res = app.clone().oneshot(req).await.unwrap();
	assert_eq!(res.status(), StatusCode::CREATED);

	let res = app.oneshot(Request::builder().uri("/api/folders").body(Body::empty()).unwrap())
		.await.unwrap();
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
	lib.write().await.add_folder(dir.path().to_path_buf(), true).unwrap();
	let app = router(lib.clone());
	let body = serde_json::json!({ "path": dir.path().to_str().unwrap() });
	let req = Request::builder()
		.method("DELETE")
		.uri("/api/folders")
		.header("content-type", "application/json")
		.body(Body::from(body.to_string())).unwrap();
	let res = app.oneshot(req).await.unwrap();
	assert_eq!(res.status(), StatusCode::NO_CONTENT);
	assert_eq!(lib.read().await.folders().len(), 0);
}

#[tokio::test]
async fn status_endpoint_lists_folders() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	lib.write().await.add_folder(dir.path().to_path_buf(), true).unwrap();
	let app = router(lib);
	let res = app.oneshot(Request::builder().uri("/status").body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let s = std::str::from_utf8(&bytes).unwrap();
	assert!(s.contains("1 files"));
}
