# OpenAgents Updates

`oa-updates` is the owned Expo Updates-compatible OTA and signed-release
service behind `updates.openagents.com`. It is an active release boundary, not
archival code, and it serves exactly two surfaces:

- **OpenAgents Mobile OTA.** `apps/openagents-mobile` points `expo-updates` at
  `https://updates.openagents.com/openagents-mobile/manifest`. This service owns
  manifest resolution, signing, asset serving, and Expo Updates Protocol
  responses (`multipart/mixed` manifest/directive parts, `/assets/<hash>`).
  Publication runs through [`scripts/publish-ota.sh`](scripts/publish-ota.sh).
- **Pylon release feed.** `/pylon/<channel>/<platform>/feed.json` serves signed,
  per-platform Pylon binaries seeded by
  [`scripts/publish-pylon-release.ts`](scripts/publish-pylon-release.ts). The
  self-updater verifies each release against the pinned ed25519 key in
  [`keys/release-pubkey.json`](keys/release-pubkey.json) and fails closed.

Release signing for both surfaces is documented in
[`docs/release-signing-runbook.md`](docs/release-signing-runbook.md).

The Electron/Electrobun desktop surface this service used to carry — the
`/desktop/...` feeds, the Electrobun OTA files, the legacy desktop lockout, and
the ReleaseSet v2 pointer/candidate feed — was removed when the desktop app was
deleted. Those routes are now ordinary 404s.

## Verification

From this directory:

```sh
pnpm run typecheck
pnpm run test
```

The strict, no-emit test project covers production source and mechanically
proves that every TypeScript test/spec file is a project root. The canonical
typecheck also compiles a valid update fixture, removes its required
`id`, and proves that TypeScript rejects the broken fixture before the native
Expo Updates client could receive an invalid manifest. The
project currently has no diagnostic baseline: all surfaced errors are fixed.

OpenAgents Mobile owns its client-side update polling and identity tests under
`apps/openagents-mobile`. This service owns manifest resolution, signing,
assets, release seeding, and protocol responses.

## Deployment

[`scripts/deploy-cloudrun.sh`](scripts/deploy-cloudrun.sh) deploys to Cloud Run.
A deploy that publishes a fresh Expo export (`OA_SEED_DIST`) does a full
`--source` rebuild; every other deploy takes the incremental path from the
currently-ready image digest, so a code push can never blank the mobile export
or Pylon binaries already baked into the running image.
