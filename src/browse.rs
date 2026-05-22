use crate::library::{clean_title, is_media_file, Entry, FileMeta, Library};
use librqbit_upnp_serve::services::content_directory::{
	browse::response::{Container, Item, ItemOrContainer},
	ContentDirectoryBrowseProvider,
};
use std::collections::{BTreeSet, HashMap};
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
			let folder_paths: Vec<_> = lib
				.folders()
				.iter()
				.filter(|f| folder_has_playable_files(f.path.as_path(), &f.files))
				.map(|f| f.path.clone())
				.collect();
			let duplicate_names = duplicate_source_names(&folder_paths);
			for folder_path in folder_paths {
				let children_count = playable_child_count(&lib, &folder_path);
				let id = lib.intern(Entry::Dir(folder_path.clone()));
				let title = source_folder_title(&folder_path, &duplicate_names);
				out.push(ItemOrContainer::Container(Container {
					id,
					parent_id: Some(0),
					children_count,
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
			Entry::Dir(p) => {
				if !dir_has_indexed_files(&lib, &p) {
					return Vec::new();
				}
				vec![ItemOrContainer::Container(Container {
					id: object_id,
					parent_id: Some(0),
					children_count: None,
					title: p
						.file_name()
						.map(|n| n.to_string_lossy().into_owned())
						.unwrap_or_default(),
				})]
			}
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
						.map(|n| clean_title(&n.to_string_lossy()))
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
		let path = folder_path.join(&file.rel);
		if !is_media_file(&path) {
			continue;
		}
		let Some(child) = child_under_dir(&base_rel, &file.rel) else {
			continue;
		};
		match child {
			Child::File => {
				let id = lib.intern(Entry::File(path.clone()));
				let mime = mime_guess::from_path(&path).first();
				let url = format!("http://{http_hostname}/stream/{id}");
				let title = path
					.file_name()
					.map(|n| clean_title(&n.to_string_lossy()))
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
		let children_count = playable_child_count(lib, &path);
		let id = lib.intern(Entry::Dir(path));
		out.push(ItemOrContainer::Container(Container {
			id,
			parent_id: Some(parent_id),
			children_count,
			title: name.to_string_lossy().into_owned(),
		}));
	}

	out
}

fn playable_child_count(lib: &Library, dir: &Path) -> Option<usize> {
	let (folder_path, files, base_rel) = folder_files_for_dir(lib, dir)?;
	let mut files_count = 0usize;
	let mut child_dirs = BTreeSet::new();
	for file in &files {
		let path = folder_path.join(&file.rel);
		if !is_media_file(&path) {
			continue;
		}
		match child_under_dir(&base_rel, &file.rel) {
			Some(Child::File) => files_count += 1,
			Some(Child::Dir(name)) => {
				child_dirs.insert(name);
			}
			None => {}
		}
	}
	Some(files_count + child_dirs.len())
}

fn folder_files_for_dir(lib: &Library, dir: &Path) -> Option<(PathBuf, Vec<FileMeta>, PathBuf)> {
	lib.folders().iter().find_map(|folder| {
		let base_rel = dir.strip_prefix(&folder.path).ok()?.to_path_buf();
		Some((folder.path.clone(), folder.files.clone(), base_rel))
	})
}

fn dir_has_indexed_files(lib: &Library, dir: &Path) -> bool {
	lib.folders().iter().any(|folder| {
		let Ok(base_rel) = dir.strip_prefix(&folder.path) else {
			return false;
		};
		folder.files.iter().any(|file| {
			file.rel.strip_prefix(base_rel).is_ok() && is_media_file(&folder.path.join(&file.rel))
		})
	})
}

fn folder_has_playable_files(folder_path: &Path, files: &[FileMeta]) -> bool {
	files
		.iter()
		.any(|file| is_media_file(&folder_path.join(&file.rel)))
}

fn duplicate_source_names(paths: &[PathBuf]) -> HashMap<String, usize> {
	let mut counts = HashMap::new();
	for path in paths {
		*counts
			.entry(source_folder_name(path).to_lowercase())
			.or_insert(0) += 1;
	}
	counts
}

fn source_folder_title(path: &Path, duplicate_names: &HashMap<String, usize>) -> String {
	let name = source_folder_name(path);
	if duplicate_names
		.get(&name.to_lowercase())
		.copied()
		.unwrap_or(0)
		< 2
	{
		return name;
	}
	match source_drive_name(path) {
		Some(drive) => format!("{name} ({drive})"),
		None => name,
	}
}

fn source_folder_name(path: &Path) -> String {
	path.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn source_drive_name(path: &Path) -> Option<String> {
	let mut components = path.components();
	while let Some(component) = components.next() {
		let Component::Normal(name) = component else {
			continue;
		};
		if name.eq_ignore_ascii_case("Volumes") {
			return components.next().and_then(component_name);
		}
	}
	path.parent()
		.and_then(Path::file_name)
		.map(|name| name.to_string_lossy().into_owned())
}

fn component_name(component: Component<'_>) -> Option<String> {
	match component {
		Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
		_ => None,
	}
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
