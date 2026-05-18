use crate::library::{is_media_file, Entry, FileMeta, Library};
use librqbit_upnp_serve::services::content_directory::{
	browse::response::{Container, Item, ItemOrContainer},
	ContentDirectoryBrowseProvider,
};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct LibraryProvider(pub Arc<RwLock<Library>>);

impl ContentDirectoryBrowseProvider for LibraryProvider {
	fn browse_direct_children(
		&self,
		parent_id: usize,
		http_hostname: &str,
	) -> Vec<ItemOrContainer> {
		let Ok(mut lib) = self.0.try_write() else {
			return Vec::new();
		};
		if parent_id == 0 {
			let mut out = Vec::new();
			let folder_paths: Vec<_> = lib.folders().iter().map(|f| f.path.clone()).collect();
			for folder_path in folder_paths {
				let id = lib.intern(Entry::Dir(folder_path.clone()));
				let title = folder_path
					.file_name()
					.map(|n| n.to_string_lossy().into_owned())
					.unwrap_or_else(|| folder_path.to_string_lossy().into_owned());
				out.push(ItemOrContainer::Container(Container {
					id,
					parent_id: Some(0),
					children_count: None,
					title,
				}));
			}
			return out;
		}
		let entry = match lib.ids().get(parent_id) {
			Some(e) => e.clone(),
			None => return Vec::new(),
		};
		let dir = match entry {
			Entry::Dir(p) => p,
			Entry::File(_) => return Vec::new(),
		};
		children_for_dir(&mut lib, &dir, parent_id, http_hostname)
	}

	fn browse_metadata(&self, object_id: usize, http_hostname: &str) -> Vec<ItemOrContainer> {
		if object_id == 0 {
			return vec![ItemOrContainer::Container(Container {
				id: 0,
				parent_id: None,
				children_count: None,
				title: "dlna-rs".to_string(),
			})];
		}
		let Ok(lib) = self.0.try_read() else {
			return Vec::new();
		};
		let entry = match lib.ids().get(object_id) {
			Some(e) => e.clone(),
			None => return Vec::new(),
		};
		match entry {
			Entry::Dir(p) => vec![ItemOrContainer::Container(Container {
				id: object_id,
				parent_id: Some(0),
				children_count: None,
				title: p
					.file_name()
					.map(|n| n.to_string_lossy().into_owned())
					.unwrap_or_default(),
			})],
			Entry::File(p) => {
				if !is_media_file(&p) {
					return Vec::new();
				}
				let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
				let mime = mime_guess::from_path(&p).first();
				let url = format!("http://{http_hostname}/stream/{object_id}");
				vec![ItemOrContainer::Item(Item {
					id: object_id,
					parent_id: 0,
					title: p
						.file_name()
						.map(|n| n.to_string_lossy().into_owned())
						.unwrap_or_default(),
					mime_type: mime,
					url,
					size,
				})]
			}
		}
	}
}

fn children_for_dir(
	lib: &mut Library,
	dir: &Path,
	parent_id: usize,
	http_hostname: &str,
) -> Vec<ItemOrContainer> {
	let Some((folder_path, files, base_rel)) = folder_files_for_dir(lib, dir) else {
		return Vec::new();
	};

	let mut out = Vec::new();
	let mut child_dirs = BTreeSet::new();

	for file in files {
		let Some(child) = child_under_dir(&base_rel, &file.rel) else {
			continue;
		};
		match child {
			Child::File => {
				let path = folder_path.join(&file.rel);
				let id = lib.intern(Entry::File(path.clone()));
				let mime = mime_guess::from_path(&path).first();
				let url = format!("http://{http_hostname}/stream/{id}");
				let title = path
					.file_name()
					.map(|n| n.to_string_lossy().into_owned())
					.unwrap_or_default();
				out.push(ItemOrContainer::Item(Item {
					id,
					parent_id,
					title,
					mime_type: mime,
					url,
					size: file.size,
				}));
			}
			Child::Dir(name) => {
				child_dirs.insert(name);
			}
		}
	}

	for name in child_dirs {
		let path = dir.join(&name);
		let id = lib.intern(Entry::Dir(path));
		out.push(ItemOrContainer::Container(Container {
			id,
			parent_id: Some(parent_id),
			children_count: None,
			title: name.to_string_lossy().into_owned(),
		}));
	}

	out
}

fn folder_files_for_dir(
	lib: &Library,
	dir: &Path,
) -> Option<(PathBuf, Vec<FileMeta>, PathBuf)> {
	lib.folders()
		.iter()
		.find_map(|folder| {
			let base_rel = dir.strip_prefix(&folder.path).ok()?.to_path_buf();
			Some((folder.path.clone(), folder.files.clone(), base_rel))
		})
}

enum Child {
	File,
	Dir(PathBuf),
}

fn child_under_dir(base_rel: &Path, file_rel: &Path) -> Option<Child> {
	let rel = file_rel.strip_prefix(base_rel).ok()?;
	let mut components = rel.components();
	let first = components.next()?;
	if components.next().is_none() {
		return Some(Child::File);
	}
	match first {
		Component::Normal(name) => Some(Child::Dir(PathBuf::from(name))),
		_ => None,
	}
}
