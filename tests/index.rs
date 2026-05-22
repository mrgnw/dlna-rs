use dlna_rs::library::{index_folders, FileMeta, Library};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

// A folder whose scan blocks (a stuck filesystem syscall in the real world) must not
// prevent the other configured folders from being indexed.
#[tokio::test]
async fn a_blocked_folder_scan_does_not_block_other_folders() {
	let slow = tempfile::tempdir().unwrap();
	let fast = tempfile::tempdir().unwrap();
	let slow_canon = slow.path().canonicalize().unwrap();

	let mut lib = Library::new();
	// Add the slow folder first: a sequential scan would block here before reaching `fast`.
	lib.add_folder_unscanned(slow.path().to_path_buf(), true).unwrap();
	lib.add_folder_unscanned(fast.path().to_path_buf(), true).unwrap();
	let lib = Arc::new(RwLock::new(lib));

	let scan: Arc<dyn Fn(&Path, bool) -> Vec<FileMeta> + Send + Sync> = {
		let slow_canon = slow_canon.clone();
		Arc::new(move |path: &Path, _recursive: bool| {
			if path == slow_canon {
				std::thread::sleep(Duration::from_secs(10));
			}
			vec![FileMeta { rel: PathBuf::from("movie.mp4"), size: 1 }]
		})
	};

	tokio::spawn(index_folders(lib.clone(), scan));

	tokio::time::sleep(Duration::from_secs(1)).await;

	let fast_canon = fast.path().canonicalize().unwrap();
	let g = lib.read().await;
	let fast_folder = g
		.folders()
		.iter()
		.find(|f| f.path == fast_canon)
		.expect("fast folder present");
	assert_eq!(
		fast_folder.file_count, 1,
		"fast folder must be indexed within 1s even though the slow folder is still scanning"
	);
}
