---
title: Tauri iOS Build & App Store Submission
---

This guide explains how to build the OpenAgents iOS app with Tauri and submit it to App Store Connect/TestFlight. See `docs/ios-deployment/README.md` for a higher‑level overview and rationale of our setup (no dev server in release, local HTTP on iOS, versioning, icons, ATS, export compliance).

## Prerequisites

- Xcode 17.0+ (tested on 17.1.1 / 17B100)
- iOS 17.0+ deployment target
- Apple Developer account (Team ID: HQWSG26L43)
- Bun installed
- Rust toolchain

## Project Configuration

Key files:

- `tauri/src-tauri/tauri.conf.json`: base Tauri config for desktop/mobile
- `tauri/src-tauri/tauri.ios.conf.json`: disables default window on iOS so we never touch `devUrl`
- `tauri/src-tauri/src/lib.rs`: initializes localhost plugin and creates the main window to `http://localhost:1420`
- `tauri/src-tauri/gen/apple/project.yml`: XcodeGen source, code signing and bundle metadata
- `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist`: iOS entitlements and ATS, version resolved from build settings

Important settings:

- Version/Build
  - `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` control app version and build number
  - Xcode project: `tauri/src-tauri/gen/apple/openagents.xcodeproj/project.pbxproj:343` and `:434`
  - XcodeGen: `tauri/src-tauri/gen/apple/project.yml:81` and `:82`
  - Plist resolves values: `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist:16` and `:20`

- Export Compliance
  - `ITSAppUsesNonExemptEncryption = false` (plist and XcodeGen)
  - Plist: `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist:31`
  - XcodeGen: `tauri/src-tauri/gen/apple/project.yml:56`

- App Transport Security (ATS)
  - Localhost exceptions present in `Info.plist` so WebView can load `http://localhost`

- App Icons
  - `Assets.xcassets/AppIcon.appiconset` contains opaque PNGs (no alpha)
  - Replace icons only with flattened/opaque PNGs; see the README for a quick command

## Build & Archive

Build frontend and iOS app assets:
```bash
cd tauri
bun run build
cd src-tauri
cargo tauri ios build
```

Open in Xcode and archive:
```bash
cargo tauri ios build -- --open
# Xcode: Product > Archive, then Distribute to App Store Connect
```

If you need a CLI export:
```bash
xcodebuild -exportArchive \
  -archivePath <path-to>.xcarchive \
  -exportPath <out-dir> \
  -exportOptionsPlist tauri/src-tauri/gen/apple/ExportOptions.plist
```

## Dev vs Release Behavior

- Dev: `bun run dev` + `bun tauri ios dev` uses Vite dev server.
- Release/TestFlight: no dev server. The app serves the built `dist/` on `http://localhost:1420` via `tauri-plugin-localhost` and creates the iOS window to that URL at startup.

## Troubleshooting

- “Failed to request http://localhost:1420” on TestFlight
  - Confirm `tauri/src-tauri/tauri.ios.conf.json` exists so no default window is created from config.
  - Confirm `tauri/src-tauri/src/lib.rs` creates/navigates the window to `http://localhost:1420` (see `:258` and `:295`).
  - Rebuild and archive again.

- “Invalid large app icon”
  - Ensure AppIcon PNGs are fully opaque (no alpha). Flatten them before committing.

- “bun: command not found” during Xcode build
  - Provide absolute Bun path in scripts, or run `cargo tauri ios build --open` then archive from the GUI.

## Links

- Tauri App Store docs: https://v2.tauri.app/distribute/app-store/
- Localhost plugin: https://v2.tauri.app/plugin/localhost/

