use dlna_rs::library::{Entry, FileMeta, Library};
use std::fs;
use tempfile::tempdir;

fn make_tree(root: &std::path::Path, files: &[&str]) {
	for f in files {
		let p = root.join(f);
		fs::create_dir_all(p.parent().unwrap()).unwrap();
		fs::write(&p, b"x").unwrap();
	}
}

#[test]
fn add_folder_counts_files_recursively() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4", "sub/b.mp4", "sub/deep/c.mp4"]);

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), true).unwrap();
	assert_eq!(f.file_count, 3);
	assert_eq!(lib.folders().len(), 1);
}

#[test]
fn add_folder_non_recursive_counts_only_top() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4", "sub/b.mp4"]);

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), false).unwrap();
	assert_eq!(f.file_count, 1);
}

#[test]
fn remove_folder_drops_it() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4"]);
	let mut lib = Library::new();
	lib.add_folder(dir.path().to_path_buf(), true).unwrap();
	assert!(lib.remove_folder(dir.path()));
	assert_eq!(lib.folders().len(), 0);
	assert!(!lib.remove_folder(dir.path()));
}

#[test]
fn id_allocation_is_stable() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4"]);
	let mut lib = Library::new();
	let id1 = lib.intern(Entry::File(dir.path().join("a.mp4")));
	let id2 = lib.intern(Entry::File(dir.path().join("a.mp4")));
	assert_eq!(id1, id2);
	assert_ne!(id1, 0);
}

#[test]
fn add_folder_populates_files_with_relative_paths_and_sizes() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4", "sub/b.mp4", "sub/deep/c.mp4"]);
	std::fs::write(dir.path().join("a.mp4"), b"hello").unwrap();

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), true).unwrap();

	assert_eq!(f.files.len(), 3);
	let rels: Vec<String> = f
		.files
		.iter()
		.map(|m| m.rel.to_string_lossy().into_owned())
		.collect();
	assert!(rels.contains(&"a.mp4".to_string()));
	assert!(rels.contains(&"sub/b.mp4".to_string()) || rels.contains(&"sub\\b.mp4".to_string()));
	let a = f
		.files
		.iter()
		.find(|m| m.rel.to_string_lossy() == "a.mp4")
		.unwrap();
	assert_eq!(a.size, 5);
}

#[test]
fn files_are_sorted_by_relative_path() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["z.mp4", "a.mp4", "m.mp4"]);

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), true).unwrap();

	let rels: Vec<String> = f
		.files
		.iter()
		.map(|m| m.rel.to_string_lossy().into_owned())
		.collect();
	assert_eq!(rels, vec!["a.mp4", "m.mp4", "z.mp4"]);
}

#[test]
fn non_recursive_files_only_top_level() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4", "sub/b.mp4"]);

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), false).unwrap();

	assert_eq!(f.files.len(), 1);
	assert_eq!(f.files[0].rel.to_string_lossy(), "a.mp4");
}
