# Velo

A fast, native GTK4 web browser with a dark "Lexus cockpit" aesthetic — built on
WebKitGTK instead of Chromium. Tabs, history, bookmarks, and a quick-notes
applet are all backed by a small local Axum/SQLite service, with the UI
reacting (dimming, focus shifts) when applets open — a subtle Hyprland-style
"the environment moves with you" feel.

## Features

- Native WebKitGTK rendering (no Electron/Chromium)
- Tabbed browsing with `libadwaita` `TabView`/`TabBar`
- **History** panel (`Ctrl+H`) with search, backed by SQLite
- **Bookmarks** panel (`Ctrl+B`, add with `Ctrl+D`)
- **Notes** applet (`Ctrl+N`) — a scratchpad that persists to disk
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

| Component        | Arch package                        | Debian/Ubuntu package                          | Fedora package                          |
|-------------------|--------------------------------------|-------------------------------------------------|-------------------------------------------|
| Build tools       | `base-devel`                         | `build-essential`, `pkg-config`                 | `gcc`, `pkg-config`                       |
| WebKitGTK         | `webkit2gtk-4.1`                     | `libwebkit2gtk-4.1-dev`                          | `webkit2gtk4.1-devel`                     |
| libadwaita        | `libadwaita`                         | `libadwaita-1-dev`                               | `libadwaita-devel`                        |
| GStreamer (audio) | `gst-plugins-base`, `gst-plugins-good` | `gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good` | `gstreamer1-plugins-base`, `gstreamer1-plugins-good` |

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
sudo make uninstall PREFIX=/usr/local
```

### Run without installing

```bash
cargo run
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
| `Ctrl+H`                  | Toggle history                   |
| `Ctrl+B`                  | Toggle bookmarks                 |
| `Ctrl+D`                  | Bookmark current page            |
| `Ctrl+N`                  | Toggle notes                     |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom in / out / reset       |
| `Escape`                  | Stop loading                     |
