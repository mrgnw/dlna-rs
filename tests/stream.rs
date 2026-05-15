use axum::body::Body;
use axum::http::{Request, StatusCode};
use dlna_rs::api::AppState;
use dlna_rs::library::{Entry, Library};
use dlna_rs::stream::router as stream_router;
use http_body_util::BodyExt;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;
use tower::ServiceExt;

#[tokio::test]
async fn full_get_returns_200() {
	let dir = tempdir().unwrap();
	let path = dir.path().join("a.bin");
	std::fs::write(&path, b"hello world").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	let id = lib.write().await.intern(Entry::File(path.clone()));
	let state = AppState { lib, cfg_path: Arc::new(dir.path().join("cfg.styx")) };
	let app = stream_router(state);
	let res = app.oneshot(Request::builder().uri(format!("/stream/{id}")).body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let headers = res.headers().clone();
	assert_eq!(headers["accept-ranges"], "bytes");
	assert_eq!(headers["content-length"], "11");
	assert_eq!(headers["transfermode.dlna.org"], "Streaming");
	let body = res.into_body().collect().await.unwrap().to_bytes();
	assert_eq!(&body[..], b"hello world");
}

#[tokio::test]
async fn range_get_returns_206() {
	let dir = tempdir().unwrap();
	let path = dir.path().join("a.bin");
	std::fs::write(&path, b"hello world").unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	let id = lib.write().await.intern(Entry::File(path.clone()));
	let state = AppState { lib, cfg_path: Arc::new(dir.path().join("cfg.styx")) };
	let app = stream_router(state);
	let req = Request::builder()
		.uri(format!("/stream/{id}"))
		.header("range", "bytes=6-10")
		.body(Body::empty()).unwrap();
	let res = app.oneshot(req).await.unwrap();
	assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
	assert_eq!(res.headers()["content-length"], "5");
	assert_eq!(res.headers()["content-range"], "bytes 6-10/11");
	let body = res.into_body().collect().await.unwrap().to_bytes();
	assert_eq!(&body[..], b"world");
}

#[tokio::test]
async fn unknown_id_returns_404() {
	let dir = tempdir().unwrap();
	let lib = Arc::new(RwLock::new(Library::new()));
	let state = AppState { lib, cfg_path: Arc::new(dir.path().join("cfg.styx")) };
	let app = stream_router(state);
	let res = app.oneshot(Request::builder().uri("/stream/999").body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
