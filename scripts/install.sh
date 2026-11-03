#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(awk -F'"' '/^version = "/ { print $2; exit }' "$ROOT/Cargo.toml")"
if [[ -z "$VERSION" ]]; then
    echo "Could not read Feather version from $ROOT/Cargo.toml" >&2
    exit 1
fi
echo "Installing Feather v$VERSION from: $ROOT"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
STYLE_DIR="$DATA_HOME/gtksourceview-5/styles"
ICON_DIR="$DATA_HOME/icons/hicolor/scalable/apps"
APP_DIR="$DATA_HOME/applications"
VSCODE_ICON_DIR="$DATA_HOME/feather/vscode-icons/icons"

mkdir -p "$STYLE_DIR" "$ICON_DIR" "$APP_DIR" "$VSCODE_ICON_DIR"
install -m 644 "$ROOT/data/styles/feather-mocha.xml" "$STYLE_DIR/feather-mocha.xml"
install -m 644 "$ROOT/data/icons/feather.svg" "$ICON_DIR/feather.svg"
"$ROOT/scripts/download-vscode-icons.sh"
install -m 644 "$ROOT/data/icons/vscode-icons/"*.svg "$VSCODE_ICON_DIR/"
install -m 644 "$ROOT/data/icons/vscode-icons/ATTRIBUTION.md" "$VSCODE_ICON_DIR/ATTRIBUTION.md"
install -m 644 "$ROOT/data/feather.desktop" "$APP_DIR/feather.desktop"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
cargo install --quiet --path "$ROOT" --locked --force

printf '\nFeather v%s installed.\n' "$VERSION"
printf 'Installed binary: %s/feather\n' "${CARGO_HOME:-$HOME/.cargo}/bin"
printf 'Run `feather` to open the current directory, or `feather /path/to/project`.\n'
