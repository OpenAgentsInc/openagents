# OpenAgents mobile — v0.4.0 build 103 (iOS TestFlight) first upload receipt (2026-07-09)

First TestFlight upload of the greenfield **OpenAgents** app
(`apps/openagents-mobile`, #8597). Local-first path only — `expo prebuild` +
`xcodebuild` + `altool`; **no EAS**. This mirrors
`docs/mobile/2026-06-26-khala-testflight-release-runbook.md` (the proven
fastlane-cert + manual-signing-export path; Xcode cloud signing is BLOCKED for
our ASC key), adapted for this Expo app's `OpenAgents` workspace/scheme.

## Store identity (verified against the live record BEFORE upload)

- App Store Connect app: **`OpenAgents`**, bundle `com.openagents.app`,
  ASC app id `6748620735`, Team `HQWSG26L43` — the owner-designated EXISTING
  app record (issue #8597 identity lock).
- Latest pre-existing build: **102 (v0.3.0**, uploaded 2025-11-13, expired**)**,
  queried via `fastlane run latest_testflight_build_number`. This release is
  therefore **`expo.version` 0.4.0, `ios.buildNumber` 103** — monotonic against
  the real store record, NOT inherited from any legacy Khala app numbering.
- Version bump commit on `main`: `fc68a5de57` (cut from a fresh worktree off
  `origin/main` at `bcc505a83c`, the greenfield-scaffold head).

## Steps run (from the worktree)

```sh
# 0. secrets
set -a; . ~/work/.secrets/appstoreconnect.env; set +a   # ASC_API_KEY_ID/ISSUER/KEY_PATH

# 1. prebuild (CNG; ios/ is gitignored)
cd apps/openagents-mobile && bunx expo prebuild --platform ios --clean   # 88 pods

# 2. archive (Release, generic iOS device; dev-cert signing is fine here)
xcodebuild -workspace ios/OpenAgents.xcworkspace -scheme OpenAgents \
  -configuration Release -destination 'generic/platform=iOS' \
  -archivePath <scratch>/OpenAgents.xcarchive archive \
  -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
  -authenticationKeyID "$ASC_API_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_API_ISSUER_ID" \
  DEVELOPMENT_TEAM=HQWSG26L43
# -> ** ARCHIVE SUCCEEDED **

# 3. App Store provisioning profile incl. the local Apple Distribution cert
#    (cert already in keychain from the khala build-21 release)
fastlane run get_provisioning_profile api_key_path:<key.json> \
  app_identifier:com.openagents.app force:true output_path:<dir>
# -> "com.openagents.app AppStore", installed to ~/Library/MobileDevice/Provisioning Profiles/

# 4. export IPA with MANUAL signing (never -allowProvisioningUpdates here)
xcodebuild -exportArchive -archivePath <scratch>/OpenAgents.xcarchive \
  -exportPath <scratch>/OpenAgents-export -exportOptionsPlist ExportOptions.plist
# ExportOptions: method app-store-connect, teamID HQWSG26L43, signingStyle manual,
#   provisioningProfiles { com.openagents.app: "com.openagents.app AppStore" }
# -> ** EXPORT SUCCEEDED ** (OpenAgents.ipa, 13.4 MB)

# 5. upload
xcrun altool --upload-app -f <scratch>/OpenAgents-export/OpenAgents.ipa -t ios \
  --apiKey "$ASC_API_KEY_ID" --apiIssuer "$ASC_API_ISSUER_ID"
```

## Result

- **`UPLOAD SUCCEEDED with no errors`**, Delivery UUID
  `58d98f83-4e3c-427b-816f-9832e290d655` (2026-07-09 ~18:25 CT).
- Archived-app identity verified pre-upload:
  `CFBundleIdentifier=com.openagents.app`, `CFBundleShortVersionString=0.4.0`,
  `CFBundleVersion=103`, display name `OpenAgents`, compiled `Assets.car` with
  `CFBundlePrimaryIcon/CFBundleIconName=AppIcon` (the pinned icon from the
  identity oracle), `ITSAppUsesNonExemptEncryption=false` (export compliance
  auto-clears).
- Build 103 registered against prerelease version 0.4.0 in ASC within ~3
  minutes of upload, and reached **`processingState=VALID`** by ~18:31 CT
  (verified via `latest_testflight_build_number` and the ASC `/v1/builds` API:
  `build 103 processingState=VALID uploaded=2026-07-09T16:27:16-07:00`).

## Notes for the next release

- Bump `expo.version` and `ios.buildNumber` in `apps/openagents-mobile/app.json`
  (build number must exceed the ASC latest — check first), commit to `main`,
  then run the steps above from a fresh worktree.
- No OTA/update feed is configured in this build — deliberate (#8597): the
  owned `apps/oa-updates` channel for OpenAgents mobile is a separate lane, and
  the identity test fails if an updates URL sneaks in early.
- The icon check that matters for validation is the NESTED
  `CFBundleIcons/CFBundlePrimaryIcon/CFBundleIconName` actool injects — the
  top-level `CFBundleIconName` note in the native-Khala runbook applies only to
  hand-maintained Info.plists.
