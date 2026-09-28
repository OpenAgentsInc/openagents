# OpenAgents for Android

OpenAgents for Android is the Android build of the
[OpenAgents iPhone app](../openagents-ios/README.md). Both apps run the same
Rust library, [`openagents-mobile`](../../crates/openagents-mobile), and
exchange the same JSON requests and packets with it. Rust builds every screen
as a Rust Native view; the Android host in `host/` is thin Kotlin that
renders those views with Android widgets and supplies what a view can't:
tabs, navigation, the camera, keyboards, the Keystore, and the Verse
`SurfaceView`. It follows the pattern of [Coder for Android](../coder-android/README.md).

## What the app has

The app has four tabs, shown as white icons on black:

- **Coder** (`</>`) chats with Coder on your computers. Sending a message
  starts a NIP-HOST `task.create` on the chosen computer, and the chat follows
  the task's transcript; while Coder works, the send control stops the task.
  This is the iPhone app's Coder tab; Rust decides everything it shows.
- **Verse** (globe) mounts Verse's bare world (`coder_mobile::VerseHandle` in
  bare mode) in a `SurfaceView` and forwards touches, pinch, and rotation
  samples as Coder's `coder.verse.v1` requests. The hand/gyroscope button
  switches touch and motion look, and the crosshair recenters the camera.
  See [Known gaps](#known-gaps): the shared renderer doesn't start on
  Android's GLES backend yet.
- **Wallet** is a placeholder that says **Coming soon.**
- **Account** holds **Computers** (Coder's shared Computers screens, with
  their input requests, QR scanning, secret fields, and the invitation QR
  code), **Chats on your computers**, **Tailnet** (Tailscale sign-in with
  `tailscale-rs`; **Sign in with Tailscale** opens the sign-in page in the
  browser and then waits for approval), and **About this device** (the
  device's public key and the app version).

Tailnet admission works as on iOS: after you sign in on the Tailnet screen,
the app asks each device on the tailnet for an invitation, and a computer
running `coder host serve --tailnet-admission standard` adds itself and its
chats. The phone must be on the tailnet through the Tailscale app to reach
the computer.

The conversation elements (transcript, message, Markdown, tool, working, and
composer) follow the iOS design in
[`NativeChat.swift`](../coder-ios/host/App/NativeChat.swift):

- The transcript is a bottom-anchored `RecyclerView` that rebinds only rows
  whose content changed. It follows new rows while you are at the bottom,
  stops when you scroll up, and then shows a jump-to-bottom button. **Load
  earlier** is its first row.
- A user message is a trailing bubble, an assistant message is full width,
  and a system message is a quiet centered row. Long-press a message to copy
  its text.
- Markdown is drawn from the blocks Rust parsed, with Android text spans:
  headings, paragraphs with bold, italic, strikethrough, inline code, and
  links (styled, never opened), lists with task states, code blocks with a
  **Copy** control, quotes, tables that scroll sideways, and rules.
- A tool row expands to its output; the expansion survives new revisions.
- The composer grows to six lines, enforces the byte bound, and sends a
  `coder_input` answer bound to its token. While Coder works it becomes a
  stop control that activates the composer node.

The host polls as the iPhone app does: the Computers surface every 3 seconds
while the Coder tab or the Computers screen shows (not while a value is being
entered), and a snapshot every second while the Chats or Tailnet screen is
loading. The terminal opens full screen when Rust reports one; it sizes the
grid from the monospace cell, polls every 120 ms, and forwards typed text,
Backspace, Enter, and hardware keys with their modifiers.

## Security and storage

- The device's Nostr key is 32 random bytes, encrypted with AES-GCM under an
  Android Keystore key and stored in `noBackupFilesDir`. It goes to Rust only
  as the `secret_hex` field of the app configuration and is never logged.
- Rust keeps grants, pairings, and the Tailscale node keys in its own
  encrypted stores under `noBackupFilesDir/openagents-v1`.
- Backup and device transfer exclude all app data, and cleartext traffic is
  off.
- Camera access is requested only when you start a scan. Frames stay on the
  device, and a scanned value is bounded text that Rust validates.
- Secret input fields are masked and have suggestions, autofill, and
  personalized learning turned off; they clear after each send.

## The native bridge

`crates/openagents-mobile/src/android.rs` is the JNI surface beside the C
ABI, exported to `com.openagents.app.OpenAgentsNative`:

| Kotlin | C ABI equivalent | Thread |
| --- | --- | --- |
| `create(config)`, `call(handle, request)`, `destroy(handle)` | `openagents_mobile_create`, `_call`, `_destroy` | One background worker |
| `verseCreate(surface, config)`, `verseCall`, `verseDestroy` | `openagents_verse_create`, `_call`, `_destroy` | Main thread |
| `verseAttach(handle, surface, config)`, `verseDetach(handle)` | None; Android can lose its window while keeping the world | Main thread |

Handles are counters, never pointers, and are refused on the wrong thread.
Each entry point bounds its input as the C ABI does (16 KiB configuration,
128 KiB request, 4 KiB world request) and turns a Rust error or panic into a
`RuntimeException` that names the failure; panics are also written to the
Android log under the `OpenAgents` tag. Rust tests in `android.rs` cover the
shared request path, the bounds, and panic handling on the host.

The library also contains Coder's `coder-mobile` exports, because the Verse
tab links that crate. They are bound to Coder's Java class and are never
called here.

## Build and run

Prerequisites:

- The repository's pinned Rust toolchain with `aarch64-linux-android` (and
  `x86_64-linux-android` for x86 emulators), and `cargo-ndk`.
- Android SDK platform 35, build tools 35.0.0, NDK `27.1.12297006`, platform
  tools, and JDK 17. On this Mac, the build script finds the Homebrew SDK and
  JDK 17; elsewhere, set `ANDROID_HOME`, `ANDROID_NDK_HOME`, and `JAVA_HOME`.

```sh
rustup target add aarch64-linux-android

# Build the Rust library and a debug APK.
scripts/build-openagents-android.sh package

# Install and open it on a running emulator or device.
adb devices -l
OPENAGENTS_ANDROID_SERIAL=emulator-5554 scripts/build-openagents-android.sh run

# Android lint and unit tests.
scripts/build-openagents-android.sh check
```

`bins/openagents-android/build.sh` is the same entry point. `rust` builds
only the library, `apk` packages an already built library, `install` updates
the app without clearing its data, and `launch` opens it.
`OPENAGENTS_ANDROID_ABI=x86_64` builds for an x86_64 emulator. Build outputs
go to `$CARGO_TARGET_DIR/openagents-android` (by default
`../target/openagents-android`); the debug APK is
`gradle/app/outputs/apk/debug/app-debug.apk` there. The script checks the
APK's 16 KiB alignment with `zipalign -c -P 16`, and links the Rust library
with 16 KiB pages.

Debug builds accept launch extras that open a tab or an Account screen, and
one that shows Rust Native's sample conversation
(`crates/rust-native/fixtures/conversation.json`) on the Coder tab:

```sh
adb shell am start -n com.openagents.app/.MainActivity --es tab verse
adb shell am start -n com.openagents.app/.MainActivity --es account_route tailnet
adb shell am start -n com.openagents.app/.MainActivity --ez rust_native_fixture true
```

## Release

| Setting | Value |
| --- | --- |
| Application ID | `com.openagents.app` |
| Version name and code | `1.0.0` / `1` (`OPENAGENTS_ANDROID_VERSION_CODE` overrides the code) |
| Minimum and target API | 26 / 35 |
| ABI in a release bundle | `arm64-v8a` |

Google Play takes an Android App Bundle signed with the upload key. With the
owner's upload keystore outside the checkout:

```sh
OPENAGENTS_ANDROID_KEYSTORE=/path/to/upload-keystore.jks \
OPENAGENTS_ANDROID_KEY_ALIAS=upload \
OPENAGENTS_ANDROID_KEYSTORE_PASSWORD=... \
OPENAGENTS_ANDROID_KEY_PASSWORD=... \
OPENAGENTS_ANDROID_VERSION_CODE=1 \
  scripts/build-openagents-android.sh bundle
```

The bundle is `gradle/app/outputs/bundle/release/app-release.aab` under the
output directory. Upload it in Play Console under **Test and release >
Testing > Internal testing > Create new release**. Raise the version code for
every upload. The script never uploads, and never commits or prints a key;
Git ignores `*.jks` and `*.keystore` files here. Creating the Play Console
app, the upload key, and the testers list are owner steps.

## Verification

On 2026-09-28, the debug APK ran on the `coder_mobile_api35` emulator
(Pixel 7 profile, Android 15, ARM64, SwiftShader):

- Coder: the empty state, and the sample conversation from the fixture
  extra with a user bubble, an expanded tool row, a bulleted list, a code
  block, a table, a system row, the working row, and the composer in its
  stop state.
- Account: the screen list; Computers with its tabs, **Add a computer**, a
  paste input request, and Rust's refusal of a malformed invitation;
  Chats; Tailnet, where `tailscale-rs` reached Tailscale's control server
  from the emulator, returned a sign-in URL, and **Sign in with Tailscale**
  opened it in Chrome; About this device with the key and `1.0.0 (1)`.
- Wallet: the placeholder.
- Verse: the host mounted the surface and created the world, and the error
  shown is the renderer's (see below).

Rust: `cargo test`, `cargo clippy`, and `cargo fmt --check` for
`openagents-mobile` on the host; `cargo ndk clippy` for `aarch64-linux-android`
and `cargo ndk check` for `x86_64-linux-android`; Android lint and unit tests
pass. A live host (tailnet admission, a Coder chat, the terminal) wasn't
checked: the emulator isn't on a tailnet.

## Known gaps

- **The Verse world doesn't render on Android yet.** The shared photographic
  renderer (`crates/verse/src/pbr`) fails to build its pipelines on wgpu's
  GLES backend, which Android uses: `photo.wgsl` reads the shadow depth
  texture with `textureLoad`, and a vertex output uses a `noperspective`
  qualifier, neither of which GLES supports. Coder for Android uses the same
  renderer. The host shows the renderer's error with **Retry**; the fix
  belongs in the Verse renderer.
- The terminal screen, QR scanning, and motion look are built but haven't
  been exercised on an emulator or a device.
- Text with the Markdown role outside the conversation elements shows as
  plain text; iOS styles its inline Markdown.
- No instrumentation tests yet; checks are Rust tests, lint, and the
  emulator run above.
