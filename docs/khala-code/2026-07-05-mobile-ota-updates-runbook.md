# Khala Mobile OTA updates runbook

This is the operator/agent runbook for publishing an over-the-air (OTA)
JavaScript update to the Khala Code mobile app (`clients/khala-mobile/`)
through our own OpenAgents Updates server, and for verifying a real
end-to-end round trip. Read this before touching anything under
`apps/oa-updates/` or `clients/khala-mobile/`'s `updates`/`runtimeVersion`
config.

## Why we run our own OTA server

Per `openagents/CLAUDE.md`'s mobile policy: builds are local (`expo
prebuild` + Xcode/Gradle, no `eas build`), and JS/OTA updates ship through
our own drop-in EAS Updates replacement — `apps/oa-updates` — never `eas
update`. `apps/oa-updates` implements the real `expo-updates` protocol v1
(manifest serving, signed manifests via `expo-signature` code signing,
asset storage, channel/branch/runtime-fingerprint matching) and is deployed
to Google Cloud Run, publicly reachable at `updates.openagents.com`.

The installed app (a real Xcode/Gradle local build, not an Expo Go/EAS
build) has its `updates.url` baked to
`https://updates.openagents.com/khala-mobile/manifest`. On launch it asks
that server "do you have a newer JS bundle for my exact runtime
fingerprint and channel?" — if yes, it downloads and applies it without a
new App Store/TestFlight build.

## The moving parts

- **`clients/khala-mobile/app.json`** — the `updates` block: `url`,
  `requestHeaders` (must include `"expo-channel-name": "production"` — see
  the bug below), and top-level `runtimeVersion: { "policy": "fingerprint" }`.
  The fingerprint policy means the "version" an update must match is a hash
  of the native build's actual contents (native modules, config plugins,
  etc.), not a hand-set string — change any native-affecting config and the
  fingerprint changes, and old OTA updates stop matching (correctly — they
  weren't built for the new native shape).
- **`apps/oa-updates/scripts/publish-ota.sh`** — the one command that does
  the whole publish: computes the runtime fingerprint, runs `expo export`,
  resolves the public Expo config, and deploys the seed to Cloud Run.
- **`apps/oa-updates/scripts/deploy-cloudrun.sh`** — the underlying
  `gcloud run deploy` wrapper; not meant to be run standalone, called by
  `publish-ota.sh`.
- **`apps/oa-updates/src/serve.ts`** — the manifest-serving logic (branch
  matching, response body construction).
- **`apps/oa-updates/src/export-reader.ts`** — reads the `expo export`
  output directory (JS bundle + assets) into what the server serves.
- **`.secrets/oa-updates-codesign-private.pem`** — the code-signing private
  key (`keyid: "main"`, `rsa-v1_5-sha256`). If present, `publish-ota.sh`
  signs every manifest automatically. The installed app has the matching
  public certificate embedded and will reject/ignore an unsigned or
  wrongly-signed manifest.

## How to publish an update

From the repo root, on a machine with a working `gcloud` session
(`gcloud auth login` first if needed — this needs your real interactive
Google Cloud login, not a service account, for the `oa-node`/Cloud Run
deploy path used here):

```sh
bash apps/oa-updates/scripts/publish-ota.sh
```

Android: `OA_MOBILE_PLATFORM=android bash apps/oa-updates/scripts/publish-ota.sh`.

That single command:
1. Computes the exact runtime fingerprint for the current native build
   (`bunx expo-updates fingerprint:generate`).
2. Runs `expo export` to produce the JS bundle + assets.
3. Resolves the public Expo config (`expo config --type public --json`) and
   bundles it as `expo-client.json` — this is required, see bug #4 below.
4. Deploys all of it as the Cloud Run seed for that runtime + the
   `production` branch, signing the manifest if the private key is present.

The script prints a ready-to-run `curl` command at the end that hits the
manifest endpoint directly with the right headers — use it as a first,
fast sanity check before trusting a real device/simulator.

## How to actually prove the round trip worked (don't just trust "publish succeeded")

A successful `publish-ota.sh` run does NOT by itself prove the installed
app will actually download and apply the update without crashing or
silently rolling back — that requires an end-to-end device/simulator
check, because several of the real bugs found in this session (see below)
only manifested once expo-updates tried to actually launch the downloaded
bundle.

The definitive proof method used to validate this end-to-end (2026-07-05):
1. Fresh-install the app (old runtime/build, so there's a real update to
   pull).
2. Launch once — this triggers the background download of the new
   manifest/bundle.
3. Fully terminate the app.
4. Relaunch — this is the launch that actually **applies** the downloaded
   update.
5. Inspect the app's own local `expo-v11.db` (SQLite, part of
   `expo-updates`' internal state) for the just-applied update's launch
   counters:
   ```sh
   sqlite3 <path-to-app-sandbox>/expo-v11.db \
     "SELECT id, status, successful_launch_count, failed_launch_count FROM updates ORDER BY commit_time DESC LIMIT 5;"
   ```
   `successful_launch_count >= 1` and `failed_launch_count = 0` for the new
   update's row is the real proof — the update was downloaded, applied, and
   the app ran without expo-updates' embedded crash-detection rolling it
   back to the previous (embedded/cached) bundle.

A `noUpdateAvailable` response, an app that launches but immediately
crashes, or an app that silently reverts to old behavior are all real
failure modes that a bare `publish-ota.sh` success does not catch — always
do the device-level check above for anything beyond a routine,
already-proven-safe publish.

## Real bugs found and fixed (2026-07-05) — read before assuming OTA "just works"

The very first real round-trip attempt for `khala-mobile` surfaced five
distinct, previously-undiscovered production bugs. All are now fixed, but
understanding them matters if OTA breaks again after a future change:

1. **Missing `Expo-Channel-Name` request header.** `clients/khala-mobile/app.json`
   had no `updates.requestHeaders`, so the client never told the server
   which branch/channel to match against — the server always returned
   `noUpdateAvailable` even with a live, correctly-signed manifest for the
   right runtime. Fixed by adding
   `"requestHeaders": { "expo-channel-name": "production" }` to the
   `updates` block, matching the branch `publish-ota.sh` always deploys to.
2. **Non-UUID manifest `id`.** The manifest response's `id` field must be a
   real UUID — a non-UUID value crashed the app on launch
   (`NSInternalInconsistencyException`) the instant it tried to apply the
   update. Fixed in `apps/oa-updates/src/serve.ts`.
3. **Path-separator asset keys.** Asset keys containing path separators
   broke every asset write (`AssetsFailedToLoad` for all assets, not just
   some). Fixed in `apps/oa-updates/src/export-reader.ts`.
4. **Empty manifest `extra`.** `expo-constants`/`expo-linking` need
   `Constants.expoConfig` to exist on a *downloaded* update, not just the
   embedded native one — without it, the app throws "runtime not ready"
   immediately after applying an OTA update, and expo-updates silently rolls
   back to the previous cached/embedded bundle (this failure mode is easy to
   miss because the app doesn't crash — it just quietly reverts). Fixed by
   threading the resolved public Expo config (`expo config --type public
   --json`) through the whole publish pipeline
   (`publish-ota.sh` → `publish-builder.ts`/`publish.ts` →
   `export-reader.ts` → `serve.ts`) as `expo-client.json`, embedded in the
   manifest as `extra.expoClient`.
5. **`deploy-cloudrun.sh` didn't forward the new env var.** Once
   `publish-ota.sh` started producing `OA_SEED_EXPO_CLIENT_PATH`, the
   deploy wrapper needed to actually pass it through to Cloud Run — fixed.
6. **Optional release seed directories must exist in the build context.**
   `apps/oa-updates/Dockerfile` copies `pylon-dist/` and `desktop-ota/` so
   the same service can also serve Pylon and desktop artifacts. A fresh
   mobile-only worktree did not have those ignored directories, so Cloud Build
   failed at `COPY pylon-dist ./pylon-dist`. Fixed by tracking `.gitignore`
   keep-files in both optional directories while continuing to ignore generated
   release payloads.

The lesson: OTA correctness needs both server-side (manifest shape, asset
serving) and client-runtime (expo-updates' own launch/rollback behavior)
verification. A green `publish-ota.sh` run only proves the server side.

## Prerequisites checklist for a future agent

- `gcloud auth login` has been run interactively on this machine (service
  accounts / ADC alone are not sufficient for this deploy path — confirmed
  during this session that `gcloud auth application-default print-access-token`
  can be expired/irrelevant while the CLI's own user session is what
  actually matters for `gcloud run deploy --source`).
- `.secrets/oa-updates-codesign-private.pem` exists if you want signed
  manifests (recommended — the installed app's embedded certificate expects
  a signed manifest).
- `clients/khala-mobile/app.json`'s `updates.requestHeaders` still includes
  `expo-channel-name: production` (or whatever channel you're actually
  targeting) — a future edit to this file could silently regress bug #1
  above.
- If you change any native-affecting config (a config plugin, a new native
  module, `runtimeVersion` policy), remember the fingerprint changes and
  every previously-published OTA update stops matching for new installs —
  that's correct behavior, not a bug, but it means you need a fresh publish
  after any native change, and a fresh native build (TestFlight/local
  install) to pick up the new fingerprint baseline.

2026-07-05 #8470 note: Khala Mobile GitHub sign-in added
`expo-auth-session`, `expo-crypto`, and the `expo-web-browser` config plugin.
That is native-affecting. The native build metadata was bumped to iOS build
`8` and Android versionCode `2`. The local build pass is complete for this
change: iOS prebuild/build finished with `** BUILD SUCCEEDED **`; Android
prebuild/build finished with `BUILD SUCCESSFUL` when run with
`JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home` and
`ANDROID_HOME=/opt/homebrew/share/android-commandlinetools`. The signed iOS OTA
baseline was then published from clean pushed `main` by
`bash apps/oa-updates/scripts/publish-ota.sh`: runtime fingerprint
`d72044f835d38b35da4a3559784593b45fce2ad8`, Cloud Run revision
`oa-updates-00054-w5t`, and public manifest verification returned HTTP 200
multipart with an `expo-signature` manifest part, the same runtime, 20 assets,
and `extra.expoClient.name = "Khala Code"`.

## Where the OTA server itself lives

`apps/oa-updates/` is a small Bun app deployed to Cloud Run
(`gcloud run deploy oa-updates --source apps/oa-updates ...`, wrapped by
`deploy-cloudrun.sh`). It also serves desktop (`Khala Code` Electrobun app)
and Pylon release artifacts from the same service — `publish-desktop-release.ts`
and `publish-pylon-release.ts` in `apps/oa-updates/scripts/` are the
equivalent publish paths for those surfaces, not covered by this runbook.
