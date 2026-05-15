# dlna-rs

Single-binary DLNA media server in Rust + embedded Svelte UI.

## Hard rules

- **TABS for indentation in every file.** Not spaces. Never spaces. `.editorconfig` enforces this — match it.
- Do not run the dev server (`bun run dev`, `cargo run` is fine for backend smoke).
- Spec: `docs/superpowers/specs/2026-05-09-dlna-rs-design.md`
- Plan: `docs/superpowers/plans/2026-05-11-dlna-rs.md`

## Stack

- Rust (axum, tokio, walkdir, mime_guess, rust-embed, clap, facet-styx)
- `librqbit-upnp-serve` (currently patched from git; see `[patch.crates-io]` in `Cargo.toml`)
- Svelte 5 SPA built with vite + bun

## Project port

`9119` (HTTP) — see config.
