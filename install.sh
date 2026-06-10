#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'
DIM='\033[2m'
BLUE='\033[34m'
GREEN='\033[32m'
RED='\033[31m'
RESET='\033[0m'

banner() { echo -e "\n${BOLD}${BLUE}  $*${RESET}\n"; }
ok()     { echo -e "  ${GREEN}✓${RESET}  $*"; }
err()    { echo -e "  ${RED}✗${RESET}  $*"; }
info()   { echo -e "  ${DIM}→${RESET}  $*"; }

banner "Velo Browser — Installer"

# ── Detect package manager ────────────────────────────────────────────────────

PM=""
if command -v pacman &>/dev/null; then
    PM="pacman"
elif command -v apt-get &>/dev/null; then
    PM="apt"
elif command -v dnf &>/dev/null; then
    PM="dnf"
fi

install_deps_pacman() {
    local pkgs=(base-devel webkit2gtk-4.1 libadwaita
                gst-plugins-base gst-plugins-good gst-plugins-bad
                gst-plugins-ugly gst-libav)
    info "Installing dependencies via pacman..."
    sudo pacman -S --needed --noconfirm "${pkgs[@]}"
}

install_deps_apt() {
    local pkgs=(build-essential pkg-config
                libwebkit2gtk-4.1-dev libadwaita-1-dev
                gstreamer1.0-plugins-base gstreamer1.0-plugins-good
                gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly
                gstreamer1.0-libav)
    info "Installing dependencies via apt..."
    sudo apt-get update -q
    sudo apt-get install -y "${pkgs[@]}"
}

install_deps_dnf() {
    local pkgs=(gcc pkg-config
                webkit2gtk4.1-devel libadwaita-devel
                gstreamer1-plugins-base gstreamer1-plugins-good
                gstreamer1-plugins-bad-free)
    info "Installing dependencies via dnf..."
    sudo dnf install -y "${pkgs[@]}"

    # H.264/AAC/MP3 etc. are patent-encumbered and ship from RPM Fusion, not
    # Fedora's main repos — install them if available, but don't fail the
    # whole script if RPM Fusion isn't enabled.
    local nonfree=(gstreamer1-plugins-ugly gstreamer1-plugins-bad-freeworld gstreamer1-libav)
    if ! sudo dnf install -y "${nonfree[@]}" 2>/dev/null; then
        info "Skipped patent-encumbered codecs (H.264/AAC/MP3 playback)."
        info "Enable RPM Fusion for full media support:"
        info "  https://rpmfusion.org/Configuration"
    fi
}

# ── Rust check ────────────────────────────────────────────────────────────────

if ! command -v cargo &>/dev/null; then
    err "Rust/Cargo not found."
    info "Install Rust:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    info "Then re-run this script."
    exit 1
fi
ok "Rust $(rustc --version | cut -d' ' -f2)"

# ── System dependencies ───────────────────────────────────────────────────────

banner "System dependencies"

MISSING_HINT=""
if [[ "$PM" == "pacman" ]]; then
    install_deps_pacman
elif [[ "$PM" == "apt" ]]; then
    install_deps_apt
elif [[ "$PM" == "dnf" ]]; then
    install_deps_dnf
else
    info "Package manager not detected. Make sure these are installed:"
    info "  WebKitGTK 4.1, libadwaita, GStreamer (base/good/bad/ugly + libav)"
    MISSING_HINT="true"
fi

if [[ -z "$MISSING_HINT" ]]; then
    ok "Dependencies installed"
fi

# ── Build ─────────────────────────────────────────────────────────────────────

banner "Building Velo"

info "Building main browser (release)..."
cargo build --release

info "Building backend service (release)..."
cargo build --release --manifest-path backend/Cargo.toml

ok "Build complete"

# ── Install ───────────────────────────────────────────────────────────────────

banner "Installing"

sudo make install PREFIX=/usr/local

ok "velo           →  /usr/local/bin/velo"
ok "velo-backend   →  /usr/local/bin/velo-backend"
ok "velo.desktop   →  app launcher"
ok "velo.svg       →  icon theme"

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}  Velo is installed.${RESET}"
echo ""
echo -e "  Launch from your app menu or run:  ${BOLD}velo${RESET}"
echo ""
echo -e "  ${DIM}History & bookmarks need the backend running.${RESET}"
echo -e "  ${DIM}Velo starts it automatically — or use Docker:${RESET}"
echo -e "  ${DIM}  docker compose up -d${RESET}"
echo ""
