# Ship Autopilot Remote Control to TestFlight (fast-track)

Goal: get the current Expo app (the shell) onto a personal device via TestFlight,
ASAP, then iterate. EAS Build compiles a signed `.ipa` in the cloud (no local
Xcode archive needed); EAS Submit uploads it to App Store Connect → TestFlight.

## What's already wired (in this repo)
- `app.config.ts` — iOS `bundleIdentifier: com.openagents.autopilot-mobile`,
  `ITSAppUsesNonExemptEncryption: false` (skips the export-compliance prompt),
  EAS `owner: openagents` + `extra.eas.projectId` (linked via `eas init`).
- `eas.json` — `production` build profile + `submit.production.ios` with
  `appleTeamId: HQWSG26L43` (OpenAgents, Inc.). No `ascAppId` is pinned: EAS
  Submit looks up / creates the App Store Connect app record automatically.
- Expo SDK 55 app that builds (boots on simulator + web today).
- iOS distribution certificate + provisioning profile were created on EAS by
  the first `eas build` (the credential prompts answered "yes" once).

Known identifiers:
- iOS bundle id: `com.openagents.autopilot-mobile`
- Apple Team: OpenAgents, Inc. — `HQWSG26L43`
- EAS project: `@openagents/autopilot-remote-control`
  (`33dc1fb6-1b11-486d-baa0-7946302fdc68`)

## Owner prerequisites (one-time, can't be automated for you)
1. **Apple Developer Program** membership ($99/yr) — https://developer.apple.com/account
2. **Expo/EAS account** — `eas login` (free tier is fine for builds).

The App Store Connect app record no longer needs to be created by hand:
`eas submit` creates/links it on first submit. If you'd rather pin it, add the
numeric Apple ID back as `submit.production.ios.ascAppId` in `eas.json`.

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

# 3) submit the latest finished build to App Store Connect → TestFlight
#    (creates the ASC app record on first run if it doesn't exist yet)
eas submit --platform ios --latest

# …or do 2+3 in one shot (once an ASC app record exists):
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
