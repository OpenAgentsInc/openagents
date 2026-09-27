# Coder Android mobile acceptance — September 26, 2026

[Issue #9702](https://github.com/OpenAgentsInc/openagents/issues/9702) brings the
current iOS mobile feature set to Android. The public Android host is
[`bins/coder-android`](../../../bins/coder-android/README.md). It uses the same
Rust reader, encrypted cache, Verse world, controller, and Gym client as iOS.
This is an emulator-tested development APK, not a Play release or a claim of
acceptance on every physical device.

## Scope and provenance

The authorized Android reference was the local Coder checkout at
`f2d85b120ac96b4a4be9116e823e7795472088c1`. The adapter preserves the application
identity `com.openagents.coder`, Android API 26 minimum, API 35 target, and
version `0.5.0`. This public replacement uses version code **4**. Its native
host is reimplemented over OpenAgents' Rust library. The old GPUI runtime,
private authentication, private service endpoints, and backend implementation
are not copied.

Parity refers to the current iOS build 42 feature slice: Verse home, the
in-world computer, retained Codex/Claude chat reading, full-screen rendering,
touch/motion controls, and the entry-scoped Gym. Desktop agent chat, XP, replay,
chat writing, paid operations, and historical private-app features are outside
both current mobile surfaces. See the [mobile guide](../guides/mobile-readonly.md)
and [Verse guide](../../verse/mobile.md).

## Platform boundary

| Responsibility | Implementation |
| --- | --- |
| Application state and permissions | `coder-mobile`, `coder-connect`, and Verse in Rust |
| Reader presentation contract | Generic Rust Native stacks, lists, text, buttons, styles, and revision-bound intents |
| Native reader rendering | Android framework widgets; stable keyed views preserve text selection and scroll position |
| World rendering | Shared Verse mesh, camera, and shaders; Android `SurfaceView` and acquired native window through wgpu GLES |
| World and reader lifetimes | Separate native handles and execution threads; reader calls use one serial background worker |
| Device identity | Separate reader and Verse secrets, encrypted with separate Android Keystore entries |
| Transcript persistence | Rust's encrypted retained-history cache in the app's no-backup directory |
| Pairing | CameraX preview, local ZXing QR decoding, or pasted invitation; Rust verifies the invitation and scoped grant |
| Motion input | Android rotation-vector samples normalized into the shared Rust portrait camera contract |
| Gym | Shared Rust entry policy, authorization, observation, run selection, and exact recipe admission; native lists and charts |

JNI handles are monotonic IDs, not Java-supplied native pointers. Calls reject
wrong-thread, wrong-kind, disposed, malformed, and oversized inputs. The render
bridge owns its acquired native-window reference until the renderer releases
it. Temporary surface loss suspends observation and drops the GPU/window while
retaining the Rust scene. Reattachment preserves position, camera mode, and
selected world relay; final Activity disposal releases the scene too. App theme colors and platform adapters remain outside `crates/rust-native`.

Markdown is formatted locally, selectable, and available as original text.
Exact retained records remain separately accessible. Transcript links and
images do not open external resources or gain execution authority.

Android sensor samples use a different clock from frame callbacks. The adapter
normalizes sample timestamps into the frame clock while preserving freshness
checks. It conjugates Android's device-to-world quaternion into the existing
world-to-device contract. Rust owns recentering, roll removal, pitch limits,
left-hold movement, and suspension. Emulator input checks use labeled synthetic
attitudes; they do not establish physical sensor direction, comfort, or drift.

## Emulator and build environment

The dedicated `coder_mobile_api35` emulator uses a Pixel 7 profile, Android 15
API 35, ARM64, a 1080 × 2400 display, and SwiftShader. The existing unrelated
emulator profile was left unchanged. The native acceptance suite uses synthetic
identities, generated transcripts, offline Gym fixtures, and an existing public
synthetic invitation image. It reads no personal chat history and launches no
model, training job, or benchmark.

| Component | Version |
| --- | --- |
| Rust | Pinned 1.97.1 |
| cargo-ndk | 4.1.2 |
| Android NDK | 27.1.12297006 |
| SDK platform and build tools | 35 and 35.0.0 |
| Java | OpenJDK 17.0.19 |
| Gradle wrapper | 8.13, with distribution SHA-256 pinned |
| Android Gradle Plugin | 8.13.0 |
| Kotlin Android plugin | 2.0.21 |
| Emulator | 36.6.11.0 |

The APK is built outside git at
`../target/coder-android/gradle/app/outputs/apk/debug/app-debug.apk`.
The build helper strips provider credentials and injected Java options from
compiler environments. Device commands require an explicit serial and matching
ABI. Installation uses replacement without data erasure. Tests retain installed
APKs and refuse automatic removal of incompatible installations.

## Results

| Check | Result | Evidence |
| --- | --- | --- |
| Native Android acceptance | 13/13 passed; 30.676 seconds of reported test time | [JUnit](2026-09-26-android-mobile/native-final.xml), [helper log](2026-09-26-android-mobile/native-final.log) |
| Rust mobile library | 25/25 passed, including local authenticated relay integration | [Tests](2026-09-26-android-mobile/rust-tests.log) |
| Verse renderer invariants | 4/4 passed | [Tests](2026-09-26-android-mobile/verse-render-tests.log) |
| Android Rust Clippy | Passed with warnings denied | [Clippy](2026-09-26-android-mobile/android-clippy.log) |
| iOS compilation regression | Passed for `aarch64-apple-ios` | [Check](2026-09-26-android-mobile/ios-check.log) |
| Android lint | 0 errors, 32 warnings; no baseline suppressions | [Log](2026-09-26-android-mobile/android-lint.log), [warning inventory](2026-09-26-android-mobile/lint-summary.json) |
| Native architectures | ARM64 and x86_64 built; only ARM64 executed | [ARM64 build](2026-09-26-android-mobile/android-rust-build.log), [x86 receipt](2026-09-26-android-mobile/x86-receipt.json) |
| APK and 16 KiB binary alignment | All three packaged libraries and ZIP alignment passed | [Inspection](2026-09-26-android-mobile/apk-inspection.json) |
| Normal offline app launch | Passed after synthetic acceptance; installed app retained | [Launch log](2026-09-26-android-mobile/normal-launch.log), [world screenshot](2026-09-26-android-mobile/world.png) |

The JVM unit-test task has no sources; it is not counted as a passing test suite.
Lint warnings include pinned versions and SDK levels, native framework widget
choices, English strings, portrait orientation, and the single ABI per APK.
Verification follows the repository's targeted development policy; no full
release matrix or external model workload was needed.

## Acceptance coverage

The native suite exercises these behaviors through the installed Android app:

- Full-window canvas dimensions, including the clock area; presented frames;
  actual pointer events moving the shared player; and jump input.
- Transcript paging, pinned pages while follow is off, follow resumption,
  formatted and exact-record views, and preserved native text selection across
  an unchanged refresh.
- Visible pairing errors and the QR/paste entry points; the bundled QR decoder
  reproduces the exact 191-byte invitation with SHA-256
  `a9e0729c887fb856a06c83f06993d458692b7ebe848af864d29a88e5e31ce037`.
- Reader identity across background/resume and activity recreation; world
  position and camera mode across backgrounding and native surface replacement.
- Injected device attitude, recentering, hold-to-move, and touch-mode fallback.
- Walking into the Gym before observation starts, approaching its board,
  chart and exact-value display, recipe review without submitting work, and
  observation stopping after exit.
- Invalid, cross-thread, wrong-kind, and stale JNI handles.
- Atomic identity recovery, concurrent first creation, corrupt and oversized
  ciphertext preservation, and isolation between synthetic and normal storage.

Shared Rust checks separately cover authenticated local relay invitation
redemption, transcript paging and refresh, encrypted restart recovery,
revocation, stale view actions, and input/lifecycle math. That evidence is not
an Android-to-production-relay session.

The [evidence directory](2026-09-26-android-mobile/) retains build/test logs,
JUnit results, source and artifact identities, and screenshots. Initial failed
checks are retained alongside the corrected result.

## Problems found and corrected

1. **Vulkan initialization stalled on this emulator.** The first launch remained
   on its splash with the main thread busy in graphics initialization. Android
   now selects GLES explicitly. The replacement rendered the actual shared
   world, and the observed cold launch completed in 1.14 seconds. This single
   observation is not a performance benchmark; desktop and Apple backend
   selection remain unchanged.
2. **The first QR dependency initialized telemetry.** It was replaced with local
   ZXing decoding over CameraX frames. The replacement APK has no MLKit,
   Firebase, or DataTransport manifest entries or barcode-model native library.
3. **Identity recovery checked the base file too early.** Storage now lets
   `AtomicFile` restore an interrupted write before checking lengths. One
   process-wide lock prevents concurrent Activities from creating competing
   identities. Corrupt data is retained and reported rather than replaced.
4. **Reader refresh could discard selection.** Native views now reconcile by
   stable Rust node key; unchanged text retains its native object and selection.
   Reader lifetime operations share one worker so a retired Activity cannot
   write a stale cache after a replacement reader erases it.
5. **Native lifecycle and in-progress input needed protection.** Camera
   permission requests survive the system permission pause, inactive camera
   work is canceled, and incoming Gym updates preserve connection edits and
   show connection errors in the panel.
6. **The first Gym test skipped the required approach.** It timed out correctly
   outside board range. The corrected test walks through the actual entrance,
   opens the board only when near, and verifies that leaving stops observation.
7. **Backgrounding could reset the world.** Android can destroy its surface
   without destroying the Activity. Native detach/attach operations now preserve
   the suspended Rust scene while releasing and reacquiring the GPU surface.
   This is separate from preserving the reader identity after Activity recreation.
8. **Gradle removed APKs after the first instrumentation run.** The first run
   used only a dedicated synthetic emulator. The repository now explicitly
   retains APKs and disables incompatible-install removal. The subsequent run
   confirmed that the installed app remained available afterward.

## Remaining acceptance

Physical camera capture and permission interaction, real phone attitude
accuracy and battery use, TalkBack, Android versions other than API 35,
production-relay pairing on Android, and a physical 16 KiB page-size device
remain unverified. ARM64 emulator execution and an x86_64 cross-build are
different evidence. ELF segment and APK ZIP alignment checks do not replace a
16 KiB runtime test. No store signing configuration or Play publication is
included.

These limits do not change the existing permission model: chat pairing is
read-only, world presence uses a separate identity and explicit join, and a
Gym connection cannot start an unlisted recipe or bypass confirmation.
