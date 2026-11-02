# Coder Mac dictation input selection

GitHub #12671 adds **OpenAgents → Dictation Input…**. The system default is an
explicit choice; selecting another input persists its CoreAudio UID locally.
A missing selected device refuses capture instead of silently using another
microphone. CoreAudio UIDs are machine-local identifiers, not portable settings.

[Coax policy](../../../apps/shared/dictation-device.coax) owns selected/default
identity and exact rate/channel/stride/time admission: 1–32 channels, planar or
interleaved stride, valid host timestamp, and exactly 16/24/44.1/48 kHz. UIDs
admit at most 512 bytes and no ASCII control bytes. Fractional rate near-matches
refuse. Swift transports float32 channel zero, enumerates devices, sets the native input,
persists the UID and handles hardware/configuration notifications. It installs
listeners before starting IO and revalidates the selection after registration.

Selection and observed route/configuration changes stop the old capture
generation, clear its audio and invalidate pending insertion. Queued old-device
callbacks and an old recognition result cannot enter the replacement request.
The resident model is reused. A route failure requires an explicit available
selection or rearm; it does not silently fall back or resume the old hold.

[record.json](record.json) pins sources, producers and artifacts. Changed-code
checks passed:

- Two Coax policy groups, 409 cases, including UID limits and exact rates.
- Fourteen native foreign-descriptor/format cases at O2 and ASan/fatal UBSan.
- Eighteen host-effect cases at O2 and Swift ASan with generated C under
  ASan/fatal UBSan. New cases exercise persisted selection, switches during a
  hold/inference, old callbacks, route loss and actual stereo/interleaved
  `AVAudioPCMBuffer` layouts. Injected devices/recognizers never open a mic.
- The full desktop app generated, linked and signed using the shared current
  compiler and Parakeet archive. Only the pre-existing Swift `header` warning
  and signature replacement were emitted.
- The [actual recorded-audio app](integration/receipt.json) passed keyboard
  hold, exact draft, Undo, mouse hold and zero sends/message parts. The PNG is
  byte-identical to the preceding qualified app fixture. This is not physical
  input-device qualification or a release-latency benchmark.

CoreAudio's installed Apple SDK documents that device UID and name properties
return retained CFObjects; the host consumes that ownership explicitly.
The sanitizer runs disable leak detection. No leak-check result is claimed.
Original model arithmetic/clearing qualification is reused, with the copied
archive hash verified; no full model corpus or full mutation run was repeated.
The compiler is a focused development build, not a full bootstrap/release gate.

The policy and foreign-boundary checks can be run from the repository root:

```shell
coax/bin/coax test apps/shared/dictation-device.coax
coax/bin/coax c apps/shared/dictation.coax --release --library --header /tmp/dictation.h > /tmp/dictation.c
clang -std=c11 -O2 -I/tmp -Iapps/openagents-desktop apps/openagents-desktop/tests/dictation-device-native.c apps/openagents-desktop/dictation-face.c /tmp/dictation.c -lm -o /tmp/dictation-device-check
/tmp/dictation-device-check
```

Physical menu selection, permission, device removal and rearm checks remain
owner-only in [NEEDS_OWNER.md](../../../NEEDS_OWNER.md). No microphone, visible
window, sound playback, clipboard change or message send was used by automation.
