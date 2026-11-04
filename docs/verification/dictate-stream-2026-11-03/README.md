# Linux CLI streaming dictation, 2026-11-03

Issue #12745 adopts the qualified original-weight English RNNT engine from
#12673 in **`openagents dictate --stream`**, with real partials during capture,
one guarded final after stop and cancellation while GPU work is pending.
The ordinary multilingual offline mode and its final-only contract are preserved.

## Delivered behavior

```sh
apps/openagents-cli/build.sh
apps/openagents-cli/build/openagents dictate --stream
apps/openagents-cli/build/openagents dictate --stream --events --seconds 10
```

The last two commands are physical microphone actions for the owner, not checks
run for this record. The original streaming model must already be installed at
`~/.cache/openagents/models/parakeet/nemotron-streaming-en-0.6b-ebe59e5a`, or
selected by `--model-dir` / `OPENAGENTS_STREAMING_MODEL`. Its
[manifest](../../../psionic/fixtures/parakeet-streaming/manifest.json) retains
the complete original-file hashes. Runtime never downloads a model.

The CLI executes a separate `openagents-dictate-stream` helper beside itself.
The shared capture controller retains readiness/consent, input selection,
resampling/VAD, 450 ms pre-roll, 300 ms minimum hold, 200 ms tail, the complete
60-second cap and current-ticket final speech admission. The new read-only
[`dictation-stream.coax`](../../../apps/shared/dictation-stream.coax) authority
copies only current-ticket, consented/active/ready held, trailing or frozen
capture into a bounded batch. The frozen-clip API is unchanged.

[`dictate-stream-rules.coax`](../../../apps/openagents-cli/dictate-stream-rules.coax)
owns one in-flight call, exact submission sequences, 2,560-sample batches,
960,000-sample bounds, current-ticket publication and revocation. A native worker
owns a copy of each submitted batch, calls the same resident streaming API and
copies its result back. It never borrows the controller or caller stack and
never prints. The main loop continues reading capture and polling controls.
The existing fixed-size clip holds backlog; no unbounded audio/job queue grows.

Partials are full provisional text snapshots: stderr in plain mode, or the new
[`openagents.dictation.stream.v1`](../../protocol/dictation-stream-v1.md) JSONL
projection. Its semantic schema starts in
[`stream.proto`](../../../proto/openagents/cli/dictation/v1/stream.proto).
Status and outcome events identify the explicit model/profile, ticket,
submission sequence and sample count. The prior `openagents.dictation.v1`
contract and terminal-composer helper are unchanged.

Stop drains the remaining admitted samples into the same encoder/RNNT state,
then submits one empty final push. Only capture's speech/ticket gate and an
eligible finalized model result can produce a final. Cancellation closes input,
revokes publication and ends the process without waiting for an in-flight GPU
call; its detached worker owns all buffers and cannot print a late result.
Existing process lifetime releases a still-running worker/model. This is not a
forensic-erasure or hung-driver latency claim.

Output uses nonblocking native write attempts, with retry/deadline and control
policy in Coax. A consumer that stops reading cannot accumulate an output
queue or block indefinitely: retries fail after 250 ms and still observe
cancellation. A failed last line can be incomplete and must be discarded.
Already published partials or a completely published final cannot be retracted.

The mode refuses CPU, macOS, `--json`, terms/vocabulary, code formatting and
alternate-language/model options before capture. Plain text and `--events`
are supported. The default multilingual model/provider is unchanged. This
English profile does not inherit TDT vocabulary rescoring or offline accuracy.

## Checks and actual observations

[current-check.log](current-check.log) records **14/14 passing cases through
the actual `openagents` executable**, which execs its adjacent streaming helper.
There is no transcript stand-in. Model open checks the original complete
checkpoint/tokenizer digests on every process.

| Case | Observed behavior |
| --- | --- |
| usage | Five incompatible option combinations refuse with exit 2 before model/capture effects. |
| missing-model / bad-model | Missing or wrong original files refuse with exit 6/1 before opening input. |
| speech | Three partials before stop; exactly one final `Wait, stop`; 44,800 submitted samples. |
| plain | Only `Wait, stop` and a newline on stdout; provisional snapshots on stderr. |
| silence | No partial/final, one no_speech and exit 9. |
| enter-stop | The parent sends Enter after receiving the first partial; exactly one nonempty final and fewer samples than the scheduled full recording. |
| cancel-escape / cancel-signal | The parent cancels after a real partial; one cancelled event, exit 130 and zero finals. |
| cancel-finalizing | SIGTERM after capture freezes, while work is pending; cancelled/130 and zero finals. |
| capture-format | Unsupported captured rate is an error with no final. |
| wait | Start/stop on a virtual timeline preserves 450 ms pre-roll: 52,000 samples and one final. |
| recorder | The real Linux spawn/pipe/F32-carry/resampling path uses a Coax stand-in for pw-record: ten actual partials, one final containing the public recording's complete sentence, and no surviving recorder process group. |
| backpressure | The parent leaves stdout undrained until the child exits. Real repeated-speech partials fill the pipe; the process exits 1 with no final, rather than hanging. |

Normal current Escape/SIGINT/SIGTERM control-to-exit observations are
**109 / 92 / 75 ms**; Enter-to-final/process-exit is 597 ms. They include
native process/GPU teardown, are finite checks on the shared RTX 4080 and are
not physical key/microphone/UI release measurements. Model startup was about
8.6 seconds. The current backpressure case ended in 45.05 seconds including
model startup and the original-weight inference needed to fill the pipe; this
is not a 45-second output retry. Its retry bound is the separate 250 ms policy.
The raw receipt has 113 complete partial events and zero finals; no incomplete
event was observed in this run. Interrupted larger events may be incomplete.
The harness does not decode partial counters for that undrained-output case.

The fixture cases use the existing recorded `wait-stop.wav`. The recorder
stand-in emits public LibriSpeech `6930-75918-0011.pcm` as paced mono 48 kHz F32
(three copies of each 16 kHz sample), then silence. It is explicitly installed
on an isolated PATH; a marker is required and the harness never falls back to a
system recorder. No actual microphone, visible browser/window, playback,
composer insertion, submit or network recognizer was used. Backpressure input
is a generated 60-second repetition of that same public PCM; the harness
regenerates it outside Git. Source hashes are in `qualification.json`.

This is consumer/ordering qualification on recorded inputs. Model tensor/token
parity, cache continuity, quality and fixed device storage retain their separate
[#12673 profile evidence](../incremental-asr-2026-11-03/README.md); unchanged
passing model corpora were not replayed merely to confirm them. No population
quality, new multilingual profile or physical microphone quality is claimed.

## Policy, native and protocol validation

- Flow/output policy: **78 mutants killed, zero survivors**. Initial four checks
  plus the selected output-bound check cover five groups, including the full
  phase/busy/ticket/final truth table. The final mutation baseline runs them all.
- Current live-copy policy: **25 mutants killed, zero survivors**; its 74-group
  controller/dependency baseline passes. The original clamp had a `>`/`>=`
  boundary mutation with the same result. It was replaced with a bounded copy
  loop rather than marking/exempting a survivor; the prior failure is retained.
- **ASan/UBSan speech and cancellation during finalization pass** with checked
  generated Coax CPU code, native effects and actual CUDA recognition. The
  cancellation check exits 130 with no final, 85 ms after SIGTERM. Leak checking
  is disabled for CUDA; this is host memory/UB instrumentation, not a CUDA
  device-memory sanitizer or new kernel qualification.
- Existing final-only CLI recorded-capture regression: **25/25 pass**. Existing
  published default text/JSON, formatting, capture errors and controls remain
  as previously qualified. No unchanged original-model corpus gate was rerun.
- Pinned protoc 29.3 and Python protobuf 5.29.3: all modules, published
  compatibility/migration and consumer profiles pass, including 30 conformance
  tests. Only the owned current descriptor changes. After rebase it also includes
  upstream combat schema additions, whose sources are unchanged by this task;
  the rebased protocol check passes all 29 owned schemas.
- Standard CLI build and the focused normal/sanitized helper builds pass. The
  development compiler is identified by its SHA-256; this is not a fresh full
  compiler bootstrap/release qualification.

The sanitizer environment is the same qualified CUDA setup as #12673:
`ASAN_OPTIONS=detect_leaks=0:protect_shadow_gap=0`,
`UBSAN_OPTIONS=halt_on_error=1`, and
`LD_LIBRARY_PATH=$HOME/cuda-13.2/lib:/run/opengl-driver/lib`.
Checked original-weight hashing adds about two minutes to sanitized startup;
that startup is excluded from the control-to-exit observation.

An initial recorder test setup failed because the stand-in executable had not
been copied into its isolated PATH. The actual CLI correctly returned program
missing/exit 8 without opening another device. Its log is retained separately;
installing the stand-in produced the passing current recorder case. Compiler
integration also caught and fixed unsupported string foreign/cast signatures
and the missing generated f32-view header before the native checks ran.

## Reproduction

Run from a worktree. Ordinary builds include all three CLI executables. Set
`MODEL` to the installed original streaming model directory.

```sh
apps/openagents-cli/build.sh
coax/bin/coax test apps/openagents-cli/dictate-stream-rules.coax --library
coax/bin/coax mutate apps/openagents-cli/dictate-stream-rules.coax --library --root-only
coax/bin/coax test apps/shared/dictation-stream.coax --library
coax/bin/coax mutate apps/shared/dictation-stream.coax --library --root-only
coax/web/tool.sh apps/openagents-cli/tests/dictate-stream-check.coax "$MODEL" speech
```

The default integration suite runs the fixture cases and backpressure; it skips
its recorder case unless the explicit stand-in directory is installed. Build
and install the Coax recorder fixture outside Git, then run the full suite:

```sh
coax/web/tool.sh apps/openagents-cli/tests/dictate-stream-recorder.coax
mkdir -p /tmp/oa-stream-recorder
COMPILER_SHA=$(sha256sum coax/bin/coax | cut -d ' ' -f 1)
cp "coax/web/build/tools/dictate-stream-recorder-$COMPILER_SHA" /tmp/oa-stream-recorder/pw-record
printf 'dictate-stream-recorder-v1' > /tmp/oa-stream-recorder/.coax-recorder
OPENAGENTS_DICTATE_TEST_RECORDER_DIR=/tmp/oa-stream-recorder \
  coax/web/tool.sh apps/openagents-cli/tests/dictate-stream-check.coax "$MODEL"
```

The harness keeps a 14-minute total deadline and accepts a case-name filter.
Use separate selected cases for sanitizers (their hashing startup is slower):

```sh
coax/web/tool.sh apps/openagents-cli/dictate-stream-build.coax \
  apps/openagents-cli/build/stream-sanitized --sanitize
cp apps/openagents-cli/build/openagents apps/openagents-cli/build/stream-sanitized/openagents
ASAN_OPTIONS=detect_leaks=0:protect_shadow_gap=0 UBSAN_OPTIONS=halt_on_error=1 \
LD_LIBRARY_PATH="$HOME/cuda-13.2/lib:/run/opengl-driver/lib" \
OPENAGENTS_DICTATE_TEST_CLI="$PWD/apps/openagents-cli/build/stream-sanitized/openagents" \
  coax/web/tool.sh apps/openagents-cli/tests/dictate-stream-check.coax "$MODEL" cancel-finalizing
```

Mac/Metal streaming, default composer/Verse partial UI adoption and vocabulary
or alternative model profiles remain engineering work. Only physical Linux
microphone acceptance is added to `NEEDS_OWNER.md` for this delivery.

The focused Psionic chapter/profile/adoption/dependency registry check passes.
The full book gate stops at the pre-existing stale `coax.wgsl.current` evidence
binding for `verse/everglade/gpu.bindings` ([log](book-check.log)). The required
[tracked-tool audit](tools-check.log) reports 12 unrelated files lacking a firm
reason; every new tool here is Coax. These repository-wide failures are retained
and are not counted as passes or repaired by refreshing unrelated proof hashes.

After rebase, [protocol compatibility](proto-check-rebased.log) still passes.
All tested source hashes remain identical. The shared owned descriptor was
regenerated from both this new CLI schema and upstream combat additions.
[Book](book-rebased.log) and [tool audit](tools-check-rebased.log) retain the
same unrelated repository-wide failures. Runnable normal/sanitized binaries
are preserved at `/tmp/openagents-dictate-stream-20261103/` after worktree cleanup.
