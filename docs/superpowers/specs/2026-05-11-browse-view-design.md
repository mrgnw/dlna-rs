# Browse view + folder list redesign

Adds a per-folder browse view to dlna-rs that lists media files with sortable display, and improves the folder list row layout. Lays groundwork for a future non-blocking duration probe.

## Goals

- Click a folder in the admin UI → navigate to a browse view showing every file under it.
- Browse view supports two sort modes: by filename (flat), or by path (grouped by subdirectory, collapsible).
- Folder list row layout makes the folder name the primary identifier and action.
- Backend collects file metadata (relative path + size) at the same time as the existing file count walk — zero extra IO.

## Folder list row (redesign)

```
Movies  ▸          ↻   136 files   recursive   /Users/m/Movies          ✕
```

Order, left to right:

1. **`Movies ▸`** — bold basename + chevron. Whole element is the navigation trigger. Cursor pointer.
2. **`↻`** — refresh button (existing).
3. **`136 files`** — count, `tabular-nums`.
4. **`recursive`** — small dim tag (existing).
5. **`/Users/m/Movies`** — full path, secondary muted color, ellipsis-truncated when narrow.
6. **`✕`** — remove button.

## Navigation

State-based view switch in `App.svelte`:

```ts
let view = $state<{ kind: 'folders' } | { kind: 'browse'; folder: string }>({ kind: 'folders' });
```

Clicking the folder title sets `view = { kind: 'browse', folder: path }`. A `← back` link in the browse view returns to folders.

No URL routing. No browser history. Acceptable for a local admin tool.

## Browse view layout

```
← dlna-rs
─────────────────────────────────────────────
Movies                                  recursive
/Users/m/Movies                            136 files
─────────────────────────────────────────────
sort:  ● filename   ○ path
─────────────────────────────────────────────
some-movie-a.mkv                          4.2 GB
some-movie-b.mp4                          1.1 GB
sub/another.mkv                           2.0 GB
sub/deep/even-more.mp4                  812 MB
```

### Sort modes

- **filename** — flat alphabetical list by basename across the whole source folder.
- **path** — grouped by relative subdirectory. Top-level files appear ungrouped first; each subdirectory below is a collapsible section with `▾`/`▸` toggle. Default expanded.

```
sort:  ○ filename   ● path
─────────────────────────────────────────────
some-movie-a.mkv                          4.2 GB
some-movie-b.mp4                          1.1 GB

sub/  ▾                                   (2 files)
  another.mkv                             2.0 GB
  more.mkv                                900 MB

sub/deep/  ▾                              (1 file)
  even-more.mp4                           812 MB
```

Subdirectory headers group by the immediate parent of each relative path. A file at `sub/deep/even-more.mp4` belongs to the `sub/deep/` group, not `sub/` — flat per-parent grouping, not nested tree. Keeps it simple.

Collapse state is per-section, in-component (`$state` map keyed by subdir name). Not persisted.

## Backend changes

### `Library` extension

```rust
pub struct FileMeta {
    pub rel: PathBuf,   // relative to folder.path
    pub size: u64,
}

pub struct Folder {
    pub path: PathBuf,
    pub recursive: bool,
    pub file_count: usize,
    pub files: Vec<FileMeta>,   // NEW: populated by the same walkdir pass
}
```

`Library::count_files` becomes `walk_files(root, recursive) -> Vec<FileMeta>`. `file_count = files.len()`. Used by `add_folder` and `refresh_count`. Same walk, same cost.

Files are sorted at the source by relative path string (so JSON output is deterministic).

### New API endpoint

```
GET /api/files?path=<folder>
```

Response:

```json
{
  "folder": "/Users/m/Movies",
  "recursive": true,
  "files": [
    { "rel": "some-movie-a.mkv", "size": 4500000000 },
    { "rel": "sub/another.mkv", "size": 2147483648 }
  ]
}
```

Errors:
- 400 if `path` query param missing
- 404 if folder is not configured (not just a folder anywhere on disk)

The existing `/api/folders` response is unchanged. The browse endpoint is separate so the folder list doesn't pay the JSON-serialization cost of the full file list.

## Frontend changes

### Components

- `web/src/App.svelte` — top-level view switch.
- `web/src/FolderList.svelte` — extracted from current App.svelte. Same behavior, redesigned row layout, emits `onBrowse(path)`.
- `web/src/BrowseView.svelte` — new. Takes `folder: string` prop, fetches files, renders sort + list.
- `web/src/api.ts` — add `listFiles(folder)`.
- `web/src/format.ts` — new tiny helper for `formatSize(bytes)`.

### Folder list row CSS

```css
li {
	display: grid;
	grid-template-columns: auto auto 1fr auto auto auto auto;
	gap: 0.75rem;
	align-items: center;
}
.name { font-weight: 600; cursor: pointer; }
.path { color: #888; font-family: ui-monospace; font-size: 0.85rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
```

The path column gets `1fr` so it eats remaining width. Count and tag are auto-sized.

## Out of scope (this iteration)

- Duration column / probing (designed below; built separately).
- Sorting by size or duration.
- Search / filter.
- Multi-folder browse view.
- Persistent collapse state.
- URL routing / browser history.

## Duration phase (designed, not built here)

For the next iteration after this lands:

- A `DurationStore` shared via `Arc<RwLock<HashMap<PathBuf, u32>>>` (seconds).
- A worker `tokio::spawn`'d at startup that drains a queue of paths and runs `ffprobe -v error -show_entries format=duration -of csv=p=0 <path>` per file. One probe at a time, low priority.
- `Library` enqueues all files in newly-added folders into the worker.
- `GET /api/files` includes `"duration": number|null` per file.
- Browse view shows `—` for `null`, polls (every 5s while view is open) for updates.
- No persistence in MVP — re-probe on restart.

Key invariants: probe worker never touches `Library`, `Library` never blocks on probing, stream handler never reads `DurationStore`. Total isolation.

## Testing strategy

- Unit (Rust): `walk_files` returns expected relative paths + sizes for a fixture tree.
- Integration (axum tests): `GET /api/files?path=…` returns the expected JSON; 400 on missing param; 404 on unknown folder.
- Manual (web UI): smoke-test the navigation and sort toggle.

No frontend tests — too low-value for a 2-component admin UI at this stage.
