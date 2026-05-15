use axum::{
	extract::Path,
	http::{header, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Router,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub fn router() -> Router {
	Router::new()
		.route("/", get(|| serve("index.html".into())))
		.route("/{*path}", get(|Path(p): Path<String>| serve(p)))
}

async fn serve(path: String) -> Response {
	let candidate = if path.is_empty() { "index.html".to_string() } else { path };
	match Assets::get(&candidate) {
		Some(file) => {
			let mime = mime_guess::from_path(&candidate).first_or_octet_stream();
			(
				[(header::CONTENT_TYPE, mime.essence_str().to_string())],
				file.data.into_owned(),
			).into_response()
		}
		None => match Assets::get("index.html") {
			Some(file) => (
				[(header::CONTENT_TYPE, "text/html".to_string())],
				file.data.into_owned(),
			).into_response(),
			None => (StatusCode::NOT_FOUND, "web UI not built").into_response(),
		},
	}
}
