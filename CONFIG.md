# rmp Configuration

`rmp` reads its settings from `config.toml`. This document explains every
option.

## Config file location

The config lives in the rmp2 config directory together with the library
database, the daemon socket and logs:

| Path                                             | Notes                                    |
| ------------------------------------------------ | ---------------------------------------- |
| `~/.config/rmp2/config.toml`                     | this file                                |
| `~/.config/rmp2/library.sqlite3`                 | media library database                   |
| `~/.config/rmp2/last-state.json`                 | volume/repeat/shuffle/active tags        |
| `~/.config/rmp2/rmp.sock` / `rmp.pid` / `rmp.log` | daemon IPC, pid file, log               |

`~/.config` follows `$XDG_CONFIG_HOME`. Set `RMP2_DIR` to an absolute path to
use a completely different directory instead:

```sh
RMP2_DIR=/mnt/media/rmp rmp
```

On the first launch `rmp` writes a fully commented copy of the default config
to `config.toml` and starts with those defaults. Edit that file and restart.

## Validation and errors

The config is validated on every startup (both the daemon and the TUI). If it
contains an error, `rmp` refuses to start and prints one line per problem:

- `invalid action "..."` - the left-hand side is not a known action name.
- `invalid key "..." for action "..."` - the key string is not recognized.
- `key "..." bound to multiple actions (...)  - the same key is used twice.
- TOML parse errors (bad syntax, duplicate keys) are reported as well.

Binding a key to nothing disables that action's default key(s):

```toml
[keybindings]
delete = "none"   # 'd' no longer deletes
```

## Full annotated example

```toml
# Binary used for playback. Any mpv-compatible or yt-dlp command works.
mpv_binary = "mpv"

# Amount the volume changes per press of volume_up / volume_down.
volume_step = 5

# Seek length in seconds for seek_back / seek_fwd.
seek_step = 5.0

# Border titles of the four panes. Optional; defaults shown.
[titles]
filter = "Filter"      # tag filter pane (top-left)
queue = "Queue"        # main queue pane (right)
mini = "Mini Queue"    # mini-queue pane (bottom-left)
info = "Info"          # media details pane

# Controls. Format: <action> = <key or list of keys>
# Multi-key lists must be quoted strings inside square brackets.
[keybindings]
move_down   = ["j", "down"]
move_up     = ["k", "up"]
prev_section   = "h"
next_section   = "l"
cycle_focus    = "tab"
cycle_focus_back = "backtab"
activate    = "enter"
toggle      = "space"
next_track  = "n"
prev_track  = "p"
volume_up   = "K"          # uppercase letter means Shift
volume_down = "J"
seek_back   = ["H", "left"]
seek_fwd    = ["L", "right"]
repeat      = "r"
shuffle     = "s"
add_media   = "a"
edit_media  = "e"
search      = "/"
favorite    = "f"
add_mini    = "A"
focus_mini  = "b"
delete      = "d"
mini_move_up    = "ctrl+k"
mini_move_down  = "ctrl+j"
confirm_quit = ["q", "esc"]
detach      = "Q"
```

## Top-level options

| Option         | Type    | Default | Meaning                                        |
| -------------- | ------- | ------- | ---------------------------------------------- |
| `mpv_binary`   | string  | `"mpv"` | Executable used for audio playback             |
| `volume_step`  | integer | `5`     | Volume change per `volume_up`/`volume_down`    |
| `seek_step`    | float   | `5.0`   | Seek length in seconds per `seek_back`/`seek_fwd` |

## `[titles]`

Renames the four pane borders. Useful if you show different content or want a
different language. Missing entries fall back to the defaults.

## `[keybindings]`

Every line binds exactly one action to a key (or a list of keys):

```toml
toggle = "space"        # one key
confirm_quit = ["q", "esc"]  # several keys trigger the same action
repeat = "none"         # unbind
search = "ctrl+s"       # a control chord
```

Rules:

- Actions must use their exact names (see below).
- A key may appear in only one action.
- An empty list `[]` also unbinds the action.
- Defaults fill in for anything you do not set; you never need to write the
  whole table.

> Shift caveat: terminals report `Shift+letter` as the uppercase letter, so
> `volume_up = "K"` is bound to `Shift+k`. There is no separate "shift modifier"
> syntax.

## Key syntax

A key is written as one of:

| Form              | Example        | Notes                              |
| ----------------- | -------------- | ---------------------------------- |
| Single character  | `"j"`, `"/"`, `"A"` | letters, digits, symbols; uppercase = Shift |
| `ctrl+<char>`     | `"ctrl+k"`     | control chord                      |
| `f<N>`            | `"f5"`         | function keys `f1` .. `f24`        |
| Named keys        | `"space"`      | see list below                     |

Named keys: `space`, `enter`, `esc`, `tab`, `backtab`, `up`, `down`, `left`,
`right`, `home`, `end`, `pageup`, `pagedown`, `insert`, `delete`,
`backspace`.

## Actions

| Action            | What it does                                              |
| ----------------- | --------------------------------------------------------- |
| `move_up`         | Move the cursor up in the focused list                    |
| `move_down`       | Move the cursor down in the focused list                  |
| `prev_section`    | Focus the previous pane                                   |
| `next_section`    | Focus the next pane                                       |
| `cycle_focus`     | Focus the next pane (same as `next_section`)              |
| `cycle_focus_back`| Focus the previous pane                                   |
| `activate`        | Queue: play selected track; Filter: toggle the tag        |
| `toggle`          | Queue: play/pause; Filter: toggle the tag                 |
| `next_track`      | Skip to the next track in the queue                       |
| `prev_track`      | Return to the previous track                              |
| `volume_up`       | Raise volume by `volume_step`                             |
| `volume_down`     | Lower volume by `volume_step`                             |
| `seek_back`       | Seek backward by `seek_step` seconds                      |
| `seek_fwd`        | Seek forward by `seek_step` seconds                       |
| `repeat`          | Cycle repeat off -> all -> one                            |
| `shuffle`         | Toggle shuffle                                            |
| `add_media`       | Open a dialog to add a file, path, or stream URL          |
| `edit_media`      | Open a dialog to edit the selected media                  |
| `search`          | Open the regex search bar                                 |
| `favorite`        | Toggle the reserved `favorite` tag                        |
| `add_mini`        | Pin the selected track to the mini-queue (plays next)     |
| `focus_mini`      | Jump focus to/from the mini-queue                         |
| `delete`          | Delete the selected item (confirmed in the main queue)    |
| `mini_move_up`    | Move the selected mini-queue item up                      |
| `mini_move_down`  | Move the selected mini-queue item down                    |
| `confirm_quit`    | Quit the app (with confirmation); stops the daemon        |
| `detach`          | Detach the TUI; playback continues in the daemon          |

## Default keybindings

If a section is missing entirely from `config.toml`, these defaults apply:

| Action             | Keys                    |
| ------------------ | ----------------------- |
| `move_down`        | `j`, `down`             |
| `move_up`          | `k`, `up`               |
| `prev_section`     | `h`                     |
| `next_section`     | `l`                     |
| `cycle_focus`      | `tab`                   |
| `cycle_focus_back` | `backtab`               |
| `activate`         | `enter`                 |
| `toggle`           | `space`                 |
| `next_track`       | `n`                     |
| `prev_track`       | `p`                     |
| `volume_up`        | `K`                     |
| `volume_down`      | `J`                     |
| `seek_back`        | `H`, `left`             |
| `seek_fwd`         | `L`, `right`            |
| `repeat`           | `r`                     |
| `shuffle`          | `s`                     |
| `add_media`        | `a`                     |
| `edit_media`       | `e`                     |
| `search`           | `/`                     |
| `favorite`         | `f`                     |
| `add_mini`         | `A`                     |
| `focus_mini`       | `b`                     |
| `delete`           | `d`                     |
| `mini_move_up`     | `ctrl+k`                |
| `mini_move_down`   | `ctrl+j`                |
| `confirm_quit`     | `q`, `esc`              |
| `detach`           | `Q`                     |