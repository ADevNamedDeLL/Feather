# Feather Linux production packaging

This repository now has a release pipeline for **x86_64 Linux** with two end-user formats:

- ZIP: precompiled executable, VS Code icon SVG assets, GtkSourceView theme, install and uninstall scripts.
- AppImage: portable AppDir built with linuxdeploy's GTK plugin.

End users do not need Rust, Cargo, a compiler, curl, or unzip to install a ZIP release. The ZIP archive contains the precompiled binary and assets. Runtime GTK 4 and GtkSourceView 5 libraries must be installed by the operating system package manager.

## Supported distributions

The ZIP installer is distribution-neutral. It checks runtime dependencies at launch time and prints the relevant package names when libraries are missing:

- Ubuntu / Debian: `libgtk-4-1 libgtksourceview-5-0`
- Fedora: `gtk4 gtksourceview5`
- Arch / Manjaro / CachyOS: `gtk4 gtksourceview5`

The AppImage bundles the GTK runtime dependencies discovered by linuxdeploy. AppImages still depend on the host kernel and a sufficiently recent glibc. This first pipeline targets x86_64; it does not claim ARM64 support.

## Build a release locally

On a Linux x86_64 build machine with Rust, GTK4 development files, GtkSourceView 5 development files, curl, unzip, zip, and linuxdeploy with its GTK plugin installed:

```bash
cargo test --locked
cargo build --release --locked
./scripts/package-release.sh
```

Artifacts are written to `dist/`:

```text
Feather-<version>-linux-x86_64.zip
Feather-<version>-x86_64.AppImage
```

The packager downloads the upstream vscode-icons SVG collection when it is not already present. It includes the upstream attribution and license notice.

## GitHub Actions release

Push a version tag matching the Cargo package version, for example:

```bash
git tag v0.5.7
git push origin v0.5.7
```

The workflow tests, builds and packages on Ubuntu 22.04, then attaches the ZIP and AppImage to the GitHub Release. You can also start the workflow manually from the Actions tab; manual runs upload artifacts without publishing a GitHub Release.

## ZIP installation

```bash
unzip Feather-*-linux-x86_64.zip
cd Feather-*-linux-x86_64
./install.sh
```

Default install locations are `~/.local/bin` and `~/.local/share`. Override the executable prefix with `FEATHER_PREFIX`. Remove Feather with `./uninstall.sh` from the extracted release folder or run the installed copy's uninstall script before deleting the folder.

## AppImage installation

```bash
chmod +x Feather-*-x86_64.AppImage
./Feather-*-x86_64.AppImage
```

No installation step is required. The AppImage carries the application, GtkSourceView theme, and VS Code icons. Its launcher sets `FEATHER_BUNDLE_DIR` so Feather loads bundled resources without modifying the user's XDG data directories.

## Release verification

Before publishing, verify both artifacts on clean Ubuntu/Debian, Fedora, and Arch-family virtual machines. In particular, test:
- AppImage launch on a host without GTK development packages.
- ZIP installation and removal.
- Desktop menu entry and icon refresh.
- Syntax highlighting and VS Code file/folder icons.
- File open/save, tabs, dialogs, and workspace restore.
