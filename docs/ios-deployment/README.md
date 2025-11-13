# iOS Deployment (Tauri v2)

This folder documents how we build, archive, and ship the OpenAgents iOS app using Tauri v2 + React.

If you just want the step‑by‑step, start with the Quick Start below. For deeper context, see the sections that follow.

## Quick Start (Release/TestFlight)

Prerequisites:
- Xcode 17.0+ with an Apple Developer account
- Bun installed (`curl -fsSL https://bun.sh/install | bash`)
- Rust toolchain (installed automatically by Tauri)

Commands:
1) Build the frontend
   ```bash
   cd tauri
   bun run build
   ```
2) Build iOS app payload (staticlib + assets)
   ```bash
   cd tauri/src-tauri
   cargo tauri ios build
   ```
3) Open Xcode and Archive (recommended)
   ```bash
   cargo tauri ios build -- --open   # or: bun tauri ios build --open
   # Xcode: Product > Archive, then Distribute (App Store Connect)
   ```

Notes:
- We do not use the dev server in TestFlight. The app serves the built `dist/` via a localhost server on iOS.
- Build number and version are controlled by Xcode build settings (see Versioning).

## Architecture: Release on iOS (No Dev Server)

To prevent TestFlight from trying to connect to the Vite dev server (`devUrl`), the iOS build:
- Starts a local HTTP server using `tauri-plugin-localhost` bound to port `1420`.
- Does not create the default window from config on iOS, avoiding any `devUrl` usage.
- Programmatically creates the main window that navigates to `http://localhost:1420`.

Relevant code:
- Initialize localhost server (iOS only): `tauri/src-tauri/src/lib.rs:258`
- Create/navigate window to `http://localhost:1420`: `tauri/src-tauri/src/lib.rs:295`
- Disable default config window on iOS: `tauri/src-tauri/tauri.ios.conf.json:1`

Why this matters: If a default window is created from config, Tauri may try to reach `build.devUrl` and show the dev error page in TestFlight. Our iOS boot flow never touches `devUrl`.

## Versioning and Build Number

Use Xcode build settings and XcodeGen source so values are consistent:
- Marketing Version (user‑visible): `0.3.0`
- Build Number: `101` (must increment for every App Store upload)

Where to change:
- Xcode project settings: `tauri/src-tauri/gen/apple/openagents.xcodeproj/project.pbxproj:343` and `tauri/src-tauri/gen/apple/openagents.xcodeproj/project.pbxproj:434`
- XcodeGen source of truth (keeps project regenerations in sync): `tauri/src-tauri/gen/apple/project.yml:81` (MARKETING_VERSION) and `:82` (CURRENT_PROJECT_VERSION)
- Plist resolves from those settings: `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist:16` (ShortVersion) and `:20` (Version)

## App Icons (No Alpha)

Apple rejects marketing icons with transparency. Ensure every AppIcon image is fully opaque:
- All PNGs in `tauri/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/` are flattened (no alpha) and committed.
- If you update icons, flatten them before archiving. Example command (ImageMagick):
  ```bash
  for f in tauri/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/*.png; do \
    magick "$f" -background black -alpha remove -alpha off "$f"; \
  done
  ```
- Verify no alpha:
  ```bash
  sips -g hasAlpha tauri/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/AppIcon-512@2x.png
  # hasAlpha: no
  ```

## Export Compliance (Encryption)

We opt out automatically to avoid manual toggles in App Store Connect:
- `ITSAppUsesNonExemptEncryption = false`
- Set both in plist and XcodeGen so regenerations keep it:
  - `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist:31`
  - `tauri/src-tauri/gen/apple/project.yml:56`

## ATS (Localhost)

Allow loading `http://localhost` content in the WebView on iOS:
- `NSAppTransportSecurity` with exceptions for `localhost` and `127.0.0.1` in
  `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist:33`.

## Build and Archive

CLI build (produces the iOS static library and prepares the Xcode project):
```bash
cd tauri
bun run build           # build frontend
cd src-tauri
cargo tauri ios build   # or: bun tauri ios build
```

Archive with Xcode (recommended for App Store uploads):
```bash
cargo tauri ios build -- --open
# In Xcode: Product > Archive > Distribute App > App Store Connect > Upload
```

Export options (CLI alternative):
- Use an ExportOptions.plist with `method = app-store-connect` for TestFlight/App Store.
- You can export and upload via altool, but Organizer upload is simpler and recommended.

## Bun path during Xcode build

If Xcode cannot find Bun during a dev build (e.g. `bun: command not found`), set an absolute path in the XcodeGen project script or ensure your PATH is propagated. Our current setup uses a Rust build wrapper (`build-rust.sh`) so that archive builds avoid the dev WebSocket step.

File references:
- Script entry: `tauri/src-tauri/gen/apple/project.yml:92`
- Wrapper: `tauri/src-tauri/gen/apple/build-rust.sh:1`

## Troubleshooting

Symptoms and fixes we’ve already baked into the repo:
- TestFlight shows “Failed to request http://localhost:1420”
  - Fixed by: no default config window on iOS (`tauri.ios.conf.json`) and explicit window to `http://localhost:1420` in code. Confirm lines in `tauri/src-tauri/src/lib.rs:258` and `:295`.
- “Invalid large app icon (alpha channel)”
  - All AppIcon PNGs are flattened (no alpha). If you replace icons, flatten before archiving.
- “bun: command not found” during Xcode build
  - Use absolute Bun path or run `cargo tauri ios build --open` and archive from Xcode.
- Build numbers rejected
  - Increment `CURRENT_PROJECT_VERSION` in both Xcode project and `project.yml`.

## Reference

- Tauri iOS distribution guide: https://v2.tauri.app/distribute/app-store/
- Localhost plugin: https://v2.tauri.app/plugin/localhost/

