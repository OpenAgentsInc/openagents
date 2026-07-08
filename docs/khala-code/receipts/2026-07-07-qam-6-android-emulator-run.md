# QAM-6 Android Emulator Lane Receipt

Date: 2026-07-07

Issue: #8541

Status: FULL DEVICE RECEIPT — all three launch/sign-in flows GREEN on a real
Android emulator, capture parity produced, nightly row wired. This is the
#8541 exit receipt.

This receipt records a real local Android emulator run for the QAM-6 lane on the
owned macOS (Apple Silicon) machine. The previously-blocking gap
(`SignedInThreadSmoke` needed a public-safe seeded signed-in environment) is now
closed with the seeded public-safe AgentFlampy account.

## Environment

- Host: macOS (arm64/Apple Silicon).
- Android SDK: Homebrew `android-commandlinetools` at
  `/opt/homebrew/share/android-commandlinetools` (no `~/Library/Android/sdk`).
- JDK: Homebrew `openjdk@17`
  (`/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home`).
- AVD: `khala_test`, device profile `pixel_7`, system image
  `system-images;android-35;google_apis;arm64-v8a` — Android 15 (API 35).
- Emulator boot: `emulator -avd khala_test -no-boot-anim -no-snapshot
  -netdelay none -netspeed full -no-audio -gpu swiftshader_indirect`;
  `adb shell getprop sys.boot_completed` returned `1` (boot completed);
  `adb devices` showed `emulator-5554 device`.
- App id: `com.openagents.khala.mobile`. Version `0.1.0`, `versionCode` `3`.
- Builds (both from a fresh `expo prebuild --platform android`, Gradle
  `--no-daemon`):
  - Non-baked **Release** APK
    (`android/app/build/outputs/apk/release/app-release.apk`, ~161 MB) — used for
    `LaunchFallback` + `LaunchGitHubSignInInteraction` (they assert the sign-in
    screen, so the build must NOT auto-sign-in).
  - Baked **Release** APK — same build with the seeded AgentFlampy creds inlined
    via a temporary `.env.local`
    (`EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID`/`_TOKEN`), so the app
    auto-signs-in (the app has no manual-sign-in UI). The `.env.local` is always
    removed after the build (trap); the shippable AAB was built BEFORE any bake
    and verified free of baked creds.
- Maestro: `~/.maestro/bin/maestro`, run with `--device emulator-5554`.

## Result

- PASS: emulator boot proof (`sys.boot_completed=1`, `emulator-5554 device`).
- PASS: Release APK assembled and installed on AVD `khala_test`.
- PASS: Maestro `LaunchFallback.yaml` (all steps COMPLETED).
- PASS: Maestro `LaunchGitHubSignInInteraction.yaml` (tap on "Log in with GitHub"
  leaves the app process for the external browser surface; all steps COMPLETED).
- PASS: Maestro `SignedInThreadSmoke.yaml` on the baked Release APK — TWO green
  runs (exit 0, 14/14 steps COMPLETED, 0 FAILED): auto-signs-in as the seeded
  public-safe AgentFlampy account, the thread list renders, the seeded thread
  opens, the composer lane picker (`Send with Claude`) is visible, and a typed
  public-safe message sends and renders in the transcript.
- PASS: `adb exec-out screencap -p` capture parity for the visual tier — Android
  screencaps captured for launch-fallback, github-sign-in-interaction, and
  signed-in-thread-smoke.
- PASS: Android-keyed visual baselines blessed from emulator screencaps
  (`khala.mobile.android.launch-fallback.pixel-8.dark`,
  `khala.mobile.android.github-sign-in-interaction.pixel-8.dark`).

## Repeatable runners

- Launch + sign-in + capture lane (nightly Android row):
  `clients/khala-mobile/scripts/android-emulator-test-run.sh`
  (`bun run --cwd clients/khala-mobile qa:android:emulator`).
- Signed-in thread smoke (baked auto-sign-in, Android parity of the iOS
  `signed-in-thread-smoke-run.sh`):
  `clients/khala-mobile/scripts/signed-in-thread-smoke-android-run.sh`.

## Nightly wiring

`scripts/qa-nightly-matrix.ts` appends the opt-in Android step
`mobile-android-emulator-smoke`
(`bash clients/khala-mobile/scripts/android-emulator-test-run.sh`) when
`OA_QA_NIGHTLY_INCLUDE_MOBILE_ANDROID=1` on a macOS runner with a booted
emulator. Documented in `docs/qa/khala-code-nightly-matrix.md`.

## Android Visual Baselines

- Baseline report:
  `docs/khala-code/receipts/2026-07-07-qam-6-android-visual-baselines.json`
- Baseline manifest: `docs/khala-code/receipts/qam-4-baselines/manifest.json`
- Android baseline entries:
  - `khala.mobile.android.launch-fallback.pixel-8.dark`
  - `khala.mobile.android.github-sign-in-interaction.pixel-8.dark`
- Blessed baseline PNGs:
  - `docs/khala-code/receipts/qam-4-baselines/screenshots/khala.mobile.android.launch-fallback.pixel-8.dark.png`
  - `docs/khala-code/receipts/qam-4-baselines/screenshots/khala.mobile.android.github-sign-in-interaction.pixel-8.dark.png`

## Release AAB (deliverable — Play internal testing)

- Built (non-baked) via `./android/gradlew -p android :app:bundleRelease`:
  `android/app/build/outputs/bundle/release/app-release.aab` (~119 MB),
  `versionCode` 3 / `versionName` 0.1.0.
- Signing: DEBUG-signed (`CN=Android Debug`, SHA1
  `5E:8F:16:06:2E:A3:CD:2C:4A:0D:54:78:76:BA:A6:F3:8C:AB:F6:25`) — the Gradle
  release config still points at `signingConfigs.debug`. A real Play **upload
  keystore** + Play App Signing are owner-gated (NEEDS_OWNER.md, #8490); the AAB
  is not Play-uploadable until that lands. See
  `docs/khala-mobile/2026-07-06-android-build-and-upload-runbook.md` §3.
