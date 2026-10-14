---
name: mcomix-rs-release
description: Cut a mcomix-rs release (version bump, merge to main, tag, CI artifacts) and keep the Arch AUR package in sync. Use when the user asks for "a release", "version bump", "trigger the release", or to update the PKGBUILD checksums.
---

# Releasing mcomix-rs

Releases are produced entirely by `.github/workflows/release.yml`; **pushing a
`v*` tag is what triggers it**. Work happens on `develop`, releases come from
`main`.

## Steps

```bash
# 0. Work on develop; make sure it is green
cd rust && cargo test && cargo build --release

# 1. Bump the version (Cargo.toml AND Cargo.lock)
#    Cargo.toml: [package] version = "X.Y.Z"
#    Cargo.lock: the "mcomix-rs" entry's version
cargo build            # refresh the lockfile

# 2. Commit, push develop, merge into main
source ~/.loadsshkeys  # required: pushes use an SSH agent key
git add rust/Cargo.toml rust/Cargo.lock && git commit -m "Bump version to X.Y.Z"
git push origin develop
git checkout main && git merge develop --no-ff -m "Merge develop into main: MComix-rs X.Y.Z"
git push origin main

# 3. Tag → triggers CI (and returns you to develop)
git tag -a vX.Y.Z -m "MComix-rs X.Y.Z" && git push origin vX.Y.Z
git checkout develop
```

Then watch **Actions**; when all jobs are green, **publish the draft release**
from the Releases page (the workflow only creates a draft).

Re-running after a fix: move the tag and force-push it
(`git tag -f vX.Y.Z && git push -f origin vX.Y.Z`).

## CI jobs

| Job | Runner | Artifacts |
|---|---|---|
| `linux` | ubuntu-24.04 | `cargo deb` `.deb` + standalone tarball (runs `cargo test`) |
| `appimage` | ubuntu-24.04 | self-contained `.AppImage` (manual AppDir assembly, no linuxdeploy) |
| `windows-cross` | ubuntu-24.04 | portable `.zip` + NSIS `setup.exe`, **cross-compiled from Linux** |
| `release` | ubuntu-24.04 | draft GitHub Release with every artifact |

The workflow needs `permissions: contents: write` (otherwise publishing fails
with "Resource not accessible by integration").

## After the release

Bump the Arch package and refresh its checksums:

```bash
cd rust/packaging/arch
# pkgver=X.Y.Z, then regenerate the two sha256sums:
updpkgsums            # from pacman-contrib
```

Verify the hashes point at the published artifacts (download and compare with
`sha256sum` if in doubt), then commit on `develop` and merge to `main` (no tag).

AUR package name: **`mcomix-rs-bin`** (binary package built from the release
tarball). Details and local packaging commands: `references/packaging.md`.
