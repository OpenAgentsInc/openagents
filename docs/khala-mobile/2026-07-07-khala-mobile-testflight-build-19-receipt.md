# Khala Mobile — TestFlight build 19 release receipt (2026-07-07)

Local-only iOS release of the Expo React Native app `clients/khala-mobile`
(App Store Connect app "Khala Code", SKU `khalacode`, id `6787620136`) to
TestFlight, per the repo mobile build policy (`expo prebuild` + `xcodebuild`
archive/export + `xcrun altool` upload — never `eas build`/`eas submit`).

## Build

- App: Khala Code (Expo) — `clients/khala-mobile`
- Bundle id: `com.openagents.khala.mobile`
- Apple Team: `HQWSG26L43`
- Marketing version (`CFBundleShortVersionString`): `0.1.0`
- Build number (`CFBundleVersion`): **19** (bumped 18 → 19; build 18 was already
  uploaded + VALID on ASC earlier the same day, so 19 is the next unique build)
- Xcode workspace/scheme: `ios/KhalaCode.xcworkspace` / `KhalaCode`
- Configuration: `Release`, destination `generic/platform=iOS` (device-only;
  the booted simulators were intentionally left untouched — another agent was
  running Maestro against a signed-in sim)

## Signing

- Archive signed with Apple Development (team provisioning) — normal for the
  archive step.
- Export re-signed for App Store with **manual** signing:
  - Apple Distribution cert `OpenAgents, Inc. (HQWSG26L43)` (already in keychain)
  - Provisioning profile `com.openagents.khala.mobile AppStore` (already installed,
    valid to 2027-06-26)
- No new certs/profiles were minted; existing distribution material was reused.

## Artifacts

- Archive: `/tmp/KhalaMobile.xcarchive` (CFBundleVersion 19, 0.1.0,
  com.openagents.khala.mobile)
- Exported IPA: `/tmp/KhalaMobile-export/KhalaCode.ipa` (~32 MB)
- Built from a clean detached worktree off `origin/main`
  (`git worktree add --detach /tmp/oa-testflight origin/main`).

## Upload

- `xcrun altool --validate-app` → **VERIFY SUCCEEDED with no errors**
- `xcrun altool --upload-app` → **UPLOAD SUCCEEDED with no errors**
- Delivery UUID: `6163566f-fc3d-4b1c-b24f-ae494cd59586`
- Upload timestamp: 2026-07-07 20:47 UTC (altool success)
- ASC processing: build 19 uploaded; TestFlight processing is Apple-side and
  finishes a few minutes after upload (builds 1–18 all reached VALID this way).

## Owner-gated remaining steps

TestFlight processing, export-compliance/beta-review, TestFlight group
assignment, and any actual App Store review/metadata are Apple-side/owner-gated.
`ITSAppUsesNonExemptEncryption=false` is set in `app.json`, so export compliance
should auto-clear. Remaining owner actions are tracked in `~/work/NEEDS_OWNER.md`.

The final iOS build (19) is UP on TestFlight (submission receipt = the altool
UPLOAD SUCCEEDED + Delivery UUID above).
