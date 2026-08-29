# Installing rmp

The project builds from source; there are no prebuilt packages yet. You only
need the Rust toolchain and the two runtime prerequisites, `mpv` and
`yt-dlp`.

## Prerequisites

- [Rust](https://rustup.rs) (edition 2024; a recent stable toolchain)
- [mpv](https://mpv.io) - playback engine
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) - required only for online streams

## Building from source

```sh
git clone https://github.com/adix57/rmp2.git
cd rmp2
cargo build --release
```

The binary is produced at `target/release/rmp`. Install it somewhere on your
`PATH`:

```sh
install -Dm755 target/release/rmp ~/.local/bin/rmp   # per-user
# or, system-wide:
sudo install -Dm755 target/release/rmp /usr/local/bin/rmp
```

Verify:

```sh
rmp --help   # prints usage if installed correctly
rmp          # starts the daemon on first use and opens the TUI
```

## Installing by operating system

### Linux

Install the runtime dependencies with your package manager, then follow the
build steps above.

Debian / Ubuntu:

```sh
sudo apt install mpv yt-dlp
```

Fedora:

```sh
sudo dnf install mpv yt-dlp
```

Arch / Manjaro:

```sh
sudo pacman -S mpv yt-dlp
```

If you do not have Rust yet: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

### macOS

```sh
brew install mpv yt-dlp rust
```

Then build from source as above.

### Windows

Windows is not natively supported: the daemon talks to the TUI over Unix
domain sockets, which Windows does not provide. Use
[WSL](https://learn.microsoft.com/windows/wsl/install) (Windows Subsystem for
Linux). Inside WSL, follow the Linux instructions; run `rmp` from within the
WSL terminal (e.g. Windows Terminal).

## Optional: autostart the daemon

The daemon survives the TUI and keeps playing in the background. If you want
it to always be available, run it as a `systemd` user service.

```sh
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/rmp2.service <<'EOF'
[Unit]
Description=rmp2 media player daemon

[Service]
ExecStart=%h/.local/bin/rmp --daemon
Restart=on-failure

[Install]
WantedBy=default.target
EOF
systemctl --user enable --now rmp2.service
```

The TUI connects to the already-running daemon instead of spawning a new one,
and `q`/`esc` only shuts down `rmp` itself in that case.

## Uninstalling

```sh
rm ~/.local/bin/rmp                          # remove the binary
rm -rf ~/.config/rmp2                        # remove library + config
systemctl --user disable --now rmp2.service  # if you added the service
```