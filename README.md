# Feather 0.5.5

Feather is a small native Linux code editor built with Rust, GTK4 and GtkSourceView 5. It deliberately avoids Electron and browser-based UI layers.

## Changes in 0.5.0

- Refined Catppuccin Mocha interface with subdued selection and cursor colors; no pure-white UI highlights.
- Custom Feather feather-mark beside the application title.
- Smaller initial window size: 980 × 640.
- Close button on every editor tab.
- Drag tabs horizontally to reorder them. Tab/document lookup is tied to the page widget, so Save and tab switching continue to target the correct document after reordering.
- Open a workspace or file from the terminal using a positional path.
- Existing keyboard shortcuts are retained.

## Ubuntu dependencies

```bash
sudo apt update
sudo apt install -y \
  build-essential \
  pkg-config \
  libgtk-4-dev \
  libgtksourceview-5-dev
```

Install Rust if needed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

## Build and run

Development build:

```bash
cargo build
cargo run
```

Release build:

```bash
cargo build --release
./target/release/feather
```

## Terminal command

Install Feather and its Catppuccin syntax scheme into your user environment:

```bash
./scripts/install.sh
```

The executable is installed into Cargo's user binary directory (normally `~/.cargo/bin`). Ensure that directory is in your `PATH` (for Bash, add `export PATH="$HOME/.cargo/bin:$PATH"` to `~/.bashrc`), then:

```bash
feather                 # Open the current working directory
feather .               # Open the current working directory
feather ~/Projects/app  # Open this folder as the workspace
feather src/main.rs     # Open this file; Explorer starts in the current directory
```

A quoted `"~/Projects/app"` is also expanded by Feather itself.

## Existing keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+N` | Create a new file (prompts for filename and extension) |
| `Ctrl+O` | Open file |
| `Ctrl+S` | Save |
| `Ctrl+Shift+S` | Save As |
| `Ctrl+Shift+O` | Open folder |
| `Alt+Left` | Previous workspace directory |
| `Alt+Right` | Next workspace directory |
| `Alt+Up` | Open parent directory as workspace |
| `Ctrl+Shift+R` | Refresh Explorer |

## Explorer

- Click a folder to expand or collapse it.
- Click a file to open it.
- Folder entries are sorted before files, alphabetically within each group.
- Folders are populated lazily when expanded.
- Use the navigation controls or shortcuts to move between workspace roots.
- Hidden files can be toggled from the Explorer toolbar.

## Tabs

- Click the close icon on a tab to close it.
- Modified tabs show a centered, dark save/discard confirmation.
- Drag a tab left or right to change its position.
- Tab selection and saving remain associated with the document even after tabs are reordered.

## Current limitations

This remains an early editor build. Find/Replace, Go to Line and crash recovery are not implemented yet. File I/O is currently synchronous and files must be valid UTF-8 text. Keep backups of important files while testing.


## Feather 0.3.2 UI refinement

- A Catppuccin-styled GTK titlebar with visible window controls.
- A welcome view is shown when no document is open; it replaces the blank editor canvas.
- The Help button beside Save and `F1` open the keyboard-shortcut reference.
- The editor and welcome surfaces use the Catppuccin base color.
- Scrollbars, tabs, close controls, and welcome actions have refined styling.

### Help

Use the **Help** toolbar button or press `F1` to view the shortcuts supported by this build.


## Feather 0.5.0

- Refined Explorer and Workspace spacing, tree row rhythm and selection styling.
- Added a custom Feather quill mark used in the title area and welcome screen.
- Added an original scalable SVG Linux application icon and desktop entry installation.
- Replaced the native white MessageDialog help UI with a fully themed, scrollable Catppuccin help window.
- Expanded file-type icon mapping for languages, configuration, documentation, media and archives.
- Refined tab, scrollbar, toolbar and status-bar styling.


## Feather 0.5.0 — MVP reliability

- Unsaved-change confirmation when closing a tab or exiting the application.
- Gio file monitoring for opened documents: clean buffers reload; dirty buffers are preserved and flagged.
- Binary-file detection and a 10 MiB text-file safety limit.
- Persistent last-workspace restoration under `$XDG_CONFIG_HOME/feather/state` (or `~/.config/feather/state`).
- Unit tests for binary detection and size formatting.
- GTK notebook reorder regression test (display required; ignored by default).
- Installed desktop icon and style smoke-test script.

### Verification

```bash
cargo test
cargo test -- --ignored --test-threads=1   # requires an active GTK desktop session
./tests/mvp-smoke-test.sh
```

External-change policy: Feather never silently replaces a modified editor buffer. If the buffer is dirty, the external change is reported in the status bar and the user's in-memory edits remain intact. Resolve the conflict manually before saving.


## Feather 0.5.1 — UI and installation corrections

This is a separate versioned source directory so it cannot be confused with an older `~/Feather` checkout.

```bash
unzip Feather-0.5.1.zip -d ~/Apps
cd ~/Apps/Feather-0.5.1
grep '^version' Cargo.toml
./scripts/install.sh
cargo test
```

The installer prints the exact source directory and version before building, and uses `cargo install --force` so the installed `~/.cargo/bin/feather` is replaced by this release.

UI fixes:
- Feather quill is drawn in a square coordinate space with uniform scaling and centered alignment.
- Help uses a GTK client-side `HeaderBar`, avoiding the compositor's default light titlebar where GTK client-side decorations are supported.
- Help titlebar, close controls and body use Catppuccin surfaces.
- Added a dedicated C/C++ file icon and corrected C/C++ icon mapping.
- Replaced an unsupported Explorer collapse icon name with a standard symbolic icon.

Verification note: run `cargo test` from the extracted `Feather-0.5.1` directory. Running it from an older `Feather` directory will correctly test that older checkout instead.


## Feather 0.5.2 — Rust temporary lifetime fix

Fixed the modified-tab-title update by binding the formatted title to an owned `String` before passing it to GTK. This avoids borrowing a temporary value that is dropped at the end of the conditional expression.

```rust
let display_title = if buffer.is_modified() {
    format!("{title} •")
} else {
    title
};
title_for_modified.set_text(&display_title);
```


## Feather 0.5.3 — WSL installer version parsing

Fixed `scripts/install.sh`: the previous version extraction used an incorrectly escaped `sed` expression and compared against a stale hard-coded `0.5.1` value. The installer now reads the package version using `awk -F'"'` and validates that the value is non-empty, so the manifest and installer cannot drift apart.

This does not change the GUI or editor behavior.


## Feather 0.5.4 — upstream VSCode file icons

- Replaced Feather-authored file-type SVGs with unmodified SVG assets from `vscode-icons/vscode-icons`.
- Installer downloads the upstream repository's `icons/` directory and installs it under `$XDG_DATA_HOME/feather/vscode-icons/icons` (normally `~/.local/share/feather/vscode-icons/icons`).
- File icon resolution uses upstream `file_type_<id>.svg` naming; unknown file types use upstream `default_file.svg`.
- Folder rows use upstream `default_folder.svg` and `default_folder_opened.svg`, with folder-name-specific variants when present.
- Added upstream attribution and licensing notice. Branded icons may have separate copyright licenses.
- Removed invalid GTK CSS declarations (`text-align: left` and `margin: auto`) that caused GTK parser warnings.
- Added `feather --version` and `feather --help` handling before GTK initialization.

The file and folder SVG assets are downloaded unmodified from https://github.com/vscode-icons/vscode-icons.

## Feather 0.5.5 — polished dialogs and new-file workflow

- Replaced light GTK message dialogs with Feather-themed dark modal windows.
- Unsaved-change and error dialogs are attached to the main window as transient windows, so they remain centered relative to the editor.
- New File now prompts for a filename including its extension before creating the file in the active workspace. Existing files are never overwritten.
- Removed unused Rust captures and quieted installer/build output for a cleaner release experience.


## Production Linux packages

See [RELEASE-PACKAGING.md](RELEASE-PACKAGING.md) for the production ZIP installer and AppImage pipeline. End-user packages include a precompiled binary; Rust and Cargo are only required on the build machine.
