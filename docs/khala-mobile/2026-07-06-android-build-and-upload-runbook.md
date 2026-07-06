# Khala Mobile Android — local build + Play internal-testing upload runbook

Date: 2026-07-06

Issue: [#8490](https://github.com/OpenAgentsInc/openagents/issues/8490) (MM-I1)

Mirrors `docs/mobile/2026-06-26-khala-testflight-release-runbook.md` (the iOS
upload runbook for the native `Khala` app) and this repo's standing mobile
policy in `CLAUDE.md` ("Builds are local for now" — `expo prebuild` + local
Xcode/Gradle; `eas build`/`eas submit` stay unused). This is the Android
analog for the Expo `clients/khala-mobile` app (`com.openagents.khala.mobile`).

## What's proven today vs. what's owner-gated

| Step | Status |
|---|---|
| Local Android SDK/emulator bring-up on a fresh machine | **Proven this pass** — see `2026-07-06-android-emulator-launch-smoke-receipt.md` |
| `expo prebuild --platform android` + `gradlew assembleDebug` | **Proven** — `BUILD SUCCESSFUL`, real APK, real install, real launch |
| Debug APK boot + interaction smoke | **Proven this pass** (Maestro, see the receipt above) |
| Release build (signed AAB) | **Runbook below is written and internally consistent; not yet run**, because it needs an upload keystore + a decision on who holds it — see the NEEDS_OWNER entry |
| Play Console app record, signing config, internal-testing track, versionCode discipline in the store | **Owner-gated** — no Play Console account/access exists in this environment |

## 1. Prerequisites (one-time, per machine)

Android SDK via Homebrew (no full Android Studio needed):

```sh
brew install --cask android-commandlinetools     # if not already present
SDK=/opt/homebrew/share/android-commandlinetools
export JAVA_HOME="/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home"
export PATH="$JAVA_HOME/bin:$PATH"
yes | sdkmanager --sdk_root=$SDK --licenses
yes | sdkmanager --sdk_root=$SDK \
  "platform-tools" "platforms;android-35" "build-tools;35.0.0" \
  "emulator" "system-images;android-35;google_apis;arm64-v8a"
export ANDROID_HOME=$SDK ANDROID_SDK_ROOT=$SDK
```

For an emulator smoke (not required for a real device build):

```sh
export PATH="$PATH:$SDK/emulator:$SDK/platform-tools"
echo "no" | avdmanager create avd -n khala_test \
  -k "system-images;android-35;google_apis;arm64-v8a" -d "pixel_7" --force
emulator -avd khala_test -no-boot-anim -no-snapshot &
adb wait-for-device
```

## 2. Debug build (dev/QA — what this pass proved)

```sh
cd clients/khala-mobile
bun install
bun run prebuild:android          # expo prebuild --platform android (android/ is gitignored, regenerated each time)
bun run build:android:local       # ./android/gradlew -p android :app:assembleDebug
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

Debug builds load JS from Metro, not an embedded bundle — start Metro with
`bunx expo start --dev-client --host lan --port 8081` (see the "one big
gotcha" below) and `adb reverse tcp:8081 tcp:8081` if the emulator's
`10.0.2.2` NAT alias needs it.

**One big gotcha (cost real time this pass):** on macOS, `expo start
--dev-client --host localhost` can bind Metro to `[::1]:8081` (IPv6-only). The
Android emulator's `10.0.2.2` host alias is IPv4-only and cannot reach an
IPv6-only listener even with `adb reverse` in place — the app shows a real
"Unable to load script" / `Failed to connect to /10.0.2.2:8081` redbox.
**Use `--host lan`** (binds `*:8081`, dual-stack) instead of `--host
localhost` when serving an Android emulator/device from this kind of host.

## 3. Release build (signed AAB) — for Play internal testing

This section is written and internally consistent but **has not been run
end-to-end** in this pass, because it terminates in an owner decision (who
holds the upload keystore) and an owner Play Console action (uploading it).
Do not treat "written" as "proven" — see the status table above.

### 3.1 Generate an upload keystore (one-time; the key that signs what you upload to Play)

```sh
keytool -genkeypair -v \
  -keystore khala-mobile-upload.keystore \
  -alias khala-mobile-upload \
  -keyalg RSA -keysize 2048 -validity 10000
```

Treat the resulting `.keystore` file and its passwords as secrets — store
them the same way as other release-signing material
(`~/work/.secrets/`), **never commit them**, and never print the passwords
into tracked files, commits, or terminal output that becomes a receipt.

### 3.2 Wire the keystore into the Gradle release config

`expo prebuild` regenerates `android/` from scratch, so the signing config
must be supplied via a config plugin or `eas.json`-independent Gradle
properties **outside** the generated tree, e.g. a `~/.gradle/gradle.properties`
(or a repo-local, gitignored `android/keystore.properties` re-applied by a
small `withAndroidManifest`/Gradle-mod config plugin) — do not hand-edit the
generated `android/app/build.gradle` directly since it is regenerated on every
`prebuild`. This wiring is the next concrete implementation step once the
owner has decided where the keystore lives.

### 3.3 Bump `versionCode` (REQUIRED — every Play upload needs a strictly higher `versionCode`)

`android.versionCode` lives in `clients/khala-mobile/app.json` (currently `3`
as of this pass). Bump it there and commit before every release build headed
to Play, mirroring the iOS `CFBundleVersion` bump discipline in the TestFlight
runbook.

### 3.4 Build the release AAB (Play requires an Android App Bundle, not a raw APK, for new apps)

```sh
cd clients/khala-mobile
bun run prebuild:android
./android/gradlew -p android :app:bundleRelease
# -> android/app/build/outputs/bundle/release/app-release.aab
```

## 4. Play Console setup (owner-gated — see NEEDS_OWNER.md)

None of this is agent-runnable without real Play Console access:

1. Create the app record in Play Console (package `com.openagents.khala.mobile`).
2. Choose **Play App Signing** (Google-managed final signing key; you upload
   with your own **upload key** from step 3.1 — this is the modern
   recommended path and avoids the operator ever holding the final
   distribution key).
3. Complete the mandatory store listing minimums even for internal testing
   (short/full description, icon, at least one screenshot, content rating
   questionnaire, data-safety form, target audience/ads declarations) — Play
   blocks internal-testing rollout without these on a brand-new app record.
4. Create the **Internal testing** track, upload `app-release.aab`, add
   tester emails or a Google Group, and publish the internal release.
5. Testers install via the opt-in link Play Console generates — no public
   Play Store listing goes live at this stage.

## 5. What this runbook does not cover yet

- Push (FCM via Expo) and IAP (Play Billing via RevenueCat, though IAP itself
  is postponed for the first MVP build per the 2026-07-06 owner decision)
  verified specifically on a **real Android device** — the emulator smoke in
  this pass does not exercise real FCM delivery or Play Billing sandbox
  purchases. Track as a fast-follow once a real device and Play Console
  sandbox tester are available.
- The Gradle signing-config wiring in §3.2 is designed but unimplemented —
  it is the concrete next step once the owner decides keystore custody.
