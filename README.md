# rmp - Rust Media Player

A simple TUI media player backed by [mpv](https://mpv.io) (audio-only). Songs
are categorized with a tag system; checked tags define the play queue. Supports
both offline files and online streams (mpv/yt-dlp handles the stream).

## Features

- Tag-based queue filtering: only media matching ALL checked tags play.
- Favorite tag (`f`) to mark songs you like.
- Repeat (`r`) cycles off -> repeat-all -> repeat-one.
- Shuffle (`s`) plays in random order without reordering the list.
- Regex search (`/`) over title, artist, uri, and tags.
- Mini-queue (below the Filter pane, 35% height): `Shift+a` pins the selected
  track so it plays next regardless of tag filters; it drains in the order
  added before the regular queue continues.
- Background daemon: `Shift+q` detaches while playback continues; reattach by
  running `rmp` again. `q`/`esc` (with confirmation) shuts the daemon down.
- Configurable keybindings via `config.toml`.
- Metadata pulled directly from mpv properties (no metadata crate).

## Requirements

- [mpv](https://mpv.io)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) for online streams
- A Rust toolchain (edition 2024) to build.

## Build & Run

```sh
cargo build --release
rmp              # starts the daemon if needed, then opens the TUI
rmp --daemon     # headless background server only
```

Everything lives under `~/.config/rmp2` (override with `RMP2_DIR`):

- `config.toml` - keybinds, mpv binary, defaults
- `library.sqlite3` - media library DB
- `last-state.json` - persisted volume/repeat/shuffle/active tags/position
- `rmp.sock` / `rmp.pid` / `rmp.log` - daemon IPC + logging

## Default Keybindings

Bound in `src/config.rs` (all remappable in `config.toml`):

- `Enter` play selected, `Space` play/pause, `j`/`k` navigate, `h`/`l` cycle panes
- `n`/`p` next/previous, `r` repeat cycle, `s` shuffle
- `a` add media, `e` edit media, `f` toggle favorite
- `Shift+a` add selected media to the mini-queue (exception list), `b` toggle focus to/from it
- `d` delete (confirm in main queue; from the mini-queue removes without confirmation)
- `Ctrl+j`/`Ctrl+k` reorder the selected mini-queue item down/up
- `/` search, `Shift+h`/`Shift+l` or `left`/`right` seek backward/forward (5s)
- `Tab` cycle sections, `q`/`esc` quit (confirm), `Shift+q` detach to background
