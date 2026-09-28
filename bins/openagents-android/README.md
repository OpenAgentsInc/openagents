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
  the task's transcript. Later messages are durable `task.command`s that
  continue, queue, steer, or answer; a long press on send offers the other
  ways to send, as a menu.
  The chats list has a round **New chat** button (the compose glyph) that
  opens the New chat screen with the cursor in its field; an open chat
  has a breadcrumb back to the list beside its phase and computer; a
  waiting question makes the composer answer it, and an approval request
  adds **Approve** and **Deny**. Chats last shown stay listed across a
  relaunch. This is the iPhone app's Coder tab; Rust decides everything
  it shows.
- **Verse** (globe) mounts Verse's bare world (`coder_mobile::VerseHandle` in
  bare mode) in a `SurfaceView` and forwards touches, pinch, and rotation
  samples as Coder's `coder.verse.v1` requests. Rust draws the movement
  stick at the bottom left and, in touch look, the look stick at the bottom
  right. The pointers Rust takes for the sticks (`stick_pointer`,
  `look_stick_pointer`) stay out of pinch arbitration, so a pinch with two
  other fingers zooms while both thumbs keep walking and looking; the pinch
  scale comes from the two pinching fingers alone. The hand/gyroscope
  button, at the bottom center between them, switches touch and motion
  look, and the crosshair recenters the camera. The world draws with
  Verse's shared renderer on Vulkan or OpenGL ES; see
  [Graphics backends](#graphics-backends).
  The Grid's Gym has both native panels, as on iOS: a tap on the Gym board
  opens the **Gym** panel (runs, a run's metrics, recipes with confirmed
  starts, and the Gym connection, where you paste a `gym-connect:` code),
  and a tap on the RESULTS board opens **Results** (the boards, one board
  with its filters and caveats, one attempt, and the trace viewer with its
  timeline, play, step, and the Jev, Briefing, Agent, and Verifier tabs).
  Each panel hangs from a line to its board. TalkBack reads each row's
  `accessibility` text from Rust and offers **Open Gym board** and **Open
  results board** actions on the world when the player is in reach. The
  Gym connection code is kept encrypted under its own Keystore key; the
  verified results are cached in the app's cache directory.
- **Wallet** runs Breez's Spark SDK on Bitcoin mainnet, as the iPhone
  app's Wallet tab does, from the same Rust state: the balance and sync
  status in BIP 177 amounts (`₿12,345`, with **Show amounts as** switching
  every amount to legacy BTC and a one-time note explaining the change), a
  balance warning above ₿1,000,000, **Receive** (a Lightning
  invoice, the Spark address, or the Bitcoin deposit address, each with a
  QR code, Copy, and Share, and **Nostr** with your npub and a switch to
  publish your Spark address), **Send** (saved people, then paste or scan
  an invoice, Lightning address, LNURL, npub, Spark, or Bitcoin address;
  a Lightning address or LNURL asks for an amount and an optional comment
  within the recipient's terms; review the person, amount, fee, and a fee
  speed where one applies, then confirm; afterward save a new address as a
  contact), **Buy** (the provider's page opens in the browser), deposits
  that need attention (claim at a quoted fee, or refund on-chain to an
  address at a chosen speed after a review), history, **Agent payments**
  (the computers that may ask and their payments), **Recovery**, and the
  **Exit backup**, exported to a file you pick. An agent's payment request
  opens an approval sheet over any tab: the amount and fee, who asked and
  why, the payee read from the invoice, and the computer's remaining
  budget; above Rust's threshold, **Approve** asks for the screen lock
  first. Nothing pays without the tap. The info button
  opens the trust note, and closing it acknowledges it. **Show recovery
  words** asks first, then shows the words from Rust's direct reply;
  **Restore** takes 12 or 24 words, which Rust checks, and asks again when
  the wallet holds bitcoin.
- **Account** holds **Trainer** (the trainer card for the Verse world key:
  level, XP and the way to the next level under `trainer-curve-v1`,
  titles, the counted awards with links, and the trainer key with a
  warned **Reveal nsec**; the level also shows over each player's head in
  the Grid), **Computers** (a native list of your computers with a
  status dot and short status; a tap opens a computer's shared screens, a
  long press offers its menu, and a destructive choice asks first; the
  header's **More** menu has Activity, Refresh, and the owner-directory
  controls, and **+** adds a computer, with its input requests, QR
  scanning, secret fields, and the invitation QR code), **Tailnet**
  (Tailscale sign-in with `tailscale-rs`; **Sign in with Tailscale** opens
  the sign-in page in the browser and then waits for approval),
  **Identity keys** (the npub, the hex key, and the nsec only after
  **Reveal nsec** and a warning), **About this device** (the device's npub
  and hex key, where the key comes from, and the app version),
  **Changelog**, and links to the source code and to OpenAgents on X. The
  Coder tab reads your computers' chats, as on iOS; Account no longer
  lists them.

Tailnet admission works as on iOS: after you sign in on the Tailnet screen,
the app asks each device on the tailnet for an invitation, and a computer
running `coder host serve --tailnet-admission standard` adds itself and its
chats. The phone must be on the tailnet through the Tailscale app to reach
the computer.

The transcript is painted from Rust's layout, as on iOS
([`TranscriptPainter.kt`](host/app/src/main/java/com/openagents/app/TranscriptPainter.kt);
see the transcript layout section of
[the Rust Native spec](../../crates/rust-native/docs/spec.md#transcript-layout)):

- Rust holds each chat's rows in a transcript source, so they never cross
  the view, and lays them out on a worker thread: it shapes text with the
  bundled Inter and JetBrains Mono fonts, decides every row's exact height,
  and returns display lists with each text run's position. The app draws the
  runs with the same fonts at those positions.
- The list is a `RecyclerView` whose rows take Rust's heights, so it follows
  new rows while you are at the bottom, stops when you scroll up, shows a
  jump-to-bottom button, and keeps your place when rows arrive above. Wide
  code blocks and tables scroll sideways. **Load earlier** is its first row.
- Long-press a row for **Copy** and **Select Text**, as on iOS. **Select
  Text** selects the painted text in place
  ([`SelectionLayer.kt`](host/app/src/main/java/com/openagents/app/SelectionLayer.kt)):
  a highlight under the text, two handles to drag, and the system's floating
  toolbar with **Copy** and **Select All**. Carets sit at Rust's run
  positions, with stops from the bundled font's advances scaled to each run's
  width ([`TextSelection.kt`](host/app/src/main/java/com/openagents/app/TextSelection.kt)),
  so a caret never lands inside a surrogate pair or a ligature. A selection
  stays in one row, like iOS's; a tap elsewhere or a new version of the row
  ends it, and Copy copies and ends it, as Android text does.
- Debug launch extras: `--ez rust_native_fixture true` shows the sample
  conversation, `--ei rust_native_fixture_rows N` adds N rows,
  `--ez rust_native_transcript_pull true` sends the rows through a transcript
  source, `--ez rust_native_transcript_bench true` flings through the
  list and logs frame times under `TranscriptBench`, and
  `--es rust_native_transcript_select KEY` selects that row's text.
- `scripts/build-openagents-android.sh bench` builds release Rust and a
  separate, non-debuggable app with these extras,
  `com.openagents.app.bench` ("OpenAgents Bench", signed with the debug key),
  and installs it on `OPENAGENTS_ANDROID_SERIAL`. It never replaces the
  installed app or its data. The script's usage text shows how to launch the
  benchmark and remove the app.

Messages, Markdown, tool rows, and the composer outside a transcript follow
the iOS design in [`NativeChat.swift`](../coder-ios/host/App/NativeChat.swift):

- A user message is a trailing bubble, an assistant message is full width,
  and a system message is a quiet centered row. Long-press a message to copy
  its text.
- Markdown is drawn from the blocks Rust parsed, with Android text spans:
  headings, paragraphs with bold, italic, strikethrough, inline code, and
  links (styled, never opened), lists with task states, code blocks with a
  **Copy** control, quotes, tables that scroll sideways, and rules.
- A tool row expands to its output; the expansion survives new revisions.
- Buttons with a glyph (back, compose) draw it: alone in a 44 dp circle
  with the label as its spoken name, or before the label as a back link.
  An end-aligned glyph button sits at the end of its row.
- The composer is one capsule with the send control inside its trailing
  end. It grows to six lines, enforces the byte bound, and sends a
  `coder_input` answer bound to its token; a composer with `focus` takes
  the cursor once per token. While Coder works it becomes a
  stop control that activates the composer node.

The host polls as the iPhone app does: the Computers surface every 3 seconds
while the Coder tab or the Computers screen shows (not while a value is being
entered), and a snapshot every second while the Tailnet screen is
loading. The terminal opens full screen when Rust reports one; it sizes the
grid from the monospace cell, polls every 120 ms, and forwards typed text,
Backspace, Enter, and hardware keys with their modifiers.

## Security and storage

- The device's Nostr key is 32 random bytes, encrypted with AES-GCM under an
  Android Keystore key and stored in `noBackupFilesDir`. It goes to Rust only
  as the `secret_hex` field of the app configuration and is never logged.
- The Spark wallet's seed (16 bytes of BIP39 entropy, or the 16 or 32 bytes
  a restore saved) is encrypted the same way under its own Keystore key.
  It reaches Rust only in `wallet_open`. The recovery words arrive only in
  Rust's direct reply to `wallet_words`, never in the app packet, and exist
  only while their dialog is open. **Copy words** marks the clip sensitive
  and clears it after a minute; the words dialog and the restore dialog
  block screenshots and screen recording (`FLAG_SECURE`).
- The Verse world key and the Gym connection code each have their own
  Keystore key too.
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
# Levels from the labeled tutorial fixture, offline (trainer card and Grid tags).
adb shell am start -n com.openagents.app/.MainActivity --es account_route trainer --ez xp_preview true
# Coder's offline Computers fixture: sample computers, no host or relay.
adb shell am start -n com.openagents.app/.MainActivity --es account_route computers --ez computers_fixture true
adb shell am start -n com.openagents.app/.MainActivity --ez rust_native_fixture true
# An offline fixture wallet with no money (a deposit needing attention, a contact).
adb shell am start -n com.openagents.app/.MainActivity --es tab wallet --ez wallet_fixture true
# Let captures include the recovery words and restore dialogs (debug only).
adb shell am start -n com.openagents.app/.MainActivity --es tab wallet --ez allow_secret_captures true
# The labeled synthetic Gym board, and a scripted walk up to it (steps as
# in the iOS --verse-script: walk, right, turn, look, walkpinch, board,
# results, r=do:value, wait).
adb shell am start -n com.openagents.app/.MainActivity --es tab verse \
  --ez gym_preview true --es verse_script walk,walk,walk,board
```

## Release

| Setting | Value |
| --- | --- |
| Application ID | `com.openagents.app` |
| Version name and code | The iPhone app's: `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` in [`project.yml`](../openagents-ios/host/project.yml), `1.0.0` / `16` for the 2026-09-29 playtest APK (`OPENAGENTS_ANDROID_VERSION_CODE` overrides the code) |
| Minimum and target API | 26 / 35 |
| ABI in a release APK or bundle | `arm64-v8a` (`OPENAGENTS_ANDROID_ABI=x86_64` for an x86_64 APK) |
| Release signing key | alias `openagents`, certificate SHA-256 `DB:D0:E9:65:5A:7A:0D:E2:E7:A4:F6:D4:59:F9:AF:52:A8:54:D8:CB:C1:FC:3B:B0:62:7E:2B:C2:AD:44:44:80` |

The Android version code is the iPhone build number of the same commit, so
**Account > About this device** and the shared **Changelog** (whose newest
entry is that build, `crates/openagents-mobile/src/account.rs`) name the
same build on both phones, and a playtest report names the platform and
that pair. Build an APK from a commit whose iPhone build number is higher
than the last APK's; Android installs an update over an older one only when
the code is higher and the signing key is the same. Gradle alone defaults
to `1.0.0` / `1`.

### Direct-download APK

The playtest APK is a signed release build, downloaded from a GitHub
release and installed by hand:

```sh
OPENAGENTS_ANDROID_SIGNING_ENV=/Users/christopherdavid/work/.secrets/openagents-android-release.env \
  bins/openagents-android/build.sh release

# Install it on a device or emulator (an existing debug install has another
# signing key; uninstall that first, which erases its data).
OPENAGENTS_ANDROID_SERIAL=emulator-5600 bins/openagents-android/build.sh install-release
```

`release` builds the Rust library with the release profile, then
`assembleRelease` with R8 shrinking and resource shrinking. Release builds
have `BuildConfig.DEBUG` false, so the debug launch extras above (fixtures,
previews, scripted walks, secret captures) are compiled out, and the debug
sample conversation isn't packaged. The script refuses to build an unsigned
release, checks 16 KiB alignment, verifies the signature with `apksigner`,
and writes `release/OpenAgents-<version>-<code>-<abi>.apk`, its `.sha256`, and
the R8 `-mapping.txt` (keep it to read release stack traces) under the output
directory.

The release key is the keystore
`/Users/christopherdavid/work/.secrets/openagents-android-release.jks`
(PKCS12, alias `openagents`, created 2026-09-28, valid to 2054). Its
passwords are in `openagents-android-release.env` beside it, which the
script reads through `OPENAGENTS_ANDROID_SIGNING_ENV` and passes to Gradle
in `ORG_GRADLE_PROJECT_` variables, never on a command line. Both files are
outside Git (the workspace ignores `.secrets/`) and must never be committed
or printed. Losing the key means testers must uninstall, losing the app's
data, before a build signed with a new key installs; the owner keeps an
offline backup (workspace `NEEDS_OWNER.md`).

### Google Play

Google Play takes an Android App Bundle signed with the upload key:

```sh
OPENAGENTS_ANDROID_SIGNING_ENV=/Users/christopherdavid/work/.secrets/openagents-android-release.env \
  scripts/build-openagents-android.sh bundle
```

The bundle is `gradle/app/outputs/bundle/release/app-release.aab` under the
output directory. Upload it in Play Console under **Test and release >
Testing > Internal testing > Create new release**. So that people who
installed the APK can update from Play, choose Play App Signing with **Use
the key from a Java keystore** and upload this release key as the app
signing key; letting Google generate one would make the Play build a
different app to Android. The script never uploads, and never commits or
prints a key; Git ignores `*.jks` and `*.keystore` files here. Creating the
Play Console app and the testers list are owner steps.

## Graphics backends

The Verse tab uses the shared wgpu renderer. A device tries Vulkan first and
falls back to OpenGL ES 3.0 when Vulkan has no adapter or device for the
window. The emulator uses OpenGL ES only, because enumerating Vulkan can stall
in emulator drivers. To force one backend, set a system property and reopen
the Verse tab:

```sh
adb shell setprop debug.verse.backend vulkan   # or gl
adb shell setprop debug.verse.backend "''"     # restore the default
```

On OpenGL ES, the renderer draws the same passes with these differences:

- Sun and studio-light shadows keep their soft 16-tap filter, but the
  penumbra width is fixed instead of following each occluder's distance.
  GLSL ES can't read a depth texture that is also sampled with comparison.
- Screen-space line coverage and the Sun, Earth, and Moon discs interpolate
  without the `noperspective` qualifier, with an exact substitute.
- Frames are drawn into an sRGB texture and encoded into a linear surface in
  one extra full-screen pass, because the emulator's EGL ignores an sRGB
  window colorspace.

The Metal, Vulkan, and desktop output doesn't change. See
[`docs/verse/README.md`](../../docs/verse/README.md#graphics-backends).

## Verification

On 2026-09-28, the debug APK ran on the `coder_mobile_api35` emulator
(Pixel 7 profile, Android 15, ARM64, SwiftShader):

- Coder: the empty state, and the sample conversation from the fixture
  extra with a user bubble, an expanded tool row, a bulleted list, a code
  block, a table, a system row, the working row, and the composer in its
  stop state.
- Account: the screen list; Computers with its tabs, **Add a computer**, a
  paste input request, and Rust's refusal of a malformed invitation;
  Tailnet, where `tailscale-rs` reached Tailscale's control server
  from the emulator, returned a sign-in URL, and **Sign in with Tailscale**
  opened it in Chrome; About this device with the key and `1.0.0 (1)`.
  Later on 2026-09-28: the new Account list, Identity keys with the reveal
  warning and the nsec, Changelog, About this device with the npub, and
  the native Computers list on Coder's offline fixture (rows, a row's
  menu, a computer's screen, More, and Add). Captures are in
  `verification/2026-09-28-account`.
- Wallet (2026-09-28, on mainnet): a fresh wallet opened and synced,
  showed its Spark address and a new Lightning invoice as QR codes, the
  trust note, Send, the recovery words after the warning, Rust's refusal
  of three words, and a restore from the public BIP39 test words, which
  replaced the wallet and read its history. Captures are in
  `verification/2026-09-28-wallet` (the words themselves are left out).
- Verse, after the OpenGL ES renderer fix (#9838): the bare world's grid,
  the player, the lit ball with its shadow and light pool, and two remote
  avatars on the horizon. Walking into the ball rolled it. The same frame
  rendered with `debug.verse.backend` set to `vulkan`. `adb logcat` showed no
  wgpu errors on either backend.

Release APK (2026-09-28): `OpenAgents-1.0.0-16-arm64-v8a.apk` (69 MB; the
stripped Rust library is 68 MB of it, stored uncompressed and page-aligned),
signed with the release key and built by `release`, installed on a fresh
`coder_mobile_api35` instance (not debuggable, `1.0.0 (16)`). It opened on
Coder's empty state; Verse drew the Grid with the Gym, the ball, and the
sticks; Wallet created and synced a new mainnet wallet at ₿0 (no funds
moved) and opened it again after a force stop and relaunch; Account showed
Trainer, Computers, Tailnet, About this device, and the Changelog headed
`1.0.0 (16)`. `logcat` showed no
crash, and the release dex has none of the debug launch extras. Captures
are in `verification/2026-09-28-release`.

- Transcript selection (later on 2026-09-28): a long press offered **Copy**
  and **Select Text**; **Select Text** selected the user message in place
  with both handles and the floating **Copy** and **Select all** toolbar; an
  `adb` swipe dragged the end handle back to "Fix the flak", and **Copy**
  put exactly that on the clipboard (pasted into the composer) and ended the
  selection. With `rust_native_transcript_select`, a reply's selection
  covered its paragraphs, list, code block, and table, and the handles
  stayed inside a wide code block while it scrolled sideways and followed
  the list when it scrolled. Captures are in
  `verification/2026-09-28-transcript-selection`.

Rust: `cargo test`, `cargo clippy`, and `cargo fmt --check` for
`openagents-mobile` on the host; `cargo ndk clippy` for `aarch64-linux-android`
and `cargo ndk check` for `x86_64-linux-android`; Android lint and unit tests
pass. A live host (tailnet admission, a Coder chat, the terminal) wasn't
checked: the emulator isn't on a tailnet.

## Known gaps

- Vulkan on a physical device hasn't been checked; the emulator's Vulkan
  rendered the Verse tab correctly.
- The terminal screen, QR scanning, and motion look are built but haven't
  been exercised on an emulator or a device.
- Text with the Markdown role outside the conversation elements shows as
  plain text; iOS styles its inline Markdown.
- The transcript's streaming fade isn't built on Android (#9833).
- No Android device numbers for the transcript yet; use the bench build.
- Two-thumb sticks with a pinch from other fingers are covered by unit
  tests (`PinchAdmissionTest`) but not by an emulator run, since `adb`
  can't inject multi-touch.
- The agent payment approval sheet, the Coder queue panel, and long-press
  send choices are built from the iOS design but haven't been exercised
  on an emulator: neither the fixture wallet nor Coder's offline fixture
  produces a payment request or a writable running chat.
- **Report a problem**, **Playtest session**, and **My reports** (iOS,
  `74f2f90be0`) aren't on Android yet, although the shared Changelog's
  `1.0.0 (16)` entry lists them. Android testers report through the GitHub
  **Playtest report** template or the playtest email.
- No instrumentation tests yet; checks are Rust tests, lint, unit tests,
  and the emulator runs above.
