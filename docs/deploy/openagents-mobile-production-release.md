# OpenAgents mobile production release

This is the operator runbook for building and distributing the active
OpenAgents mobile app from a clean `origin/main` worktree. The product identity
is fixed:

- app: `apps/openagents-mobile`
- name: `OpenAgents`
- iOS and Android ID: `com.openagents.app`
- Apple team: `HQWSG26L43`
- App Store Connect app ID: `6748620735`
- owned OTA endpoint: `https://updates.openagents.com/openagents-mobile/manifest`
- OTA channel: `openagents-production`

Builds and submissions are local. Do not use EAS Build, Submit, Update, or the
Expo CDN. Native binary changes require a new store build; JavaScript-only
changes may use the fingerprint-matched owned OTA path below.

## Clean-source and release preflight

Use a fresh worktree at exact `origin/main`. Never archive from a checkout with
another agent's changes.

```sh
git fetch origin main
git worktree add --detach /tmp/openagents-mobile-release origin/main
cd /tmp/openagents-mobile-release
test -z "$(git status --porcelain)"

pnpm install --frozen-lockfile
pnpm --dir apps/openagents-mobile run test
pnpm --dir apps/openagents-mobile run typecheck
```

Before a store upload, increase `expo.ios.buildNumber` in
`apps/openagents-mobile/app.json`; Apple requires a unique increasing build.
Change `expo.version` only for an intentional marketing-version release. Commit
and push those release identities to `main` before archiving.

## Generate native projects

Expo prebuild output is derived and untracked. Regenerate it from the exact
release commit:

```sh
CI=1 pnpm --dir apps/openagents-mobile exec expo prebuild --clean --platform ios
CI=1 pnpm --dir apps/openagents-mobile exec expo prebuild --clean --platform android
```

Confirm the derived iOS identity before continuing:

```sh
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' \
  apps/openagents-mobile/ios/OpenAgents/Info.plist
/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  apps/openagents-mobile/ios/OpenAgents/Info.plist
/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' \
  apps/openagents-mobile/ios/OpenAgents/Info.plist
```

The values must match `com.openagents.app`, `expo.version`, and
`expo.ios.buildNumber` respectively.

## iOS archive and TestFlight

Load the App Store Connect API configuration without printing it:

```sh
set -a
source ~/work/.secrets/appstoreconnect.env
set +a
```

Archive the Release build locally:

```sh
rm -rf /tmp/OpenAgents.xcarchive /tmp/OpenAgents-export
xcodebuild \
  -workspace apps/openagents-mobile/ios/OpenAgents.xcworkspace \
  -scheme OpenAgents \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath /tmp/OpenAgents.xcarchive archive \
  -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
  -authenticationKeyID "$ASC_API_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_API_ISSUER_ID" \
  DEVELOPMENT_TEAM=HQWSG26L43
```

The archive may use development signing. Distribution uses an Apple
Distribution certificate and an App Store profile installed locally. Xcode
cloud signing is not authorized for the current API key, so export manually.
If the certificate/profile is missing, create it with the established
`fastlane get_certificates` / `get_provisioning_profile` procedure in
[`../mobile/2026-06-26-khala-testflight-release-runbook.md`](../mobile/2026-06-26-khala-testflight-release-runbook.md), substituting
`com.openagents.app` and profile name `com.openagents.app AppStore`.

```sh
cat > /tmp/OpenAgents-ExportOptions.plist <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>teamID</key><string>HQWSG26L43</string>
  <key>signingStyle</key><string>manual</string>
  <key>provisioningProfiles</key><dict>
    <key>com.openagents.app</key><string>com.openagents.app AppStore</string>
  </dict>
  <key>destination</key><string>export</string>
  <key>uploadSymbols</key><true/>
</dict></plist>
EOF

xcodebuild -exportArchive \
  -archivePath /tmp/OpenAgents.xcarchive \
  -exportPath /tmp/OpenAgents-export \
  -exportOptionsPlist /tmp/OpenAgents-ExportOptions.plist
```

Verify the exported bytes and upload the exact IPA:

```sh
IPA=/tmp/OpenAgents-export/OpenAgents.ipa
shasum -a 256 "$IPA"
xcrun altool --upload-app -f "$IPA" -t ios \
  --apiKey "$ASC_API_KEY_ID" --apiIssuer "$ASC_API_ISSUER_ID"
```

Do not call the release valid until App Store Connect reports the uploaded
build `VALID`. Record the source SHA, version/build, archive runtime
fingerprint, IPA SHA-256, upload Delivery UUID, App Store Connect build ID, and
processing state in `docs/mobile/`.

For a signed physical-device smoke without a TestFlight upload, the same source
can be built and installed in place while preserving its container:

```sh
xcodebuild \
  -workspace apps/openagents-mobile/ios/OpenAgents.xcworkspace \
  -scheme OpenAgents -configuration Release \
  -destination 'generic/platform=iOS' \
  -derivedDataPath /tmp/openagents-mobile-derived build \
  CODE_SIGN_STYLE=Automatic -allowProvisioningUpdates

xcrun devicectl device install app --device '<device-id>' \
  /tmp/openagents-mobile-derived/Build/Products/Release-iphoneos/OpenAgents.app
```

Never uninstall before this smoke when continuity of an existing native
session/Sync store is part of the acceptance test.

## Android Release artifact

Generate the Android project from the same source and build the embedded
Release artifact locally:

```sh
cd apps/openagents-mobile/android
NODE_ENV=production ./gradlew app:bundleRelease
NODE_ENV=production ./gradlew app:assembleRelease
```

Artifacts land under `app/build/outputs/bundle/release/` and
`app/build/outputs/apk/release/`. A debug keystore is acceptable only for an
emulator verification receipt; it is not a Play production signature. Before a
Play upload, configure the owner-held production keystore outside Git, verify
the AAB is signed by that key, and record its SHA-256 and Play Console release
ID. Do not claim Play production distribution from an emulator/debug-signed
APK.

## Owned OTA publication

OTA is valid only when the exported update's fingerprint exactly matches the
installed native runtime. Publish iOS by default or set the platform explicitly
for Android:

```sh
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
  bash apps/oa-updates/scripts/publish-ota.sh

CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
  OA_MOBILE_PLATFORM=android \
  bash apps/oa-updates/scripts/publish-ota.sh
```

The script computes the runtime fingerprint, exports the bundle, embeds public
Expo config, mounts the signing key from Secret Manager, and deploys
`oa-updates`. It must never receive raw signing material on the command line.

Verify the candidate before shifting traffic. Use GET, not HEAD:

```sh
curl -fsS https://updates.openagents.com/openagents-mobile/manifest \
  -o /tmp/openagents-mobile-manifest \
  -H 'expo-protocol-version: 1' \
  -H 'expo-platform: ios' \
  -H 'expo-runtime-version: <exact-fingerprint>' \
  -H 'expo-channel-name: openagents-production'
```

Require HTTP 200 / Expo protocol v1, the exact runtime/channel/platform, a
signed manifest, retrievable launch asset with matching byte length/hash, and a
launch asset large enough to be a real bundle. A mismatched runtime must return
`noUpdateAvailable`; never bypass fingerprint safety to force an OTA onto an
older native build. Also verify the Desktop release feeds still resolve before
promoting the revision, because `oa-updates` serves both products.

## Rollback and evidence

- Native rollback: restore the previous TestFlight/Play build through the
  store; never reuse a build number.
- OTA rollback: route Cloud Run traffic to the previous ready revision or
  publish a newer corrective update for the same compatible runtime. Do not
  delete immutable release evidence.
- After install/update, test cold launch, native-session recovery, Sync
  continuity, named provider turn, offline/reconnect, sign-out/revocation,
  process replacement, accessibility, diagnostics, and crash-free relaunch.
- Keep secrets, raw tokens, prompts, account IDs, and unredacted device stores
  out of receipts.
