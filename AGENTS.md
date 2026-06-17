# AGENTS.md

## Stack

- **UI**: `ratatui` + `crossterm` (async TUI)
- **Runtime**: `tokio`
- **DB**: `rusqlite` (SQLite, file `iptv_cache.db`) — tables: `categories`, `movies`, `wishlist`
- **HTTP**: `reqwest` (Xtream Codes API)
- **Images**: `ratatui-image` (Kitty / Sixel / Halfblocks), `image` crate for JPEG decode
- **Regex**: `regex` crate for movie filter
- **Config**: `dotenvy` (`.env`)

## Entrypoint

`src/main.rs` — all other source lives under `src/`.

## Architecture

- **Auth**: On startup, check `.env` for `XTREAM_URL`, `XTREAM_USER`, `XTREAM_PASS`. If missing, show a full-screen config form. On submit, write `.env`, reload, transition to main view.
- **DB schemas**: `categories(id, name)`, `movies(id, name, category_id, stream_id, container_extension, stream_icon, rating, release_date, plot, genre)`, `wishlist(movie_id)`. Indices on `movies(category_id)`, `movies(name)`.
- **Wishlist**: synthetic category with id=-1 prepended to categories list. Persisted in `wishlist` table.
- **Filter**: `/` key in Movies focus toggles regex/substring filter. Case-insensitive. `Esc` clears, `Enter` keeps applied.
- **Sort**: `movie_sort` field (0=name, 1=year↓, 2=year↑, 3=rating↓, 4=rating↑, 5=name↑, 6=name↓). Saved per session in `iptv_layout.dat`. Categories sort via `cat_sort_mode` (0=name, 1=id, 2=name desc).
- **Poster pipeline**:
  - Memory cache hit (`poster_img_cache`, 50-entry LRU of `DynamicImage`): create fresh protocol instantly on main thread.
  - Disk cache hit (`poster_cache/{hash}`): async `load_poster_fast` (tokio spawn + spawn_blocking decode).
  - HTTP miss: `load_poster` with 50ms debounce + HTTP fetch.
  - Old poster stays visible during scrolling; `drain_posters` swaps in new result when gen matches.
- **Layout** (Ratatui `Layout`):
  - Top bar: height 3, app title + version.
  - Main workspace (remaining): 3 columns — Categories, Movies, Details.
  - Details only rendered when focus is Movies.
  - Log pane: height 3, toggled via `Ctrl+D`.
- **Key bindings** (help screen via `?`):
  - Navigation: `j`/`k`/`Up`/`Down` scroll, `gg`/`G` jump to begin/end, `Tab`/`BackTab` cycle Categories↔Movies.
  - Sorting: `r` (rating), `a` (alpha), `y` (year), `o` (default), `c` (category order).
  - Actions: `Enter` select cat, `v` VLC, `f` full poster, `Space` wishlist, `L` reload poster, `w` wishlist cat, `h` stats, `/` filter, `Ctrl+R` sync, `Ctrl+D` log, `q` quit, `?` help.
  - Resize: `(`/`)` columns, mouse drag gutters.
  - All prefs saved to `iptv_layout.dat` on quit.

## Conventions

- **`tokio::main`** on `main`.
- App state in a single struct, drawn each frame with `ratatui`'s immediate-mode loop.
- Async DB ops spawned via `tokio::spawn`, results sent back via channels or shared state.
- Modular: `app.rs` (state/loop), `db.rs` (SQLite), `api.rs` (Xtream client), `ui/` (render functions per pane), `config.rs` (`.env` handling).
- Use `anyhow` for error propagation.
- Keep `.env` out of version control (already in `.gitignore`).

## Commands

- `cargo run` — launches the TUI.
- `cargo run --release` — optimized build.
- `cargo build` — standard debug build.
- `cargo fmt && cargo check` — style + typecheck before committing.
- `cargo test` — runs 25+ unit tests (pure functions, DB ops, API deserialization).

## Tests

- Pure functions: `hash_url`, `is_printable`, `parse_movie_tags`, `Config::api_base`, `Config::stream_url` (no setup needed).
- DB: `toggle_wishlist`, `load_wishlist`, `open_creates_tables` (uses `:memory:` SQLite, no file I/O).
- API: `parse_category_id` deserialization via `serde_json`.

## Notes

- `.env` is created at runtime by the user, never shipped.
- Xtream Codes API base URL is `{XTREAM_URL}/player_api.php?username={XTREAM_USER}&password={XTREAM_PASS}`.
- VOD endpoints: `action=get_vod_categories`, `action=get_vod_streams`.
- Stream URL: `{XTREAM_URL}/movie/{USER}/{PASS}/{STREAM_ID}.{EXT}`.
- `poster_cache/` is gitignored.
- `iptv_layout.dat` stores column ratios, sort modes, last category/movie offset (8 comma-separated values).