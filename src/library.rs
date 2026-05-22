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
pub struct FileMeta {
	pub rel: std::path::PathBuf,
	pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
	pub path: PathBuf,
	pub recursive: bool,
	pub file_count: usize,
	pub files: Vec<FileMeta>,
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
		let files = Self::scan_files(&canonical, recursive);
		let folder = Folder {
			path: canonical,
			recursive,
			file_count: files.len(),
			files,
		};
		self.folders.push(folder.clone());
		Ok(folder)
	}

	pub fn add_folder_unscanned(&mut self, path: PathBuf, recursive: bool) -> Result<Folder> {
		let canonical = path
			.canonicalize()
			.with_context(|| format!("canonicalize {}", path.display()))?;
		if self.folders.iter().any(|f| f.path == canonical) {
			anyhow::bail!("folder already added: {}", canonical.display());
		}
		let folder = Folder {
			path: canonical,
			recursive,
			file_count: 0,
			files: Vec::new(),
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
		let files = Self::scan_files(&f.path, f.recursive);
		f.file_count = files.len();
		f.files = files;
		Some(f.file_count)
	}

	pub fn scan_files(root: &Path, recursive: bool) -> Vec<FileMeta> {
		let max_depth = if recursive { usize::MAX } else { 1 };
		let mut out: Vec<FileMeta> = walkdir::WalkDir::new(root)
			.max_depth(max_depth)
			.follow_links(false)
			.into_iter()
			.filter_entry(should_walk_entry)
			.filter_map(|e| e.ok())
			.filter(|e| e.file_type().is_file())
			.filter(|e| is_media_file(e.path()))
			.filter_map(|e| {
				let rel = e.path().strip_prefix(root).ok()?.to_path_buf();
				let size = e.metadata().ok()?.len();
				Some(FileMeta { rel, size })
			})
			.collect();
		out.sort_by(|a, b| a.rel.cmp(&b.rel));
		out
	}

	pub fn replace_folder_files(&mut self, path: &Path, files: Vec<FileMeta>) -> Option<usize> {
		let f = self.folders.iter_mut().find(|f| f.path == path)?;
		f.file_count = files.len();
		f.files = files;
		Some(f.file_count)
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

	pub fn from_config_unscanned(folders: &[crate::config::Folder]) -> Self {
		let mut lib = Self::new();
		for f in folders {
			let path = std::path::PathBuf::from(&f.path);
			if let Err(e) = lib.add_folder_unscanned(path.clone(), f.recursive) {
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

pub fn is_media_file(path: &Path) -> bool {
	match mime_guess::from_path(path).first() {
		Some(mime) => mime.essence_str().starts_with("video/"),
		None => false,
	}
}

const MEDIA_EXTS: &[&str] = &[
	"mp4", "mkv", "avi", "mov", "m4v", "webm", "ts", "wmv", "flv", "mpg", "mpeg", "m2ts",
];

/// Turn a media filename into a clean display title.
///
/// Movies become `Title (Year)`, TV episodes keep their `SxxExx` marker, and
/// release-scene junk (resolution, codec, source, group tags) is dropped. Falls
/// back to the filename without its extension if cleaning would empty the title.
pub fn clean_title(file_name: &str) -> String {
	let stem = strip_media_ext(file_name);
	let normalized: String = stem
		.chars()
		.map(|c| if c == '.' || c == '_' { ' ' } else { c })
		.collect();
	let tokens: Vec<&str> = normalized.split_whitespace().collect();
	if tokens.is_empty() {
		return stem.to_string();
	}

	if let Some(idx) = tokens.iter().position(|t| is_tv_marker(t)) {
		if idx >= 1 {
			let title = tokens[..idx].join(" ");
			let marker = tokens[idx].to_ascii_uppercase();
			return non_empty_or_stem(format!("{title} {marker}"), stem);
		}
	}

	// Cut at the first release tag, but never at index 0 (keep at least one title token).
	let cut = tokens
		.iter()
		.enumerate()
		.find(|(i, t)| *i >= 1 && is_release_tag(t))
		.map(|(i, _)| i)
		.unwrap_or(tokens.len());
	let mut title_tokens = tokens[..cut].to_vec();

	let year = title_tokens.last().and_then(|t| parse_year(t));
	if year.is_some() {
		title_tokens.pop();
	}
	let mut result = title_tokens.join(" ");
	if let Some(y) = year {
		result = if result.is_empty() {
			format!("({y})")
		} else {
			format!("{result} ({y})")
		};
	}
	non_empty_or_stem(result, stem)
}

fn strip_media_ext(name: &str) -> &str {
	if let Some((stem, ext)) = name.rsplit_once('.') {
		if MEDIA_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
			return stem;
		}
	}
	name
}

fn non_empty_or_stem(result: String, stem: &str) -> String {
	if result.trim().is_empty() {
		stem.to_string()
	} else {
		result
	}
}

fn is_tv_marker(tok: &str) -> bool {
	let b = tok.as_bytes();
	let mut i = 0;
	if b.first() != Some(&b's') && b.first() != Some(&b'S') {
		return false;
	}
	i += 1;
	let s_start = i;
	while i < b.len() && b[i].is_ascii_digit() {
		i += 1;
	}
	if i == s_start || i - s_start > 2 {
		return false;
	}
	if b.get(i) != Some(&b'e') && b.get(i) != Some(&b'E') {
		return false;
	}
	i += 1;
	let e_start = i;
	while i < b.len() && b[i].is_ascii_digit() {
		i += 1;
	}
	i != e_start && i - e_start <= 3 && i == b.len()
}

fn parse_year(tok: &str) -> Option<u32> {
	let trimmed = tok.trim_start_matches('(').trim_end_matches(')');
	if trimmed.len() != 4 || !trimmed.bytes().all(|c| c.is_ascii_digit()) {
		return None;
	}
	let year: u32 = trimmed.parse().ok()?;
	(1900..=2099).contains(&year).then_some(year)
}

fn is_release_tag(tok: &str) -> bool {
	let lower = tok.to_ascii_lowercase();
	let head = lower.split('-').next().unwrap_or(&lower);
	if head.len() >= 3
		&& head.ends_with('p')
		&& head[..head.len() - 1].bytes().all(|c| c.is_ascii_digit())
	{
		return true;
	}
	const TAGS: &[&str] = &[
		"web", "webrip", "webdl", "hdtv", "bluray", "bdrip", "brrip", "dvdrip", "dvd", "hdrip",
		"hdcam", "cam", "ts", "telesync", "remux", "uhd", "4k", "x264", "x265", "h264", "h265",
		"hevc", "avc", "xvid", "divx", "aac", "ac3", "eac3", "dts", "ddp", "ddp5", "dd5", "truehd",
		"atmos", "flac", "mp3", "dual", "multi", "hdr", "hdr10", "dv", "sdr", "10bit", "8bit",
		"proper", "repack", "extended", "unrated", "internal", "limited", "imax", "remastered",
	];
	TAGS.contains(&head) || TAGS.contains(&lower.as_str())
}

/// Index every configured folder, replacing each folder's cached file list as its
/// scan completes. `scan` is injected so tests can substitute a slow/blocking scan.
pub async fn index_folders(
	lib: std::sync::Arc<tokio::sync::RwLock<Library>>,
	scan: std::sync::Arc<dyn Fn(&Path, bool) -> Vec<FileMeta> + Send + Sync>,
) {
	let folders = {
		let g = lib.read().await;
		g.folders().to_vec()
	};

	let mut tasks = Vec::with_capacity(folders.len());
	for folder in folders {
		let lib = lib.clone();
		let scan = scan.clone();
		tasks.push(tokio::spawn(async move {
			let path = folder.path.clone();
			let recursive = folder.recursive;
			tracing::info!(path = %path.display(), recursive, "indexing folder");
			let scan_path = path.clone();
			let files =
				match tokio::task::spawn_blocking(move || scan(&scan_path, recursive)).await {
					Ok(files) => files,
					Err(e) => {
						tracing::warn!(path = %path.display(), "index task failed: {e}");
						return;
					}
				};
			let n = files.len();
			let mut g = lib.write().await;
			if g.replace_folder_files(&path, files).is_some() {
				tracing::info!(path = %path.display(), files = n, "indexed folder");
			}
		}));
	}

	for task in tasks {
		let _ = task.await;
	}
}

fn should_walk_entry(entry: &walkdir::DirEntry) -> bool {
	if entry.depth() == 0 || !entry.file_type().is_dir() {
		return true;
	}
	let name = entry.file_name().to_string_lossy();
	if name.starts_with('.') || name.starts_with('@') {
		return false;
	}
	let lowercase = name.to_lowercase();
	if lowercase.contains("trickplay") || lowercase.contains("cover art") {
		return false;
	}
	let path = entry.path();
	if is_apple_tv_app_container(path) {
		return false;
	}
	!matches!(
		path.extension().and_then(|ext| ext.to_str()),
		Some("tvlibrary" | "photoslibrary" | "musiclibrary")
	)
}

fn is_apple_tv_app_container(path: &Path) -> bool {
	if path.file_name().and_then(|name| name.to_str()) != Some("TV") {
		return false;
	}
	path.parent()
		.and_then(|parent| parent.file_name())
		.and_then(|name| name.to_str())
		== Some("Movies")
}
