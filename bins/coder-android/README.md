# Coder for Android

Coder opens into the shared Rust Verse world. Walk toward the computer to pair
with your computer and read its retained Codex and Claude chats. Walk into the
Gym to observe its separately granted Microcoder and Terminal-Bench boards.
Android uses the same application state, Nostr protocol, encrypted reader cache,
world, controller, and renderer as the iOS app.

The Android framework host is thin Kotlin: native widgets, a `SurfaceView`,
Keystore storage, camera scanning, device motion, and lifecycle callbacks. The
Rust Native tree supplies the reader's text, lists, buttons, and revision-bound
intents. Application state and permission checks remain in `coder-mobile`,
`coder-connect`, and Verse. Rust Native itself contains no Coder theme, identity,
or network implementation.

## Source and app identity

This adapter reimplements the native Android setup inspected in the authorized
local Coder checkout at `f2d85b120ac96b4a4be9116e823e7795472088c1`. It preserves
`com.openagents.coder`, Android API 26 as the minimum, API 35 as the target,
and marketing version `0.5.0`. The public replacement has version code `5`.
The computer prompt is rendered on the shared 3D monitor; approach it and tap
its screen. See the [world-computer verification](../../docs/coder/verification/2026-09-26-world-computer.md).
The old private GPUI runtime, sign-in, service endpoints, credentials, and
backend are not imported. Android parity means the current iOS mobile feature
slice, not every feature in the historical private app or desktop Verse.

## Build and run

Prerequisites:

- The repository's pinned Rust toolchain and `cargo-ndk`.
- `aarch64-linux-android` for ARM64 or `x86_64-linux-android` for x86 emulators.
- Android SDK platform 35, NDK `27.1.12297006`, platform tools, and JDK 17.
- An explicitly selected running emulator or connected device.

The checked-in Gradle wrapper pins its distribution and checksum. On this Mac,
the helper discovers the Homebrew Android SDK and JDK 17. Elsewhere, set
`ANDROID_HOME`, `ANDROID_NDK_HOME`, and `JAVA_HOME` to installed toolchains.

```sh
rustup target add aarch64-linux-android
scripts/build-coder-android.sh package
adb devices -l

# Replace this serial with the intended emulator or device.
CODER_ANDROID_SERIAL=emulator-5554 scripts/build-coder-android.sh run --synthetic

# Open the normal app with its separate identity and saved pairing.
CODER_ANDROID_SERIAL=emulator-5554 scripts/build-coder-android.sh launch

scripts/build-coder-android.sh check
CODER_ANDROID_SERIAL=emulator-5554 scripts/build-coder-android.sh test
```

`bins/coder-android/build.sh` is an equivalent entry point. `rust` builds the
shared library; `apk` packages an already built library; `install` updates the
app without erasing its data. `CODER_ANDROID_ABI=x86_64` selects that target.
`CARGO_TARGET_DIR` and `CODER_ANDROID_OUTPUT` override the per-worktree output
paths. The default APK is outside the checkout at
`../target/coder-android/gradle/app/outputs/apk/debug/app-debug.apk`.

The helper excludes provider credentials from compiler environments. It does
not create or erase an emulator, clear app data, sign a store release, or
publish an app. A signing-key mismatch with an older installed Coder fails
visibly; it does not trigger an automatic uninstall. The debug APK is for local
installation, not a Google Play release.

## Push wakes

Push is off by default. Without `host/app/google-services.json`, Gradle
doesn't load the Google services plugin or Firebase Messaging, compiles a
stub from `src/nopush`, and disables the manifest's messaging service. The
build and tests work exactly as before. The manifest declares
`POST_NOTIFICATIONS`, but a default build never requests it.

To turn push on:

1. In the Firebase console, under **Project settings** > **General**, add an
   Android app with package `com.openagents.coder`, then download
   `google-services.json`. Put it at
   `bins/coder-android/host/app/google-services.json`. Git ignores that path;
   don't commit the file.
2. Build with the push settings. The helper passes them to Gradle as the
   `coderPushRelayUrl`, `coderPushGatewayUrl`, and `coderPushAppProfile`
   properties:

   ```sh
   CODER_PUSH_RELAY_URL=wss://relay.example.com \
   CODER_PUSH_GATEWAY_URL=https://push.example.com \
   CODER_PUSH_APP_PROFILE=coder-android \
   scripts/build-coder-android.sh package
   ```

With both, Gradle applies the Google services plugin, adds Firebase
Messaging, and compiles `src/push`. The app requests `POST_NOTIFICATIONS` on
Android 13 and later, fetches the FCM token at every launch, and passes it to
Rust's `push_token`. `onNewToken` passes a rotated token the same way. Tokens
stay in memory. A failure shows as the wake status in the Computer panel.
The app profile must match the gateway's `PUSH_GATEWAY_FCM_APP_PROFILE`; see
the [push gateway runbook](../../docs/deployment/push-gateway.md).

`pushConfiguredBuildReportsItsWakeStatus` runs only in a push-configured build
and is skipped otherwise.

## Use the app

- The canvas paints behind the system bars; native controls respect their
  insets. Drag the left half to move and the right half to look. Double-tap
  the world to jump. Pinch with two fingers to zoom: spread them to move the
  camera closer and bring them together to move it farther away. Two fingers
  reserve zoom immediately; movement and touch look resume after all fingers
  lift and you start a new gesture.
- Select **Motion look** to aim by rotating the device while holding the left
  side to move. **Recenter** resets its reference. Sensors stop when the app
  is inactive or a world panel is open. A device without a usable rotation
  sensor keeps touch controls available.
- Approach the world computer, select **Use computer**, and follow the
  [pairing guide](../../docs/coder/guides/mobile-readonly.md). Scan the QR code
  from `cargo run --release -p coder-connect -- connect`, or paste the full
  invitation. Camera access is requested only for scanning.
- In the computer panel, **Computers** lists the hosts this phone enrolled
  with a `coder-host:` invitation from `coder host invite`, each with its
  live status. The **Access** screen lists enrolled devices and creates
  invitations with narrowed rights, shown as a QR code. See
  [Coder Computers](../../crates/coder-computers/README.md) and the
  [live verification record](../../docs/coder/verification/2026-09-26-computers-live.md),
  which also describes the debug-only `loopback_test` extra for a host on
  the build computer.
- The read-only reader supports bounded catalog/transcript pages, follow/pause,
  original record bytes, and selectable text. Viewing a transcript cannot
  execute its contents or submit a message to an external harness.
- Enter the Gym before opening its boards. Its connection and execution grant
  are separate from chat pairing. Recipe actions require explicit confirmation;
  viewing the world or a board starts no benchmark or model.

The world keeps the camera mode control, **Recenter** in motion mode, nearby
interactions, and actionable errors. The title, connection status, idle labels,
diagnostics, instructions, walk/sprint toggle, and jump/zoom buttons are removed
from the canvas. Synthetic test metadata remains available through accessibility.

Reader and world identities use separate Keystore-protected storage. Synthetic
acceptance uses separate identities, files, and offline fixtures. Normal
connections still verify the original grants, recipients, expiry, and source
cursors in Rust. Closing a panel or backgrounding pauses observation; it does
not imply cancellation of a separately admitted host task.

The [Android verification record](../../docs/coder/verification/2026-09-26-android-mobile.md)
records the actual toolchain, checks, screenshots, and limitations. The later
HUD cleanup and double-tap/pinch changes passed main and instrumentation Kotlin
compilation; their updated Android interaction tests have not been run on an
emulator or physical device. Physical camera and sensor behavior, TalkBack,
additional OS and device versions, and store release acceptance require separate
evidence. Desktop-only agent chat, XP, and replay UI remain outside both current
mobile surfaces.
