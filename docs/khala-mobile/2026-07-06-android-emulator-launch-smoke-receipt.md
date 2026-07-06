# Khala Mobile Android Emulator Launch + Interaction Smoke Receipt

Date: 2026-07-06, America/Chicago

Issue: [#8490](https://github.com/OpenAgentsInc/openagents/issues/8490) (MM-I1)

## What this closes and what it does not

This is the Android analog of the iOS receipt in
`docs/khala-mobile/2026-07-05-maestro-launched-app-smoke-receipt.md`. It
proves, for the first time, that the built Android app actually **boots on a
real Android emulator and renders**, and that the primary sign-in control is
**genuinely interactive** (a tap hands off to a real external browser
surface), not just visible text in a build log.

It does **not** prove a real GitHub login round trip, a thread open, or a
message send on Android — no seeded public-safe GitHub test account/token was
available in this environment, exactly the same honest gap already recorded
for iOS. `khala_mobile.platform.launched_app_interaction_smoke.v1` therefore
stays `pending` in the contract registry; this receipt narrows, but does not
close, that pending contract's platform gap.

## Environment

- Host: macOS (arm64/Apple Silicon), Android SDK via Homebrew
  `android-commandlinetools` at `/opt/homebrew/share/android-commandlinetools`
  (no pre-existing `~/Library/Android/sdk`).
- Android SDK packages installed for this receipt: `emulator` (36.6.11),
  `system-images;android-35;google_apis;arm64-v8a`, `platform-tools` (37.0.0),
  `platforms;android-35`/`36`, `build-tools;34/35/36.0.0` (already present).
- AVD created: `khala_test`, device profile `pixel_7`, target
  `google_apis/arm64-v8a` Android 15 ("VanillaIceCream" / API 35).
- Emulator boot: `emulator -avd khala_test -no-boot-anim -no-snapshot
  -netdelay none -netspeed full`. Cold boot completed in 28.3s
  (`Boot completed in 28334 ms`), confirmed via
  `adb shell getprop sys.boot_completed` returning `1`.
- App id: `com.openagents.khala.mobile`. App name: Khala Code.
- App version: `0.1.0`. Android `versionCode`: `3` (from `app.json`).
- Build: `bun run build:android:local` (`./android/gradlew -p android
  :app:assembleDebug`) against a fresh `expo prebuild --platform android` in a
  clean worktree from `origin/main`. Result: `BUILD SUCCESSFUL`, artifact
  `android/app/build/outputs/apk/debug/app-debug.apk` (~283 MB unstripped
  debug APK with all ABIs).
- Install: `adb install -r app-debug.apk` → `Success`.
- Metro: local `bunx expo start --dev-client --host lan --port 8081`. Note: a
  first attempt with `--host localhost` bound Metro to `[::1]:8081`
  (IPv6-only) on this host, which the emulator's IPv4-only `10.0.2.2` NAT
  alias cannot reach even with `adb reverse tcp:8081 tcp:8081` in place —
  this produced a real "Unable to load script" / `Failed to connect to
  /10.0.2.2:8081` redbox on first launch. Restarting Metro with `--host lan`
  bound it to `*:8081` (dual-stack), after which the app connected
  immediately. This host-networking gotcha is recorded in the runbook below
  so it is not rediscovered from scratch next time.

## Result

**PASS** (both flows, real emulator, real build, real Metro-served bundle —
no stub/fake harness):

1. `clients/khala-mobile/.maestro/flows/LaunchFallback.yaml` — the SAME flow
   file already proven on iOS — passed unmodified on the Android emulator:
   launches with clear state, asserts "Khala Code" and "Sign in with GitHub"
   are visible, and asserts the "No desktop, Tailnet, or manual token is
   required." copy renders. This is real cross-platform flow reuse: the exact
   same `.yaml` needed zero edits to pass on both platforms.
2. `clients/khala-mobile/.maestro/flows/LaunchGitHubSignInInteraction.yaml`
   (new, added in this pass) — launches, asserts the sign-in screen, **taps
   "Sign in with GitHub"**, and asserts the app itself is no longer showing
   that control (because focus handed off to an external browser surface).
   On this emulator that surface was Chrome's first-run screen ("Welcome to
   Chrome"), confirming `expo-web-browser`/`expo-auth-session` genuinely
   launched an external authorization surface rather than no-op'ing or
   crashing. A `adb exec-out screencap` at each step visually confirmed: (a)
   the rendered sign-in screen with real button text, (b) the external
   browser surface after the tap.

Both flows are now committed, re-runnable regression assets (not one-off
manual checks) — the same discipline as the existing iOS Maestro flow.

## Commands

```sh
# SDK setup (one-time on a host without a prior Android SDK)
SDK=/opt/homebrew/share/android-commandlinetools
export JAVA_HOME="/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home"
export PATH="$JAVA_HOME/bin:$PATH"
yes | sdkmanager --sdk_root=$SDK --licenses
yes | sdkmanager --sdk_root=$SDK "emulator" "system-images;android-35;google_apis;arm64-v8a"
echo "no" | avdmanager create avd -n khala_test -k "system-images;android-35;google_apis;arm64-v8a" -d "pixel_7" --force

# Boot
export PATH="$PATH:$SDK/emulator:$SDK/platform-tools"
emulator -avd khala_test -no-boot-anim -no-snapshot -netdelay none -netspeed full &
adb wait-for-device

# Build + install (from clients/khala-mobile)
export ANDROID_HOME=$SDK ANDROID_SDK_ROOT=$SDK
bun run prebuild:android
bun run build:android:local
adb install -r android/app/build/outputs/apk/debug/app-debug.apk

# Metro — MUST bind dual-stack/IPv4, not IPv6-only localhost (see gotcha above)
bunx expo start --dev-client --host lan --port 8081 &

# Smoke
export PATH="$PATH:$HOME/.maestro/bin:/opt/homebrew/opt/openjdk@17/bin"
export MAESTRO_CLI_NO_ANALYTICS=1
export MAESTRO_APP_ID=com.openagents.khala.mobile
maestro test clients/khala-mobile/.maestro/flows/LaunchFallback.yaml
maestro test clients/khala-mobile/.maestro/flows/LaunchGitHubSignInInteraction.yaml
```

## Output summary

```text
Running on khala_test
 > Flow LaunchFallback
Assert that "Khala Code" is visible... COMPLETED
Assert that "Sign in with GitHub" is visible... COMPLETED
Assert that "No desktop, Tailnet, or manual token is required." is visible... COMPLETED
Assert that "Khala Code" is visible... COMPLETED

Running on khala_test
 > Flow LaunchGitHubSignInInteraction
Assert that "Khala Code" is visible... COMPLETED
Assert that "Sign in with GitHub" is visible... COMPLETED
Tap on "Sign in with GitHub"... COMPLETED
Wait for animation to end... COMPLETED
Assert that "Sign in with GitHub" is not visible... COMPLETED
```

## Public-Safe Boundary

This receipt intentionally records only public-safe metadata, visible UI
labels, and generic system-app names (Chrome). It does not include tokens,
credentials, chat bodies, raw sync rows, or private local machine data.
`SignedInThreadSmoke.yaml` was still not run on Android for the same reason it
was not run on iOS: no public-safe seeded owner/token/thread precondition was
available in this environment.

## What's still open after this receipt

- A real GitHub OAuth round trip on a device/emulator (needs a seeded
  public-safe test GitHub account — this is an owner-provisioning step, not
  an agent-runnable one from this sandbox; see `~/work/NEEDS_OWNER.md`).
- `SignedInThreadSmoke.yaml` on Android (same seeded-account blocker).
- Push notification and IAP verification specifically on Android hardware
  (issue #8490 also scopes this; both are out of reach without, respectively,
  a real FCM-registered device path and RevenueCat sandbox credentials — the
  IAP half is moot regardless since IAP is postponed for the first MVP build
  per the 2026-07-06 owner decision recorded in the launch audit §12).
- Play Console app setup (signing config, Play App Signing, internal-testing
  track upload) — owner-gated, see the Android build+upload runbook and
  `~/work/NEEDS_OWNER.md`.
