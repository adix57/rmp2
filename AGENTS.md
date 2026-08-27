# AGENTS.md

## Project

rmp - a simple Rust TUI media player. Backed by mpv (audio-only). Songs are
categorized by a tag system; checked tags define the play queue. Supports both
offline files and online streams (mpv/yt-dlp handles the stream).

## Commands

- Build: `cargo build`
- Check: `cargo check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test`
- Format: `cargo fmt --check` (format with `cargo fmt`)

## Architecture

Single binary with two modes:

- `rmp --daemon`: headless long-running server. Owns the mpv subprocess, the
  SQLite library, and the queue/playback engine. Exposes a JSON protocol over a
  Unix socket at `~/.config/rmp2/rmp.sock`. Survives TUI exit (background play).
- `rmp` (default): spawns the daemon if absent, then runs the ratatui TUI. The
  TUI is a thin client: it forwards keybindings and renders daemon state
  snapshots. `Shift+q` detaches only; `q`/`esc` (after confirm) shuts the
  daemon down.

Everything lives under `~/.config/rmp2`:
- `config.toml` - user config (keybinds, paths, defaults)
- `library.sqlite3` - media library DB
- `rmp.sock` - daemon IPC socket
- `rmp.pid` - daemon pid file
- `last-state.json` - persisted volume/repeat/shuffle/active tags
- `rmp.log` - daemon log

## Module layout

- `src/main.rs` - entry, argument handling, daemon spawn/attach
- `src/daemon.rs` - socket server, playback + queue engine, mpv lifecycle
- `src/config.rs` - config dir paths, config.toml parsing/defaults
- `src/db.rs` - SQLite schema + queries (rusqlite bundled)
- `src/mpv.rs` - mpv subprocess + JSON-RPC IPC client
- `src/proto.rs` - JSON protocol messages shared by daemon and TUI
- `src/state.rs` - persisted last-state
- `src/ui/mod.rs` - ratatui app shell
- `src/ui/keymap.rs` - configurable keybindings -> actions
- `src/ui/panes.rs` - Filter / Queue / State panes + status bar
- `src/ui/dialog.rs` - modal overlays (add/edit/search/confirm)

## Conventions

- No code comments unless explicitly requested; keep code self-explanatory.
- ASCII only in the UI - no unicode/special symbols.
- Keybindings are configurable; never hardcode a key in the UI layer.
- Queue is newest-added-first; shuffle affects play order, not list display.
- Repeat (`r`) cycles off -> repeat-all -> repeat-one.
- `Space` toggles tags in the Filter pane and play/pause in the Queue pane.
- `Shift+letter` keys arrive as uppercase from terminals (documented caveat).

## Key details

- mpv is spawned with `--input-ipc-server` and controlled over JSON IPC. Pull
  metadata (title/artist/bitrate/duration) from mpv properties - no metadata
  crate.
- Active tag filter = media matching ALL checked tags.
- `/` search applies a regex over a precomposed lowercase searchable blob
  (title + artist + uri + tags).
- `f` toggles the reserved `favorite` tag.

## Testing

Keep queue/tag filtering, repeat/shuffle state machine, and protocol
serialization pure so they are unit-testable. TUI and mpv are not unit-tested.