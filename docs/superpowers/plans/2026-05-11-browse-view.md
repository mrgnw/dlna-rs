# Browse view + folder list redesign — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax. Repo enforces TABS for indentation via `.editorconfig` + `rustfmt.toml` — match.

**Goal:** Add a per-folder browse view (filename / path sorts, collapsible subdirs), and redesign the folder list row to make the folder name the primary navigation target.

**Architecture:** Extend `Library` to collect `Vec<FileMeta>` during the existing walkdir pass (no extra IO). Add `GET /api/files?path=…`. Split the web UI into `FolderList` + `BrowseView` driven by a top-level `view` state in `App.svelte`.

**Tech Stack:** Rust (axum 0.8, walkdir, anyhow), Svelte 5 (runes), bun build.

**Spec:** `docs/superpowers/specs/2026-05-11-browse-view-design.md`

**Branch:** continue on `dev`.

---

## Files

Backend:
- `src/library.rs` — add `FileMeta`, extend `Folder.files`, replace `count_files` with `walk_files`.
- `src/api.rs` — add `files` handler + route. Update `FolderView` to keep file list off the folders endpoint (don't include `files`).
- `tests/library.rs` — extend existing tests.
- `tests/api.rs` — add `/api/files` tests.

Frontend:
- `web/src/format.ts` (new) — `formatSize(bytes)`.
- `web/src/api.ts` — add `listFiles(folder)`.
- `web/src/FolderList.svelte` (new) — extracted from App.svelte. Redesigned row.
- `web/src/BrowseView.svelte` (new) — sort + list.
- `web/src/App.svelte` — top-level view switch.

---

## Task 1: Extend Library with file metadata

**Files:**
- Modify: `src/library.rs`
- Modify: `tests/library.rs`

- [ ] **Step 1: Update `tests/library.rs` — add failing test alongside existing ones**

Append to `tests/library.rs`:

```rust
#[test]
fn add_folder_populates_files_with_relative_paths_and_sizes() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["a.mp4", "sub/b.mp4", "sub/deep/c.mp4"]);
	std::fs::write(dir.path().join("a.mp4"), b"hello").unwrap();

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), true).unwrap();

	assert_eq!(f.files.len(), 3);
	let rels: Vec<String> = f.files.iter().map(|m| m.rel.to_string_lossy().into_owned()).collect();
	assert!(rels.contains(&"a.mp4".to_string()));
	assert!(rels.contains(&"sub/b.mp4".to_string()) || rels.contains(&"sub\\b.mp4".to_string()));
	let a = f.files.iter().find(|m| m.rel.to_string_lossy() == "a.mp4").unwrap();
	assert_eq!(a.size, 5);
}

#[test]
fn files_are_sorted_by_relative_path() {
	let dir = tempdir().unwrap();
	make_tree(dir.path(), &["z.mp4", "a.mp4", "m.mp4"]);

	let mut lib = Library::new();
	let f = lib.add_folder(dir.path().to_path_buf(), true).unwrap();

	let rels: Vec<String> = f.files.iter().map(|m| m.rel.to_string_lossy().into_owned()).collect();
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
```

- [ ] **Step 2: Run tests, confirm failures**

```bash
cargo test --test library 2>&1 | tail -15
```

Expect: 3 new tests fail (no `files` field).

- [ ] **Step 3: Edit `src/library.rs`**

Add `FileMeta` near the top (above `Folder`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
	pub rel: std::path::PathBuf,
	pub size: u64,
}
```

Add `files` to `Folder`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
	pub path: PathBuf,
	pub recursive: bool,
	pub file_count: usize,
	pub files: Vec<FileMeta>,
}
```

Replace `fn count_files(root: &Path, recursive: bool) -> usize` with:

```rust
fn walk_files(root: &Path, recursive: bool) -> Vec<FileMeta> {
	let max_depth = if recursive { usize::MAX } else { 1 };
	let mut out: Vec<FileMeta> = walkdir::WalkDir::new(root)
		.max_depth(max_depth)
		.follow_links(false)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.file_type().is_file())
		.filter_map(|e| {
			let rel = e.path().strip_prefix(root).ok()?.to_path_buf();
			let size = e.metadata().ok()?.len();
			Some(FileMeta { rel, size })
		})
		.collect();
	out.sort_by(|a, b| a.rel.cmp(&b.rel));
	out
}
```

In `add_folder`:

```rust
pub fn add_folder(&mut self, path: PathBuf, recursive: bool) -> Result<Folder> {
	let canonical = path.canonicalize()
		.with_context(|| format!("canonicalize {}", path.display()))?;
	if self.folders.iter().any(|f| f.path == canonical) {
		anyhow::bail!("folder already added: {}", canonical.display());
	}
	let files = Self::walk_files(&canonical, recursive);
	let folder = Folder {
		path: canonical,
		recursive,
		file_count: files.len(),
		files,
	};
	self.folders.push(folder.clone());
	Ok(folder)
}
```

In `refresh_count`:

```rust
pub fn refresh_count(&mut self, path: &Path) -> Option<usize> {
	let target = path.canonicalize().ok()?;
	let f = self.folders.iter_mut().find(|f| f.path == target)?;
	let files = Self::walk_files(&f.path, f.recursive);
	f.file_count = files.len();
	f.files = files;
	Some(f.file_count)
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test --test library 2>&1 | tail -15
```

Expect: 7 passed (4 original + 3 new).

- [ ] **Step 5: Run full test suite to confirm nothing broke**

```bash
cargo test 2>&1 | grep -E '^(test result|running)' | head
```

Expect: every test file reports `0 failed`.

- [ ] **Step 6: Commit**

```bash
git add src/library.rs tests/library.rs
git commit -m "library: collect FileMeta during walk"
```

---

## Task 2: Drop `files` from the folders JSON response

The `FolderView` returned by `/api/folders` should NOT include `files` (keeps that endpoint light). Confirm and lock that behavior with a test.

**Files:**
- Modify: `tests/api.rs`

- [ ] **Step 1: Append a regression test to `tests/api.rs`**

```rust
#[tokio::test]
async fn list_folders_does_not_include_file_list() {
	let dir = tempdir().unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
	std::fs::write(dir.path().join("b.mp4"), b"x").unwrap();

	let lib = Arc::new(RwLock::new(Library::new()));
	lib.write().await.add_folder(dir.path().to_path_buf(), true).unwrap();
	let state = dlna_rs::api::AppState {
		lib,
		cfg_path: Arc::new(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
	};
	let app = dlna_rs::api::router(state);

	let res = app.oneshot(Request::builder().uri("/api/folders").body(Body::empty()).unwrap())
		.await.unwrap();
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
	assert!(v[0].get("files").is_none(), "folders list must not include file array: got {v}");
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test api 2>&1 | tail
```

Expect: pass (since `FolderView` already only serializes `path, recursive, file_count`).

If it fails, edit `src/api.rs::FolderView` to ensure no `files` field is included.

- [ ] **Step 3: Commit**

```bash
git add tests/api.rs
git commit -m "api: lock folder-list payload (no files field)"
```

---

## Task 3: `GET /api/files?path=…`

**Files:**
- Modify: `src/api.rs`
- Modify: `tests/api.rs`

- [ ] **Step 1: Write failing tests in `tests/api.rs`**

Append:

```rust
#[tokio::test]
async fn list_files_for_configured_folder() {
	let dir = tempdir().unwrap();
	std::fs::create_dir(dir.path().join("sub")).unwrap();
	std::fs::write(dir.path().join("a.mp4"), b"hello").unwrap();
	std::fs::write(dir.path().join("sub/b.mp4"), b"world!").unwrap();

	let lib = Arc::new(RwLock::new(Library::new()));
	let canonical = lib.write().await.add_folder(dir.path().to_path_buf(), true).unwrap().path;
	let state = dlna_rs::api::AppState {
		lib,
		cfg_path: Arc::new(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
	};
	let app = dlna_rs::api::router(state);

	let uri = format!("/api/files?path={}", urlencoding_simple(canonical.to_str().unwrap()));
	let res = app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	let bytes = res.into_body().collect().await.unwrap().to_bytes();
	let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
	assert_eq!(v["folder"], canonical.to_string_lossy().as_ref());
	assert_eq!(v["recursive"], true);
	let files = v["files"].as_array().unwrap();
	assert_eq!(files.len(), 2);
	assert_eq!(files[0]["rel"], "a.mp4");
	assert_eq!(files[0]["size"], 5);
	assert_eq!(files[1]["rel"], "sub/b.mp4");
}

#[tokio::test]
async fn list_files_404_for_unknown_folder() {
	let lib = Arc::new(RwLock::new(Library::new()));
	let state = dlna_rs::api::AppState {
		lib,
		cfg_path: Arc::new(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
	};
	let app = dlna_rs::api::router(state);

	let res = app.oneshot(Request::builder()
		.uri("/api/files?path=/nope/nada")
		.body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_files_400_without_path_param() {
	let lib = Arc::new(RwLock::new(Library::new()));
	let state = dlna_rs::api::AppState {
		lib,
		cfg_path: Arc::new(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
	};
	let app = dlna_rs::api::router(state);

	let res = app.oneshot(Request::builder()
		.uri("/api/files")
		.body(Body::empty()).unwrap())
		.await.unwrap();
	assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

fn urlencoding_simple(s: &str) -> String {
	let mut out = String::new();
	for b in s.bytes() {
		match b {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
			_ => out.push_str(&format!("%{:02X}", b)),
		}
	}
	out
}
```

- [ ] **Step 2: Run tests, confirm failures**

```bash
cargo test --test api 2>&1 | tail
```

Expect: 3 new tests fail (404 because route missing, or other).

- [ ] **Step 3: Add the route handler in `src/api.rs`**

Add types near other request/response types:

```rust
#[derive(Deserialize)]
pub struct FilesQuery {
	pub path: Option<String>,
}

#[derive(Serialize)]
pub struct FileMetaView {
	pub rel: String,
	pub size: u64,
}

#[derive(Serialize)]
pub struct FilesResp {
	pub folder: String,
	pub recursive: bool,
	pub files: Vec<FileMetaView>,
}
```

Add the route in `router(state: AppState)`:

```rust
.route("/api/files", get(list_files))
```

Handler:

```rust
async fn list_files(State(state): State<AppState>, Query(q): Query<FilesQuery>) -> Response {
	let raw = match q.path {
		Some(p) if !p.is_empty() => p,
		_ => return (StatusCode::BAD_REQUEST, "missing path query parameter").into_response(),
	};
	let target = match std::path::PathBuf::from(&raw).canonicalize() {
		Ok(p) => p,
		Err(_) => return (StatusCode::NOT_FOUND, "folder not configured").into_response(),
	};
	let g = state.lib.read().await;
	let folder = match g.folders().iter().find(|f| f.path == target) {
		Some(f) => f,
		None => return (StatusCode::NOT_FOUND, "folder not configured").into_response(),
	};
	let files: Vec<FileMetaView> = folder.files.iter()
		.map(|m| FileMetaView {
			rel: m.rel.to_string_lossy().into_owned(),
			size: m.size,
		})
		.collect();
	Json(FilesResp {
		folder: folder.path.to_string_lossy().into_owned(),
		recursive: folder.recursive,
		files,
	}).into_response()
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test --test api 2>&1 | tail
```

Expect: all api tests pass.

- [ ] **Step 5: Run full suite**

```bash
cargo test 2>&1 | grep -E '^test result'
```

All pass.

- [ ] **Step 6: Commit**

```bash
git add src/api.rs tests/api.rs
git commit -m "api: GET /api/files for per-folder file listing"
```

---

## Task 4: Add `format.ts` helper

**Files:**
- Create: `web/src/format.ts`

- [ ] **Step 1: Create the file**

```ts
export function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	const units = ['KB', 'MB', 'GB', 'TB'];
	let v = bytes / 1024;
	let i = 0;
	while (v >= 1024 && i < units.length - 1) {
		v /= 1024;
		i++;
	}
	return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/format.ts
git commit -m "web: formatSize helper"
```

---

## Task 5: Extend `api.ts` with `listFiles`

**Files:**
- Modify: `web/src/api.ts`

- [ ] **Step 1: Append to `web/src/api.ts`**

```ts
export type FileMeta = { rel: string; size: number };
export type FilesResp = { folder: string; recursive: boolean; files: FileMeta[] };

export async function listFiles(folder: string): Promise<FilesResp> {
	const r = await fetch(`/api/files?path=${encodeURIComponent(folder)}`);
	if (!r.ok) throw new Error(await r.text());
	return r.json();
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/api.ts
git commit -m "web: listFiles client"
```

---

## Task 6: Extract `FolderList.svelte` with redesigned row

**Files:**
- Create: `web/src/FolderList.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
	import { listFolders, removeFolder, refreshFolder, type Folder } from './api';

	let { onbrowse }: { onbrowse: (path: string) => void } = $props();

	let folders = $state<Folder[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function load() {
		try { folders = await listFolders(); error = null; }
		catch (e) { error = String(e); }
		finally { loading = false; }
	}
	load();

	async function onRemove(path: string) {
		if (!confirm(`Remove ${path}?`)) return;
		await removeFolder(path);
		await load();
	}
	async function onRefresh(path: string) {
		await refreshFolder(path);
		await load();
	}

	export async function reload() { await load(); }

	function basename(p: string): string {
		const parts = p.split('/').filter(Boolean);
		return parts[parts.length - 1] ?? p;
	}
</script>

{#if loading}
	<p>Loading…</p>
{:else if error}
	<p class="err">{error}</p>
{:else}
	<ul>
		{#each folders as f (f.path)}
			<li>
				<button class="name" onclick={() => onbrowse(f.path)} title="Browse files">
					{basename(f.path)} <span class="chev">▸</span>
				</button>
				<button class="icon" title="Refresh count" onclick={() => onRefresh(f.path)}>↻</button>
				<span class="count">{f.file_count} files</span>
				{#if f.recursive}<span class="tag">recursive</span>{/if}
				<span class="path" title={f.path}>{f.path}</span>
				<button class="icon" title="Remove" onclick={() => onRemove(f.path)}>✕</button>
			</li>
		{/each}
	</ul>
{/if}

<style>
	ul { list-style: none; padding: 0; margin: 0; }
	li {
		display: grid;
		grid-template-columns: auto auto auto auto 1fr auto;
		gap: 0.75rem;
		align-items: center;
		padding: 0.45rem 0;
		border-bottom: 1px solid #eee;
	}
	.name {
		background: none;
		border: none;
		padding: 0;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
		color: #111;
	}
	.name:hover { color: #000; }
	.chev { color: #999; font-weight: normal; }
	.count { font-variant-numeric: tabular-nums; color: #666; font-size: 0.85rem; white-space: nowrap; }
	.tag { font-size: 0.7rem; color: #888; background: #f3f3f3; padding: 0.05rem 0.4rem; border-radius: 3px; white-space: nowrap; }
	.path {
		color: #999;
		font-family: ui-monospace, monospace;
		font-size: 0.8rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}
	.icon { background: none; border: 1px solid transparent; padding: 0.15rem 0.35rem; cursor: pointer; color: #777; font-size: 0.9rem; }
	.icon:hover { color: #111; border-color: #ddd; }
	.err { color: #c33; }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add web/src/FolderList.svelte
git commit -m "web: FolderList component with redesigned row"
```

---

## Task 7: Create `BrowseView.svelte`

**Files:**
- Create: `web/src/BrowseView.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
	import { listFiles, type FileMeta } from './api';
	import { formatSize } from './format';

	let { folder, onback }: { folder: string; onback: () => void } = $props();

	let recursive = $state(false);
	let files = $state<FileMeta[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let sortMode = $state<'filename' | 'path'>('filename');
	let collapsed = $state<Record<string, boolean>>({});

	async function load() {
		loading = true;
		try {
			const r = await listFiles(folder);
			files = r.files;
			recursive = r.recursive;
			error = null;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}
	load();

	function basename(p: string): string {
		const parts = p.split('/').filter(Boolean);
		return parts[parts.length - 1] ?? p;
	}

	function parentDir(rel: string): string {
		const i = rel.lastIndexOf('/');
		return i < 0 ? '' : rel.slice(0, i + 1);
	}

	const sortedByFilename = $derived(
		[...files].sort((a, b) => basename(a.rel).localeCompare(basename(b.rel)))
	);

	type Group = { dir: string; files: FileMeta[] };
	const groupedByPath = $derived.by<Group[]>(() => {
		const map = new Map<string, FileMeta[]>();
		for (const f of [...files].sort((a, b) => a.rel.localeCompare(b.rel))) {
			const dir = parentDir(f.rel);
			if (!map.has(dir)) map.set(dir, []);
			map.get(dir)!.push(f);
		}
		const dirs = [...map.keys()].sort();
		return dirs.map(dir => ({ dir, files: map.get(dir)! }));
	});

	function toggle(dir: string) {
		collapsed = { ...collapsed, [dir]: !collapsed[dir] };
	}
</script>

<header class="page">
	<button class="back" onclick={onback}>← dlna-rs</button>
</header>

<section class="hero">
	<div class="row">
		<h2>{basename(folder)}</h2>
		{#if recursive}<span class="tag">recursive</span>{/if}
		<span class="count">{files.length} files</span>
	</div>
	<div class="path">{folder}</div>
</section>

<nav class="sort">
	<span class="label">sort:</span>
	<label><input type="radio" name="sort" value="filename" bind:group={sortMode} /> filename</label>
	<label><input type="radio" name="sort" value="path" bind:group={sortMode} /> path</label>
</nav>

{#if loading}
	<p>Loading…</p>
{:else if error}
	<p class="err">{error}</p>
{:else if sortMode === 'filename'}
	<ul class="files">
		{#each sortedByFilename as f (f.rel)}
			<li>
				<span class="name">{basename(f.rel)}</span>
				<span class="size">{formatSize(f.size)}</span>
			</li>
		{/each}
	</ul>
{:else}
	{#each groupedByPath as g (g.dir)}
		{#if g.dir === ''}
			<ul class="files">
				{#each g.files as f (f.rel)}
					<li>
						<span class="name">{f.rel}</span>
						<span class="size">{formatSize(f.size)}</span>
					</li>
				{/each}
			</ul>
		{:else}
			<button class="group-head" onclick={() => toggle(g.dir)}>
				<span class="chev">{collapsed[g.dir] ? '▸' : '▾'}</span>
				<code>{g.dir}</code>
				<span class="count">{g.files.length} files</span>
			</button>
			{#if !collapsed[g.dir]}
				<ul class="files indent">
					{#each g.files as f (f.rel)}
						<li>
							<span class="name">{f.rel.slice(g.dir.length)}</span>
							<span class="size">{formatSize(f.size)}</span>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	{/each}
{/if}

<style>
	.page { margin-bottom: 1rem; }
	.back { background: none; border: none; padding: 0; font: inherit; color: #555; cursor: pointer; }
	.back:hover { color: #000; }

	.hero { padding: 0.5rem 0 1rem; border-bottom: 1px solid #eee; }
	.hero .row { display: flex; align-items: baseline; gap: 0.75rem; }
	.hero h2 { font-size: 1.05rem; margin: 0; }
	.tag { font-size: 0.7rem; color: #888; background: #f3f3f3; padding: 0.05rem 0.4rem; border-radius: 3px; }
	.count { font-variant-numeric: tabular-nums; color: #666; font-size: 0.85rem; margin-left: auto; }
	.path { color: #999; font-family: ui-monospace, monospace; font-size: 0.8rem; margin-top: 0.2rem; }

	.sort { display: flex; gap: 1rem; align-items: center; padding: 0.6rem 0; font-size: 0.9rem; }
	.sort .label { color: #888; }

	.files { list-style: none; padding: 0; margin: 0; }
	.files li {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 1rem;
		padding: 0.2rem 0;
		font-size: 0.9rem;
	}
	.files .name { font-family: ui-monospace, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.files .size { color: #777; font-variant-numeric: tabular-nums; }
	.indent { padding-left: 1rem; }

	.group-head {
		display: flex; align-items: baseline; gap: 0.5rem;
		background: none; border: none; padding: 0.5rem 0 0.2rem;
		font: inherit; cursor: pointer; width: 100%; text-align: left;
		color: #444;
	}
	.group-head:hover { color: #000; }
	.group-head .chev { width: 1rem; color: #999; }
	.group-head code { font-size: 0.85rem; }
	.group-head .count { margin-left: 0; color: #999; font-size: 0.8rem; }

	.err { color: #c33; }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add web/src/BrowseView.svelte
git commit -m "web: BrowseView with filename/path sort and collapsible subdirs"
```

---

## Task 8: Wire view switch in `App.svelte`

**Files:**
- Modify: `web/src/App.svelte`

- [ ] **Step 1: Replace contents with:**

```svelte
<script lang="ts">
	import FolderList from './FolderList.svelte';
	import BrowseView from './BrowseView.svelte';
	import AddFolderModal from './AddFolderModal.svelte';

	type View = { kind: 'folders' } | { kind: 'browse'; folder: string };

	let view = $state<View>({ kind: 'folders' });
	let showAdd = $state(false);
	let listRef = $state<FolderList | undefined>();

	async function onAdded() {
		showAdd = false;
		await listRef?.reload();
	}
</script>

<main>
	{#if view.kind === 'folders'}
		<header>
			<h1>dlna-rs</h1>
		</header>
		<FolderList bind:this={listRef} onbrowse={(p) => view = { kind: 'browse', folder: p }} />
		<button class="add" onclick={() => showAdd = true}>+ add folder…</button>
	{:else}
		<BrowseView folder={view.folder} onback={() => view = { kind: 'folders' }} />
	{/if}

	{#if showAdd}
		<AddFolderModal onclose={() => showAdd = false} onadded={onAdded} />
	{/if}
</main>

<style>
	main { max-width: 720px; margin: 2rem auto; font-family: ui-sans-serif, system-ui, sans-serif; padding: 0 1rem; }
	header { display: flex; align-items: baseline; justify-content: space-between; }
	h1 { font-size: 1.25rem; margin: 0 0 1rem; }
	.add { margin-top: 1rem; border: 1px solid #ddd; background: #fafafa; padding: 0.3rem 0.6rem; cursor: pointer; }
	.add:hover { border-color: #aaa; }
</style>
```

- [ ] **Step 2: Build the web bundle to confirm it compiles**

```bash
cd web && bun run build && cd ..
```

Expect: success. If TS errors about `FolderList`'s `reload` method (Svelte 5 component instance typing), the simplest fix is to drop the `bind:this` typing and use `let listRef: any = $state();` — but try the typed version first.

- [ ] **Step 3: End-to-end smoke**

```bash
cargo build 2>&1 | tail -5
mkdir -p /tmp/dlna-rs-media/sub1 /tmp/dlna-rs-media/sub2
touch /tmp/dlna-rs-media/a.mp4 /tmp/dlna-rs-media/sub1/b.mp4 /tmp/dlna-rs-media/sub2/c.mp4
cargo run -- --folder /tmp/dlna-rs-media &
SERVER_PID=$!
sleep 2
curl -s "http://localhost:9119/api/files?path=/private/tmp/dlna-rs-media" | head -c 400
echo
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null
```

Expect: JSON with 3 files.

(On macOS, `/tmp` canonicalizes to `/private/tmp`, hence the query path.)

- [ ] **Step 4: Commit**

```bash
git add web/src/App.svelte
git commit -m "web: view switch between folder list and browse"
```

---

## Self-review

**Spec coverage:**
- Folder list row redesign → Task 6 (`FolderList.svelte`)
- Click-to-navigate → Task 6 (`onbrowse` prop) + Task 8 (view switch)
- Browse view with sort filename/path → Task 7
- Collapsible subdirs (default expanded) → Task 7 (`collapsed` defaults `{}`, treated as not collapsed)
- Backend file list captured during walk → Task 1
- `/api/files` endpoint → Task 3
- Folder list endpoint stays light → Task 2 (regression test)
- Out-of-scope (duration) → designed in spec, not in any task ✓

**Placeholder scan:** No `TBD`/`TODO`/vague-handling strings. Every step shows complete code or exact commands.

**Type consistency:**
- `FileMeta { rel: PathBuf, size: u64 }` in `library.rs`, serialized as `FileMetaView { rel: String, size: u64 }` in `api.rs`, typed as `FileMeta { rel: string; size: number }` in `api.ts`. Consistent.
- `Folder.files: Vec<FileMeta>` in Rust; never exposed in `/api/folders` (Task 2 locks).
- `FilesResp` matches across Rust + TS.
- `onbrowse(path: string)` in `FolderList.svelte`, consumed by `App.svelte` as `(p) => view = { kind: 'browse', folder: p }`. ✓

One risk: Svelte 5 `bind:this` of a component returns a component instance type. The `reload()` export is a snippet pattern — if Svelte 5 doesn't expose component-script exports through `bind:this`, fall back to triggering reload via a `$state`-shared "reloadCounter" prop or just refetch on every view → `folders` transition. The fallback is noted inline in Task 8 Step 2.
