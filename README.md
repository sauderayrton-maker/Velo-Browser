# Velo

A fast, native GTK4 web browser with a dark "Lexus cockpit" aesthetic — built on
WebKitGTK instead of Chromium. Tabs, history, bookmarks, and a quick-notes
applet are all backed by a small local Axum/SQLite service. Applets dock as a
side panel built into the window itself, dimming the page behind them when
open — a subtle Hyprland-style "the environment moves with you" feel.

## Features

- Native WebKitGTK rendering (no Electron/Chromium)
- Tabbed browsing with `libadwaita` `TabView`/`TabBar`
- Applets dock in a side panel built into the window chrome — **History**
  (`Ctrl+H`), **Bookmarks** (`Ctrl+B`, add with `Ctrl+D`), **Notes** (`Ctrl+N`),
  and **Downloads** (`Ctrl+J`)
- **History** panel with search, backed by SQLite
- **Bookmarks** panel
- **Notes** applet — a scratchpad that persists to disk
- **Downloads** applet — files save to `~/Downloads` with live progress,
  cancel, and "show in folder"
- **Self-update** — "Check for Updates…" in the menu pulls, rebuilds, and
  reinstalls the latest commit
- Local-only backend service for history/bookmarks, auto-started on launch
- Dark, low-glare cockpit theme tuned for long sessions

## Compatibility

Velo targets **Linux desktops** running:

- **GTK4** (4.12+) and **libadwaita** (1.4+)
- **WebKitGTK 6.0** (the `webkit2gtk-4.1` package on most distros)
- X11 or Wayland (anything GTK4 supports)

It's been built and tested against Arch Linux. The install script also
supports **Debian/Ubuntu** (`apt`) and **Fedora** (`dnf`) by installing the
equivalent packages. Other distros will need the same dependencies installed
manually:

| Component         | Arch package                           | Debian/Ubuntu package                                    | Fedora package                                       |
|--------------------|------------------------------------------|-------------------------------------------------------------|----------------------------------------------------------|
| Build tools        | `base-devel`                            | `build-essential`, `pkg-config`                            | `gcc`, `pkg-config`                                      |
| WebKitGTK          | `webkit2gtk-4.1`                        | `libwebkit2gtk-4.1-dev`                                     | `webkit2gtk4.1-devel`                                    |
| libadwaita         | `libadwaita`                            | `libadwaita-1-dev`                                          | `libadwaita-devel`                                       |
| GStreamer (core)   | `gst-plugins-base`, `gst-plugins-good`  | `gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good`    | `gstreamer1-plugins-base`, `gstreamer1-plugins-good`     |
| GStreamer (codecs) | `gst-plugins-bad`, `gst-plugins-ugly`, `gst-libav` | `gstreamer1.0-plugins-bad`, `gstreamer1.0-plugins-ugly`, `gstreamer1.0-libav` | `gstreamer1-plugins-bad-free`, plus `gstreamer1-plugins-ugly`, `gstreamer1-plugins-bad-freeworld`, `gstreamer1-libav` from [RPM Fusion](https://rpmfusion.org/Configuration) |

The "codecs" row covers H.264, AAC, MP3 and similar formats used for video
playback (e.g. YouTube). Without them, pages still load but video may fail to
play. On Fedora these are patent-encumbered and ship from RPM Fusion rather
than the main repos — `install.sh` installs what it can and prints a note if
RPM Fusion isn't enabled.

You'll also need a recent [Rust toolchain](https://rustup.rs) (stable, 2021
edition).

## Installation

### Quick install (recommended)

```bash
git clone https://github.com/sauderayrton-maker/Velo-Browser.git
cd Velo-Browser
./install.sh
```

This detects your package manager, installs the system dependencies above,
builds `velo` and `velo-backend` in release mode, and installs both binaries
plus a desktop entry and icon via `make install` (requires `sudo` for the
final install step).

### Manual build

```bash
cargo build --release                              # the browser
cargo build --release --manifest-path backend/Cargo.toml  # the backend
sudo make install PREFIX=/usr/local                # install both + desktop entry
```

Run it with `velo`, or launch "Velo" from your application menu.

To remove everything Velo installed:

```bash
make uninstall            # or: ./uninstall.sh
```

This prompts for `sudo` itself, removes `velo`, `velo-backend`, the desktop
entry, and the icon (refreshing the icon cache), and asks before deleting
your local history/bookmarks/notes data. A copy is also installed as
`velo-uninstall`, so it works even if you've deleted this cloned repo.

### Run without installing

```bash
cargo run
```

## Updating

Velo can update itself: open the menu and choose **"Check for Updates…"**.
This fetches the repo it was built from, and if a newer commit exists on the
remote, offers to pull, rebuild, and reinstall it (you'll be prompted for
your password via `pkexec` to finish the install). When it's done, choose
"Restart Now" to relaunch with the new version.

This only works if the cloned repo Velo was built from is still present and
unmodified on disk. From the command line, the same process is:

```bash
./update.sh
```

## The backend service

History and bookmarks are stored by `velo-backend`, a small local REST API
over SQLite. Velo automatically starts it on launch if it isn't already
running — no manual setup required.

- **Storage location**: `$XDG_DATA_HOME/velo` (defaults to `~/.local/share/velo`)
- **Listens on**: `127.0.0.1:7777`, loopback-only
- If the backend fails to start or isn't installed, Velo still works — history
  and bookmarks just won't persist.

### Optional: run the backend in Docker instead

```bash
docker compose up -d
```

This builds and runs `velo-backend` in a container, publishing it to
`127.0.0.1:7777` with its data in a named volume (`velo-data`). Velo will
detect and use it automatically — there's no need to also run the natively
installed copy.

## Security & privacy

- Velo makes no network requests of its own beyond the pages you visit and
  the local backend on `127.0.0.1`.
- The backend binds to **loopback only** by default and sends **no CORS
  headers**, so pages loaded in a tab cannot read or modify your history or
  bookmarks via `fetch()` from another origin.
- All history/bookmark data is stored locally in SQLite — nothing is sent
  off your machine.
- The Docker variant runs the backend as a non-root user and is published
  only to `127.0.0.1`.
- "Check for Updates" runs `git fetch`/`git rev-parse` against the repo's
  configured remote to compare commit hashes — no other network requests are
  made until you choose "Update Now". The actual update (`git pull`, build,
  install) only escalates privileges for the final file-copy step, via
  `pkexec`/`sudo`.

## Keyboard shortcuts

| Shortcut                 | Action                          |
|---------------------------|----------------------------------|
| `Ctrl+T`                  | New tab                          |
| `Ctrl+W`                  | Close tab                        |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab          |
| `Ctrl+1`–`9`              | Jump to tab                      |
| `Ctrl+L` / `F6`           | Focus address bar                |
| `Ctrl+R` / `F5`           | Reload                           |
| `Ctrl+Shift+R` / `Ctrl+F5`| Hard reload (bypass cache)       |
| `Alt+Left` / `Alt+Right`  | Back / forward                   |
| `Ctrl+H`                  | Toggle history panel             |
| `Ctrl+B`                  | Toggle bookmarks panel           |
| `Ctrl+D`                  | Bookmark current page            |
| `Ctrl+N`                  | Toggle notes panel                |
| `Ctrl+J`                  | Toggle downloads panel           |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom in / out / reset       |
| `Escape`                  | Close side panel, or stop loading |
