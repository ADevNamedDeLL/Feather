#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/data/icons/vscode-icons"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
REPO_URL="https://codeload.github.com/vscode-icons/vscode-icons/zip/refs/heads/master"
ARCHIVE="$TMP/vscode-icons.zip"
command -v curl >/dev/null || { echo "curl is required." >&2; exit 1; }
command -v unzip >/dev/null || { echo "unzip is required." >&2; exit 1; }
curl --silent --show-error --fail --location --retry 3 --connect-timeout 20 "$REPO_URL" -o "$ARCHIVE"
unzip -q "$ARCHIVE" -d "$TMP"
UPSTREAM="$(find "$TMP" -mindepth 2 -maxdepth 2 -type d -name icons -print -quit)"
if [[ -z "$UPSTREAM" ]] || [[ ! -f "$UPSTREAM/default_file.svg" ]] || [[ ! -f "$UPSTREAM/default_folder.svg" ]]; then
    echo "Downloaded repository did not contain the expected VSCode icon assets." >&2
    exit 1
fi
mkdir -p "$DEST"
find "$DEST" -maxdepth 1 -type f -name '*.svg' -delete
cp "$UPSTREAM"/*.svg "$DEST/"
cat > "$DEST/ATTRIBUTION.md" <<'NOTICE'
# File and folder icon assets

Unmodified SVG assets from vscode-icons:
- Project: https://github.com/vscode-icons/vscode-icons
- Authors: vscode-icons project contributors
- Icon license: Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)
- License: https://creativecommons.org/licenses/by-sa/4.0/

The upstream project states that branded icons may be subject to separate copyright licenses.
Feather does not claim ownership of these assets. The Feather application logo is separate.
NOTICE
