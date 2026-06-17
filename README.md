# tuiptv

A terminal-based IPTV media browser for VOD (Video on Demand) catalogs using the Xtream Codes API. Browse movies, view posters, filter, and launch streams in VLC — all from your terminal.

## Features

- **Browse categories and movies** from any Xtream Codes API provider
- **Poster images** displayed in the details pane with Kitty/Sixel/Halfblocks support
- **Wishlist** — mark favorites with Space, persist across sessions via SQLite
- **Sort** movies by name, year, or rating (toggle asc/desc)
- **Filter** movies with `/` — regex or substring matching, case-insensitive
- **Launch VLC** directly from the TUI with `v`
- **Full-screen poster** with `f`
- **Sync** data from server with `Ctrl+R`
- **Help screen** with `?`
- **Disk cache** for poster images (persistent across restarts)
- **In-memory image cache** for instant revisit (50-entry LRU)

## Key Bindings

| Key | Action |
|---|---|
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `gg` | Go to beginning of list |
| `G` | Go to end of list |
| `h` / `Left` | Focus categories pane |
| `l` / `Right` | Focus movies pane |
| `Tab` / `BackTab` | Toggle focus between panes |
| `Enter` | Select category (load movies, focus movies pane) |
| `q` | Quit |
| `Ctrl+R` | Sync data from server |
| `i` | Toggle poster images |
| `r` | Sort movies by rating (toggle) |
| `a` | Sort movies by name (toggle) |
| `y` | Sort movies by year (toggle) |
| `o` | Reset to default sort order |
| `c` | Cycle category sort (name A→Z, provider, name Z→A) |
| `w` | Jump to wishlist category |
| `v` | Open selected movie in VLC |
| `f` | Full-size poster (lightbox) |
| `Space` | Toggle wishlist (add/remove) |
| `L` | Reload poster from disk cache |
| `Ctrl+D` | Toggle log window |
| `(` / `)` | Shrink / grow focused column |
| `/` | Filter movies (regex) |
| `Esc` | Exit filter mode (clears query) |
| `Enter` | Exit filter mode (keeps filter applied) |
| `?` | Toggle help screen |
| Mouse | Click to focus / toggle poster, drag gutters to resize |

## Getting Started

### Prerequisites

- [VLC](https://www.videolan.org/vlc/) installed and in PATH (for the `v` key)
- An Xtream Codes API provider (URL, username, password)

### Configuration

Create a `.env` file in the project root:

```
XTREAM_URL=http://your-provider.com:8080
XTREAM_USER=your_username
XTREAM_PASS=your_password
```

Or run the app — it will show a configuration form on first launch if `.env` is missing.

### Run

```bash
cargo run --release
```

Press `r` to sync the movie catalog from your provider, then browse with `j`/`k`.

### Terminal Support

Poster images work best with terminals supporting the **Kitty** protocol (Kitty terminal, Ghostty, WezTerm). Falls back to **Sixel** (iTerm2, foot) or **Halfblocks** (macOS Terminal, Linux console).

## Data

- **SQLite database** (`iptv_cache.db`) — stores categories, movies, wishlist
- **Poster cache** (`poster_cache/`) — JPEG files keyed by URL hash
- **Layout preferences** (`iptv_layout.dat`) — column widths, sort modes, last-selected category

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Tech Stack

- **UI**: ratatui + crossterm
- **Images**: ratatui-image (Kitty / Sixel / Halfblocks protocols)
- **Runtime**: tokio
- **Database**: rusqlite (bundled SQLite)
- **HTTP**: reqwest
- **Config**: dotenvy