# OpenAgents Updates

`oa-updates` is the owned signed-release service behind `updates.openagents.com`.
It is an active release boundary, not archival code, and it now serves exactly
one product surface:

- **Pylon release feed.** `/pylon/<channel>/<platform>/feed.json` serves signed,
  per-platform Pylon binaries seeded by
  [`scripts/publish-pylon-release.ts`](scripts/publish-pylon-release.ts). The
  self-updater verifies each release against the pinned ed25519 key in
  [`keys/release-pubkey.json`](keys/release-pubkey.json) and fails closed.

Two supporting routes travel with it: `/assets/<hash>`, which streams a
disk-backed Pylon binary when one is baked into the image rather than served
from GCS, and `/<owner>/nodes`, the in-memory Pylon node discovery registry
that `apps/pylon/src/node/discovery-register.ts` posts to.

Release signing is documented in
[`docs/release-signing-runbook.md`](docs/release-signing-runbook.md).

## Retired surfaces

Both of the other surfaces this service used to carry are gone, and their
routes are ordinary 404s:

- **Electron/Electrobun desktop** — the `/desktop/...` feeds, the Electrobun OTA
  files, the legacy desktop lockout, and the ReleaseSet v2 pointer/candidate
  feed were removed when the desktop app was deleted (2026-08-04, #9325).
- **OpenAgents Mobile Expo OTA** — the `/<owner>/manifest` route, the Expo
  Updates Protocol `multipart/mixed` manifest/directive responses, manifest
  resolution and channel/branch negotiation, `expo-signature` manifest code
  signing, the `expo export` reader and publish builder, the in-memory
  published-asset store, and `publish-ota.sh` were retired on 2026-08-05
  (#9325) at owner direction — there are no installed mobile users.
  `apps/openagents-mobile/app.json` now sets `updates.enabled: false` and
  configures no update URL, and JS changes ship only in a new store build.
  The `OA_SEED_*` seed variables and the `OA_SIGNING_KEY` code-signing secret
  are no longer read or attached, so a stale operator environment cannot
  resurrect the feed.

The pinned public key in `keys/release-pubkey.json` stays: it backs Pylon
verification and the `PRODUCTION_RELEASE_KEY_PIN` drift oracle in
`packages/release-contract`.

## Verification

From this directory:

```sh
pnpm run typecheck
pnpm run test
```

The strict, no-emit test project covers production source and mechanically
proves that every TypeScript test/spec file is a project root. The canonical
typecheck also compiles a valid Pylon release manifest fixture, removes its
required `sha256` artifact digest, and proves that TypeScript rejects the
broken fixture before an unverifiable release could reach the feed. The
project currently has no diagnostic baseline: all surfaced errors are fixed.

## Deployment

[`scripts/deploy-cloudrun.sh`](scripts/deploy-cloudrun.sh) deploys to Cloud Run.
A deploy that publishes fresh signed Pylon binaries (`OA_PYLON_RELEASES_DIST`)
does a full `--source` rebuild; every other deploy takes the incremental path
from the currently-ready image digest, so a code push can never blank the Pylon
binaries already baked into the running image.
