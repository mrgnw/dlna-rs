use dlna_rs::browse::LibraryProvider;
use dlna_rs::library::{Entry, Library};
use librqbit_upnp_serve::services::content_directory::{
	browse::response::ItemOrContainer, ContentDirectoryBrowseProvider,
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
			assert_eq!(item.title, "movie");
			assert_eq!(
				item.mime_type.as_ref().map(|m| m.essence_str()),
				Some("video/mp4")
			);
		}
		other => panic!("expected media item, got {other:?}"),
	}
	match &children[1] {
		ItemOrContainer::Container(container) => assert_eq!(container.title, "shows"),
		other => panic!("expected media-bearing subdirectory, got {other:?}"),
	}
}

#[tokio::test]
async fn provider_reports_child_counts_on_containers() {
	let dir = tempdir().unwrap();
	std::fs::create_dir(dir.path().join("empty")).unwrap();
	std::fs::create_dir(dir.path().join("docs")).unwrap();
	std::fs::create_dir_all(dir.path().join("shows/season1")).unwrap();
	std::fs::write(dir.path().join("movie.mp4"), b"x").unwrap();
	std::fs::write(dir.path().join("docs/readme.txt"), b"x").unwrap();
	std::fs::write(dir.path().join("shows/season1/episode.mkv"), b"x").unwrap();

	let mut library = Library::new();
	library.add_folder(dir.path().to_path_buf(), true).unwrap();
	let provider = LibraryProvider(Arc::new(RwLock::new(library)));

	let root = provider.browse_direct_children(0, "127.0.0.1:9119");
	let folder = match root.as_slice() {
		[ItemOrContainer::Container(folder)] => folder.clone(),
		other => panic!("expected one root folder, got {other:?}"),
	};
	// movie.mp4 + shows/ are playable; empty/ and docs/ are not.
	assert_eq!(folder.children_count, Some(2));

	let children = provider.browse_direct_children(folder.id, "127.0.0.1:9119");
	let shows = children
		.iter()
		.find_map(|c| match c {
			ItemOrContainer::Container(c) if c.title == "shows" => Some(c.clone()),
			_ => None,
		})
		.expect("shows container");
	assert_eq!(shows.children_count, Some(1));
}

#[tokio::test]
async fn provider_hides_configured_folders_without_playable_videos() {
	let with_video = tempdir().unwrap();
	let images_only = tempdir().unwrap();
	std::fs::write(with_video.path().join("movie.mp4"), b"x").unwrap();
	std::fs::write(images_only.path().join("poster.jpg"), b"x").unwrap();

	let mut library = Library::new();
	library
		.add_folder(with_video.path().to_path_buf(), true)
		.unwrap();
	library
		.add_folder(images_only.path().to_path_buf(), true)
		.unwrap();
	let provider = LibraryProvider(Arc::new(RwLock::new(library)));

	let root = provider.browse_direct_children(0, "127.0.0.1:9119");
	assert_eq!(root.len(), 1);
	match &root[0] {
		ItemOrContainer::Container(container) => {
			assert_eq!(
				container.title,
				with_video.path().file_name().unwrap().to_string_lossy()
			);
		}
		other => panic!("expected one playable source folder, got {other:?}"),
	}
}

#[tokio::test]
async fn provider_disambiguates_matching_source_folder_names() {
	let dir = tempdir().unwrap();
	let drive_a = dir.path().join("DriveA");
	let drive_b = dir.path().join("DriveB");
	let movies_a = drive_a.join("Movies");
	let movies_b = drive_b.join("movies");
	std::fs::create_dir_all(&movies_a).unwrap();
	std::fs::create_dir_all(&movies_b).unwrap();
	std::fs::write(movies_a.join("a.mp4"), b"x").unwrap();
	std::fs::write(movies_b.join("b.mkv"), b"x").unwrap();

	let mut library = Library::new();
	library.add_folder(movies_a, true).unwrap();
	library.add_folder(movies_b, true).unwrap();
	let provider = LibraryProvider(Arc::new(RwLock::new(library)));

	let titles: Vec<String> = provider
		.browse_direct_children(0, "127.0.0.1:9119")
		.into_iter()
		.map(|entry| match entry {
			ItemOrContainer::Container(container) => container.title,
			other => panic!("expected source folder container, got {other:?}"),
		})
		.collect();
	assert_eq!(titles, vec!["Movies (DriveA)", "movies (DriveB)"]);
}

#[tokio::test]
async fn provider_rejects_metadata_for_cached_non_video_directories() {
	let dir = tempdir().unwrap();
	let metadata_dir = dir.path().join("@nfo and other stuff");
	let cover_dir = metadata_dir.join("cover art images from Plex");
	std::fs::create_dir_all(&cover_dir).unwrap();
	std::fs::write(dir.path().join("movie.mp4"), b"x").unwrap();
	std::fs::write(cover_dir.join("poster.jpg"), b"x").unwrap();

	let mut library = Library::new();
	library.add_folder(dir.path().to_path_buf(), true).unwrap();
	let metadata_id = library.intern(Entry::Dir(metadata_dir));
	let cover_id = library.intern(Entry::Dir(cover_dir));
	let provider = LibraryProvider(Arc::new(RwLock::new(library)));

	assert!(provider
		.browse_metadata(metadata_id, "127.0.0.1:9119")
		.is_empty());
	assert!(provider
		.browse_metadata(cover_id, "127.0.0.1:9119")
		.is_empty());
}
