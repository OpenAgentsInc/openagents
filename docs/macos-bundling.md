# macOS Bundling & Distribution (Autopilot Desktop)

This doc is the definitive guide for building, signing, notarizing, and distributing the WGPU Autopilot desktop app.

## What exists today
- Build script: `script/bundle-mac` (Zed-style flow).
- Bundle metadata: `apps/autopilot-desktop/Cargo.toml` under `[package.metadata.bundle]`.
- Icon assets: `apps/autopilot-desktop/resources/icon.icns` and `icon.png` (pulled from the legacy Tauri app).
- Entitlements: `apps/autopilot-desktop/resources/autopilot.entitlements` (minimal, GPU-safe).

## Requirements
- macOS with Xcode installed (for `codesign`, `notarytool`, `stapler`).
- Rust toolchain + `cargo`.
- `cargo-bundle` (Zed fork): the script auto-installs if missing.
- Apple Developer account with:
  - Developer ID Application certificate (exported as `.p12`).
  - App Store Connect API key (`.p8`).

Optional:
- Node + `npm` if you want DMG license UI (see `script/terms/terms.json`).

## Quick start (unsigned, local use)
```bash
script/bundle-mac
```
Outputs:
- `.app`: `target/<arch>/release/bundle/osx/Autopilot.app`
- `.dmg`: `target/<arch>/release/Autopilot-<arch>.dmg`

This is **unsigned** and will trigger Gatekeeper warnings on other machines.

## Signed + notarized distribution
Export these environment variables before running the script:
```bash
export MACOS_CERTIFICATE="<base64-encoded .p12 contents>"
export MACOS_CERTIFICATE_PASSWORD="<p12 password>"
export APPLE_NOTARIZATION_KEY="<contents of .p8 key>"
export APPLE_NOTARIZATION_KEY_ID="<key id>"
export APPLE_NOTARIZATION_ISSUER_ID="<issuer id>"
# optional: override signing identity
export MACOS_SIGNING_IDENTITY="Developer ID Application: OpenAgents, Inc. (HQWSG26L43)"

script/bundle-mac
```
The script will:
1) Build the app for your host arch.
2) Create a `.app` bundle via cargo-bundle.
3) Codesign the bundle with entitlements.
4) Package a DMG.
5) Notarize + staple the DMG.

## Script options
```
-d   Build debug bundle (skips DMG/signing)
-o   Open result after bundling
-i   Install bundle into /Applications
```
You can also pass a target triple as an argument:
```bash
script/bundle-mac aarch64-apple-darwin
```

## Gatekeeper (local testing)
If you run an unsigned bundle locally and Gatekeeper blocks it:
```bash
xattr -dr com.apple.quarantine /Applications/Autopilot.app
```

## Where outputs go
- `target/<arch>/release/bundle/osx/Autopilot.app`
- `target/<arch>/release/Autopilot-<arch>.dmg`

## Entitlements
`apps/autopilot-desktop/resources/autopilot.entitlements` currently enables:
- `com.apple.security.cs.allow-jit`
- `com.apple.security.cs.allow-unsigned-executable-memory`

Add additional entitlements here if the app needs access to user files, microphone, etc.

## Troubleshooting
- **cargo-bundle wrong version**: the script auto-installs `cargo-bundle v0.6.1-zed` if needed.
- **Signing fails**: verify `MACOS_CERTIFICATE` and `MACOS_CERTIFICATE_PASSWORD`, and that the identity matches.
- **Notarization fails**: verify `APPLE_NOTARIZATION_KEY`, `APPLE_NOTARIZATION_KEY_ID`, `APPLE_NOTARIZATION_ISSUER_ID`.
- **DMG too small / broken**: re-run `script/bundle-mac` (it recreates the DMG directory).

## Optional: DMG license UI
If you want a license prompt in the DMG:
1) Add `script/terms/terms.json` (format compatible with `dmg-license`).
2) Ensure `npm` is available.
3) Re-run `script/bundle-mac`.

## Related files
- `script/bundle-mac`
- `apps/autopilot-desktop/Cargo.toml`
- `apps/autopilot-desktop/resources/icon.icns`
- `apps/autopilot-desktop/resources/icon.png`
- `apps/autopilot-desktop/resources/autopilot.entitlements`
