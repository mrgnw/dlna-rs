use crate::library::{Entry, Library};
use librqbit_upnp_serve::services::content_directory::{
	browse::response::{Container, Item, ItemOrContainer},
	ContentDirectoryBrowseProvider,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct LibraryProvider(pub Arc<RwLock<Library>>);

impl ContentDirectoryBrowseProvider for LibraryProvider {
	fn browse_direct_children(
		&self,
		parent_id: usize,
		http_hostname: &str,
	) -> Vec<ItemOrContainer> {
		let mut lib = self.0.blocking_write();
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
		let read = match std::fs::read_dir(&dir) {
			Ok(r) => r,
			Err(_) => return Vec::new(),
		};
		let mut out = Vec::new();
		for de in read.flatten() {
			let path = de.path();
			let name = de.file_name().to_string_lossy().into_owned();
			if name.starts_with('.') {
				continue;
			}
			let ft = match de.file_type() {
				Ok(f) => f,
				Err(_) => continue,
			};
			if ft.is_dir() {
				let id = lib.intern(Entry::Dir(path.clone()));
				out.push(ItemOrContainer::Container(Container {
					id,
					parent_id: Some(parent_id),
					children_count: None,
					title: name,
				}));
			} else if ft.is_file() {
				let meta = match de.metadata() {
					Ok(m) => m,
					Err(_) => continue,
				};
				let id = lib.intern(Entry::File(path.clone()));
				let mime = mime_guess::from_path(&path).first();
				let url = format!("http://{http_hostname}/stream/{id}");
				out.push(ItemOrContainer::Item(Item {
					id,
					parent_id,
					title: name,
					mime_type: mime,
					url,
					size: meta.len(),
				}));
			}
		}
		out
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
		let lib = self.0.blocking_read();
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
