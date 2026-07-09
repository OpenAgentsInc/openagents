# OpenAgents mobile — v0.4.1 build 104 (iOS TestFlight) + owned-server OTA wiring receipt (2026-07-09)

Second TestFlight build of `apps/openagents-mobile` (#8597): adds **OTA
updates from the owned OpenAgents Updates server** (`apps/oa-updates`,
`updates.openagents.com`) with the **temporary 3-second foreground poll**, on
the app's own new channel. Never EAS. Same local release path as the build-103
receipt (`docs/mobile/2026-07-09-openagents-mobile-testflight-build-103-receipt.md`).

## What changed (commit `4954eb2fc0` on main)

- `expo-updates` 57.0.6 added (native module → new native build required:
  **version 0.4.1, iOS build 104**).
- `app.json` updates config: url
  `https://updates.openagents.com/openagents-mobile/manifest`, channel
  **`openagents-production`** (this app's OWN channel — identity oracles now
  enforce the exact URL/channel and reject any khala/AutopilotRemoteControl/
  bare-`production` legacy feed), `runtimeVersion.policy=fingerprint`.
- `src/updates/ota-polling.ts`: **TEMPORARY** aggressive 3s cadence
  (`TEMPORARY_OTA_POLL_INTERVAL_MS = 3000`, constant + comment + cadence test
  so it gets dialed down deliberately later). Loop:
  `checkForUpdateAsync` → on available `fetchUpdateAsync` → `reloadAsync`.
  All errors soft (offline never crashes/stops the loop); no-op in Expo Go/dev.
- `BUNDLE_TAG` on the Home card (`src/screens/home-core.ts`) makes each OTA
  swap owner-visible. Build 104 embeds `2026-07-09.embedded-104`.
- **TS-8 repoint**: `apps/oa-updates/scripts/publish-ota.sh` default target is
  now `apps/openagents-mobile` (owner `openagents-mobile`, channel
  `openagents-production`); the khala path is explicit-override-only. Seed
  branch parameterized end-to-end (`OA_SEED_BRANCH` in `serve.ts` +
  `deploy-cloudrun.sh`).

## OTA publish + signed-manifest proof

- Runtime fingerprint of this build (computed by `publish-ota.sh` from the
  same tree that was archived): `2e4e445d8280e0a8990c41f1317ce0e25009d27d`.
- Publish: `bash apps/oa-updates/scripts/publish-ota.sh` (fingerprint →
  `expo export` → seed → Cloud Run deploy of `oa-updates`, code signing via
  Secret Manager `oa-updates-codesign-key`, keyid `main`).
- Server-side note: the oa-updates mobile seed is single-slot per deploy —
  this publish REPLACES the frozen khala-mobile seed (runtime `f1b2f8c0…`,
  channel `production`). That app is deprecated/frozen with no releases;
  its installed builds keep their embedded bundles and simply get
  no-update-available.
- Deploy result: Cloud Run revision `oa-updates-00082-j92` serving 100%.
- **Signed-manifest proof** (live curl against the public host):

  ```sh
  curl -H 'expo-protocol-version: 1' -H 'expo-platform: ios' \
    -H 'expo-runtime-version: 2e4e445d8280e0a8990c41f1317ce0e25009d27d' \
    -H 'expo-channel-name: openagents-production' \
    https://updates.openagents.com/openagents-mobile/manifest
  ```

  → `HTTP/2 200`, `content-type: multipart/mixed`, manifest part carrying
  `expo-signature: sig=…, keyid="main", alg="rsa-v1_5-sha256"`, body
  `"branch":"openagents-production"`, correct runtime, and the embedded
  `extra.expoClient` app config (name `OpenAgents`, `com.openagents.app`,
  buildNumber 104).
- Embedded-runtime match: the archived app bundle's
  `EXUpdates.bundle/fingerprint` = `2e4e445d8280e0a8990c41f1317ce0e25009d27d`
  — exactly the published seed runtime, so installed build 104 resolves this
  feed.

## Build 104 release receipt

- `** ARCHIVE SUCCEEDED **` (Release, generic iOS device, Team `HQWSG26L43`,
  ASC-key auth) — archived identity verified: `0.4.1` / `104` /
  `com.openagents.app`, `EXUpdatesURL` + channel baked into `Expo.plist`.
- `** EXPORT SUCCEEDED **` — manual signing with the existing
  `com.openagents.app AppStore` profile (13.7 MB IPA incl. EXUpdates).
- `xcrun altool --upload-app` → **`UPLOAD SUCCEEDED with no errors`**,
  Delivery UUID `5d4ecfa8-152f-499a-be71-b02804abcb86` (2026-07-09 ~18:46 CT).
- ASC `/v1/builds`: build 104 `processingState=VALID` (see #8597 for the
  confirmation comment).

## How the owner tests OTA end-to-end

1. Install **build 104 (0.4.1)** from TestFlight. Home card shows
   `Bundle: 2026-07-09.embedded-104`.
2. Ask for (or run) a tiny OTA publish: bump `BUNDLE_TAG` in
   `src/screens/home-core.ts` (e.g. `2026-07-09.ota-1`), commit, then
   `cd apps/openagents-mobile && bun run publish:ota`.
3. With the app foregrounded, within ~3 seconds of the deploy going live the
   poll finds the update, downloads it, and reloads — the Home card flips to
   the new `Bundle` tag. (Offline or server-down states are silent no-ops;
   the loop keeps trying.)
