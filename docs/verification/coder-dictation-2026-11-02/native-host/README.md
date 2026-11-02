# Native dictation effects

The Swift host performs permission, capture, queueing and resident model effects.
Coax owns consent/focus/readiness admission, sample conversion/resampling, the
450 ms pre-roll, 300 ms minimum hold, 200 ms tail, 60-second cap, silence gate,
bounded model segments, text joining and result publication. No host callback
submits a message. The separate composer-insertion record qualifies that boundary.

[record.json](record.json) binds 14 focused native effect cases in normal and ASan builds,
their source/producer identities and the actual recorded-audio Metal witness.
The final tests cover permission/model completion races, silence and taps,
exact audio windows, cancellation during inference, focus changes, resident
model reuse, failures/retry, empty results, nonfinite/stalled capture and three
model-sized segments for a bounded minute. No test requests microphone access
or opens a device. The fake-device suite writes no audio files or network data.
The real-model witness reads the existing public English PCM fixture only.

The original effect suite and recorded-model success preceded the final retry
fix. Review found that a *previous* recognition error was being mistaken for a
new capture error when healthy warm audio arrived. Restoring that one old branch
against the strengthened oracle fails with the expected assertion in
`retry-red.stderr`; final normal and ASan runs pass the failure/audio/retry case
with the same resident model. Raw earlier receipts remain. The successful model
path's input, windows, model call and delivery are unchanged, so the passing
actual-model run is reused rather than repeated just for a source hash.

The actual successful executable remains outside Git and its hash is recorded.
`producer/Dictation-before-retry.swift` reconstructs its host source by reversing
only the known later retry-branch delta; it is identified as a reconstruction,
not a contemporaneous raw capture. `source-final.tar.gz` and the generated C/header
retain the final qualified controller and host source. No binary or model weights
are committed.

ASan covers the Swift effect host and generated controller/C bridge. macOS leak
detection is disabled explicitly; no leak-check result is claimed. Separate
controller qualification supplies fatal UBSan evidence. Model code and its prior
full-corpus checks are reused; this is an integration witness, not another model
qualification or a key-release latency benchmark. A person still needs to grant
the app microphone access and try the real hardware path, as recorded in
`NEEDS_OWNER.md` by the integration owner.

Build the controller with the Coax tool, then link the native oracle against the
same generated objects and resident library used by the app:

```shell
coax/web/tool.sh apps/openagents-desktop/dictation-build.coax "$PWD" /tmp/coder-dictation/c
xcrun -sdk macosx swiftc -O -import-objc-header apps/openagents-desktop/face.h \
  apps/openagents-desktop/Sources/Dictation.swift apps/openagents-desktop/tests/dictation-host.swift \
  /tmp/coder-dictation/c/dictation.o /tmp/coder-dictation/c/dictation-face.o \
  psionic/build/parakeet/libpsionic-parakeet.a -framework AVFoundation -framework Metal \
  -framework Foundation -lpthread -lm -o /tmp/coder-dictation/host-oracle
/tmp/coder-dictation/host-oracle
```
