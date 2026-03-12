# OpenAgents macOS Signing and Notarization

This document is the setup/runbook for signed macOS releases using:

- `scripts/release/macos-release.sh`

## Required Accounts and Access

1. Apple Developer Program membership for the OpenAgents team.
2. A valid Developer ID Application certificate in your login keychain.
3. App Store Connect API key (`.p8`) for notarization API access.
4. Swift available on the release machine (`xcode-select --install` is sufficient) so the bundled `foundation-bridge` helper can be built during packaging.

## Script Inputs (Required for Signed Mode)

The release script requires these env vars when `--allow-unsigned` is not used:

- `MACOS_SIGNING_IDENTITY`
- `MACOS_TEAM_ID`
- `APPLE_NOTARIZATION_KEY`
- `APPLE_NOTARIZATION_KEY_ID`
- `APPLE_NOTARIZATION_ISSUER_ID`

## 1) Configure Signing Identity

List available signing identities:

```bash
security find-identity -v -p codesigning
```

Use the exact certificate label for:

```bash
export MACOS_SIGNING_IDENTITY="Developer ID Application: OpenAgents Inc (TEAMID)"
```

Set your Apple Team ID:

```bash
export MACOS_TEAM_ID="TEAMID"
```

## 2) Configure Notarization API Key

Create/download App Store Connect API key (Integrations). Then set:

```bash
export APPLE_NOTARIZATION_KEY_ID="ABC123XYZ9"
export APPLE_NOTARIZATION_ISSUER_ID="00000000-0000-0000-0000-000000000000"
export APPLE_NOTARIZATION_KEY="$(cat ~/keys/AuthKey_ABC123XYZ9.p8)"
```

Important:

- `APPLE_NOTARIZATION_KEY` must contain the key contents, not a file path.
- The `.p8` is only downloadable once. Store it securely.

## 3) Quick Credential Verification

Validate notarization credentials before a release:

```bash
tmp_key="$(mktemp)"
printf '%s' "$APPLE_NOTARIZATION_KEY" > "$tmp_key"
xcrun notarytool history \
  --key "$tmp_key" \
  --key-id "$APPLE_NOTARIZATION_KEY_ID" \
  --issuer "$APPLE_NOTARIZATION_ISSUER_ID" \
  --team-id "$MACOS_TEAM_ID"
rm -f "$tmp_key"
```

If this command returns history output (or an empty history response without auth errors), credentials are valid.

## 4) Run Signed Release

Example:

```bash
./scripts/release/macos-release.sh --version 0.1.1 --publish
```

The script will:

1. Build and bundle `Autopilot.app`
2. Build `swift/foundation-bridge` and copy the helper into `Autopilot.app/Contents/MacOS/foundation-bridge`
3. Sign each executable inside the app bundle, then sign the app bundle
4. Create `Autopilot-<version>.dmg`
5. Submit DMG for notarization
6. Staple notarization ticket
7. Create checksum and publish assets (if `--publish`)

## 5) Post-release Validation

After build/publish, validate locally:

```bash
codesign --verify --deep --strict target/release/bundle/osx/Autopilot.app
spctl --assess --type exec --verbose=4 target/release/bundle/osx/Autopilot.app
xcrun stapler validate target/release/Autopilot-0.1.1.dmg
spctl --assess --type open --verbose=4 target/release/Autopilot-0.1.1.dmg
shasum -a 256 -c target/release/Autopilot-0.1.1.dmg.sha256
```

## 6) Runtime Log Verification

Autopilot persists per-launch runtime logs by default under:

- `~/.openagents/logs/autopilot/sessions/<session-id>.jsonl`
- `~/.openagents/logs/autopilot/latest.jsonl`

Optional development override:

- `OPENAGENTS_AUTOPILOT_LOG_DIR`

After launching the signed `.app`, verify the runtime log path exists and is receiving entries:

```bash
ls ~/.openagents/logs/autopilot/sessions
tail -n 50 ~/.openagents/logs/autopilot/latest.jsonl
```

## Operational Notes

- Keep secrets in shell profile, 1Password shell plugin, or CI secret store.
- Never commit `.p8` keys, cert exports, or private key material.
- For local dry runs without signing/notarization, use `--allow-unsigned`.
- The current shipping path is Developer ID distribution outside the Mac App Store; no App Store provisioning profile is part of this flow.
- The packaged Apple Foundation Models lane still requires end users to be on macOS 26+ with Apple Silicon and Apple Intelligence enabled.
