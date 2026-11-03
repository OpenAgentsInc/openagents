# Default Parakeet and complete offline files

This completes the software scope of #12666 and #12620. The default
`openagents transcribe FILE` selects pinned local Parakeet, not an implicit
Whisper model/download. Explicit legacy backend/model combinations retain their
meaning; Parakeet-only `--model-dir` on either legacy backend refuses before
build/fetch effects. The ordinary command supports text, JSON, SRT and WebVTT.

`psionic/lib/parakeet/file.coax` owns the exact window plan, strict WAV admission,
global-phase resampling, token-frame sample arithmetic and writers. The host
only maps/allocates, calls the resident engine, copies inspections, clears each
utterance and publishes output. No model arithmetic changed. The input mapping
has file-sized virtual extent; fixed inference scratch does not mean constant
whole-process virtual memory or OS page-cache use. Input backing must stay
immutable during recognition.

| Gate | Result |
| --- | --- |
| Default backend selection | Focused Coax selector and actual CLI transport/refusal checks pass. |
| Current complete CLI | Built with a focused current-source compiler; default public Count recording is exact, with pinned model/revision and versioned JSON. |
| Unsupported legacy model directory | Actual `psionic` and `whispercpp` commands both exit 2 with empty stdout, before a legacy effect. |
| File policy | Seven selected Coax groups cover exact short tails, bounds, strict/truncated WAVs, emission timing, formats/escaping and global 44.1/48 kHz phase parity. |
| Standard file-engine build | The changed `parakeet-build.coax` orchestration generates/compiles/links successfully. |
| Native file effects | Eleven independent model-ABI fixture cases pass normally and under ASan/UBSan: all writers, nine-window coverage, short-tail redistribution, silence, malformed/short input, second-window model failure and cancellation. |
| Before-publication cancellation | Injected second-window SIGINT: exit 130, zero stdout. |
| Blocked publication cancellation | The original libc `fwrite` attempt failed to exit after SIGINT. The corrected raw-write loop exits 130 after 1.70 ms in the named fixture, with an explicit partial-output diagnostic. This is not a general hard real-time bound. |
| Original weights | A 2,880,003-sample file (180 seconds plus three samples) completes nine requests with exact coverage and exact per-window NeMo tokens/frame IDs/text. |
| Speech crossing a real boundary | Two requests totaling 356,280 samples match pinned NeMo exactly. Joint normalized WER is 0/10 and CER 0/43; first/last words remain, with punctuation/case differences retained. |

The 180-second fixture repeats one public Count utterance with silence; it is
an exact-coverage/residency witness, not a natural three-minute quality corpus.
The separate crossing fixture cuts the spoken sentence at the actual model
boundary. These finite results do not imply population seam accuracy or
independent-window/full-context equivalence. Per-half NeMo score fields use the
whole sentence as their label and are **not** interpreted as half-aligned
accuracy; the joint comparison is in `original-weight/comparison.json`.

JSON is explicitly `openagents.parakeet.file.v1`. Single-window output retains
the earlier top-level feature/encoded/token/frame fields; every window includes
its exact sample range and raw global token sample positions. One nonempty
request becomes one subtitle cue. Its times are first/last TDT emission bounds,
not acoustic word alignment: the long padded window emits the final period at
frame 253, long after the voice ended. A frame is 1,280 samples (80 ms), and
conversion to milliseconds happens after adding the exact global sample offset.

The native CLI spools before publication. Inference/cancellation before that
point emits no transcript. Output I/O failure or cancellation during publication
can leave partial bytes and returns nonzero; an arbitrarily long transcript is
not one atomic POSIX write. The OpenAgents wrapper additionally refuses a child
that exceeds 14 minutes or its 4 MiB captured-output buffer. Explicit inspection
dumps are single-window effects and may leave partial inspection files on error.

## Reproduce the small gates

```shell
coax/bin/coax test apps/openagents-cli/parakeet-rules.coax --library
coax/bin/coax test psionic/lib/parakeet/file.coax --library --only 'Parakeet file'
coax/web/tool.sh psionic/tests/parakeet-file-inputs.coax psionic/fixtures/parakeet/6930-75918-0011.pcm /tmp/parakeet-file-inputs
psionic/platform/parakeet-build.sh
psionic/build/parakeet/psionic-parakeet transcribe /path/to/pinned-model /tmp/parakeet-file-inputs/count-180s-tail.wav --gpu --json
apps/openagents-cli/build.sh
apps/openagents-cli/build/openagents transcribe /tmp/parakeet-file-inputs/count-short.wav --json
```

The native fixture source and Coax checker are
`psionic/tests/parakeet-file-effects-fixture.c` and
`psionic/tests/parakeet-file-effects-check.coax`. They deliberately load no model.
The original-weight result uses the same pinned safetensors and independent
NeMo revision as chapter 153. Only four distinct new NeMo window profiles were
needed; their exact PCM recipes and token records are retained here. Metadata
retains hashes for raw reference arrays left outside Git; this is a fresh
subset record, not a claim that all original reference arrays are bundled.

The long/seam inference witnesses were obtained before the final output-only
cancellation correction. Their original producer and pre-correction C source
are preserved. Model arithmetic/input conversion stayed unchanged; normalized
runtime/fast/Metal emissions match the fresh standard build. The corrected
publication effect is qualified separately by native/sanitizer/fault checks and
a fresh short recording through the current CLI. Unchanged 29-recording model
qualification is reused from chapter 153 rather than rerun.

The compiler is a focused development build from current authored sources and
checked seeds, not a normal full-bootstrap/release qualification. Full mutation
and full Psionic release runs were not performed under the owner's focused-check
instruction. This does not complete microphone CLI/TUI capture, Linux inference,
vocabulary, latency evaluation or genuine streaming.
