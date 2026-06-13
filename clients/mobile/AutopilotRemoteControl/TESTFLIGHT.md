# Ship Autopilot Remote Control — local build (our infra)

**We have switched off Expo/EAS.** Builds compile **locally on our own Mac**
and JS updates ship over **our own OTA server** (`updates.openagents.com`).
EAS Build / EAS Submit / `eas update` (u.expo.dev) are **no longer used** — do
not reintroduce them.

## The two delivery paths

1. **OTA (JS-only changes)** → our update server. No build, no Apple, no Expo.
   - The app's `app.config.ts` `updates.url` points at `updates.openagents.com`.
   - Publish with our pipeline: `apps/oa-updates/scripts/publish-ota.sh`
     (fingerprint → `expo export` → bake → deploy to our Cloud Run server).
   - The on-device `expo-updates` runtime pulls the new JS bundle on next launch
     **only when the native fingerprint matches** the installed binary.

2. **Native build (anything that changes native code/modules)** → local Xcode
   archive on this Mac, uploaded to TestFlight via Apple. Required when native
   deps change (e.g. adding `react-native-reanimated` / `react-native-gesture-handler`
   for the drawer) — those can't ship OTA.

TestFlight distribution itself is **Apple's App Store Connect** — that is the
only external dependency and is unavoidable for any iOS app. It is not Expo.

## Known identifiers
- iOS bundle id: `com.openagents.autopilot-mobile`
- Apple Team: OpenAgents, Inc. — `HQWSG26L43`
- App Store Connect app id (`ascAppId`): `6779949704`

## One-time local setup
- **Xcode** (Xcode 26.5 confirmed on this Mac) + command-line tools.
- **CocoaPods** (`pod`, confirmed) and **fastlane** (`fastlane`, confirmed).
- iOS **distribution certificate** + **provisioning profile** for the bundle id
  installed in the login keychain.
- An **App Store Connect API key** (`.p8` + key id + issuer id) for non-interactive
  TestFlight upload, stored under `.secrets/` (git-ignored), never committed.

## Local build → TestFlight (run from clients/mobile/AutopilotRemoteControl)
```sh
# 1) generate the native iOS project from the Expo config (managed → bare, local)
npx expo prebuild --platform ios --clean

# 2) install pods
cd ios && pod install && cd ..

# 3) archive + export a signed .ipa locally (fastlane gym, or raw xcodebuild)
fastlane gym --scheme AutopilotRemoteControl --export_method app-store \
  --output_directory build --output_name AutopilotRemoteControl.ipa

# 4) upload to App Store Connect → TestFlight via Apple (fastlane pilot, or
#    Apple Transporter / `xcrun altool`), using the ASC API key:
fastlane pilot upload --ipa build/AutopilotRemoteControl.ipa \
  --api_key_path .secrets/asc_api_key.json
```
Apple processing takes ~10–15 min; the build then appears in **TestFlight**
(Internal Testing → install via the TestFlight app). No App Review for internal.

Bump the build number per native build (CFBundleVersion) before archiving.

## What goes OTA vs. native (decide before shipping)
- JS / React / styles / assets only → **OTA** (our server), no build.
- New/updated native module, native config, app icon, entitlements, SDK bump
  → **native build** (local, above), then OTA resumes against the new binary.

Use the `ship-mode` / `fingerprint-classify` protocol cores to classify a change
(OTA vs rebuild) from the Expo fingerprint before shipping.
