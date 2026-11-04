#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(awk -F'"' '/^version = "/ { print $2; exit }' "$ROOT/Cargo.toml")"
ARCH="$(uname -m)"
[[ "$ARCH" == "x86_64" ]] || { echo "This release builder currently targets x86_64; got $ARCH." >&2; exit 1; }

BIN="$ROOT/target/release/feather"
[[ -x "$BIN" ]] || { echo "Missing release binary. Run: cargo build --release --locked" >&2; exit 1; }

if [[ ! -f "$ROOT/data/icons/vscode-icons/default_file.svg" ]]; then
  bash "$ROOT/scripts/download-vscode-icons.sh"
fi

DIST="$ROOT/dist"
STAGE="$DIST/stage"
ZIPROOT="$STAGE/Feather-$VERSION-linux-x86_64"
APPDIR="$STAGE/Feather.AppDir"
rm -rf "$STAGE"
mkdir -p "$ZIPROOT/bin" "$ZIPROOT/share/applications" \
  "$ZIPROOT/share/icons/hicolor/scalable/apps" \
  "$ZIPROOT/share/gtksourceview-5/styles" \
  "$ZIPROOT/share/feather/vscode-icons/icons" \
  "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
  "$APPDIR/usr/share/gtksourceview-5/styles" \
  "$APPDIR/usr/share/feather/vscode-icons/icons"

install -m 755 "$BIN" "$ZIPROOT/bin/feather"
install -m 644 "$ROOT/data/feather.desktop" "$ZIPROOT/share/applications/feather.desktop"
install -m 644 "$ROOT/data/icons/feather.svg" "$ZIPROOT/share/icons/hicolor/scalable/apps/feather.svg"
install -m 644 "$ROOT/data/styles/feather-mocha.xml" "$ZIPROOT/share/gtksourceview-5/styles/feather-mocha.xml"
install -m 644 "$ROOT/LICENSE" "$ZIPROOT/LICENSE"
cp "$ROOT"/data/icons/vscode-icons/*.svg "$ZIPROOT/share/feather/vscode-icons/icons/"
cp "$ROOT"/data/icons/vscode-icons/ATTRIBUTION.md "$ZIPROOT/share/feather/vscode-icons/icons/"

install -m 755 "$BIN" "$APPDIR/usr/bin/feather"
install -m 644 "$ROOT/data/icons/feather.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/feather.svg"
install -m 644 "$ROOT/data/styles/feather-mocha.xml" "$APPDIR/usr/share/gtksourceview-5/styles/feather-mocha.xml"
cp "$ROOT"/data/icons/vscode-icons/*.svg "$APPDIR/usr/share/feather/vscode-icons/icons/"
cp "$ROOT"/data/icons/vscode-icons/ATTRIBUTION.md "$APPDIR/usr/share/feather/vscode-icons/icons/"

cat > "$ZIPROOT/install.sh" <<'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${FEATHER_PREFIX:-$HOME/.local}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="$PREFIX/bin"
STYLE_DIR="$DATA_HOME/gtksourceview-5/styles"
ICON_DIR="$DATA_HOME/icons/hicolor/scalable/apps"
APP_DIR="$DATA_HOME/applications"
ASSET_DIR="$DATA_HOME/feather"
# Detect missing shared libraries before changing the user's installation.
if command -v ldd >/dev/null 2>&1; then
  MISSING="$(ldd "$ROOT/bin/feather" 2>/dev/null | awk '/not found/ { print $1 }' || true)"
  if [[ -n "$MISSING" ]]; then
    echo "Feather is missing runtime libraries: $MISSING" >&2
    echo "Install the GTK 4 and GtkSourceView 5 runtime packages for your distribution:" >&2
    echo "  Ubuntu/Debian: sudo apt install libgtk-4-1 libgtksourceview-5-0" >&2
    echo "  Fedora:        sudo dnf install gtk4 gtksourceview5" >&2
    echo "  Arch family:   sudo pacman -S gtk4 gtksourceview5" >&2
    exit 1
  fi
fi
mkdir -p "$BIN_DIR" "$STYLE_DIR" "$ICON_DIR" "$APP_DIR" "$ASSET_DIR"
install -m 755 "$ROOT/bin/feather" "$BIN_DIR/feather"
install -m 644 "$ROOT/share/gtksourceview-5/styles/feather-mocha.xml" "$STYLE_DIR/"
install -m 644 "$ROOT/share/icons/hicolor/scalable/apps/feather.svg" "$ICON_DIR/"
rm -rf "$ASSET_DIR/vscode-icons"
mkdir -p "$ASSET_DIR/vscode-icons"
cp -a "$ROOT/share/feather/vscode-icons" "$ASSET_DIR/"
install -m 644 "$ROOT/share/applications/feather.desktop" "$APP_DIR/feather.desktop"
sed -i "s|^Exec=.*|Exec=$BIN_DIR/feather %F|" "$APP_DIR/feather.desktop"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -f "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true; fi
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true; fi
printf 'Feather installed to %s/feather\n' "$BIN_DIR"
printf 'Launch with: %s/feather\n' "$BIN_DIR"
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) printf 'Add %s to PATH if it is not already present.\n' "$BIN_DIR";; esac
INSTALL
chmod 755 "$ZIPROOT/install.sh"

cat > "$ZIPROOT/uninstall.sh" <<'UNINSTALL'
#!/usr/bin/env bash
set -euo pipefail
PREFIX="${FEATHER_PREFIX:-$HOME/.local}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
rm -f "$PREFIX/bin/feather"
rm -f "$DATA_HOME/applications/feather.desktop"
rm -f "$DATA_HOME/icons/hicolor/scalable/apps/feather.svg"
rm -f "$DATA_HOME/gtksourceview-5/styles/feather-mocha.xml"
rm -rf "$DATA_HOME/feather/vscode-icons"
printf 'Feather application files removed. User projects and editor settings were not touched.\n'
UNINSTALL
chmod 755 "$ZIPROOT/uninstall.sh"



# Keep the AppImage launcher independent of the developer's source tree.
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "$0")")"
export FEATHER_BUNDLE_DIR="$HERE/usr/share/feather"
exec "$HERE/usr/bin/feather" "$@"
APPRUN
chmod 755 "$APPDIR/AppRun"
cat > "$APPDIR/usr/share/applications/feather.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Feather
Comment=Lightweight native code editor
Exec=AppRun %F
Icon=feather
Terminal=false
Categories=Development;TextEditor;
MimeType=text/plain;
DESKTOP
cp "$ROOT/data/icons/feather.svg" "$APPDIR/feather.svg"

mkdir -p "$DIST"
(
  cd "$STAGE"
  zip -qr "$DIST/Feather-$VERSION-linux-x86_64.zip" "Feather-$VERSION-linux-x86_64"
)

(
  cd "$ROOT"
  zip -qr "$DIST/Feather-$VERSION-source.zip" . \
    -x "./.git/*" "./target/*" "./dist/*"
)

if command -v linuxdeploy >/dev/null 2>&1; then
  (cd "$STAGE" && linuxdeploy --appdir "$APPDIR" --plugin gtk --output appimage)
  APPIMAGE="$(find "$STAGE" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
  [[ -n "$APPIMAGE" ]] || { echo "linuxdeploy did not produce an AppImage." >&2; exit 1; }
  mv "$APPIMAGE" "$DIST/Feather-$VERSION-x86_64.AppImage"
else
  echo "linuxdeploy not found; ZIP created, AppImage skipped." >&2
fi

echo "Release artifacts:"
ls -lh "$DIST"/Feather-"$VERSION"-linux-x86_64.zip "$DIST"/Feather-"$VERSION"-source.zip "$DIST"/Feather-"$VERSION"-x86_64.AppImage 2>/dev/null || true
