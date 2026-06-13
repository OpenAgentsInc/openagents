# Ship Autopilot Remote Control to TestFlight (fast-track)

Goal: get the current Expo app (the shell) onto a personal device via TestFlight,
ASAP, then iterate. EAS Build compiles a signed `.ipa` in the cloud (no local
Xcode archive needed); EAS Submit uploads it to App Store Connect → TestFlight.

## What's already wired (in this repo)
- `app.config.ts` — iOS `bundleIdentifier: com.openagents.autopilot-mobile`,
  `ITSAppUsesNonExemptEncryption: false` (skips the export-compliance prompt).
- `eas.json` — `production` build profile + `submit.production.ios` with
  `appleTeamId: HQWSG26L43` (OpenAgents, Inc.) and an `ascAppId` placeholder.
- Expo SDK 55 app that builds (boots on simulator + web today).

Known identifiers:
- iOS bundle id: `com.openagents.autopilot-mobile`
- Apple Team: OpenAgents, Inc. — `HQWSG26L43`
- `ascAppId`: create the App Store Connect record, then paste its numeric
  Apple ID into `eas.json`.

## Owner prerequisites (one-time, can't be automated for you)
1. **Apple Developer Program** membership ($99/yr) — https://developer.apple.com/account
2. **Expo/EAS account** — `eas login` (free tier is fine for builds).
3. An **App Store Connect app record** for `com.openagents.autopilot-mobile`
   under the OpenAgents, Inc. team (`HQWSG26L43`) — create via App Store Connect
   → Apps → + → New App, then copy its **Apple ID** (the numeric `ascAppId`)
   into `eas.json` (`submit.production.ios.ascAppId`).

## Steps (run from clients/mobile/AutopilotRemoteControl)
```sh
# 0) install the CLI + log in (once)
bun add --global eas-cli            # or: npm i -g eas-cli
eas login

# 1) link the project (writes extra.eas.projectId + owner to app config; once)
eas init

# 2) build a signed production .ipa in the cloud
#    EAS will interactively create/manage your iOS distribution cert +
#    provisioning profile (just log in with your Apple account when prompted).
eas build --platform ios --profile production

# 3) submit the build to App Store Connect → TestFlight
eas submit --platform ios --latest
#    (fill ascAppId in eas.json first, or let it prompt)

# …or do 2+3 in one shot:
eas build --platform ios --profile production --auto-submit
```
Processing on Apple's side takes ~10–15 min; then the build appears in
**TestFlight**. Add yourself as an internal tester (App Store Connect →
TestFlight → Internal Testing) and install via the TestFlight app on your device.
No App Review needed for internal TestFlight.

## Notes
- This ships the **shell** (nav screens). The live Pylon-connect functionality
  (CL-5/CL-6 P0) lands later; getting the shell on-device first is intentional.
- The app currently omits the native-only deps (mmkv/secure-store) that were
  placeholders; they're re-added in full CL-6. A production EAS Build still
  works without them.
- If you want CI later, EAS Workflows (`.eas/workflows/submit-ios.yml`) or
  `EXPO_TOKEN` in GitHub Actions can automate build+submit.
