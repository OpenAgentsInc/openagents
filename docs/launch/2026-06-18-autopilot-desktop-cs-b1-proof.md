# Autopilot Desktop CS-B1 Proof

Date: 2026-06-18
Issue: #5364

This is the public-safe proof record for the CS-B1 packaged headless-node and
macOS signing/notarization gate. It records refs, command shapes, and verifier
outcomes only; it does not include signing secrets, notary credentials, local
tokens, provider payloads, or raw private logs.

## Source

- Source commit: `e125ad853fef2afcd690ea974ee80303b5e4d6b0`
- Desktop config version: `1.0.0-rc.3`
- Desktop channel built: `stable`
- Apple Developer ID: `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`

## Build And Packaged Node

The build path ran the desktop stable build, including the bundled headless
Pylon node:

```sh
bun run --cwd apps/autopilot-desktop build:stable
```

The generated stable app payload contains the bundled node at:

```text
Autopilot.app/Contents/Resources/app/pylon-node/index.js
```

Focused checks passed:

```sh
bun run --cwd apps/autopilot-desktop verify:deploy
bun test --cwd apps/autopilot-desktop tests/node-launcher.test.ts
```

The direct bundled-node smoke started the packaged node entry with a fresh
managed home and confirmed the loopback control API responded.

## Signing And Notarization

The built `.app` was signed, submitted to Apple notarization, accepted, stapled,
and verified:

- App notarization submission: `2de6437f-8db9-4b3d-9895-bde89a1f49fd`
- `codesign --verify --deep --strict --verbose=2` passed
- `spctl -a -vvv -t exec` returned `accepted`
- Gatekeeper source: `Notarized Developer ID`
- Origin: `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`

The DMG was recreated from the stapled app, signed, submitted to Apple
notarization, accepted, stapled, and verified:

- DMG notarization submission: `75f3362e-a872-4876-a7eb-3337659557fe`
- `codesign --verify --verbose=2` passed
- `spctl -a -vvv -t open --context context:primary-signature` returned
  `accepted`
- `xcrun stapler validate` passed

Public artifact digest:

```text
cc1d7876b70fded1ad5b15743bdcf2ee4e72fd8c5ca6dedd3d1d73f037db41db  stable-macos-arm64-Autopilot.notarized.dmg
```

## First-Run And Composer Proofs

The packaged first-run launcher smoke used the stable app's extracted runtime
payload, no repo entry, and a fresh managed home. The desktop node launcher
resolved the bundled Pylon node and reported:

```json
{
  "ok": true,
  "mode": "launched",
  "homeState": "present",
  "pidState": "present",
  "statuses": ["launching", "online"]
}
```

The desktop composer-loop proof also passed:

```sh
bun run --cwd apps/autopilot-desktop proof:composer
```

It drove the desktop Bun control functions through spawn, live session-event
tail, approvals projection, continuation spawn, and cancel against a real
loopback control server.

## Boundary

This proof satisfies the local CS-B1 operational gate for a packaged,
notarized macOS build that can bring up a headless Pylon node and drive the
composer control path. Publishing the artifact to the public update bucket or a
GitHub release remains a separate release-publication action and must still
follow `docs/DEPLOYMENT.md` from a clean release context.
