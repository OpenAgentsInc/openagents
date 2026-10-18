# Android transcript interactions (#12085)

C2 adds presentation over C1's Rust `TranscriptFrame`. Kotlin never wraps a row,
orders sessions, interprets a retry/stop status, or changes Rust Native intents.
The source and authored fixtures are being validated against the C1-only Android shell; the current build result is recorded below.

`TranscriptSelectionHost` wraps the C1 view. Supply the same `TranscriptFonts`,
colors, and current immutable frame/viewport through `TranscriptInteractionSnapshot`.
Android ICU chooses grapheme/word boundaries; geometry uses C1's exact font and
bidirectional shaping contexts inside Rust-selected text runs. Native hit tests
and highlights respect Rust scroller rectangles and horizontal offsets.

Long press selects a word; handles return bounded row-key/UTF-16 points. Native
floating action mode copies the exact selected ranges or offers read-only
Android Process Text targets. Accessible virtual rows expose select/copy and
selection-range actions using those same ranges. Copy is bounded to 64 KiB and
4096 rows. No Activity, Gradle, SemanticViewAdapter, clipboard history, production
account, or session deletion code is changed. The shell launches requested
system text actions; unavailable processors stay absent. Package visibility
queries, if required by the app, remain B1's manifest integration.

A callback stamp includes a unique binding and frame serial. Frame replacement,
rotation/cancel, account/room change, and teardown retire old callbacks. Streaming
append and earlier-history insertion retain row keys and fixed offsets; removal
of either endpoint clears selection. A bookmark contains only row/offset state
and the caller's Rust account/conversation identity. Restore it only for that
same account/room; never use a title as an identity or persist frame handles or
selected text. Close and recreate the binding on account changes.

## C1 integration hook

The retained C1 Canvas keeps its viewport private. The separately committed
additive hook exports:

```kotlin
fun interactionSnapshot(): TranscriptInteractionSnapshot?
var interactionObserver: TranscriptInteractionObserver?
```

The snapshot borrows `frame: TranscriptFrame`, logical `top` and `height`, and
`horizontal: (ULong, UInt) -> Float`. Horizontal values come from a bounded copy
of C1's existing state; no second viewport is maintained. Use the supplier
`canvas::interactionSnapshot` when constructing the host. Install
`canvas.interactionObserver = host.observer(accountConversation)` before the
first frame. B1 agreed to reserve Canvas for this additive C2 hook.

Replacement ordering is strict: notify `beforeFrameReplacement(next)`; C2 retires
its stamp and reconciles ranges; C1 applies its viewport/painter; notify
`viewportChanged()`; close the old frame. On close, clear the observer and call
`beforeFrameClose()` before engine/frame cleanup. Scroll, size, and horizontal
changes notify `viewportChanged` without changing any Rust data.

This validation branch starts from the final B1/C1 checkpoint and carries only
the C2 source delta. It keeps C1's `TranscriptCanvas` renderer and adds the
snapshot getter and observer lifecycle hooks. The Android snapshot-ordering
fixture is included. Until the real supplier is registered by a host, selection
and system actions remain unavailable rather than reading `View.scrollY` or
inventing viewport geometry. Offscreen handle autoscroll and full native RTL,
IME, and TalkBack behavior still need integrated device validation.

## Streaming and checks

The fixtures author stop/error/retry/attachment frame states and verify that
selection survives them while display-only status widgets create no actions.
They do not prove session stop/retry behavior. Rust supplies `TextRun.revealMs`
and static Fade rectangles, but `RowDisplay` currently exposes no stable layout
transition epoch. C1 paints the static rectangles deterministically. Temporal
reveal integration must use a real Rust clock/epoch contract; C2 does not invent
one or infer session state from text labels. That remaining contract is explicit.

Authored JVM checks: `TranscriptInteractionsTest` and
`TranscriptSelectionLifecycleTest`. Android checks are
`AndroidTranscriptSelectionSnapshotTest` and
`AndroidTranscriptSelectionTextTest`, including real ICU joined-emoji boundaries.
B1's Gradle configuration includes the C2 main, JVM-test, and Android-test roots.
This validation branch excludes the unrelated D2/D3/D5 app sources and generated
binding changes. The first focused run found recursive type inference in
`TranscriptSelectionHost`; an explicit `TranscriptSelectionController` type fixed
it. The next run exposed an unavailable `AndroidJUnit4` dependency, so both C2
Android fixtures now use the configured `InstrumentationTestCase` runner.

The admitted retry passed debug and release Kotlin compilation, JVM test
compilation and execution, AndroidTest Kotlin compilation, and debug app and
test APK assembly. The five JVM XML reports under
`app/build/test-results/testDebugUnitTest/` record 26 tests, with 0 failures,
0 errors, and 0 skipped. The two C2 JVM suites account for 11 of those tests.
AndroidTest compilation produced five test classes: `SemanticViewAdapterTest`,
`AndroidScannerPresentationBoundaryTest`, `AndroidTranscriptSelectionSnapshotTest`,
`AndroidTranscriptSelectionTextTest`, and `AndroidTranscriptTextTest`. On the
existing isolated offline API 35 guest, direct `AndroidJUnitRunner` execution
passed all 17 tests. The output is preserved at
`$OPENAGENTS_SCRATCH/android-runtime-12083/evidence/c2-instrument-results.txt`;
the runner emitted text output, not instrumentation XML. Both debug APKs verify,
with matching signer SHA-256
`f1e42e894e0a5b73548b388f442ab5b2c1e4566f001d9d93e587f9bdfc95be25`. The
compile and assembly tasks and direct runner passed. Stop here; further runtime
checks require fresh coordinator admission.

```sh
nix develop ./os#android --command bash -c 'cd apps/openagents && CARGO_TARGET_DIR="$HOME/work/openagents-target-1-app" scripts/android/app.sh compile :app:assembleDebug :app:assembleDebugAndroidTest'
```

Remaining C2 work includes mounting the host through the app owner, media zoom/dismiss,
and a Rust-owned stable row-layout epoch/age for temporal streaming fade. Rust
currently exposes `TextRun.reveal_ms` but no stable epoch or age through
`TranscriptFrame`; keep rendering deterministic until that contract is explicit.
The current hit callback exposes a typed image/media widget target, but no media
payload loader or viewer-dismiss contract. Also run native selection/action-mode/clipboard,
API 26/35, TalkBack focus, real RTL caret positions, rotation and cancel, system
processor availability, long transcripts, horizontal code/tables, and warm reopen
in offline synthetic state. No executable result is claimed for checks that did not run.
