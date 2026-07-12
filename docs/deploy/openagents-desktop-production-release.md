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
security find-identity -v -p codesigning | grep HQWSG26L43
git fetch origin main
test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"
test -z "$(git status --porcelain)"
```

## Build and preflight

Set the intended version in `apps/openagents-desktop/package.json`, run the
release-contract tests, commit it to `main`, and then build:

```sh
bun test apps/openagents-desktop/tests/release-preflight.test.ts \
  apps/openagents-desktop/tests/update-contract.test.ts \
  apps/openagents-desktop/tests/publish-release.test.ts \
  apps/openagents-desktop/tests/package-macos.test.ts
bun run --cwd apps/openagents-desktop typecheck
bun run --cwd apps/openagents-desktop build
bun apps/openagents-desktop/scripts/release-preflight.ts --channel rc --json
```

Electron Forge's DMG tool reaches two native addons through Bun's isolated
dependency store. `make:mac` runs `prepare-macos-maker.ts`, which builds both
before Forge starts. For a manual diagnosis, the equivalent commands are:

```sh
(cd node_modules/.bun/macos-alias@0.2.12/node_modules/macos-alias && \
  npm exec --yes --package=node-gyp -- node-gyp rebuild)
(cd node_modules/.bun/fs-xattr@0.3.1/node_modules/fs-xattr && \
  npm exec --yes --package=node-gyp -- node-gyp rebuild)
```

Then sign, package, and notarize the app:

```sh
set -a
source ~/work/.secrets/appstoreconnect.env
set +a
bun run --cwd apps/openagents-desktop make:mac
```

Forge notarizes the `.app`; separately submit and staple the final DMG so the
distributed bytes themselves carry an offline Gatekeeper ticket:

```sh
DMG=apps/openagents-desktop/out/make/OpenAgents-<version>-arm64.dmg
xcrun notarytool submit "$DMG" \
  --key "$ASC_API_PRIVATE_KEY_PATH" \
  --key-id "$ASC_API_KEY_ID" \
  --issuer "$ASC_API_ISSUER_ID" --wait --output-format json
xcrun stapler staple "$DMG"
xcrun stapler validate "$DMG"
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
  bun apps/openagents-desktop/scripts/publish-release.ts \
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
