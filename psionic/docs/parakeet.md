# Local Parakeet TDT v3

Psionic reads NVIDIA's original `parakeet-tdt-0.6b-v3` safetensors checkpoint
and recognizes bounded utterances through its own Coax frontend, Metal
FastConformer encoder and CPU token-and-duration decoder. The model and
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
apps/openagents-cli/build/openagents transcribe clip.wav --backend parakeet --model-dir "$PARAKEET_MODEL" --json
```

The Coax build tool accepts an output directory and optional source root;
its shell entry only executes the tool. On macOS it compiles Coax-generated
Metal and links Metal/Foundation. Linux builds the explicit CPU backend.
GPU inference currently means Apple Metal; it refuses on an unsupported host
rather than falling back. Coax owns every kernel and launch sequence. No
handwritten kernels, MPS, Accelerate, cuBLAS or cuDNN execute production math.

The public CLI defaults to the cache path above; `OPENAGENTS_PARAKEET_MODEL`
or `--model-dir DIR` selects another copy of the same pinned model.
`--device cpu` explicitly selects the slower compensated CPU route.
Text and JSON are supported. Forced language, `--words`, `--prompt`, Whisper's
`--model`/`--threads`, and SRT/VTT requests are refused explicitly. The model
recognizes its language automatically; vocabulary rescoring is future work.
Whisper remains the default backend when `--backend parakeet` is absent.

## Keep an engine resident

[psionic-parakeet.h](../platform/psionic-parakeet.h) is independent of the
existing Whisper header and library. Link `libpsionic-parakeet.a`, `libm` and
pthread, plus Metal and Foundation on macOS.

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
60-second dictation by silently changing this limit. Segmentation and its
context policy belong to the dictation/streaming work.

A failed call leaves caller text and the previous successful inspection
snapshot unchanged. Token/frame and feature/encoded inspections copy into
caller storage; they do not expose a view into the next request's workspace.
Closing releases weights and GPU resources. An empty recognized transcript is
valid; silence admission/VAD is the caller's separate responsibility.

## Profiles and evidence

The resident frontend follows the pinned NeMo valid-length convention:
`floor(samples / 160)` columns participate in normalization, although centred
STFT physically stores one more. Valid columns are compacted before the
encoder. A recorded official-model probe proves that this preserves its valid
encoded outputs, including an eight-frame boundary.

The encoder uses tolerance-qualified f32 Metal math; the TDT decoder keeps
compensated binary64 sums and f32 storage. The legacy exact CPU teaching path
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
That proposal's microphone capture, opt-in UI, pre-roll/tail, vocabulary
rescoring and composer integration are separate work. This engine neither
records a microphone nor executes a transcript.
