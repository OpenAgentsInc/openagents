# OpenAgents macOS Signing and Notarization

This document is the setup/runbook for signed macOS releases using:

- `scripts/release/macos-release.sh`

## Required Accounts and Access

1. Apple Developer Program membership for the OpenAgents team.
2. A valid Developer ID Application certificate in your login keychain.
3. App Store Connect API key (`.p8`) for notarization API access.

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
2. Sign app binaries and bundle
3. Create `Autopilot-<version>.dmg`
4. Submit DMG for notarization
5. Staple notarization ticket
6. Create checksum and publish assets (if `--publish`)

## 5) Post-release Validation

After build/publish, validate locally:

```bash
codesign --verify --deep --strict target/release/bundle/osx/Autopilot.app
xcrun stapler validate target/release/Autopilot-0.1.1.dmg
spctl --assess --type open --verbose=4 target/release/Autopilot-0.1.1.dmg
shasum -a 256 -c target/release/Autopilot-0.1.1.dmg.sha256
```

## Operational Notes

- Keep secrets in shell profile, 1Password shell plugin, or CI secret store.
- Never commit `.p8` keys, cert exports, or private key material.
- For local dry runs without signing/notarization, use `--allow-unsigned`.
