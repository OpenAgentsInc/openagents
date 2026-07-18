# T3M-F2 mobile native-finish receipt

- Date: 2026-07-17
- Program: T3 Code mobile full parity
- Reference: `t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Status: source and installed-simulator implementation complete; physical,
  signed-distribution, and owner-acceptance matrix pending

## Component census

The executable census in
`apps/openagents-mobile/src/contracts/t3-mobile-component-census.ts` accounts
for all 43 named mobile components once and ties every row to source evidence.
Forty-one components are complete. Two are explicit OpenAgents adaptations:
artifact/receipt inspection stays inside the bounded Files/Changes workbench,
and visual treatment uses the OpenAgents Effect Native token system rather
than importing T3 branding.

| Area | Rows | Result |
| --- | ---: | --- |
| Shell | 5 | complete |
| Navigation | 6 | complete |
| Transcript | 8 | complete |
| Runtime interactions | 5 | complete |
| Composer | 7 | complete |
| Workbench | 5 | 4 complete, 1 adapted |
| Native finish | 7 | 6 complete, 1 adapted |

The census test also proves that A1, A2, A3, A4, B1, B2, C1, C2, D1, D2,
E1, E2, F1, and F2 are all represented and freezes the layout boundary at
767 points compact / 768 points regular.

## Native finish

- Route, drawer, sheet, and picker transitions use one restrained 180 ms
  native layout transition and are suppressed by Reduce Motion.
- A closed intent set provides selection, light-action, or warning feedback.
  Draft typing, transcript pinning, and terminal traffic never vibrate.
- Android layout animation support is enabled once at the native host.
- `expo-haptics` is packaged as an application dependency; failures remain
  non-fatal and never alter intent admission.
- iOS Liquid Glass icon controls now preserve authored accessible names. The
  installed hierarchy originally exposed the SF Symbol names `More` and
  `Compose`; the renderer now uses semantic SwiftUI button labels for grouped
  controls and an accessible React Native press target with a decorative,
  hidden SwiftUI glass island for standalone controls.

## Installed simulator matrix

### iOS

- `expo prebuild --platform ios --clean` and CocoaPods completed.
- Release configuration built for an iPhone 17 Pro, iOS 26.5 simulator with a
  bundled production JavaScript payload. Code signing was intentionally
  disabled for this simulator artifact.
- The release app installed and launched as `com.openagents.app`.
- Maestro completed: launch → Khala transcript → authored `Open settings`
  control → Settings → Notifications → explicit-enable and attention-state
  assertions, with settings and notification screenshots.
- The simulator correctly remained `permission: undetermined` and
  `registration: unregistered` until explicit enable; no device-token claim is
  made from an unsigned simulator build.

### Android

- `expo prebuild --platform android --clean` completed.
- Gradle `assembleRelease` completed and produced `app-release.apk`.
- The APK installed and launched on the `khala_test` Android API 35 arm64
  emulator as `com.openagents.app`.
- Maestro completed: clear application state → Khala transcript → authored
  `Open settings` control → Settings → Notifications → attention preference,
  with settings and notification screenshots.
- The emulator reported permission granted and native registration
  unavailable. The UI rendered that distinction instead of inventing a token
  or delivery success.

## Automated verification

- OpenAgents mobile: 55 files / 259 tests passed.
- Effect Native React Native renderer: 22 tests passed.
- Behavior contracts: 36 tests passed.
- Mobile and React Native renderer TypeScript checks passed.
- The installed iOS and Android release-mode simulator journeys passed.

## Remaining release boundary

This receipt does not claim full mobile parity or a shippable store build.
Those claims remain blocked on evidence that cannot be synthesized in source
or simulator automation:

1. Run compact-phone and regular-tablet journeys on physical iOS and Android
   hardware, including background/reconnect, terminal recovery, notifications,
   share intake, and consequential confirmations.
2. Traverse the application with VoiceOver and TalkBack and record focus
   order, authored names, Dynamic Type/font scale, rotor/action behavior, and
   escape/back restoration.
3. Produce and promote signed iOS and Android distributions from the landed
   commit, then repeat the installed journey from those artifacts.
4. Record owner acceptance against the signed physical-device matrix.

Release signing, store upload, and owner acceptance were not authorized by the
implementation direction and are therefore not inferred from these green
simulator receipts.
