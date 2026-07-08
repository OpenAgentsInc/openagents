# Khala Mobile — TestFlight build 20 release receipt (2026-07-07)

Local-only iOS release of the Expo React Native app `clients/khala-mobile`
(App Store Connect app "Khala Code", SKU `khalacode`, id `6787620136`) to
TestFlight, per the repo mobile build policy (`expo prebuild` + `xcodebuild`
archive/export + `xcrun altool` upload — never `eas build`/`eas submit`).

This build ships the **App Store reviewer demo-login mode**: long-press the
"Sign in with GitHub" button (~1s) to enter an offline demo session with
hardcoded example data (contract `khala_mobile.auth.demo_login_example_data.v1`;
see `docs/mobile/2026-07-07-app-store-reviewer-demo-login.md` for the exact App
Review Information → Notes text).

## Build

- App: Khala Code (Expo) — `clients/khala-mobile`
- Bundle id: `com.openagents.khala.mobile`
- Apple Team: `HQWSG26L43`
- Marketing version (`CFBundleShortVersionString`): `0.1.0`
- Build number (`CFBundleVersion`): **20** (bumped 19 → 20 in `app.json`)
- Xcode workspace/scheme: `ios/KhalaCode.xcworkspace` / `KhalaCode`
- Configuration: `Release`, destination `generic/platform=iOS` (device-only;
  the booted simulators were left untouched — other agents were using them)
- Built from a clean detached worktree off `origin/main`
  (`git worktree add --detach /tmp/oa-demo origin/main`), at the commit that
  landed the demo-login feature.

## Signing

- Archive and export both signed with **manual** App Store distribution:
  - Apple Distribution cert `OpenAgents, Inc. (HQWSG26L43)` (already in keychain)
  - Provisioning profile `com.openagents.khala.mobile AppStore` (already
    installed, valid to 2027-06-26)
- No new certs/profiles were minted; existing distribution material was reused.

## Commands

- `npx expo prebuild --platform ios --clean`
- `xcodebuild -workspace ios/KhalaCode.xcworkspace -scheme KhalaCode
  -configuration Release -destination 'generic/platform=iOS'
  -archivePath /tmp/KhalaMobile20.xcarchive archive CODE_SIGN_STYLE=Manual
  "CODE_SIGN_IDENTITY=Apple Distribution: OpenAgents, Inc. (HQWSG26L43)"
  "PROVISIONING_PROFILE_SPECIFIER=com.openagents.khala.mobile AppStore"
  DEVELOPMENT_TEAM=HQWSG26L43`
- `xcodebuild -exportArchive -archivePath /tmp/KhalaMobile20.xcarchive
  -exportPath /tmp/KhalaMobile20-export -exportOptionsPlist <app-store manual>`
- `xcrun altool --upload-app -f .../KhalaCode.ipa -t ios --apiKey <ASC key>
  --apiIssuer <ASC issuer>` (ASC API key from `~/work/.secrets/appstoreconnect.env`)

## Artifacts

- Archive: `/tmp/KhalaMobile20.xcarchive` (CFBundleVersion 20, 0.1.0,
  com.openagents.khala.mobile)
- Exported IPA: `/tmp/KhalaMobile20-export/KhalaCode.ipa` (~32 MB)

## Upload

- `xcrun altool --validate-app` → **VERIFY SUCCEEDED with no errors**
- `xcrun altool --upload-app` → **UPLOAD SUCCEEDED with no errors**
- Delivery UUID: `3fdee571-5b8f-49a4-a9c2-3536d9dc8022`
- Upload timestamp: 2026-07-07 ~21:21 UTC (altool success)
- ASC confirmation (App Store Connect API `/v1/builds`): build **20** shows
  **VALID**, uploaded 2026-07-07T19:22:12-07:00.

## Gate

Before the build, the mobile gate was green on the same commit:
`bun run typecheck`, `bun run architecture:check`, and `bun run qa:mobile:gate`
all pass (415 tests, 0 fail), including the new
`tests/demo-login-mode.test.ts` behavior-contract oracle.

## Owner-gated remaining steps

TestFlight group assignment, export-compliance/beta-review, and any App Store
review/metadata are Apple-side/owner-gated. `ITSAppUsesNonExemptEncryption=false`
is set in `app.json`, so export compliance auto-clears. The App Review demo-login
Notes text to paste into ASC is in `~/work/NEEDS_OWNER.md` and
`docs/mobile/2026-07-07-app-store-reviewer-demo-login.md`.

Build 20 is UP on TestFlight (VALID on ASC).
