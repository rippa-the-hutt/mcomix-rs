# Packaging

All packaging assets live under `rust/packaging/`; CI drives them, but each can
be run locally.

| Target | Where | Command |
|---|---|---|
| Debian/Ubuntu `.deb` | `rust/Cargo.toml` `[package.metadata.deb]` + `rust/debian/` | `cargo install cargo-deb && cargo deb` |
| AppImage | `rust/packaging/appimage/build-appimage.sh` | `bash packaging/appimage/build-appimage.sh` |
| Windows zip + installer | `rust/packaging/windows/cross-build-linux.sh` (+ `msys2-build.sh`, `installer.nsi`) | `bash packaging/windows/cross-build-linux.sh` |
| Arch (AUR `mcomix-rs-bin`) | `rust/packaging/arch/PKGBUILD` | `makepkg -si` |
| Linux data files | `rust/packaging/linux/` (`.desktop`, MIME XML, hicolor icons) | copied by the above |

## Notes that save time

- **AppImage**: assembled by hand (binary + `ldd` libs + GSettings schemas +
  Adwaita icons + gdk-pixbuf loaders); `AppRun` regenerates the loader cache at
  runtime. `appimagetool` requires the `.desktop` **and** its `Icon=` PNG at the
  AppDir **root**. The old `linuxdeploy-plugin-gtk` is gone (404) — do not
  reintroduce it.
- **Windows cross-compile**: MSYS2 `mingw-w64` packages are downloaded as a link
  sysroot (the mirror redirects — fetch listings with `curl -fL`). MSYS2 `.pc`
  files use `prefix=/mingw64`, so `PKG_CONFIG_LIBDIR` + `PKG_CONFIG_SYSROOT_DIR`
  must point at the extracted tree. The bundler walks DLL imports and skips
  Windows system DLLs. GStreamer DLLs **must** be bundled (`libgtk-4-1.dll`
  hard-imports them) — a missing one means the app will not start on Windows.
  - **The `PACKAGES` list is only a seed.** The closure is resolved recursively
    from each package's `.PKGINFO`, because MSYS2 turns split packages into empty
    stubs: `gcc-libs` (no files) → `cc-libs` (no files) → `libgcc` (holds
    `libgcc_s_seh-1.dll`) + `libstdc++` (holds `libstdc++-6.dll`). Release 0.4.2
    shipped without those two DLLs and would not start on Windows
    ("libgcc_s_seh-1.dll not found").
  - Names containing `+` (`libstdc++`) are **URL-encoded in the mirror listing**
    (`libstdc%2B%2B`); `pkg_file()` encodes them before searching.
  - The bundler walks the imports of the exe **and of the gdk-pixbuf loaders**
    (the SVG loader needs `librsvg-2-2.dll`), and a verification step **fails the
    build** if any non-system import is unresolved. Never publish a bundle if that
    check reports something.
- **Windows installer** is per-user (`RequestExecutionLevel user`, installs into
  `%LOCALAPPDATA%\Programs\MComix-rs`, HKCU registry) and carries version
  metadata — fewer UAC/antivirus/SmartScreen heuristics than an admin NSIS
  installer. The real fix for AV reputation is code signing; until then the
  release ships `.sha256` files next to the artifacts and the README explains
  the false-positive routine.
- **Icons**: app icon `mcomix-rs.png`, mimetype icons
  `application-x-mcomix-cb{z,r,t}.png` — deliberately distinct from the Python
  `mcomix3` package's files so both can be installed side by side.
- **`.deb` metadata**: `license-file` takes a path (or `[path, line-count]`) —
  passing a *target path* as the second element fails with "invalid number of
  lines".
- **PKGBUILD** builds the release **binary** (no compilation): it downloads the
  Linux tarball plus the source archive at the same tag (for `.desktop`, MIME
  XML and icons) and pins both sha256 sums; `arch=('x86_64')`.
  ⚠️ The PKGBUILD mirrors the **released** version — after cutting a release,
  bump `pkgver` and re-run `updkgsums`, otherwise the pinned hashes/URLs will
  not match the artifacts.

## Local runtime data (for debugging)

- Preferences: `~/.config/mcomix-rs/preferences.conf` (JSON)
- Keybindings: `~/.config/mcomix-rs/keybindings.conf`
- Library / last-read / bookmarks: `~/.local/share/mcomix-rs/`
- Thumbnails: `~/.cache/mcomix-rs/thumbnails/`
- Windows: `%APPDATA%\mcomix-rs\`
