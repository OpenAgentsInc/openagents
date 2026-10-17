# Packaging

## Linux (implemented)

```sh
scripts/package-linux.sh            # release build (thin LTO, stripped)
PROFILE=debug scripts/package-linux.sh   # fast smoke package
```

Produces `target/package/openagents-<version>-linux-<arch>.tar.gz` containing:

- `openagents` — the binary (headed by default; `openagents headless` runs the engine alone)
- `openagents.desktop` — XDG desktop entry template (`Exec=openagents` for packagers;
  the installers rewrite `Exec`, `TryExec`, and `Icon` to absolute paths under
  `~/.openagents/app/current`, since `~/.local/bin` is often not on a desktop
  session's `PATH`)
- `openagents.png` — 1024×1024 OpenAgents app icon
- `install.sh` — installs into `~/.openagents/app/<version>` behind a `current`
  symlink (the curl installer's layout, which the in-app updater manages),
  links `~/.local/bin/openagents` to it, and writes the desktop entry and icon under
  `$XDG_DATA_HOME` (default `~/.local/share`). The curl installer does the same
  from the extracted tarball; `scripts/test-linux-desktop-entry.sh` checks both

The release profile in the root `Cargo.toml` sets `lto = "thin"` and
`strip = "symbols"` for distribution builds.

## macOS

```sh
scripts/package-macos.sh    # → target/package/openagents-<version>-macos-<arch>.dmg
```

Builds the release binary, assembles `OpenAgents.app` (Info.plist + icns), ad-hoc
signs it (set `CODESIGN_IDENTITY` for a real Developer ID), and wraps it in a
dmg. The auto-update tarball retains an internal `OpenAgents.app` path so older
installed builds can update into OpenAgents. CI runs this on tags
(`.github/workflows/release.yml`). The manual steps it automates, for reference
(run on a macOS host — gpui needs Metal; no cross-build from Linux):

1. Build the universal (or per-arch) binary:
   ```sh
   cargo build --release -p openagents --target aarch64-apple-darwin
   cargo build --release -p openagents --target x86_64-apple-darwin
   lipo -create -output openagents \
     target/aarch64-apple-darwin/release/openagents \
     target/x86_64-apple-darwin/release/openagents
   ```
2. Assemble the bundle:
   ```sh
   mkdir -p OpenAgents.app/Contents/{MacOS,Resources}
   cp openagents OpenAgents.app/Contents/MacOS/openagents
   sed "s/__VERSION__/$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')/" \
     dist/macos/Info.plist > OpenAgents.app/Contents/Info.plist
   ```
3. Icon: generate `openagents.icns` from `dist/macos/icon-1024.png` (the macOS-shaped
   variant of the artwork — squircle mask, margins, and shadow pre-baked, since
   `sips` can't apply an alpha mask) and place it at
   `OpenAgents.app/Contents/Resources/openagents.icns`:
   ```sh
   mkdir openagents.iconset && sips -z 256 256 dist/macos/icon-1024.png --out openagents.iconset/icon_256x256.png
   iconutil -c icns openagents.iconset -o OpenAgents.app/Contents/Resources/openagents.icns
   ```
4. Sign + notarize (required for distribution):
   ```sh
   codesign --deep --force --options runtime --sign "Developer ID Application: …" OpenAgents.app
   xcrun notarytool submit OpenAgents.zip --keychain-profile … --wait
   xcrun stapler staple OpenAgents.app
   ```
5. Ship as a `.dmg` (`hdiutil create -volname OpenAgents -srcfolder OpenAgents.app -ov -format UDZO OpenAgents.dmg`).

## Windows

```powershell
./scripts/package-windows.ps1 -ReleasesUrl https://github.com/zeronsh/zeron/releases/latest/download
```

Produces, under `target/package/`:

- `openagents-<version>-windows-<arch>-setup.exe` — the per-user installer built
  from `dist/windows/openagents.iss` with Inno Setup 6
- `openagents-<version>-windows-<arch>.zip` — the portable package
- `openagents-<version>-windows-<arch>.exe` — the bare executable the in-app
  updater downloads

The installer and the zip both carry `openagents-update.json`, the marker that lets
the app update itself in place. CI runs `scripts/test-windows-installer.ps1`
against the setup on every Windows build.
