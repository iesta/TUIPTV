# AGENTS.md

## Stack

- **UI**: `ratatui` + `crossterm` (async TUI)
- **Runtime**: `tokio`
- **DB**: `rusqlite` (SQLite, file `iptv_cache.db`)
- **HTTP**: `reqwest` (Xtream Codes API)
- **Config**: `dotenvy` (`.env`)

## Entrypoint

`src/main.rs` — all other source lives under `src/`.

## Architecture

- **Auth**: On startup, check `.env` for `XTREAM_URL`, `XTREAM_USER`, `XTREAM_PASS`. If missing, show a full-screen config form. On submit, write `.env`, reload, transition to main view.
- **DB schemas**: `categories` (id, name), `movies` (id, name, category_id, stream_id, container_extension, stream_icon, rating, release_date, plot). Sync via `action=get_vod_categories` + `action=get_vod_streams`, using transactions.
- **Layout** (Ratatui `Layout`):
  - Top bar: height 3, app title + version.
  - Main workspace (remaining): 3 columns — Categories (10%), Movies (50%), Details (40%).
  - Bottom bar: height 2, context-aware keymaps.
- **Key bindings**: `h`/`Left` `l`/`Right` = hop focus between columns; `j`/`k` `Up`/`Down` = scroll lists; `r` = trigger Xtream sync; `q` = graceful exit.

## Conventions

- **`tokio::main`** on `main`.
- App state in a single struct, drawn each frame with `ratatui`'s immediate-mode loop.
- Async DB ops spawned via `tokio::spawn`, results sent back via channels or shared state.
- Modular: separate files for `app.rs` (state/loop), `db.rs` (SQLite), `api.rs` (Xtream client), `ui/` (render functions per pane), `config.rs` (`.env` handling).
- Use `anyhow` for error propagation.
- Keep `.env` out of version control (already in `.gitignore`).

## Commands

- `cargo run` — launches the TUI.
- `cargo build` — (no special flags needed; standard binary).
- `cargo fmt && cargo check` — style + typecheck before committing.

## Notes

- `.env` is created at runtime by the user, never shipped.
- Xtream Codes API base URL is `{XTREAM_URL}/player_api.php?username={XTREAM_USER}&password={XTREAM_PASS}`.
- VOD endpoints: `action=get_vod_categories`, `action=get_vod_streams`.
