# Android version of the current mobile app

The [October 18 parity sprint](2026-10-18-android-parity-sprint.md) turns this
assessment into tracked work and updates the identity dependencies, CoderOS
build plan, and full-parity acceptance. The branch statuses and owner
recommendations below describe the October 17 assessment.

Status: assessment, 2026-10-17, against `origin/main` at `c5ced8869f` and the
open branches for the Nostr identity epic (#12006). Owner request: "audit how
much needs doing to do android version of current mobile app".

## Summary

The current phone app is `apps/openagents/apps/ios`: a UIKit shell of about
15,600 lines of Swift over the Rust core in `apps/openagents/crates/mobile`
(about 19,500 lines of Rust). The core is already a UniFFI library built for
Swift and Kotlin, and its decisions (layout, markdown, sync, account, cloud
chat, push registration, voice signaling, onboarding) don't depend on the
platform. No Android shell for this app exists yet. Kotlin bindings have never
been built and run on a device.

Android needs a Kotlin shell of roughly 9,000 to 12,000 lines, a few
platform-store hooks in Rust, a Gradle project, and a release path. About a
third of the shell can start from Kotlin that the repository already had: the
retired OpenAgents Android app (about 12,000 lines, removed in c63b8bbe5c) and
the in-tree Coder Android host (`bins/coder-android`, about 2,150 lines).

**Estimate: 29 to 40 agent-days** for an Android build on Play internal testing
at parity with iOS, including today's identity, wallet, and QR work. With three
agents in parallel after the first two phases, that is about three calendar
weeks. Voice calls are the largest part that could move to a second release;
without voice the total is 25 to 35 agent-days.

The October 16 merge audit
([`2026-10-16-coder-desktop-monorepo-merge.md`](2026-10-16-coder-desktop-monorepo-merge.md),
step 7) put Android at 2 to 4 days. That figure is too low for the app as it is
today: the iOS shell has grown since, and the identity epic adds onboarding,
backup, a wallet, and a scanner that each need native Android code.

## What exists

### The Rust core

`apps/openagents/crates/mobile` (`openagents-mobile`, UniFFI namespace
`openagents_core`) builds as `lib`, `staticlib`, and `cdylib`, so the same
crate gives iOS a static library and Android a `.so`. It exports about 230
UniFFI items. The parts the Android shell uses as is:

| Area | Files | What Android gets |
| --- | --- | --- |
| Transcript layout | `src/layout/` | `LayoutFrame` with exact row heights and offsets, `rowsIn`, and per-row display lists (text runs at positions, boxes, links, scrollers, widgets). The shell paints; it never measures. |
| Text measurement | `src/layout/mod.rs` (`PlatformMeasurer`) | Rust measures the bundled Geist faces itself; the platform answers only for fallback glyphs. Android implements this trait. |
| Agent visuals | `src/visual/` | A worker per `show_visual` script that rasterizes canvases, charts, and wgpu scenes; the shell shows the pixels and forwards touches. |
| Account, sessions, saved chats | `src/client_ffi/account.rs`, `session.rs`, `saved.rs`, `chat_homes.rs`, `sharing.rs`, `custom_agents.rs`, `agent_lab.rs`, `labs.rs` | All state, paging, and validation. |
| Cloud chat and own computer card | `src/client_ffi/cloud_chat.rs`, `cloud_host.rs`, `own_computer.rs` | The signed-out and default chat, and when to show the "run this on your own computer" card. |
| Onboarding | `src/client_ffi/onboarding.rs` | The tutorial's words and whether it was seen (#11996, on `main`). |
| Push | `src/push.rs` | Gateway registration, delivery grants, and `Config::parse(…, "android")`, which already selects FCM. |
| Voice | `src/client_ffi/voice.rs` | Call state and signaling; the platform supplies media through `VoiceMediaListener`. |
| Wallpaper, orb, captions | `src/wallpaper.rs`, `src/orb.rs`, `src/caption.rs` | Effects and animation frames computed in Rust. |

Listener traits (`ClientListener`, `LayoutListener`, `AccountListener`,
`VoiceSessionListener`, `VoiceMediaListener`, `UploadProgress`) are
`#[uniffi::export(with_foreign)]`, so Kotlin implements them directly.

Networking uses rustls (`reqwest` with `rustls-tls`, `tokio-tungstenite` with
`webpki-roots`), so there is no OpenSSL to cross-compile. An offline
`cargo tree --target aarch64-linux-android` resolves the whole graph (754
packages). It includes native C and C++ that has to build with the NDK: `ring`,
`libsqlite3-sys` 0.30 (bundled), `mlua-sys` (Luau), `tree-sitter`, and `wgpu`.
The retired app built `ring`, SQLite, and `wgpu` for Android, so the new risk
is Luau and tree-sitter, both plain `cc` builds.

Two dependencies need an Android-only start-up call that iOS doesn't have:

- `iroh` (through `coder-sharing`, `coder-host`, and `openagents-connect`)
  uses `ndk-context` for DNS and `netdev` through `jni`. The process must
  publish the JVM and application context before the first client starts.
  The retired app did this with `OpenAgentsNative.installContext`.
- `rustls-platform-verifier` 0.7 needs `rustls-platform-verifier-android` (an
  Android library in Gradle) and an `init` call with the JNI environment and
  context.

Both fit in one small exported Rust function that Kotlin calls from
`Application.onCreate`.

### Platform stores that are Apple-only today

These Rust paths compile for Android but have no persistent store there:

| Secret | Code | iOS today | Android work |
| --- | --- | --- | --- |
| Account session | `apps/openagents/crates/mobile/src/client_ffi/account.rs` (`PlatformStore::mobile` under `cfg(target_os = "ios")`); `apps/openagents/crates/account` (`security-framework` only on macOS and iOS) | Keychain, `com.openagents.app.account` | An Android store, either a foreign trait implemented in Kotlin or a Rust store over a Keystore-wrapped file |
| Identity words (#11995) | `apps/openagents/crates/account/src/identity.rs` on `agent/11995` (`IdentityVault`, `KeychainVault`) | Keychain, `com.openagents.app.identity` | An `IdentityVault` over the Android Keystore |
| Wallet seed (#11999) | `apps/openagents/crates/mobile/src/wallet/seed.rs` on `agent/11999` (`KeyStore`, `KeyRead`) | Keychain, `com.openagents.app.spark` / `seed-v1` | A `KeyStore` over the Android Keystore, plus reading the retired Android app's seed |
| Paired-computer grants (#12000) | `apps/openagents/crates/sync/src/paired_device.rs` | Keychain | The same Android store |

The seed rules in `seed.rs` (read first, never delete or replace a stored
seed, treat a failed read as unavailable rather than missing) are in Rust, so
they hold on Android as long as the Kotlin store reports `NotFound` only when
the item is truly absent.

### The retired OpenAgents Android app

`bins/openagents-android` was removed in c63b8bbe5c (#11901). It drove an
older core (`crates/openagents-mobile`) through a JSON-over-JNI bridge, so its
screens don't port, but these files do. Read them with
`git show c63b8bbe5c^:bins/openagents-android/host/app/src/main/java/com/openagents/app/FILE`.

| File | Lines | Reuse |
| --- | --- | --- |
| `DeviceKey.kt` | 202 | Keystore AES-GCM secrets in `noBackupFilesDir`, one key and file for each purpose. Port it nearly as is for the four stores above. It also defines where the retired app kept its wallet seed (`spark-v1-key.bin`, Keystore alias `com.openagents.app.spark-v1`, the alias as additional authenticated data), which the new app must read first on an upgraded install. |
| `TranscriptPainter.kt` | 1,261 | A Canvas painter for Rust display lists over `RecyclerView`. The display-list format differs, but the paint, scroll, selection, and copy machinery is the right starting point. |
| `TextSelection.kt`, `SelectionLayer.kt`, `StreamFade.kt` | 476 | Transcript selection and the streaming fade, with JVM unit tests. |
| `NativeRenderer.kt` | 506 | A Rust Native (`rust-native.view.v2`) renderer with Android widgets, keyed reconciliation. |
| `QRScanner.kt` | 101 | CameraX and ZXing scanner whose result goes to Rust as bounded text. |
| `WalletScreen.kt`, `WalletLink.kt` | 978 | Reference for the wallet screens and restore flow. |
| `InputBar.kt`, `DraftEditor.kt`, `Shell.kt`, `Fonts.kt`, `Theme.kt` | 826 | Reference for the composer, drawer, fonts, and palette. |
| `host/app/build.gradle.kts`, `scripts/build-openagents-android.sh` | 337 | Gradle setup (SDK 35, NDK 27.1, minimum API 26, 16 KiB page alignment, R8, signing from Gradle properties), and the release, bundle, and signature checks. |

[`docs/mobile/retired-android.md`](../mobile/retired-android.md) keeps its
release notes. The release keystore it names
(`~/work/.secrets/openagents-android-release.jks`, alias `openagents`) still
exists. `crates/openagents-web/src/pages/connect.rs` already serves
`/.well-known/assetlinks.json` for `com.openagents.app` with that key's
fingerprint.

### The Coder Android host

`bins/coder-android/host` is still in the tree and builds a different app
(Verse and read-only Coder chats over `crates/coder-mobile`). Its reusable
pieces are current code rather than history:

- `app/src/push` and `app/src/nopush`: Firebase Messaging compiled only when
  `app/google-services.json` exists, with a stub otherwise. This keeps the
  rule that the file is never committed.
- `DeviceStorage.kt`, `QRScanner.kt`, and `NativeRenderer.kt` (232 lines).
- `app/src/androidTest/.../MobileAcceptanceTest.kt`: the instrumented-test
  pattern on an emulator.

### Rust Native on Android

`crates/rust-native` has no Android adapter in the tree; its spec
(`crates/rust-native/docs/spec.md`) already describes Android behavior (for
example, `PopupMenu` for context menus). The iOS app renders Rust Native views
in `Session/AgentFormController.swift` and `Session/ChatHomesController.swift`,
and draws Lab cards in `Transcript/LabCardView.swift`. The Android shell needs
the same small renderer, starting from the two `NativeRenderer.kt` files.

### Build scripts

`apps/openagents/scripts/android/build-core.sh` generates Kotlin bindings with
`uniffi-bindgen` and, when `cargo-ndk` and `ANDROID_NDK_HOME` are present,
builds `arm64-v8a` and `x86_64` libraries. Its own comment says it was never
exercised. It also writes to `$ROOT/target` and ignores `CARGO_TARGET_DIR`,
which breaks the repository's target-directory rule; fix that first.

### Server side

- Push: `crates/push-gateway` already sends FCM HTTP v1
  (`docs/deployment/push-gateway.md`). Production needs the Firebase service
  account, `PUSH_GATEWAY_FCM_APP_PROFILE`, and `CODER_SYNC_PUSH_FCM_PROFILE`
  configured, which are owner steps.
- App Links: served, as noted above.
- Abuse limits (#11994, on `agent/11994`): the gateway accepts an App Attest
  attestation or, without one, a proof of work. Android can ship on the proof
  of work. A Play Integrity check is new, additive server work.

## The iOS app, screen by screen

Line counts are Swift on `main` unless a branch is named. "Port" means a new
Kotlin file with the same job; Rust does not change unless stated.

| iOS screen or part | Swift | Android work | Native pieces |
| --- | --- | --- | --- |
| App start and state (`App/AppModel.swift`, `AppDelegate.swift`, `SceneDelegate.swift`, `main.swift`) | 1,240 | `Application`, one `Activity`, an `AppModel` that owns `CoreClient` and hops listener callbacks to the main thread | Launch extras that mirror the iOS launch arguments (`-demo`, `-route`, `-lab`) for tests |
| Fonts and fallback measurer (`Core/Fonts.swift`) | 116 | Load the same Geist bytes with `Typeface`; implement `PlatformMeasurer` with `Paint.getTextRunAdvances` | A line-break accuracy test like `OpenAgentsTests/LineBreakAccuracyTests.swift` |
| Transcript (`Transcript/TranscriptListView.swift`, `RowView.swift`, `RowModel.swift`, `FrameRelay.swift`) | 1,554 | A virtualized `View` that positions reusable row views at Rust's offsets and paints runs with `Canvas.drawText` at Rust's positions; follow-the-tail and anchor logic; syntax spans from the core | Choreographer frame pacing, text selection, copy |
| Tool groups, visuals, Lab cards, image viewer (`ToolViews.swift`, `VisualView.swift`, `LabCardView.swift`, `ImageViewer.swift`) | 1,358 | Port each; visuals show worker-rasterized bitmaps | Pinch zoom and drag to dismiss |
| Session (`Session/SessionViewController.swift`, `CoreSessionSource.swift`, `SessionSource.swift`) | 1,249 | Transcript under a bottom stack (status pill, queue, composer or questions) that follows the keyboard | `WindowInsetsAnimationCompat` for the keyboard; edge-to-edge is required at target API 35 |
| Composer (`Composer/ComposerBar.swift`, `QuestionPanel.swift`, `Mentions.swift`, `AttachmentPicker.swift`) | 1,261 | Multiline `EditText`, context chips, send, queue, steer, and stop; question cards; `@file` mentions; attachments | Photo Picker, camera capture, `ACTION_OPEN_DOCUMENT`, paste; HEIC to JPEG with `ImageDecoder`, long side at most 2,560 px |
| New session and handoff (`Session/NewSessionViewController.swift`, `DraftHandoff.swift`) | 791 | Composer-first page with project, branch, model, and effort menus; a simpler handoff animation | `PopupMenu` |
| Cloud chat and own computer card (`Session/CloudChatViewController.swift`, `OwnComputerCardView.swift`) | 504 | Port; the card's copy and timing are in Rust | Clipboard, Custom Tabs for links |
| Agent form, chat homes (`Session/AgentFormController.swift`, `ChatHomesController.swift`) | 341 | The Rust Native renderer | — |
| Drawer shell (`Shell/DrawerShellController.swift`) | 566 | `DrawerLayout` or a custom edge-swipe drawer with Sessions, search, Settings, and the floating New session capsule | System back gesture (predictive back on API 34+) |
| iPad split (`Shell/SplitRootController.swift`) | 324 | Not in the first release; phones only | — |
| Session lists, cells, saved history, new project (`Threads/*.swift`) | 1,671 | `RecyclerView` with `ListAdapter`, fixed row heights, swipe and long-press actions, long-press drag to reorder, the folder browser | `ItemTouchHelper` |
| Settings (`Shell/MoreViewController.swift`) | 386 | Account, devices and nicknames, pairing, appearance, wallpaper, archive, Our cloud, project sharing, notifications, about | Share sheet (`ACTION_SEND` chooser) for project sharing |
| Onboarding (`Shell/OnboardingViewController.swift`, #11996) | 179 | A few skippable pages from `CoreOnboarding` | — |
| Design pieces (`Design/*.swift`) | 1,423 | Palette (light and dark from `crates/oa-tokens`), status glyph, wallpaper hero, edge fades, project tile, brand marks (SVG paths to `Path`), toast | The iOS Liquid Glass (`UIGlassEffect`) has no Android counterpart; see the decisions |
| Push (`App/PushNotifications.swift`) | 169 | FCM token to `push.rs`, notification channels, quiet while the app is in front, a tap opens the session | `POST_NOTIFICATIONS` permission on API 33+; `src/push` and `src/nopush` source sets |
| Voice (`Voice/*.swift`) | 1,764 | Call controls in the capsule, the full-screen stage, the orb, voice settings, and a WebRTC peer behind `VoiceMediaListener` | A WebRTC library, audio routing and the proximity sensor, a microphone foreground service |
| Voice Live Activity (`OpenAgentsLiveActivity/`, `OpenAgentsShared/`) | 338 | An ongoing call notification (`Notification.CallStyle` on API 31+) with mute and end actions, owned by the foreground service | — |
| Debug lab (`Debug/*.swift`) | 324 | A `-lab` equivalent with a frame meter, for scroll benchmarks | — |
| Backup and restore (`Shell/MoreViewController.swift`, "Back up your account") | in Settings | Show the 12 words after a biometric check, confirm a few, restore from words | `BiometricPrompt`; `FLAG_SECURE` on key screens; a sensitive clip (`EXTRA_IS_SENSITIVE`) cleared after 60 seconds, as `INVARIANTS.md` requires |
| Wallet (#11999; `Wallet/WalletViewController.swift`, `WalletModel.swift`, `WalletKeychain.swift` on `agent/11999`) | 557 | Port, using `WalletScreen.kt` as reference; seed in the Android `KeyStore` implementation; read the retired app's seed first | QR display for invoices |
| QR scanner and pairing (#12000; `Shell/CodeScannerViewController.swift` on `agent/12000`) | 232 | CameraX and ZXing, port of `QRScanner.kt`, feeding the same Rust classifier | `CAMERA` permission; App Links for `https://openagents.com/connect` and `/projects/join/*` |
| App Attest (#11994, #11995; `App/AppAttest.swift` on `agent/11995`) | 75 | None at first: send the proof of work. Later, Play Integrity | — |

UI tests (`OpenAgentsUITests`, 21 files) don't port line for line. The flows
worth keeping on Android are session flow, history, cloud chat, onboarding,
the Design Lab tour, and scroll performance.

## Build and release

- **Location.** `apps/openagents/apps/android`, beside `apps/ios`, in the
  `apps/openagents` Cargo workspace. Package and application ID
  `com.openagents.app`; the name comes from `apps/openagents/crates/brand`.
- **Gradle.** Android Gradle Plugin and Kotlin current stable, compile and
  target SDK 35 (36 when Play requires it), JDK 17, Android Views (no
  Compose), `net.java.dev.jna:jna` (`@aar`) for UniFFI's Kotlin bindings,
  `androidx.camera`, `com.google.zxing:core`, `androidx.biometric`,
  `androidx.browser`, `rustls-platform-verifier-android`, and Firebase
  Messaging in the `push` source set only.
- **Rust.** `cargo-ndk` with NDK 27.1, targets `aarch64-linux-android` (release
  and device) and `x86_64-linux-android` (Intel emulators); Apple Silicon
  emulators run `arm64-v8a`. Use the `mobile` profile for debug builds and
  `mobile-dist` for release. Check 16 KiB page alignment, which Play requires
  for new apps.
- **Bindings.** Generate Kotlin from the built library, as Swift is generated
  today. Commit them under the Android app, like
  `apps/ios/OpenAgents/Core/Generated/`, so a Kotlin-only change builds without
  Rust.
- **Script.** One local script, `apps/openagents/scripts/android/build.sh`,
  with `core`, `apk`, `install`, `run`, `bundle`, and `check` commands. Port
  the retired script's environment scrubbing, signing from an env file that is
  never printed, `apksigner` verification, and alignment check. No GitHub
  workflows.
- **Versioning.** Use the iOS app's `MARKETING_VERSION` and build number
  (`apps/openagents/apps/ios/release/version.json`), so both phones name the
  same build. The code must be higher than the retired app's last code (16)
  for an upgrade install.
- **Signing and Play.** Sign with the existing release key. In Play Console,
  choose Play App Signing with that key uploaded as the app signing key, so
  the `assetlinks.json` fingerprint stays right and devices that have the
  retired APK upgrade in place (which keeps its wallet seed readable). Ship to
  **Internal testing** first. The Play Console app, Data safety form, content
  rating, testers list, and the Firebase project are owner steps.

## Testing

- **Rust.** `cargo test -p openagents-mobile` and the crates it uses, as
  today. Add tests for the Android store hooks with an in-memory store,
  including the seed rules.
- **Kotlin unit tests** on the JVM for pure helpers (selection, stream fade,
  attachment sizing), ported with their tests from the retired app.
- **Emulator.** An `arm64-v8a` emulator on the Mac, run with the offline demo
  (`-demo` equivalent), so nothing signs in and nothing touches the owner's
  account. Instrumented tests with UI Automator for the flows listed above.
  Live checks use the dedicated test account and `OPENAGENTS_TEST_RUN=1`.
- **Design Lab.** Add `apps/openagents/scripts/design-lab-capture-android.sh`
  that walks the same tour on the emulator, saves `adb exec-out screencap`
  pictures in light and dark mode, and writes each screen's element tree from
  `uiautomator dump` in the JSON shape `crates/design-lab` already reads. The
  board then needs a platform dimension, so iOS and Android versions of a
  screen sit side by side.
- **Text accuracy.** A test that lays out the fixture markdown in Rust and
  checks Android's line breaks against it, like the iOS CoreText test. This
  catches the one class of bug the display-list design depends on avoiding.
- **Device runs** (physical phone, push, camera, voice) are owner steps.

## Plan

Each step is one issue. Estimates are agent-days of focused work and include
tests and captures, not owner steps.

| Phase | Step | Estimate | Depends on |
| --- | --- | --- | --- |
| 0. Core on Android | A1. Make `scripts/android/build-core.sh` honor `CARGO_TARGET_DIR`, build both ABIs, and fix what fails to compile (Luau, tree-sitter, `wgpu`, Breez on `agent/11999`) | 1–2 | — |
| | A2. Android start-up call: publish the JVM and context for `ndk-context` and `rustls-platform-verifier` | 0.5 | A1 |
| | A3. Android secret stores: account session, `IdentityVault`, wallet `KeyStore`, and paired grants, from `DeviceKey.kt`; Rust tests with memory stores | 1–1.5 | A1, #11995, #11999 |
| 1. Skeleton | B1. Gradle project, generated bindings, build script, launch extras, fonts, palette; the demo workspace boots and lists sessions | 1–1.5 | A1, A2 |
| 2. Transcript | C1. Transcript host and row painter over `LayoutFrame`, `PlatformMeasurer`, the line-break test | 3–4 | B1 |
| | C2. Tool groups, syntax spans, selection and copy, streaming fade, image viewer | 1.5–2 | C1 |
| | C3. Agent visuals and Lab cards | 1–1.5 | C1 |
| 3. Shell | D1. Drawer shell, session lists, search, saved history, swipe and drag actions | 2–3 | B1 |
| | D2. Session screen, composer, question panel, mentions, attachments, keyboard | 2.5–3.5 | C1 |
| | D3. New session page, cloud chat, own computer card, new project | 1.5–2 | D2 |
| | D4. Rust Native renderer; agent form and chat homes | 1 | B1 |
| | D5. Settings: account, devices, pairing, appearance, wallpaper, archive, Our cloud, project sharing | 1.5–2 | D1 |
| 4. Identity epic | E1. Onboarding pages, backup with `BiometricPrompt`, restore, Your ID; key-screen protections (#11996) | 1–1.5 | A3, D5 |
| | E2. Wallet screens and the retired app's seed read (#11999) | 1.5–2 | A3 |
| | E3. QR scanner, account-link approval, App Links (#12000) | 1 | A3, D5 |
| 5. Platform services | F1. FCM: source sets, token to `push.rs`, channels, permission, tap to open (#11853) | 1–1.5 | D1 |
| | F2. Voice: WebRTC peer, audio routing, call controls, stage, orb, settings | 3–4 | D1 |
| | F3. Call notification and microphone foreground service (the Live Activity equivalent) | 1–1.5 | F2 |
| 6. Ship | G1. Instrumented flow tests and the Android Design Lab capture | 2–3 | D3 |
| | G2. Release bundle, signing, alignment and signature checks, internal-testing notes | 1 | B1 |
| Server (optional) | H1. Play Integrity verification beside App Attest, additive | 1.5–2 | #11994 |

Totals: phases 0 to 6 come to 29 to 40 agent-days; H1 adds 1.5 to 2. Without
F2 and F3 (voice), the total is 25 to 35.

Phases 0 and 1 are serial and take about 3.5 to 5.5 days. After B1, three lanes
can run in parallel: the transcript (C), the shell (D), and platform services
(E and F). Phase 6 starts once D3 lands.

The ranges assume the identity epic's Rust lands on `main` in its current
shape. If #11995, #11999, or #12000 change their store traits, A3 and phase 4
move with them.

## Risks

- **The core has never run on Android.** A1 is the first real build. Native C
  and C++ dependencies, `wgpu` on the emulator's OpenGL ES, and Breez's SQLite
  are the likely failures. A1 could reach 3 days.
- **Text that measures differently from how it paints.** The design relies on
  Android drawing exactly what Rust measured. The bundled fonts make this
  likely, but fallback glyphs (emoji, CJK) depend on `PlatformMeasurer`. The
  line-break test is the guard.
- **UniFFI over JNA on hot paths.** Kotlin bindings call through JNA, which
  costs more per call than Swift's direct C calls. The transcript asks for a
  display list per visible row; if scrolling hitches, batch the calls in Rust
  or move that one path to a direct JNI function.
- **The wallet seed.** A new seed must never be made where one exists. On
  Android the retired app's seed is readable only by an upgrade install with
  the same signing key on the same device. Read it before anything else
  touches the store; tests must prove a present seed is used as is.
- **WebRTC on Android.** Google no longer publishes official Android builds.
  The choice is a maintained community build or building `libwebrtc`, and
  either has to be pinned and kept current for security fixes.
- **Library size.** The core links `wgpu`, Luau, tree-sitter, iroh, and Breez.
  Expect a large `.so`; measure it in A1 and strip in `mobile-dist`.
- **Background limits.** Android stops background work more aggressively than
  iOS. Session updates arrive only while the app is open or through FCM
  wakes; a voice call needs its foreground service.
- **The Coder 1.0 freeze.** Nothing here changes a Coder binary. Server changes
  (FCM configuration, Play Integrity) must stay additive.

## Decisions for the owner

1. **Look.** iOS uses Liquid Glass, which Android can't reproduce. Recommended:
   the same palette, type, and Rust layout, with flat translucent surfaces and
   Android's own back gesture and system bars, rather than imitating glass.
2. **Voice in the first Android release?** Recommended: no. Ship chat,
   identity, wallet, pairing, and push first, and add voice in the next
   release. This saves 4 to 5.5 agent-days and the WebRTC risk.
3. **Minimum Android version.** The retired app used API 26. Recommended:
   API 29 (Android 10), which keeps nearly all active phones and simplifies
   storage, biometrics, and gesture navigation.
4. **Phones only.** Recommended: no tablet split layout in the first release,
   as the iPad split is not needed for parity on phones.
5. **Signing.** Recommended: upload the existing release key as the Play app
   signing key, so App Links and upgrades from the retired APK keep working.
   If Google generates a key instead, add its fingerprint to
   `connect.rs` and accept that retired installs can't upgrade.
6. **A cloud copy of the 12 words.** iOS plans an optional iCloud Keychain
   copy. Android's counterpart is Google's Block Store. Recommended: off for
   the first release, matching the iOS default.
7. **Abuse limits.** Ship Android on the proof-of-work path, and decide whether
   Play Integrity (H1) is needed before an open beta.
8. **Distribution.** Recommended: Play internal testing only, and no
   direct-download APK, so there is one update path.

Owner steps for `NEEDS_OWNER.md` when the work starts: create the Play Console
app for `com.openagents.app`, create the Firebase project and keep
`google-services.json` outside Git, configure FCM on the push gateway and
`coder-sync`, fill in Data safety and content rating, and run the first build
on a physical phone.
