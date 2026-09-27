# Android verification evidence

Read the [acceptance assessment](../2026-09-26-android-mobile.md) for scope,
architecture, findings, and unverified device behavior. Verification started on
September 26, 2026; the final receipts cross midnight UTC into September 27.
Raw tool output retains its original whitespace and line endings. All transcript and Gym screenshots use generated synthetic state. `world.png`
shows the normal offline app, without preview diagnostics.

| Artifact | Meaning |
| --- | --- |
| [Delivery receipt](receipt.json) | Source hashes, APK identity, toolchain, checked device, commands, and results |
| [Final native log](native-final.log) and [JUnit](native-final.xml) | 13 Android tests, including surface replacement, retained camera/pose, Gym error visibility, and protected-storage isolation |
| [Rust mobile tests](rust-tests.log) | 25 tests; includes a synthetic local authenticated relay round trip |
| [Renderer tests](verse-render-tests.log), [Android Clippy](android-clippy.log), [iOS check](ios-check.log) | Targeted shared-code and platform checks |
| [Android lint](android-lint.log) and [warning inventory](lint-summary.json) | 0 errors, 32 warnings; no unit tests claimed for the `NO-SOURCE` task |
| [APK inspection](apk-inspection.json) | Packaged library hashes, 16 KiB ELF/ZIP checks, and absence of the removed QR telemetry dependency |
| [x86 build](x86-build.log), [receipt](x86-receipt.json), [ELF](x86-elf.txt) | Cross-compilation only; no x86 emulator execution |
| [Normal app launch](normal-launch.log) | Installed app retained after tests and started without synthetic mode |
| [Initial native failure](native-initial.log) | First Gym test attempted to open a board before approaching it |
| [Intermediate 12-test pass](native-12-tests.log) and [JUnit](native-12-tests.xml) | Preceded the final surface-retention regression and storage-isolation test |
| [First successful GLES launch](gles-launch.log) | Emulator startup after replacing the stalled Vulkan initialization path |

Screenshots:

- [Normal world, full bleed](world.png).
- [Formatted transcript](transcript.png) and [exact retained record](exact-record.png).
- [Gym chart with recorded values](gym-chart.png).
- [Gym recipe review](gym-recipe.png); no start was submitted.
- [Visible Gym connection error with preserved input](gym-error.png).

The reproducible native suite is in
[`MobileAcceptanceTest.kt`](../../../../bins/coder-android/host/app/src/androidTest/java/com/openagents/coder/MobileAcceptanceTest.kt)
and [`DeviceStorageTest.kt`](../../../../bins/coder-android/host/app/src/androidTest/java/com/openagents/coder/DeviceStorageTest.kt).
The QR image was reused unchanged from the retained iOS synthetic pairing
fixture; its exact decoded bytes are checked by the native test. No credentials,
private chats, APKs, or signing keys are stored in this evidence directory.
