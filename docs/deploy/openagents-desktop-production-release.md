# OpenAgents Desktop production release

This is the operator runbook for building and publishing the signed OpenAgents
Desktop macOS release. Run it from a clean worktree at exact `origin/main`.
The first frozen release identity is:

- product: `OpenAgents`
- bundle ID: `com.openagents.desktop`
- update product: `openagents-desktop`
- channels: `rc`, then `stable`
- tags: `openagents-desktop-v*`
- first RC: `0.1.0-rc.1`
- first downloaded-artifact smoke to pass: `0.1.0-rc.5`

Never reuse the retired Khala Code or Autopilot Desktop identities or feeds.

## Prerequisites

- Apple Developer ID identity
  `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)` is available in
  the login keychain.
- `.secrets/appstoreconnect.env` contains the `ASC_API_*` notarization values
  and `OA_DEVELOPER_ID_APPLICATION`.
- `.secrets/openagents-release-signing.env` contains the production ed25519
  update-manifest key. Never print or commit either secret file.
- The automation gcloud config is active through
  `CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config`.

Confirm the signing identity and the clean source revision:

```sh
git fetch origin main
test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"
test -z "$(git status --porcelain)"
```

The release scripts verify the identity supplied through
`OA_DEVELOPER_ID_APPLICATION`. Do not probe the macOS keychain from an
unattended agent session: `security` can trigger an interactive keychain
prompt. Confirm identity access through the gated signed packaging step.

## Candidate and installed-app safety

Never splice, replace, or re-sign nested components inside
`/Applications/OpenAgents.app`. Experimental candidates stay under `/tmp` and
must never become evidence for an installed release by mutating the known-good
installation in place. Build the complete artifact, notarize it, verify it,
and install only that complete artifact through the release/update path.

Before launching any experimental candidate, verify the outer app and every
nested signed component. Every reported TeamIdentifier must be `HQWSG26L43`,
and deep strict verification must pass:

```sh
CANDIDATE=/tmp/OpenAgents-candidate/OpenAgents.app
codesign --verify --deep --strict --verbose=2 "$CANDIDATE"
find "$CANDIDATE/Contents" -type f -perm +111 -print0 | while IFS= read -r -d '' file; do
  codesign -dv --verbose=4 "$file" 2>&1 | grep -E '^(Authority|TeamIdentifier)='
done
```

`OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1` is test-only evidence. It is accepted
only with user data strictly below Electron's actual OS temporary directory
(use `$TMPDIR`, not a guessed `/tmp` alias), disables account-session custody,
and uses an in-memory browser partition. It must never be used for production,
release acceptance, or authenticated cross-device claims.

## Build and preflight

Set the intended version in `apps/openagents-desktop/package.json`, run the
release-contract tests, commit it to `main`, and then build:

```sh
pnpm exec vp test --run --max-concurrency 1 \
  apps/openagents-desktop/tests/release-preflight.test.ts \
  apps/openagents-desktop/tests/update-contract.test.ts \
  apps/openagents-desktop/tests/publish-release.test.ts \
  apps/openagents-desktop/tests/package-macos.test.ts \
  apps/openagents-desktop/tests/macos-gatekeeper.test.ts \
  apps/openagents-desktop/tests/launch-receipt.test.ts \
  apps/openagents-desktop/tests/update-rollback.test.ts
pnpm --dir apps/openagents-desktop run typecheck
pnpm --dir apps/openagents-desktop run build
set -a; source ~/work/.secrets/appstoreconnect.env; set +a
node --import tsx apps/openagents-desktop/scripts/release-preflight.ts \
  --channel rc --latest-released <latest-version> --json
```

The preflight is fail-closed on signing credentials (#8786): with no
`OA_DEVELOPER_ID_APPLICATION`/`ASC_API_*` in the environment the
`signing_credentials_present` row is RED and the lane refuses. For a
non-release dev iteration only, pass `--allow-unsigned-dev`.

Then sign, package, notarize, and staple in one gated step (#8786):

```sh
set -a
source ~/work/.secrets/appstoreconnect.env
set +a
pnpm --dir apps/openagents-desktop run make:mac
```

`make:mac` is fail-closed (Gatekeeper release oracles, issue #8786; from the
2026-07-13 T3 Gatekeeper-dead-DMG and ChatGPT dead-update incidents):

- Forge notarizes the `.app` during packaging; the `postMake` hook then
  submits the **DMG itself** to `notarytool` (the ticket covers the nested
  app) and staples the ticket to BOTH the `.app` and the `.dmg`.
- The make then refuses to finish unless every Gatekeeper oracle is green:
  `codesign --verify --deep --strict` (app), `spctl -a -t open --context
  context:primary-signature` (image), `spctl -a -t exec` (app), and
  `xcrun stapler validate` (both).
- If `OA_DEVELOPER_ID_APPLICATION` or the `ASC_API_*` credentials are absent
  the make REFUSES outright — there is no unsigned release fallback. The only
  escape valve is `OA_ALLOW_UNSIGNED_DEV=1`, which renames the output
  `-UNSIGNED-DEV`; preflight and `publish-release.ts` refuse that marker
  unconditionally.

Re-run the preflight against the built artifacts to record the oracle table
(the same checks the make already enforced):

```sh
DMG=apps/openagents-desktop/out/make/OpenAgents-<version>-arm64.dmg
APP=apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app
node --import tsx apps/openagents-desktop/scripts/release-preflight.ts \
  --channel rc --latest-released <latest-version> \
  --dmg "$DMG" --app "$APP" --json
```

Wait for stapling to finish before hashing. Hash and size the final bytes twice
if necessary; the signed update manifest must describe the post-staple file,
not the pre-staple DMG.

```sh
shasum -a 256 "$DMG"
stat -f '%z' "$DMG"
```

## Sign and publish the RC feed

Stage the production-signed manifest with the exact final DMG bytes:

```sh
DIST=/tmp/openagents-desktop-release-dist
rm -rf "$DIST" && mkdir -p "$DIST"
OPENAGENTS_RELEASE_SECRETS_PATH=~/work/.secrets/openagents-release-signing.env \
  node --import tsx apps/openagents-desktop/scripts/publish-release.ts \
  --channel rc --version <version> --artifact "$DMG" \
  --dist-dir "$DIST" --notes-ref release.notes.<version>
cp "$DIST"/*.json apps/oa-updates/openagents-desktop-dist/
```

Upload the identical artifact and confirm its public byte length:

```sh
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config gcloud storage cp "$DMG" \
  gs://openagentsgemini-oa-updates/desktop/openagents-desktop/"$(basename "$DMG")"
curl -fsSI \
  "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/openagents-desktop/$(basename "$DMG")" \
  | grep -i content-length
```

Deploy without erasing the existing mobile OTA seed. Do **not** use a fresh
`gcloud run deploy --source` unless a complete new Expo export is present in
`apps/oa-updates/dist`; a metadata-only directory will replace the baked mobile
assets and make the mobile manifest return 404. Instead, derive from the
immutable digest of the current known-good revision with the incremental
Dockerfile:

```sh
BASE_IMAGE=$(CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config gcloud run \
  revisions describe <current-ready-revision> --project openagentsgemini \
  --region us-central1 --format='value(spec.containers[0].image)')
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-updates:desktop-<version>
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config gcloud builds submit \
  apps/oa-updates --project openagentsgemini --region us-central1 \
  --config apps/oa-updates/cloudbuild.incremental.yaml \
  --substitutions="_BASE_IMAGE=$BASE_IMAGE,_IMAGE=$IMAGE"
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config gcloud run deploy oa-updates \
  --project openagentsgemini --region us-central1 --image "$IMAGE" \
  --update-env-vars OA_OPENAGENTS_DESKTOP_RELEASE_DIST=/app/openagents-desktop-dist \
  --no-traffic --tag desktop-candidate
```

## Production verification and rollback

Verify all three feed documents, the immutable artifact, and the retained
mobile feed on the tagged candidate URL. Use GET for the Expo manifest; this
server does not promise HEAD parity:

```sh
curl -fsS https://updates.openagents.com/desktop/openagents/rc/release.json
curl -fsS https://updates.openagents.com/desktop/openagents/rc/manifest.json
curl -fsS https://updates.openagents.com/desktop/openagents/rc/manifest.sig.json
curl -fsS https://desktop-candidate---oa-updates-ezxz4mgdsq-uc.a.run.app/openagents-mobile/manifest \
  -o /tmp/mobile-manifest \
  -H 'expo-protocol-version: 1' \
  -H 'expo-platform: ios' \
  -H 'expo-runtime-version: <current-mobile-runtime>' \
  -H 'expo-channel-name: openagents-production'
```

Only after both candidate feeds pass, move 100% traffic to the candidate
revision and repeat the checks through `https://updates.openagents.com`.

Download the public DMG to a new path, verify its SHA-256 against the signed
manifest, mount it, and run the packaged smoke from a pristine user-data root
before any wider lifecycle acceptance:

```sh
mkdir -p /tmp/openagents-release-mount
hdiutil attach /tmp/OpenAgents-<version>-arm64.dmg -nobrowse \
  -mountpoint /tmp/openagents-release-mount
rm -rf /tmp/openagents-release-smoke
OPENAGENTS_DESKTOP_SMOKE=1 \
OPENAGENTS_DESKTOP_USER_DATA=/tmp/openagents-release-smoke \
  /tmp/openagents-release-mount/OpenAgents.app/Contents/MacOS/OpenAgents
```

The smoke must reach `[openagents-desktop smoke] OK` and lifecycle teardown
`{"ok":true,"active":0}`. In particular, verify the packaged renderer exists
under `app.asar.unpacked/dist/renderer`; RC1 failed on a browser-specific V8
snapshot fuse and RC2 failed because Chromium could not admit the renderer from
inside ASAR while the hardened file-protocol fuse was disabled. Never relax
`GrantFileProtocolExtraPrivileges=false` to repair this.

Then run the clean-install lifecycle and record first launch, account
readiness, a coding turn, update interruption/resume, rollback,
uninstall/reinstall, and diagnostics export.

If the feed deploy is unhealthy, route Cloud Run traffic back to the previous
ready revision immediately. Do not overwrite or delete the immutable GCS
artifact; publish a strictly newer RC after fixing the cause. The client rejects
duplicate, downgrade, wrong-channel, wrong-signature, wrong-hash, and wrong-size
releases fail closed.
