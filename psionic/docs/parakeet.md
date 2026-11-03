# Local Parakeet TDT v3

Psionic reads NVIDIA's original `parakeet-tdt-0.6b-v3` safetensors checkpoint
and recognizes bounded utterances through its own Coax frontend, Metal or
CUDA FastConformer encoder and token-and-duration decoder (on the CPU, or on
the GPU with CUDA). The model and
vocabulary stay resident between calls. Python, NeMo and Torch are independent
verification tools; none is a runtime dependency.

[Chapter 153](../book/ch153-transcribe-with-parakeet.md) explains the arithmetic,
reference comparison and limits. [Current qualification and measurements](verification/parakeet-2026-11-02/optimization/README.md)
keep the exact row-parallel implementation's identities and checks. The
[original resident baseline](verification/parakeet-2026-11-02/README.md) and
its failed attempts remain intact for comparison.

## Get the pinned data

The weights are NVIDIA's, licensed CC BY 4.0, with attribution to
[NVIDIA's model](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3).
Keep them outside Git. These commands download data, never executable model
code, at revision `541d1f99c6b0c3cd0b11a95167540bb8edefd82b`:

```shell
PARAKEET_MODEL="$HOME/.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99"
mkdir -p "$PARAKEET_MODEL"
curl -fL --retry 3 -o "$PARAKEET_MODEL/model.safetensors" https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/resolve/541d1f99c6b0c3cd0b11a95167540bb8edefd82b/model.safetensors
curl -fL --retry 3 -o "$PARAKEET_MODEL/tokenizer.json" https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/resolve/541d1f99c6b0c3cd0b11a95167540bb8edefd82b/tokenizer.json
```

The loader checks the complete files on open:

| File | Identity |
| --- | --- |
| `model.safetensors` | 2,508,311,120 bytes; SHA-256 `3a2026366188c8c68598edbbff92f8d11590a08e0ae2e6775544e7b07d6a5e11` |
| `tokenizer.json` | SHA-256 `bd321b096832a3f270bd3b2a88823957920f1a5c5ada71114a26ea729d0cbe91` |

The engine checks `model.safetensors` as 38 pinned 64 MiB pieces
([model-chunks.coax](../lib/parakeet/model-chunks.coax)), one share per thread
on up to 16 threads, and admits it only when every piece matches; the piece
digests come from the copy with the whole-file digest above. This took the
open from 15-24 s of single-thread hashing to under a second of it
([record](../../docs/verification/dictation-eval-2026-11-02/README.md)).
The exact tensor names, shapes, dtypes, disjoint ranges and complete payload
coverage are checked in Coax before weights are published. The `.nemo` pickle
is never loaded by the recognizer. A wrong file is a refusal, not a request to
fetch or execute code.

## Build and run

From the repository root, with a built Coax compiler and the host C compiler:

```shell
psionic/platform/parakeet-build.sh
psionic/build/parakeet/psionic-parakeet transcribe "$PARAKEET_MODEL" clip.wav --gpu --json
apps/openagents-cli/build.sh
apps/openagents-cli/build/openagents transcribe clip.wav --model-dir "$PARAKEET_MODEL" --json
```

The Coax build tool accepts an output directory and optional source root;
its shell entry only executes the tool. On macOS it compiles Coax-generated
Metal and links Metal/Foundation. On Linux it emits Coax-generated CUDA C
(`coax cuda psionic/lib/parakeet/cuda.coax`) and links the CUDA effects; the
NVIDIA driver and NVRTC are opened with `dlopen` when the engine opens with
`--gpu`, so the build needs no CUDA toolkit. Both GPU profiles share
`fast.coax`'s schedule and `kernels.coax`. `--gpu` refuses on a host without
the backend rather than falling back. Coax owns every kernel and launch
sequence. No handwritten kernels, MPS, Accelerate, cuBLAS or cuDNN execute
production math.

### Supported hardware

| Host | `--gpu` | Status |
| --- | --- | --- |
| Apple silicon Mac | Metal | Qualified (`verification/parakeet-2026-11-02/`) |
| Linux, NVIDIA GPU with a CUDA 12/13 driver and NVRTC 13 | CUDA | Qualified on an RTX 4080 ([record](verification/parakeet-cuda-2026-11-02/README.md)): encoder and decoder on the GPU, real-time factor 0.0092 over 29 clips, 46 ms for a 3.2 s clip |
| Linux without an NVIDIA GPU, or without the driver or NVRTC | refused | No GPU path. `--cpu` works but takes 8 to 15 times the audio's length: correct, not interactive, and not a production profile |
| Intel Mac, Windows, AMD or Intel GPUs on Linux | none | Not supported |

The CUDA profile needs 2.4 GB of device memory for the encoder weights plus
the work arenas. NVRTC is found through `PSIONIC_NVRTC`, `$CUDA_HOME/lib`,
`~/cuda-13.2/lib` or the loader path.

The public CLI defaults to the cache path above; `OPENAGENTS_PARAKEET_MODEL`
or `--model-dir DIR` selects another copy of the same pinned model.
`--device cpu` explicitly selects the slower compensated CPU route.
The file CLI supports text, versioned JSON, SRT and WebVTT through exact bounded
independent windows. Forced language, `--words`, `--prompt` and Whisper's
`--model`/`--threads` are refused explicitly. The model
recognizes its language automatically. Vocabulary rescoring
(`psionic_parakeet_vocabulary`, below) is used by `openagents dictate --terms`,
not by the file CLI.
Parakeet is the default local transcription backend. `--backend parakeet`
is an explicit equivalent; `--backend psionic` retains legacy Whisper and
`--backend whispercpp` selects its independent oracle. Neither is a fallback.
`transcribe models` reports the pinned Parakeet path and file presence; full
hashes are checked when the model opens. Normal transcription never downloads
model data.

## Keep an engine resident

[psionic-parakeet.h](../platform/psionic-parakeet.h) is independent of the
existing Whisper header and library. Link `libpsionic-parakeet.a`, `libm` and
pthread, plus Metal and Foundation on macOS or `libdl` on Linux.

```jai
char error[512], text[8192];
psionic_parakeet *engine = psionic_parakeet_open(model_directory, 1, error, sizeof error);
if (engine != NULL) {
    int ok = psionic_parakeet_text(engine, samples, sample_count,
                                  text, sizeof text, error, sizeof error);
    // Copy successful text into the caller's draft; it is never submitted here.
    psionic_parakeet_close(engine);
}
```

The model is opened once per process. Calls serialize through the public API.
The CPU decoder uses up to eight participants in one persistent worker pool,
including the caller, with complete rows and unchanged compensated-sum order.
`psionic_parakeet_decoder_threads(engine)` reports the actual pool size. A
smaller available pool preserves the same arithmetic; closing joins its workers.
Inputs are mono f32 samples at 16 kHz in `[-1, 1]`, from 320 through 327,520
samples (20 ms through 20.47 seconds). The low-level model cannot supply
longer dictation by silently changing this limit. Coder's separate controller
limits capture to 60 seconds. The file CLI now uses the checked
[`file.coax`](../lib/parakeet/file.coax) policy for at most 4,096 windows.
A final remainder below 320 samples borrows from its preceding window. Audio
resampling keeps the whole file's phase and filter context across windows;
model state resets between requests. The output JSON schema is
`openagents.parakeet.file.v1`, with exact coverage and raw token emission times.
Each nonempty window is one coarse subtitle cue. These 80 ms emission bounds
are not word alignment and can include late punctuation after silence.

The file host maps the immutable input read-only and keeps fixed-size inference
scratch. Its virtual mapping grows with the file; constant total address space
or operating-system page-cache use is not claimed. Keep the input unchanged
while recognition runs. Output is privately spooled until every window succeeds;
SIGINT/model failure before publication emits no transcript. A broken output
pipe or cancellation after publication begins may leave partial bytes with a
nonzero exit. Explicit `--dump-prefix` inspection is single-window only and
can leave partial inspection files if writing fails; those files are separate
from transcript publication. [File/CLI proof](../../docs/verification/parakeet-file-cli-2026-11-02/README.md)
records the finite long-file and speech-seam qualification.

A failed call leaves caller text and the previous successful inspection
snapshot unchanged. Token/frame and feature/encoded inspections copy into
caller storage; they do not expose a view into the next request's workspace.
Closing releases weights and GPU resources. An empty recognized transcript is
valid; silence admission/VAD is the caller's separate responsibility.

`psionic_parakeet_clear(engine)` explicitly discards the live resident audio,
feature, decoder, text and inspection buffers after a call. It invalidates
last-result inspection while retaining the authenticated weights, vocabulary,
Metal pipelines or CUDA kernels and decoder workers. Coder uses it after both successful and
failed recognition. Caller-owned copies and in-flight audio remain their
owners' responsibility; this is not a forensic erasure claim.

## Profiles and evidence

The resident frontend follows the pinned NeMo valid-length convention:
`floor(samples / 160)` columns participate in normalization, although centred
STFT physically stores one more. Valid columns are compacted before the
encoder. A recorded official-model probe proves that this preserves its valid
encoded outputs, including an eight-frame boundary.

The encoder uses tolerance-qualified f32 Metal or CUDA math. The CPU TDT
decoder (the CPU and Metal profiles) keeps compensated binary64 sums and f32
storage; the CUDA decoder (`device-tdt.coax`) sums in f32 in a fixed order and
is qualified on NeMo's tokens. The legacy exact CPU teaching path
and its original four-clip observations remain in the chapter under their own
profile. The current profile's finite target/corpus qualification is in the
linked record; a finite smoke test does not establish population accuracy.

The independent reference is actual NVIDIA NeMo commit
`5f15875acd0885ed92eadfee874ae87b0529ffa2`, with strict bindings from the same
safe tensors. Four original English recordings and a frozen one-utterance
FLEURS slice for each of 25 supported languages provide complete feature,
encoder, token, frame-position and text comparisons. Multilingual WER/CER uses
the manifest's explicit Unicode normalization. It is not the legacy
ASCII-only `pk_score` metric and has no invented quality threshold.

The public info's recognition timer covers frontend, encoder, decoder and
checked text rendering. It excludes time waiting for the API lock, final
snapshot/output copies, microphone/key events and composer insertion. The
recording harness's whole-process time also includes load and artifact I/O.
This engine is the Parakeet foundation for
[the Coder voice proposal](../../docs/voice/2026-11-02-coder-voice-from-hex.md).
The [Mac dictation integration](../../docs/voice/coder-dictation.md) supplies
opt-in capture, pre-roll/tail, bounded segmentation and guarded composer drafts.
The [Verse voice host](../../docs/voice/dev.md#parakeet-local-recognition)
(`--recognizer parakeet`) keeps one engine resident for Verse and Aethyr's
hold to talk, with its own bounded segmentation and admission.
Real streaming remains separate. This engine itself neither records a
microphone nor executes a transcript.

## Vocabulary rescoring

`psionic_parakeet_vocabulary(engine, tsv, n, err, cap)` admits up to 64
terms, one line each: `TERM<TAB>SOUNDS|LIKE`, highest priority first. A
malformed file is refused whole, with its line named. Later calls do three
things after greedy decoding:

1. Find spans of one to three words, with no digit, whose sound key is
   within a third of a term's spelling or sounds_like
   ([vocab.coax](../lib/parakeet/vocab.coax), `#authority`).
2. Score the recognized tokens and the term's tokens with the transducer
   itself. The score is TDT's forward sum over the same encoded frames plus
   the next token ([rescore.coax](../lib/parakeet/rescore.coax)), computed on
   the GPU with the decoder's own kernels, or on the exact CPU rows.
3. Replace a span only when the term scores within 4 nats.

`psionic_parakeet_raw_text` returns the text before rescoring.
`psionic_parakeet_vocabulary_info` reports candidates, scores and
replacements. With no vocabulary nothing runs, and chapter 153's 29/29 parity
holds. Measurements are in the
[vocabulary record](../../docs/verification/dictation-vocab-2026-11-03/README.md).
The CPU path (Metal profile) works but has not been measured for latency.

## Incremental follow-up (#12673)

`psionic-nemotron-streaming-en-v1` is a separate, original-weight **English
RNNT** streaming profile, qualified on Linux CUDA (RTX 4080). It uses NVIDIA's
[Nemotron Speech Streaming English 0.6B](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b/tree/ebe59e5a817142986528bbbee5dba8db7b38ed50),
with a cache-aware Parakeet/FastConformer encoder. The multilingual offline
TDT v3 engine and chapter 153 retain their existing identities and evidence.

The [manifest](../fixtures/parakeet-streaming/manifest.json) pins the original
five files. The native loader authenticates the complete original F32 model
and tokenizer before exposing state; weights remain outside Git. Coax owns
preemphasis/STFT overlap, unnormalized 128-mel features, causal subsampling,
24 layers of bounded attention/convolution caches, the two-layer RNNT
prediction state and greedy emission. One lookahead token gives nominal
160 ms chunks: nine first feature frames, sixteen thereafter. Finalization
pads only the last feature chunk. It never reruns independent offline windows.

```sh
coax/web/tool.sh psionic/platform/incremental-build.coax
psionic/build/incremental/psionic-incremental "$STREAMING_MODEL" \
  psionic/fixtures/parakeet/6930-75918-0011.pcm
```

The reusable [C API](../platform/psionic-incremental.h) accepts arbitrary
finite mono 16 kHz F32 pushes (1..16,384 samples, or zero only for final),
with one strictly increasing nonzero local epoch and exact sequence numbers.
A stream lasts at most 60 seconds, with fixed token/text/device capacities.
A whole malformed push refuses before consuming audio. Reset/cancel discards
histories; backend failure terminates the epoch. Text snapshots are copied
and a cancelled/stale epoch cannot publish. Original backing files must stay
immutable while mapped. Close releases the engine and its resources.

The [Coax consumer](../examples/transcribe-incremental.coax) prints actual
append-only partials from a public recording and admits one final through
Aethyr's existing media-epoch/replay gate. Partials are display-only; barge-in
and cancellation revoke the native ticket and realm answer. This file-backed
consumer opens no microphone. Mac/TUI dictation and the default Verse voice
host retain their final-only paths; live microphone/UI partials and Metal
streaming effects require explicit adoption and qualification.

[Qualification and reproduction](../../docs/verification/incremental-asr-2026-11-03/README.md)
retain thirteen original-weight cases, exact reference tokens/text/positions,
per-chunk partials and tolerance-checked features/encoder/cache snapshots,
including a 30.46-second continuous stream and single-sample/ragged pushes.
The pinned Transformers oracle needs an explicit additive-to-boolean mask
representation adapter; its initial nonfinite/all-blank failures and corrected
reference are recorded. No unmodified-NeMo streaming equivalence is claimed.

The expanded frozen speech slice has 7 word errors / 205 reference words
(3.41% WER), 9 / 1,099 character errors (0.82% CER), first-word equality 6/7,
last-word equality 7/7, and no text on either no-speech fixture. The differing
first word is the contraction “I'm” versus “I am”. These are finite English
observations, with no invented population-quality threshold. Separate
resident-weight and bounded-transfer CUDA timings and allocations are in the
record; a busy shared GPU can be slower than real time. Microphone, pacing,
model startup, UI insertion and acoustic word alignment are separate.
