use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
	File(PathBuf),
	Dir(PathBuf),
}

impl Entry {
	pub fn path(&self) -> &Path {
		match self {
			Entry::File(p) | Entry::Dir(p) => p,
		}
	}
	pub fn is_dir(&self) -> bool {
		matches!(self, Entry::Dir(_))
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
	pub path: PathBuf,
	pub recursive: bool,
	pub file_count: usize,
}

#[derive(Debug, Default)]
pub struct IdMap {
	forward: HashMap<usize, Entry>,
	reverse: HashMap<PathBuf, usize>,
	next: usize,
}

impl IdMap {
	pub fn intern(&mut self, entry: Entry) -> usize {
		if let Some(&id) = self.reverse.get(entry.path()) {
			return id;
		}
		self.next += 1;
		let id = self.next;
		self.reverse.insert(entry.path().to_path_buf(), id);
		self.forward.insert(id, entry);
		id
	}

	pub fn get(&self, id: usize) -> Option<&Entry> {
		self.forward.get(&id)
	}
}

#[derive(Debug, Default)]
pub struct Library {
	folders: Vec<Folder>,
	ids: IdMap,
}

impl Library {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn folders(&self) -> &[Folder] {
		&self.folders
	}
	pub fn ids(&self) -> &IdMap {
		&self.ids
	}

	pub fn intern(&mut self, entry: Entry) -> usize {
		self.ids.intern(entry)
	}

	pub fn add_folder(&mut self, path: PathBuf, recursive: bool) -> Result<Folder> {
		let canonical = path
			.canonicalize()
			.with_context(|| format!("canonicalize {}", path.display()))?;
		if self.folders.iter().any(|f| f.path == canonical) {
			anyhow::bail!("folder already added: {}", canonical.display());
		}
		let file_count = Self::count_files(&canonical, recursive);
		let folder = Folder {
			path: canonical,
			recursive,
			file_count,
		};
		self.folders.push(folder.clone());
		Ok(folder)
	}

	pub fn remove_folder(&mut self, path: &Path) -> bool {
		let target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
		let before = self.folders.len();
		self.folders.retain(|f| f.path != target);
		self.folders.len() != before
	}

	pub fn refresh_count(&mut self, path: &Path) -> Option<usize> {
		let target = path.canonicalize().ok()?;
		let f = self.folders.iter_mut().find(|f| f.path == target)?;
		f.file_count = Self::count_files(&f.path, f.recursive);
		Some(f.file_count)
	}

	fn count_files(root: &Path, recursive: bool) -> usize {
		let max_depth = if recursive { usize::MAX } else { 1 };
		walkdir::WalkDir::new(root)
			.max_depth(max_depth)
			.follow_links(false)
			.into_iter()
			.filter_map(|e| e.ok())
			.filter(|e| e.file_type().is_file())
			.count()
	}

	pub fn from_config(folders: &[crate::config::Folder]) -> Self {
		let mut lib = Self::new();
		for f in folders {
			let path = std::path::PathBuf::from(&f.path);
			if let Err(e) = lib.add_folder(path.clone(), f.recursive) {
				tracing::warn!(?path, "skipping folder: {e}");
			}
		}
		lib
	}

	pub fn to_config_folders(&self) -> Vec<crate::config::Folder> {
		self.folders
			.iter()
			.map(|f| crate::config::Folder {
				path: f.path.to_string_lossy().into_owned(),
				recursive: f.recursive,
			})
			.collect()
	}
}
