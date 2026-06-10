#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'
DIM='\033[2m'
BLUE='\033[34m'
GREEN='\033[32m'
RESET='\033[0m'

banner() { echo -e "\n${BOLD}${BLUE}  $*${RESET}\n"; }
ok()     { echo -e "  ${GREEN}✓${RESET}  $*"; }
info()   { echo -e "  ${DIM}→${RESET}  $*"; }

PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDIR="$PREFIX/share/icons/hicolor/scalable/apps"

banner "Velo Browser — Uninstaller"

info "Removing files from $PREFIX..."
sudo rm -f "$BINDIR/velo" "$BINDIR/velo-backend" "$BINDIR/velo-uninstall"
sudo rm -f "$APPDIR/velo.desktop"
sudo rm -f "$ICONDIR/velo.svg"
sudo update-desktop-database "$APPDIR" 2>/dev/null || true
sudo gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true

ok "Velo removed from $PREFIX"

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/velo"
if [[ -d "$DATA_DIR" ]]; then
    echo ""
    read -rp "  Also delete your Velo data — history, bookmarks, notes (${DATA_DIR})? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
        rm -rf "$DATA_DIR"
        ok "Data removed"
    else
        info "Kept $DATA_DIR"
    fi
fi

echo ""
echo -e "${BOLD}  Velo has been uninstalled.${RESET}"
echo ""
