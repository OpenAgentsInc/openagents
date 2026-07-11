# Effect Native #64 Android production-app receipt

Date: 2026-07-10 CDT  
Consumer: `apps/openagents-mobile`  
App source base: `907bb8e2a5`  
Upstream renderer fix: `OpenAgentsInc/effect-native@796f4b9e935a5425934230dad3914b7c6fd90587` (#73)

## What was proved

The active OpenAgents mobile application—not the deprecated Khala Mobile
client and not an in-process renderer shim—was locally prebuilt, bundled as an
Android release APK, installed, cold-launched, and exercised on an Android 15
Pixel 7 AVD. No Expo/EAS cloud service was used.

The run also exposed and fixed the clean-build Metro seam: shared NodeNext
packages use emitted-ESM `.js` specifiers while Metro consumes their TypeScript
sources. The app now has a bounded resolver that tries only matching relative
`.ts` / `.tsx` files before normal Metro resolution.

The upstream #73 transcript lowering was applied hunk-wise to the vendored RN
renderer. A real composer submission containing one long unbroken token then
rendered as a user `TranscriptMessage`; its bubble wrapped inside the viewport
and the assistant pending state stayed visible. The catalog v29 contract did
not change.

## Environment and commands

- macOS host, Apple Silicon
- OpenJDK 17.0.19
- Android SDK 36 build tools
- Android Emulator 36.6.11
- AVD `khala_test`: Pixel 7, Android 15 / API 35, Google APIs, arm64-v8a
- emulator framebuffer: 1080 × 2400
- `expo prebuild --platform android --no-install`
- `NODE_ENV=production ./gradlew app:assembleRelease`
- `adb install -r app-release.apk`
- explicit cold launch of `com.openagents.app/.MainActivity`
- `adb input` composer text/send interaction
- `adb screencap` pixel capture

## Pixel evidence

[`receipts/2026-07-10-en64-android-transcript-width.png`](./receipts/2026-07-10-en64-android-transcript-width.png)

SHA-256:
`df3c2a2394f17b8cbe16b089a7790559c68592bbdf13025b632824f762d580c5`

The screenshot contains only fixture text typed for this proof and public app
chrome. It contains no token, account identifier, private conversation, path,
or device identifier.

## Acceptance boundary

This is a real Android simulator pixel/build/interaction receipt for the active
production Effect Native consumer. The existing GL-1 receipt set supplies the
corresponding real iOS simulator evidence. The protocol-honest two-session
Khala Sync convergence oracle remains separate; live staging/prod Sync was
explicitly waived as an Effect Native conversion gate and remains an OpenAgents
product dogfood milestone.
