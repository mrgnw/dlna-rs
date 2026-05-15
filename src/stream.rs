use crate::api::AppState;
use crate::library::Entry;
use axum::{
	extract::{Path, State},
	http::{header, HeaderMap, HeaderValue, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Router,
};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

const CHUNK: usize = 64 * 1024;

pub fn router(state: AppState) -> Router {
	Router::new().route("/stream/:id", get(handle)).with_state(state)
}

async fn handle(
	State(state): State<AppState>,
	Path(id): Path<usize>,
	headers: HeaderMap,
) -> Response {
	let path: PathBuf = {
		let lib = state.lib.read().await;
		match lib.ids().get(id) {
			Some(Entry::File(p)) => p.clone(),
			Some(other) => return (StatusCode::NOT_FOUND, format!("not a file: {other:?}")).into_response(),
			None => return (StatusCode::NOT_FOUND, "unknown id").into_response(),
		}
	};
	let mut file = match tokio::fs::File::open(&path).await {
		Ok(f) => f,
		Err(_) => return (StatusCode::NOT_FOUND, "file gone").into_response(),
	};
	let total = match file.metadata().await {
		Ok(m) => m.len(),
		Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "stat failed").into_response(),
	};
	let mime = mime_guess::from_path(&path).first_or_octet_stream();

	let range_hdr = headers.get(header::RANGE).and_then(|h| h.to_str().ok());
	let parsed = range_hdr.and_then(|h| http_range_header::parse_range_header(h).ok());

	let (start, end_inclusive, status) = if let Some(parsed) = parsed {
		match parsed.validate(total) {
			Ok(ranges) => {
				let r = ranges.first().cloned();
				match r {
					Some(r) => (*r.start(), *r.end(), StatusCode::PARTIAL_CONTENT),
					None => return (StatusCode::RANGE_NOT_SATISFIABLE, "no ranges").into_response(),
				}
			}
			Err(_) => return (StatusCode::RANGE_NOT_SATISFIABLE, "invalid range").into_response(),
		}
	} else {
		(0u64, total.saturating_sub(1), StatusCode::OK)
	};

	if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
		return (StatusCode::INTERNAL_SERVER_ERROR, "seek failed").into_response();
	}
	let to_take = end_inclusive.saturating_sub(start) + 1;
	let limited = file.take(to_take);
	let stream = ReaderStream::with_capacity(limited, CHUNK);
	let body = axum::body::Body::from_stream(stream);

	let mut resp = Response::builder()
		.status(status)
		.header(header::ACCEPT_RANGES, "bytes")
		.header(header::CONTENT_TYPE, mime.essence_str())
		.header(header::CONTENT_LENGTH, to_take)
		.header("transferMode.dlna.org", "Streaming")
		.header("contentFeatures.dlna.org", "DLNA.ORG_OP=01;DLNA.ORG_CI=0");
	if status == StatusCode::PARTIAL_CONTENT {
		resp = resp.header(
			header::CONTENT_RANGE,
			HeaderValue::from_str(&format!("bytes {start}-{end_inclusive}/{total}")).unwrap(),
		);
	}
	resp.body(body).unwrap()
}
