# dlna-rs — design

A single-binary DLNA/UPnP media server in Rust that exposes local folders to LAN devices (target: VLC on AppleTV). Includes an embedded Svelte web UI for managing the advertised folders.

## Goals

- One binary, run from a terminal.
- Auto-discovered by VLC on AppleTV via UPnP/DLNA.
- Configurable via `~/.config/dlna-rs/config.styx` and CLI flags.
- Multiple folders, each with optional recursion.
- `/status` endpoint with top-level folder names and file counts.
- Tiny embedded web UI to add/remove folders, with a server-side folder browser modal.

## Non-goals (MVP)

Transcoding, auth, thumbnails, subtitles handling, NFO/metadata parsing, watcher-based live re-indexing.

## Architecture

Three concurrent subsystems, all spawned on tokio in the same process:

1. **SSDP responder** — UDP multicast on `239.255.255.250:1900`. Provided by `upnp-serve` (rqbit).
2. **HTTP server (axum)** — bound to `0.0.0.0:<port>` (default `9119`). Routes:
   - UPnP routes (`/device.xml`, `/scpd.xml`, `/soap`, …) — provided by `upnp-serve` router.
   - `/stream/{file_id}` — our handler, Range + DLNA headers.
   - `/api/*` — admin JSON API.
   - `/status` — curl-friendly status text/JSON.
   - `/` — embedded Svelte SPA (rust-embed fallback).
3. **Library** — shared `Arc<RwLock<Library>>` holding folders + cached counts + `file_id ↔ PathBuf` map.

All three subsystems share the `Library` via `Arc`. SSDP and HTTP both come from `upnp-serve`; we extend the axum router with our own routes before serving.

```
                  ┌──────────────────────────┐
   AppleTV VLC ──▶│ SSDP responder (UDP 1900)│  (upnp-serve)
                  └──────────────────────────┘
                  ┌──────────────────────────┐
   AppleTV VLC ──▶│ UPnP HTTP                │  (upnp-serve router)
                  │   /device.xml, /scpd.xml │
                  │   /soap (ContentDir)     │
                  ├──────────────────────────┤
   AppleTV VLC ──▶│ /stream/<id>  (Range)    │  (our axum routes)
                  │ /status                  │
   Web browser ──▶│ /api/folders, /api/browse│
                  │ /  (embedded Svelte SPA) │
                  └──────────────────────────┘
                       :9119  (single port)
```

## Crate layout

```
src/
  main.rs        — clap CLI, load config, spawn tasks, graceful shutdown
  config.rs      — styx parse/serialize, atomic write (tmp+rename)
  library.rs     — Library, Folder, file_id allocation, walkdir count
  browse.rs      — impl ContentDirectoryBrowseProvider (lazy readdir)
  stream.rs      — GET /stream/{file_id} with Range + DLNA headers
  api.rs         — admin JSON: /api/folders, /api/browse, /api/status, /status
  web.rs         — rust-embed fallback handler for `/`
web/             — Svelte 5 source (SPA, vite build to web/dist)
```

## Library

```rust
struct Library {
    folders: Vec<Folder>,
    files: HashMap<FileId, PathBuf>,        // stable id → path
    by_path: HashMap<PathBuf, FileId>,      // reverse, for stable ids
    next_id: u64,
}

struct Folder {
    path: PathBuf,
    recursive: bool,
    file_count: usize,   // last computed; updated on add and on /folders/refresh
}
```

- `FileId` = monotonic u64 (or stable hash of canonical path — TBD; monotonic is simpler).
- `add_folder(path, recursive)`: canonicalize, walk with `walkdir` honoring `recursive`, count files (not dirs), persist to config.
- `remove_folder(path)`: drop from list, prune ids whose paths are under it, persist.
- `browse(object_id)`: invoked by the ContentDirectoryBrowseProvider. Translates DLNA object IDs to filesystem paths. Lazy `read_dir`; allocates a `FileId` for any new file encountered and returns a UPnP item with `<res>` URL pointing at `/stream/{file_id}`.

## DLNA object id scheme

`upnp-serve`'s `ContentDirectoryBrowseProvider` is given an opaque `object_id`. We use a single u64 id space; the `Library` maps each id to either a file or directory `PathBuf` plus a kind tag:

```rust
enum Entry { File(PathBuf), Dir(PathBuf) }
files: HashMap<u64, Entry>
```

- `"0"` = root (lists each configured folder as a child container).
- Any other id is the stringified u64 from the map.

Ids are allocated on demand the first time `browse` encounters a path (file or directory). Stable for the process lifetime; not persisted across restarts (acceptable — VLC re-browses on reconnect).

## Streaming handler

`GET /stream/{file_id}` mirrors rqbit's behavior:

- Look up path; 404 if unknown.
- `Accept-Ranges: bytes` always.
- Parse `Range` header (`http-range-header` or hand-rolled). Multi-range is **not** supported — first range only or 416.
- 200 full, or 206 partial with `Content-Range: bytes {s}-{e}/{total}` and `Content-Length`.
- `Content-Type` via `mime_guess::from_path`.
- DLNA headers:
  - `transferMode.dlna.org: Streaming`
  - `contentFeatures.dlna.org: DLNA.ORG_OP=01;DLNA.ORG_CI=0`
- Body via `tokio_util::io::ReaderStream` with 64 KiB chunks; `tokio::fs::File::seek` to start.

## JSON API

```
GET    /status                 → text/plain, one line per folder
GET    /api/status             → JSON: { friendly_name, port, folders: [...] }
GET    /api/folders            → [{ path, recursive, file_count }]
POST   /api/folders            { path, recursive } → 201 {folder}
DELETE /api/folders            { path }            → 204
POST   /api/folders/refresh    { path }            → 200 { file_count }
GET    /api/browse?path=…      → { path, parent: path|null, entries: [{name, is_dir}] }
```

`/api/browse` lists immediate children of an absolute server path; entries are sorted directories-first, then alphabetical. Symlinks are followed but never escape root checks (no chroot — by design, this is a local LAN tool the user runs themselves).

`POST /api/folders` and `DELETE /api/folders` rewrite the config file on success (atomic tmp-rename).

## Config

`~/.config/dlna-rs/config.styx` (override with `--config <path>`).

```styx
port: 9119
friendly_name: "dlna-rs ({hostname})"

folders: [
  { path: "/Users/m/Movies", recursive: true }
  { path: "/Volumes/external/tv", recursive: true }
]
```

CLI flags (clap):

```
--config <PATH>            default: ~/.config/dlna-rs/config.styx
--port <PORT>              override config
--friendly-name <NAME>     override config
--folder <PATH>            repeatable; appended to config folders (recursive=true)
--folder-flat <PATH>       repeatable; appended (recursive=false)
```

`{hostname}` is templated at startup via the `hostname` crate. Missing config file = create with defaults on first save.

## Web UI (Svelte 5 SPA)

Built with vite to `web/dist/`, embedded at compile time via `rust-embed`. Served at `/`.

Single page. Plain `fetch` to the JSON API. Svelte 5 runes (`$state`, `$derived`, no `$effect`).

Layout (per design-data: narrow ~720px, dense, no decoration):

```
dlna-rs                                       9119 · friendly-name
─────────────────────────────────────────────────────────────────
/Users/m/Movies                ↻  423 files  recursive   ✕
/Volumes/external/tv           ↻   88 files  recursive   ✕
                                                  + add folder…
```

Add-folder modal:

- Fetches `/api/browse?path=$HOME` initially.
- Breadcrumbs across the top, list of subdirectories below.
- "Select this folder" button + recursive checkbox.
- POSTs to `/api/folders`, closes on success.

`↻` re-runs `/api/folders/refresh`. `✕` calls `DELETE /api/folders` after a `confirm()`.

No SvelteKit; pure SPA. Build script: `cd web && bun install && bun run build`. Cargo build runs this via `build.rs`.

## Dependencies (Cargo.toml)

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "io-util", "net", "signal", "sync"] }
axum = "0.7"
tower = "0.5"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
walkdir = "2"
mime_guess = "2"
rust-embed = "8"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
hostname = "0.4"
url = "2"
percent-encoding = "2"
http-range-header = "0.4"
styx = "…"  # pin latest
upnp-serve = { git = "https://github.com/ikatson/rqbit", rev = "<pinned-rev>" }
tokio-util = { version = "0.7", features = ["io"] }
```

Pin the rqbit revision (specific commit hash) to avoid surprise breakage.

## Failure modes & handling

- **Port already in use** → exit with clear error, suggest `--port`.
- **SSDP bind fails** (no multicast) → log warning, continue HTTP-only (still useful via direct URL in VLC).
- **Folder no longer exists** at startup → log warning, keep entry, file_count = 0, browse returns empty container.
- **File deleted between browse and stream** → 404.
- **Config file unparseable** → exit with clear error and the parse position.
- **Folder add via API points at non-existent path** → 400 with message.
- **Concurrent config writes** → serialized through the `Library`'s lock; atomic rename means readers never see torn files.

## Testing strategy

MVP is exploratory; primary verification is "does VLC on AppleTV auto-detect and play?". In addition:

- Unit: config parse/serialize round-trip; range header parsing; `Library` add/remove/refresh.
- Integration (axum): `GET /api/folders`, `POST /api/folders`, `DELETE`, `GET /api/browse`, Range request on a fixture file.
- Manual: launch with a real folder, browse from VLC desktop and AppleTV.

## Out of scope (MVP)

- Authentication / TLS / rate-limiting.
- Transcoding, thumbnails, subtitle muxing.
- Persistent file_id across restarts.
- Live filesystem watching.
- Multi-user / multi-library.
- Cross-platform packaging (macOS dev only for MVP).
