use dlna_rs::browse::LibraryProvider;
use dlna_rs::library::Library;
use librqbit_upnp_serve::services::content_directory::{
	browse::response::ItemOrContainer,
	ContentDirectoryBrowseProvider,
};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;

#[tokio::test]
async fn provider_browses_inside_tokio_runtime() {
	let dir = tempdir().unwrap();
	std::fs::create_dir(dir.path().join("empty")).unwrap();
	std::fs::create_dir(dir.path().join("docs")).unwrap();
	std::fs::create_dir_all(dir.path().join("shows/season1")).unwrap();
	std::fs::write(dir.path().join("movie.mp4"), b"x").unwrap();
	std::fs::write(dir.path().join("docs/readme.txt"), b"x").unwrap();
	std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
	std::fs::write(dir.path().join("shows/season1/episode.mkv"), b"x").unwrap();

	let mut library = Library::new();
	library.add_folder(dir.path().to_path_buf(), true).unwrap();
	let provider = LibraryProvider(Arc::new(RwLock::new(library)));

	let root = provider.browse_direct_children(0, "127.0.0.1:9119");
	let folder_id = match root.as_slice() {
		[ItemOrContainer::Container(folder)] => folder.id,
		other => panic!("expected one root folder, got {other:?}"),
	};

	let children = provider.browse_direct_children(folder_id, "127.0.0.1:9119");
	assert_eq!(children.len(), 2);
	match &children[0] {
		ItemOrContainer::Item(item) => {
			assert_eq!(item.title, "movie.mp4");
			assert_eq!(item.mime_type.as_ref().map(|m| m.essence_str()), Some("video/mp4"));
		}
		other => panic!("expected media item, got {other:?}"),
	}
	match &children[1] {
		ItemOrContainer::Container(container) => assert_eq!(container.title, "shows"),
		other => panic!("expected media-bearing subdirectory, got {other:?}"),
	}
}
