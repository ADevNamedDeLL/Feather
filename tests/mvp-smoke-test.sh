#!/usr/bin/env bash
set -euo pipefail

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
ICON_DIR="$DATA_HOME/feather/vscode-icons/icons"
APP_ICON="$DATA_HOME/icons/hicolor/scalable/apps/feather.svg"
DESKTOP_FILE="$DATA_HOME/applications/feather.desktop"
STYLE_FILE="$DATA_HOME/gtksourceview-5/styles/feather-mocha.xml"

command -v feather >/dev/null || { echo "FAIL: feather binary not found in PATH"; exit 1; }
test -s "$APP_ICON" || { echo "FAIL: application icon missing"; exit 1; }
test -s "$DESKTOP_FILE" || { echo "FAIL: desktop entry missing"; exit 1; }
test -s "$STYLE_FILE" || { echo "FAIL: GtkSourceView style missing"; exit 1; }

grep -q '^Name=Feather$' "$DESKTOP_FILE" || { echo "FAIL: desktop Name entry"; exit 1; }
grep -q '^Exec=feather %F$' "$DESKTOP_FILE" || { echo "FAIL: desktop Exec entry"; exit 1; }
grep -q '^Icon=feather$' "$DESKTOP_FILE" || { echo "FAIL: desktop Icon entry"; exit 1; }

expected=100
actual="$(find "$ICON_DIR" -maxdepth 1 -type f -name '*.svg' | wc -l)"
test "$actual" -ge "$expected" || {
    echo "FAIL: expected at least $expected custom file icons, found $actual"
    exit 1
}

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$DESKTOP_FILE"
fi
if command -v xmllint >/dev/null 2>&1; then
    xmllint --noout "$APP_ICON"
    find "$ICON_DIR" -maxdepth 1 -type f -name '*.svg' -print0 | xargs -0 -r -n1 xmllint --noout
fi

echo "PASS: executable found at $(command -v feather)"
test -s "$ICON_DIR/default_file.svg" || { echo "FAIL: upstream VSCode default file icon missing"; exit 1; }
test -s "$ICON_DIR/default_folder.svg" || { echo "FAIL: upstream VSCode default folder icon missing"; exit 1; }
test -s "$ICON_DIR/ATTRIBUTION.md" || { echo "FAIL: upstream icon attribution missing"; exit 1; }
echo "PASS: application SVG, desktop entry, syntax style and $actual upstream VSCode icons installed"
echo
echo "GUI regression checklist (run in an active Ubuntu desktop session):"
echo "1. Edit a saved file; close its tab; verify the centered dark Cancel, Discard and Save dialog."
echo "2. Use New; enter a filename with an extension; verify the file is created in the workspace before editing."
echo "3. Edit a file externally: a clean buffer reloads; a dirty buffer remains unchanged and shows a conflict warning."
echo "4. Open a binary file and a file larger than 10 MiB; both must be rejected with a clear message."
echo "5. Open a workspace, exit, relaunch with no argument; the workspace should be restored."
echo "6. Drag tabs to reorder; verify tab labels, active document, and close buttons remain attached."
echo "7. Test Ctrl+N (filename prompt), Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+Shift+O, Alt+Left/Right/Up, Ctrl+Shift+R and F1."
