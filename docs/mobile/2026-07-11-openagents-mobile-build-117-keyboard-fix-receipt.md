# OpenAgents mobile — v0.5.2 build 117: keyboard lifecycle fix

- Date: 2026-07-11
- Source build-number commit: `8011cfbeaa553165bc866cdb831a188568b5428c`
- Keyboard fix commit: `27159b117fca8dac39fe7980c781d89bf1216744`
- App identity: `OpenAgents` / `com.openagents.app` / Team `HQWSG26L43`
- App Store Connect build: `117`
- App Store Connect build ID: `edf45369-949c-4d6b-abb7-97b6d1137ac7`
- Distribution state: `VALID`

## Included correction

The home composer now stays above the software keyboard, dismisses on a
background press, and dismisses before return-key or send-button submission.
The React Native host owns keyboard geometry/dismissal while the existing
Effect Native program remains the sole application/composer state authority.

The Android release receipt for the same behavior records exact before/focused/
after bounds and IME state in
[`2026-07-10-mobile-keyboard-avoidance-receipt.md`](./2026-07-10-mobile-keyboard-avoidance-receipt.md).

## Verification and build

- Live App Store Connect preflight reported build 116 as the latest upload.
- App version remained frozen at `0.5.2`; only `ios.buildNumber` advanced to
  `117`.
- Mobile tests passed: 60 tests / 236 expectations.
- Mobile TypeScript typecheck passed.
- Clean Expo iOS prebuild produced `CFBundleVersion=117` and
  `CFBundleShortVersionString=0.5.2`.
- CocoaPods linked the application-local `OpenAgentsLiquidGlass` SwiftUI module
  and the Expo/React Native release graph.
- The Release archive completed successfully.
- Archived identity: `OpenAgents`, `com.openagents.app`, `0.5.2`, build `117`,
  Team `HQWSG26L43`.
- Embedded Expo Updates runtime:
  `f5e5bbc3d8f8e64423016cba7ff61730ea7f6d86`.
- A fresh `com.openagents.app AppStore` provisioning profile was created for
  the installed Apple Distribution certificate.
- Manual App Store Connect export completed successfully.
- Exported IPA SHA-256:
  `6fa70a19d75abdc55d721c50a61d76784816df9f0456b2381b094a48ca447cf3`.
- Exported IPA signature: `Apple Distribution: OpenAgents, Inc.
  (HQWSG26L43)`; bundle/version/build/runtime re-verification matched the
  archive.
- `xcrun altool` uploaded the exact IPA. The live TestFlight build-number query
  immediately advanced to 117.
- App Store Connect `/v1/builds` reports build 117 as `VALID`, uploaded at
  `2026-07-10T22:13:21-07:00`.

This was a local Xcode/App Store Connect release. No EAS build or Expo-hosted
distribution service was used.
