# dlna-rs

Single-binary DLNA/UPnP media server in Rust. Auto-discovered by VLC on AppleTV. Embedded Svelte web UI for managing the advertised folders.

Built on [`librqbit-upnp-serve`](https://github.com/ikatson/rqbit) for SSDP + UPnP ContentDirectory; HTTP streaming with Range requests + DLNA headers handled directly.

## Build

Requires Rust (stable) and [bun](https://bun.sh).

```sh
cargo build --release
```

The web UI is built automatically via `build.rs` (runs `bun install` + `bun run build` in `web/`).

## Run

```sh
./target/release/dlna-rs --folder /path/to/media
```

Then on Apple TV: **VLC → Network → Local Network**. The server appears under its friendly name (default: `dlna-rs (hostname)`).

For the admin UI, open `http://localhost:9119/` in a browser.

## Config

`~/.config/dlna-rs/config.styx` (auto-created on first save):

```styx
port 9119
friendly_name "dlna-rs (hostname)"
folders ({path "/Users/m/Movies", recursive true})
```

(Format is whatever [`facet-styx`](https://crates.io/crates/facet-styx) 3.x parses — close to styx but not identical to bearcove's reference.)

CLI flags override:

```
--config <PATH>         path to config file
--port <PORT>           HTTP port (default 9119)
--friendly-name <NAME>  DLNA friendly name
--folder <PATH>         append a recursive folder (repeatable)
--folder-flat <PATH>    append a non-recursive folder (repeatable)
```

## Endpoints

| Path | Purpose |
|------|---------|
| `/` | Embedded Svelte admin UI |
| `/status` | Plain-text folder + count summary |
| `/api/folders` | GET / POST / DELETE folders |
| `/api/folders/refresh` | POST to recount a folder |
| `/api/browse?path=…` | Server-side directory browser |
| `/api/status` | JSON status |
| `/stream/{id}` | File streaming, Range + DLNA headers |
| `/description.xml` | UPnP device description |
| `/scpd/*`, `/control/*` | UPnP SOAP services |

## VLC AppleTV verification

1. Start: `./target/release/dlna-rs --folder ~/Movies` (or a folder with video files).
2. On macOS: System Settings → Network → Firewall → ensure incoming connections to `dlna-rs` are allowed (or temporarily disable firewall).
3. Apple TV: open VLC → "Network" tab → "Local Network".
4. Expect: a row labelled with the friendly name (`dlna-rs (<your-hostname>)`).
5. Open it; configured folders appear as containers.
6. Open a video file → playback should start.

Troubleshooting:
- `curl http://localhost:9119/description.xml` should return XML.
- `curl http://localhost:9119/status` should list your folders.
- On a Mac on the same network: `dns-sd -B _ssdp._udp` or `pip install ssdpy && ssdpy-discover` to confirm SSDP announcements.
- If VLC doesn't see it: macOS firewall and "Local Network" privacy settings (Settings → Privacy & Security → Local Network) are the usual culprits.

## License

LLM-assisted. MIT.
