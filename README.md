# rmp

A simple terminal media player. Backed by [mpv](https://mpv.io) (audio-only),
driven entirely by the keyboard. Songs are organized with tags; checked tags
define the play queue. Plays local files and online streams alike.

```
+--------------------------------------------------------------------+
| ( Filter )            | ( Queue )                                   |
| [x] rock    (42)      |   Alice In Chains - Nutshell                |
| [ ] jazz    (12)      | > Metallica    - One                        |
| [ ] podcast ( 3)      |   Nirvana      - Heart-Shaped Box           |
| [ ] favorite(18)      |                                             |
|                       +---------------------------------------------+
| ( Mini Queue )        | ( Info )                                    |
| 1. Tool - Schism      |   Title:    One                             |
+-----------------------+   Artist:   Metallica                       |
|  3:41 / 5:29 [====---] Metallica - One             vol 100 rep off  |
+--------------------------------------------------------------------+
```

## Features

- Tag-based queue filtering: media matching ALL checked tags play.
- Favorite tag to mark the songs you like.
- Repeat (off -> all -> one) and shuffle without reordering the list.
- Regex search over title, artist, uri, and tags.
- Mini-queue: pin a track with `Shift+a` so it plays next regardless of tag
  filters; it drains in order before the regular queue resumes.
- Mouse support: click to focus/select, double-click to play, click `rep`/`shf`
  in the status bar to toggle repeat/shuffle.
- Background daemon: detach the TUI with `Shift+q` and playback keeps going;
  reattach by running `rmp` again.
- Fully configurable keybindings and pane titles (see [CONFIG.md](CONFIG.md)).
- Metadata pulled straight from mpv properties - no metadata crate.

## Getting Started

The quickest way to try it:

```sh
cargo build --release          # see INSTALL.md for per-OS setup
rmp                           # spawns the daemon once, then opens the TUI
```

Press `a`, enter a path or stream URL, confirm, and `Enter` to play the
selected track. Toggle tags with `space` (or `Enter`) in the Filter pane to
shape the queue. `Esc` (after confirming) stops the daemon; `Shift+q` leaves it
running in the background.

See [INSTALL.md](INSTALL.md) for installing `mpv`/`yt-dlp` and building on
Linux, macOS, or WSL.

## Prerequisites

- [mpv](https://mpv.io)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) - for online streams
- A [Rust](https://rustup.rs) toolchain (edition 2024) to build

## Installing

Installation is from source for now. Full instructions for Linux, macOS, and
Windows (WSL), plus an optional daemon autostart: **[INSTALL.md](INSTALL.md)**.

## Usage

Everything below can be rebound; keybindings and pane titles are configured in
`config.toml` - see [CONFIG.md](CONFIG.md).

The interface has four sections: Filter (tags), Queue (playlist), Mini Queue,
and Info (details). `Tab` / `h` / `l` move focus between them.

| Key           | Action                                        |
| ------------- | --------------------------------------------- |
| `Enter`       | Play selected                                 |
| `Space`       | Play/pause (queue) or toggle tag (filter)     |
| `j` / `k`     | Move cursor (also `up`/`down`)                |
| `n` / `p`     | Next / previous track                         |
| `r` / `s`     | Repeat cycle / shuffle toggle                 |
| `/`           | Regex search                                  |
| `a` / `A`     | Add media / pin to mini-queue                 |
| `e` / `f`     | Edit selected / toggle favorite               |
| `b`           | Jump to/from the mini-queue                   |
| `d`           | Delete (confirm in main queue)                |
| `Ctrl+k` / `Ctrl+j`| Move mini-queue item up / down             |
| `J` / `K`     | Volume down / up                              |
| `H` / `L`     | Seek backward / forward                       |
| `q` / `esc`   | Quit (with confirmation)                      |
| `Shift+q`     | Detach; playback continues in the daemon      |

The daemon is a separate headless process that owns playback, the library, and
the queue. It starts automatically on first `rmp` launch and lives at
`~/.config/rmp2/` (`config.toml`, `library.sqlite3`, `last-state.json`, and
`rmp.sock` / `rmp.pid` / `rmp.log`). Override the location with `RMP2_DIR`.

## Architecture and Crate Ecosystem

A single binary with two modes. `rmp --daemon` runs a headless server: it owns
the `mpv` subprocess (driven over its JSON IPC socket), the SQLite library,
and the queue/playback engine, and exposes a JSON protocol over a Unix socket.
Plain `rmp` spawns the daemon if needed, then runs a thin TUI client that
forwards keys and renders daemon state snapshots. `Shift+q` detaches; only a
confirmed `q`/`esc` shuts the daemon down.

Source layout:

| Path                | Responsibility                                        |
| ------------------- | ----------------------------------------------------- |
| `src/daemon.rs`     | Socket server, queue/playback engine, mpv lifecycle   |
| `src/mpv.rs`        | mpv subprocess + JSON-RPC IPC client                  |
| `src/db.rs`         | SQLite schema + queries (rusqlite bundled)            |
| `src/engine.rs`     | Pure queue/tag filtering and repeat/shuffle logic     |
| `src/proto.rs`      | JSON protocol messages shared by daemon and TUI       |
| `src/ui/`           | ratatui app shell, keymap + configurable bindings     |
| `src/config.rs`     | Config parsing, defaults, key/action validation       |
| `src/state.rs`      | Persisted last-state (volume/repeat/shuffle/tags)     |

Crates:

| Crate              | Role                                        |
| ------------------ | ------------------------------------------- |
| `ratatui`          | Terminal UI rendering and layout            |
| `crossterm`        | Raw terminal, key/mouse input               |
| `rusqlite`         | SQLite library (bundled)                    |
| `serde`+`serde_json`| JSON protocol + state serialization        |
| `toml`             | `config.toml` parsing                       |
| `regex`            | Search filtering                           |
| `signal-hook`      | Daemon shutdown signal handling             |

## Configuration

All configuration lives in `config.toml` under the config directory - options,
pane titles, and every keybinding. See **[CONFIG.md](CONFIG.md)** for the full
reference (keys syntax, actions, defaults) and an annotated example file. A
commented default `config.toml` is also generated on first run.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major
changes, please open an issue first to discuss what you would like to change.
