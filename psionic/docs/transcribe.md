# Transcription: local Parakeet by default

Current behavior, 2026-11-02: `openagents transcribe FILE` selects the pinned
original Parakeet TDT v3 engine, with automatic language and the local Metal
encoder/Coax decoder. It does not fetch a model, invoke a Whisper executable or
fall back after an error. Follow [the exact model setup](parakeet.md), then:

```shell
openagents transcribe clip.wav
openagents transcribe clip.wav --model-dir /path/to/pinned-parakeet --json
openagents transcribe models
```

The file path accepts text, JSON, SRT and WebVTT through exact bounded offline
windows, each 320..327,520 resampled mono 16 kHz samples. A final remainder
shorter than 320 samples is redistributed from the previous window. No sample
is silently discarded, duplicated or padded. There are at most 4,096 windows;
the OpenAgents wrapper additionally enforces 14 minutes and 4 MiB of output.
[File policy, native checks and reference evidence](../../docs/verification/parakeet-file-cli-2026-11-02/README.md)
cover a 180-second file and speech crossing an actual window boundary.

`--json` uses `openagents.parakeet.file.v1`, with a complete flag, each window's
exact sample range and token/frame/global-sample metadata. The earlier
single-window top-level metadata remains available. SRT/VTT have one nonempty
cue per window, bounded by its first/last token emission frame and actual audio
bounds. An encoded frame is 1,280 samples (80 ms); this is not forced word
alignment. Punctuation can emit much later than the spoken word. Windows use
independent full-attention requests; they are not causal streaming or equivalent
to full-context decoding.
`--backend psionic` and `--backend whispercpp` deliberately retain the earlier
Whisper contracts, including their explicit model/fetch and subtitle behavior.
They are compatibility/oracle paths, not hidden alternatives to Parakeet.

## Historical Whisper qualification (2026-11-01)

The following record describes the earlier default and its original tests.
Its numerical results and evidence retain that scope; it does not describe
what an unqualified transcription command selects today.

Status, 2026-11-01: Psionic's Whisper transcribes end to end
and is the CLI's default (`openagents transcribe FILE`), with beam search,
temperature fallback, language detection and a vocabulary bias
(`--words`). On the comparison corpus its word error rate is at or below
whisper.cpp's in total for `tiny.en` and `base.en`. On the CPU it is
faster than whisper.cpp: 0.92× its time on `tiny.en` and 0.83× on
`base.en`. On the GPU, where both the encoder and the decoder run on Metal,
it takes about 0.55× whisper.cpp's Metal time. The in-game voice host uses
it (`voice-host --provider psionic`). Psionic became the default CLI
backend once it was faster on both devices (#12623). Tracking issue:
[#12614](https://github.com/OpenAgentsInc/openagents/issues/12614).

Speech to text is a primitive of the stack. You call it the same way
everywhere:

- From a terminal: `openagents transcribe FILE`.
- From a Coax program, or from the engine for in-game voice: the Psionic
  library in `psionic/lib/whisper/`.

Agents no longer need a stray `whisper-cli` binary and a model they
downloaded somewhere. One command fetches the model into a known cache and
prints timestamped text, SRT, WebVTT or JSON.

## Model and licence

The model family is OpenAI's Whisper. The code and the weights are MIT
licensed (github.com/openai/whisper, `LICENSE`), so we can ship, cache and
redistribute them. We read whisper.cpp's ggml conversion of the weights
(github.com/ggerganov/whisper.cpp, MIT; files at
huggingface.co/ggerganov/whisper.cpp). Psionic and whisper.cpp then run the
same bytes, which makes whisper.cpp an exact oracle.

| Name | Parameters | File | Vocabulary | Use |
| --- | ---: | ---: | --- | --- |
| `tiny`, `tiny.en` | 39 M | 78 MB | 51,865 / 51,864 | tests, fastest |
| `base`, `base.en` | 74 M | 148 MB | 51,865 / 51,864 | the default |
| `small`, `small.en` | 244 M | 488 MB | | names, accents |
| `medium`, `medium.en` | 769 M | 1.5 GB | | best English |
| `large-v3` | 1,550 M | 3.1 GB | 51,866 | 100 languages |
| `large-v3-turbo` | 809 M | 1.6 GB | 51,866 | large quality, faster |

Pinned files (SHA-256), checked by the parity tests:

- `ggml-tiny.en.bin`: `921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f`
- `ggml-base.en.bin`: `a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002`

Small models mishear names. On our test clip, `tiny.en` writes "Psionic"
as "Sionic"; `base.en` gets it right. That is the model, not the engine. The
accuracy target is therefore relative: Psionic must match whisper.cpp on
the same model, not beat it.

Parakeet (chapters 151 to 154 of the Psionic book) stays the second
recognizer. It shares the spectral code (`psionic/lib/spectral.coax`), but
its weights are CC BY 4.0 and its decoder is a transducer. Whisper comes
first because the licence is simpler, one file format covers every size,
and whisper.cpp gives us an oracle on the same bytes.

## Pipeline

```text
file ──decode──▶ 16 kHz mono f32 ──log-mel──▶ 80 × 3000 per 30 s window
      ──encoder──▶ 1500 × d ──decoder + search──▶ tokens with timestamps
      ──segments──▶ text | SRT | WebVTT | JSON
```

1. **Decode.** Any container to 16 kHz mono samples in [-1, 1]. WAV
   files are read in Coax (`wav.coax`): PCM 8, 16, 24 and 32-bit and float
   32 and 64-bit, any channel count, mixed to mono, and resampled with a
   Kaiser-windowed sinc (beta 9, 16 zero crossings, cutoff 0.97 of
   Nyquist). That is within 2.6e-5 RMS of ffmpeg's default resampler.
   Other containers go through ffmpeg as one program call. Its firm reason
   is 6 (no Coax decoder for compressed containers yet). The engine already has 16 kHz
   audio, because `verse/voice.coax` resamples the microphone.
2. **Log-mel** (`front.coax`). 30 s of zeros are appended to the audio. Each
   frame is a periodic Hann window of 400 samples with hop 160, centred with
   reflection; the 201-bin power spectrum is multiplied by 80 (128 for
   large-v3) Slaney mel filters with area normalization. Each value becomes
   `log10(max(e, 1e-10))`, clamped below at the file's maximum minus 8, then
   `(v + 4) / 4`. The last frame is dropped. The maximum is over the whole
   file, so `wf_max` makes a first pass and `wf_window` writes any 30 s
   window of 3,000 frames. Memory stays bounded for long files.
3. **Encoder** (`engine.coax`). Two 1-D convolutions of width 3, the
   second with stride 2 (3,000 frames become 1,500), GELU after each, plus
   the stored sinusoidal positions. Then `n_audio_layer` pre-norm
   Transformer blocks: LayerNorm (epsilon 1e-5), multi-head self-attention
   with biases on the query, value and output (none on the key), a residual,
   LayerNorm, an MLP of width 4d with exact (erf) GELU, a residual, and a
   final LayerNorm.
4. **Decoder** (`engine.coax`). Token embedding plus learned
   positions (448), `n_text_layer` blocks of causal self-attention with a key
   and value cache, cross-attention to the encoder output (its keys and
   values computed once per window), and the MLP. Logits are the final
   LayerNorm times the token embedding transposed.
5. **Search** (`search.coax`). Beam search (size 5) and the temperature
   fallback (0.0, 0.2, … 1.0, best of 5 when sampling), as whisper.cpp's
   `whisper_full` does. A window fails on low average log-probability or
   on low token entropy, which is whisper.cpp's stand-in for the
   compression-ratio check. `--language auto` scores the language tokens
   after the start token. `--words A,B` adds a bonus to the first token of
   each name (2.0) and to its next tokens (3.0); a larger bonus made the
   models split ordinary words into names.
   whisper.cpp's logit rules are part of the search and must be matched
   exactly: suppress blank and the non-speech tokens at the start, keep
   timestamps paired, keep timestamps non-decreasing, cap the first timestamp
   at 1.0 s, and force a timestamp when the timestamps' total probability
   beats every text token.
6. **Timestamps and segments** (`tokens.coax`, `segments.coax`). A token at
   or above `beg` is `(t - beg) × 20 ms` from the window start. Text between
   two timestamps is a segment. The window then advances (seeks) to the
   last complete timestamp. `segments.coax` writes text, SRT, WebVTT and
   JSON for every backend.
7. **Tokenizer** (`tokens.coax`). GPT-2 byte-level BPE. whisper.cpp's file
   stores each token's bytes already decoded, so text is those bytes in
   order. The special token ids follow from the vocabulary size (51,864
   English-only, 51,865 multilingual, 51,866 large-v3).

Weights (`ggml.coax`). The file is whisper.cpp's legacy ggml format, not
GGUF: the magic, eleven hyperparameters, the mel filters, the vocabulary,
then each tensor's rank, name, type, sizes and data with no padding. The
reader checks every length against the mapped bytes and refuses anything
else with a typed status. The host maps the file; Coax only reads views.
f16 is decoded exactly to f32 once, at load, into the host's weight arena.
The decoder is checked on all 65,536 bit patterns. q8_0, q5_0, q5_1, q4_0
and q4_1 blocks are dequantized at load exactly as ggml's
`dequantize_row_*` does.

## Coax module layout

| File | Contents | State |
| --- | --- | --- |
| `psionic/lib/whisper/segments.coax` | segments; text, SRT, WebVTT, JSON writers | done, checked |
| `psionic/lib/whisper/ggml.coax` | the model file reader: hyperparameters, filters, vocabulary, tensors | done, checked |
| `psionic/lib/whisper/tokens.coax` | special ids, languages, timestamps, window prompts | done, checked |
| `psionic/lib/whisper/front.coax` | Whisper's log-mel on `spectral.coax`; a frame cache, or two passes in bounded memory | done, checked |
| `psionic/lib/whisper/engine.coax` | f16 and quantized weights, tensor names and sizes, the encoder, the decoder and its caches, whisper.cpp's logit rules, seek and segments, and the `#export_c` entry points | done, checked |
| `psionic/lib/whisper/search.coax` | beam search, temperature fallback, language detection, the prompt and the word bias | done, checked |
| `psionic/lib/whisper/score.coax` | word error rate and timestamp drift for the harness | done, checked |
| `psionic/lib/whisper/wav.coax` | WAV to mono f32; the resampler | done, checked |
| `psionic/lib/whisper/device.coax` | the Metal kernels (`coax metal-compute`): the encoder's projections and attention, and a decoder step's norms, beam products (f32 or f16 weights), split-key attention, residual adds and cache stores | done, checked |
| `psionic/platform/whisper-face.c` | the host: maps files, owns memory, threads and the GPU, calls the exports (`psionic-whisper info`, `filters`, `pcm`, `transcribe`); with `-DPSIONIC_WHISPER_LIBRARY`, the library `psionic-stt.h` declares | done |
| `psionic/platform/whisper-build.sh` | builds `psionic-whisper` and `libpsionic-whisper.a` into `psionic/build/whisper` | done |

Coax has no threads yet. The host owns them, as chapter 35's `cores.coax`
does. Each heavy kernel (products, attention, mel frames) is a foreign
`cxwh_*` call. The convolutions are products over unrolled frames. The
host cuts the call into chunks that every thread takes from one counter
and calls the matching `psionic_whisper_*_part` export once per chunk.
Each output element is computed by the same code whichever thread takes
it, so the transcript is byte-identical for 1, 8 and 16 threads. With
`--gpu`, the encoder's products and attention (calls of at least 64 rows)
run `device.coax`'s kernels on the shared weight and work buffers instead;
the decoder stays on the CPU, where a launch costs more than its work.

## Command line

```text
openagents transcribe FILE [--model tiny|base|small|medium|large-v3|large-v3-turbo]
                           [--language en|auto|CODE] [--text|--srt|--vtt|--json]
                           [--backend whispercpp|psionic] [--threads N]
openagents transcribe models
openagents transcribe fetch MODEL
```

- `FILE` is any audio or video file.
- The default model is `base`, and the default language is `en`. With
  English, the four smaller sizes use their `.en` files.
- Models are cached in `~/.cache/openagents/models/whisper/ggml-NAME.bin`
  (`OPENAGENTS_MODEL_CACHE` moves it) and fetched on first use. They are
  never stored in git.
- Text output is one segment per line. `--srt` and `--vtt` write
  subtitles, and `--json` writes one object with the model, the backend,
  the language, the whole text and every segment's start, end and text in
  seconds. stdout carries only the transcript; a status line goes to
  stderr.
- Code: `apps/openagents-cli/transcribe.coax` (effects) and
  `transcribe-rules.coax` (pure, checked).

**Backends.** Both read the same cached ggml file.

- `--backend psionic`, the default, runs `psionic-whisper`, which the CLI builds from its
  checkout on first use, and again when a source is newer. It reads WAV
  files itself and other files through ffmpeg, runs the encoder on the GPU
  when there is one, and takes `--language auto`, `--prompt TEXT` and
  `--words A,B,C`.
- `--backend whispercpp` runs whisper.cpp's `whisper-cli` on request.
  `openagents transcribe compare` uses it as the reference. Psionic became
  the default once it was faster on the CPU and the GPU with the same or
  lower word error rate, except one word on `tiny.en` on the GPU (issue
  [#12623](https://github.com/OpenAgentsInc/openagents/issues/12623)).

## Engine API for in-game voice

The engine already turns microphone audio into turns. `verse/voice.coax`
resamples it to 16 kHz, detects speech and ends a turn. The voice host
(`verse/voice-host/speech.c`) then sends the turn's audio to
`speech_recognize`. With `voice-host --provider psionic` that runs
Psionic's Whisper in the same process (#12622):

```c
// psionic/platform/psionic-stt.h, linked as libpsionic-whisper.a
psionic_stt *psionic_stt_open(const char *path, int threads, int gpu, char *err, size_t errcap);
int psionic_stt_text(psionic_stt *s, const float *samples, int64_t n, const char *words,
                     char *out, size_t cap, char *err, size_t errcap);
```

- The host maps the model once at startup (`--whisper-model`, default
  `~/.cache/openagents/models/whisper/ggml-base.en.bin`) and passes a word
  list (`--words`, default `Psionic,Verse,OpenAgents,Coax,Coder`).
- Calls run one at a time; the host's workers may call from any thread.
- The voice host re-recognizes a turn every second while it is spoken, so
  the final is usually ready when the turn ends. On base.en with the
  encoder on the GPU, each recognition of up to 4.3 s of audio takes
  184 to 240 ms, and the final adds 0 ms after the end of the turn. On 8
  CPU threads one recognition takes about 600 ms.
- Replies use the offline tone: Psionic does speech to text only.
- `verse/voice-host/test.sh --offline` runs three psionic cases headless
  when the model is cached: a committed fixture, a `say` clip naming
  Psionic and the Verse, and talking over the reply.

The engine exports the same steps to Coax programs, all on views the host
owns: `psionic_whisper_read`, `psionic_whisper_net`, `psionic_whisper_load`,
`psionic_whisper_transcribe` and `psionic_whisper_format` in
`psionic/lib/whisper/engine.coax`. Aethyr programs see them as Coax
procedures and do not call the C host.

## Targets and how we measure them

The oracle is whisper.cpp 1.9.2 (`brew install whisper-cpp`), on the same
ggml file and audio, with greedy decoding (`-bs 1 -bo 1`) for parity, and
on the CPU (`-ng`) for speed.

| What | Target | Check |
| --- | --- | --- |
| Mel filters | each of our binary64 Slaney weights within 1e-7 of the file's f32 filters | the face reads the file and compares |
| Log-mel | whisper.cpp's encoder fed our mel (`whisper_set_mel`) gives the same greedy tokens as its own mel, on every fixture | `whisper_oracle.c` |
| Encoder and decoder | first-token logits within 1e-2 of whisper.cpp's (`whisper_get_logits`) on every fixture | `whisper_oracle.c` writes them |
| Transcript | greedy tokens identical to whisper.cpp's on the fixture set; WER no more than 0.5 points above whisper.cpp at the same size | `openagents transcribe --backend psionic` against `--backend whispercpp` |
| Speed, CPU | within 2× of whisper.cpp's CPU time at the same thread count for `tiny.en` and `base.en`; real-time factor under 0.1 for `base.en` | the face's timer |
| Speed, in game | under 300 ms for a 5 s turn with `base.en` | the voice host's turn log |

whisper.cpp baseline on an Apple M5 Max (18 cores), 8 threads, CPU only
(`-ng`), measured 2026-11-01 on an 8.4 s clip of `say` speech:

| Model | Encode | Total | Real-time factor |
| --- | ---: | ---: | ---: |
| `tiny.en` | 208 ms | 386 ms | 0.046 |
| `base.en` | 370 ms | 604 ms | 0.072 |

With Metal, `base.en` takes 191 ms in total. GPUs in the cloud are off for
budget, so the first Psionic target is this machine's CPU. Metal follows,
using the backend from chapters 47 and 72.

Measured 2026-11-01, Apple M5 Max, CPU only. Both backends use the same
ggml files and greedy decoding (`whisper-cli -ng -bs 1 -bo 1`), and every
time includes loading the model:

| Model | Audio | Psionic text vs whisper.cpp | Psionic, 8 threads | whisper.cpp, 8 threads | Psionic, 16 threads | whisper.cpp, 16 threads |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| `tiny.en` | `jfk.wav`, 11.0 s | identical, timestamps too | 537 ms | 267 ms | 373 ms | |
| `base.en` | `jfk.wav`, 11.0 s | identical, timestamps too | 1,078 ms | 487 ms | 738 ms | 404 ms |
| `tiny.en` | `say` clip, 8.2 s | identical, timestamps too | 619 ms | 243 ms | | |
| `base.en` | `say` clip, 8.2 s | one comma differs; timestamps identical | 1,175 ms | 478 ms | | |
| `medium.en` | `say` clip, 8.2 s | identical, timestamps too; "Psionic" heard right | 12.7 s | 5.2 s | 8.6 s | 75.9 s |

- Mel filters: our binary64 Slaney filters are within 1.3e-9 of the f32
  filters stored in `ggml-tiny.en.bin` and `ggml-base.en.bin`
  (`psionic-whisper filters MODEL`).
- Word error rate: 0 for both backends on these fixtures, after removing
  punctuation. Both models write "Psionic" as "Sionic" with greedy
  decoding. whisper.cpp's default beam search gets it right with
  `base.en`, which is why beam search is next.
- Speed: Psionic takes 2.0 to 2.5× whisper.cpp's time at 8 threads, and
  1.8× at 16 for `base.en`. whisper.cpp 1.9.2 slows down badly at 16
  threads on `medium.en` (75.9 s); Psionic does not. The first run after a reboot also pays for reading the
  model from disk (up to 0.2 s). whisper.cpp uses Accelerate's matrix units; our products are
  plain Coax loops with bounds checks.

Fixtures are public domain or generated: whisper.cpp's `jfk.wav` (a
public-domain speech, 11 s) and clips made with macOS `say` from a fixed
script, including the names Psionic, Coax, Verse and Aethyr. WER uses the
standard word edit distance after lowercasing and removing punctuation.

## Comparison harness

`openagents transcribe compare` runs both backends on the same files and
models, and prints one markdown row per file and model (#12621):

```sh
openagents transcribe corpus ~/.cache/openagents/transcribe-corpus   # the generated clips and their references
openagents transcribe compare ~/.cache/openagents/transcribe-corpus ~/code/cakbags/speedrun-video/cut.wav \
  --models tiny.en,base.en [--diffs]
```

- Words: the reference's word count. A file's reference is its sibling
  `.txt`.
- WER: word edits from the reference, over its words, after lowercasing and
  dropping punctuation (`psionic/lib/whisper/score.coax`).
- Between: the same rate with whisper.cpp's transcript as the reference.
- Drift: for each Psionic segment, the distance from its start to the
  nearest whisper.cpp segment start; mean and largest.
- Time: wall time, including loading the model. Peak: the largest resident
  set size (`/usr/bin/time -l`).
- whisper.cpp runs with its defaults: beam 5, best of 5, temperature
  fallback. `--diffs` lists the word differences; it is left out for
  private audio.

The corpus:

- `jfk.wav`, from whisper.cpp's samples.
- `say` clips: the fox sentence with "Psionic", a names sentence (Psionic,
  Coax, Nostr, OpenAgents) at two speeds, and the Gettysburg Address (80 s,
  three windows), clean and mixed with pink noise at two levels.
- `cut.wav`: a 178 s talk recorded outdoors with street noise, scored
  against a hand-corrected transcript. It is private: only its numbers are
  published here.

Before beam search, 2026-11-01, Apple M5 Max, 8 threads. Psionic decoded
greedily on the CPU, and whisper.cpp ran on its default device, the GPU:

| File | Model | Words | Psionic WER | whisper.cpp WER | Between | Drift (mean / max) | Psionic | whisper.cpp | Psionic peak | whisper.cpp peak |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| gettysburg-loud-noise.wav | tiny.en | 268 | 7.09% | 6.72% | 4.10% | 928 / 1680 ms | 2350 ms | 930 ms | 278 MB | 298 MB |
| gettysburg-noise.wav | tiny.en | 268 | 4.10% | 4.10% | 2.21% | 2785 / 9180 ms | 2501 ms | 663 ms | 278 MB | 290 MB |
| gettysburg.wav | tiny.en | 268 | 1.12% | 1.12% | 0.75% | 122 / 1160 ms | 2194 ms | 646 ms | 278 MB | 299 MB |
| jfk.wav | tiny.en | 22 | 0.00% | 0.00% | 0.00% | 20 / 40 ms | 549 ms | 185 ms | 268 MB | 238 MB |
| say-fox.wav | tiny.en | 23 | 4.35% | 4.35% | 0.00% | 1340 / 2680 ms | 542 ms | 199 ms | 268 MB | 237 MB |
| say-names-fast.wav | tiny.en | 28 | 42.86% | 32.14% | 23.33% | 60 / 120 ms | 599 ms | 211 ms | 268 MB | 237 MB |
| say-names.wav | tiny.en | 28 | 32.14% | 32.14% | 6.67% | 1535 / 2680 ms | 630 ms | 217 ms | 268 MB | 238 MB |
| cut.wav | tiny.en | 395 | 7.09% | 7.09% | 6.19% | 2653 / 7760 ms | 4361 ms | 1143 ms | 290 MB | 310 MB |
| **all** | tiny.en | 1300 | 6.38% | 6.08% |  |  | 13726 ms | 4194 ms | | |
| gettysburg-loud-noise.wav | base.en | 268 | 2.24% | 2.99% | 2.60% | 188 / 880 ms | 4261 ms | 813 ms | 506 MB | 407 MB |
| gettysburg-noise.wav | base.en | 268 | 1.87% | 1.49% | 1.12% | 876 / 2320 ms | 4154 ms | 772 ms | 506 MB | 407 MB |
| gettysburg.wav | base.en | 268 | 1.12% | 0.75% | 0.37% | 301 / 1320 ms | 4285 ms | 782 ms | 506 MB | 407 MB |
| jfk.wav | base.en | 22 | 0.00% | 0.00% | 0.00% | 0 / 0 ms | 1058 ms | 217 ms | 495 MB | 346 MB |
| say-fox.wav | base.en | 23 | 0.00% | 0.00% | 0.00% | 0 / 0 ms | 1089 ms | 232 ms | 495 MB | 346 MB |
| say-names-fast.wav | base.en | 28 | 25.00% | 28.57% | 3.33% | 3140 / 5960 ms | 1178 ms | 243 ms | 495 MB | 346 MB |
| say-names.wav | base.en | 28 | 28.57% | 28.57% | 0.00% | 7 / 20 ms | 1160 ms | 262 ms | 495 MB | 346 MB |
| cut.wav | base.en | 395 | 6.84% | 7.09% | 4.67% | 1319 / 3420 ms | 8693 ms | 1226 ms | 517 MB | 421 MB |
| **all** | base.en | 1300 | 4.31% | 4.46% |  |  | 25878 ms | 4547 ms | | |

After, 2026-11-01, Apple M5 Max, 8 threads, both on the CPU
(`--device cpu`, which runs `whisper-cli -ng`):

| File | Model | Words | Psionic WER | whisper.cpp WER | Between | Drift (mean / max) | Psionic | whisper.cpp | Psionic peak | whisper.cpp peak |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| gettysburg-loud-noise.wav | tiny.en | 268 | 6.72% | 6.72% | 0.75% | 140 / 800 ms | 2706 ms | 2191 ms | 308 MB | 406 MB |
| gettysburg-noise.wav | tiny.en | 268 | 4.48% | 5.22% | 3.69% | 391 / 1240 ms | 2507 ms | 1850 ms | 306 MB | 406 MB |
| gettysburg.wav | tiny.en | 268 | 1.12% | 0.75% | 0.37% | 39 / 200 ms | 2173 ms | 1833 ms | 307 MB | 406 MB |
| jfk.wav | tiny.en | 22 | 0.00% | 0.00% | 0.00% | 4000 / 8000 ms | 412 ms | 403 ms | 278 MB | 224 MB |
| say-fox.wav | tiny.en | 23 | 4.35% | 4.35% | 0.00% | 0 / 0 ms | 417 ms | 398 ms | 278 MB | 225 MB |
| say-names-fast.wav | tiny.en | 28 | 39.29% | 42.86% | 13.33% | 330 / 660 ms | 479 ms | 445 ms | 279 MB | 225 MB |
| say-names.wav | tiny.en | 28 | 32.14% | 32.14% | 6.67% | 10 / 20 ms | 486 ms | 454 ms | 279 MB | 225 MB |
| cut.wav | tiny.en | 395 | 6.58% | 6.58% | 4.19% | 457 / 3640 ms | 5061 ms | 3726 ms | 319 MB | 426 MB |
| **all** | tiny.en | 1300 | 6.15% | 6.31% |  |  | 14241 ms | 11300 ms | | |
| gettysburg-loud-noise.wav | base.en | 268 | 3.36% | 2.99% | 0.74% | 96 / 1720 ms | 3804 ms | 3251 ms | 557 MB | 542 MB |
| gettysburg-noise.wav | base.en | 268 | 1.12% | 1.49% | 0.37% | 445 / 3680 ms | 3783 ms | 3162 ms | 556 MB | 541 MB |
| gettysburg.wav | base.en | 268 | 0.75% | 0.75% | 0.00% | 7 / 120 ms | 3793 ms | 3179 ms | 554 MB | 541 MB |
| jfk.wav | base.en | 22 | 0.00% | 0.00% | 0.00% | 0 / 0 ms | 810 ms | 651 ms | 507 MB | 337 MB |
| say-fox.wav | base.en | 23 | 0.00% | 0.00% | 0.00% | 0 / 0 ms | 834 ms | 681 ms | 506 MB | 335 MB |
| say-names-fast.wav | base.en | 28 | 28.57% | 32.14% | 3.33% | 100 / 200 ms | 895 ms | 720 ms | 508 MB | 338 MB |
| say-names.wav | base.en | 28 | 28.57% | 28.57% | 0.00% | 7 / 20 ms | 915 ms | 767 ms | 509 MB | 335 MB |
| cut.wav | base.en | 395 | 6.08% | 6.33% | 4.20% | 1432 / 3720 ms | 7585 ms | 5964 ms | 567 MB | 560 MB |
| **all** | base.en | 1300 | 4.15% | 4.31% |  |  | 22419 ms | 18375 ms | | |

After, both on the GPU (`--device gpu`: Psionic's encoder on Metal,
whisper.cpp's default Metal build):

| File | Model | Words | Psionic WER | whisper.cpp WER | Between | Drift (mean / max) | Psionic | whisper.cpp | Psionic peak | whisper.cpp peak |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| gettysburg-loud-noise.wav | tiny.en | 268 | 6.72% | 6.72% | 0.75% | 140 / 800 ms | 1695 ms | 678 ms | 377 MB | 295 MB |
| gettysburg-noise.wav | tiny.en | 268 | 4.48% | 4.10% | 2.21% | 1771 / 10240 ms | 1836 ms | 662 ms | 377 MB | 290 MB |
| gettysburg.wav | tiny.en | 268 | 1.12% | 1.12% | 0.00% | 12 / 120 ms | 1654 ms | 649 ms | 377 MB | 296 MB |
| jfk.wav | tiny.en | 22 | 0.00% | 0.00% | 0.00% | 20 / 40 ms | 251 ms | 198 ms | 369 MB | 238 MB |
| say-fox.wav | tiny.en | 23 | 4.35% | 4.35% | 0.00% | 0 / 0 ms | 268 ms | 211 ms | 368 MB | 243 MB |
| say-names-fast.wav | tiny.en | 28 | 39.29% | 32.14% | 13.33% | 190 / 380 ms | 325 ms | 214 ms | 368 MB | 238 MB |
| say-names.wav | tiny.en | 28 | 32.14% | 32.14% | 6.67% | 10 / 20 ms | 338 ms | 221 ms | 369 MB | 237 MB |
| cut.wav | tiny.en | 395 | 6.58% | 7.09% | 4.46% | 2435 / 8360 ms | 3798 ms | 1176 ms | 389 MB | 316 MB |
| **all** | tiny.en | 1300 | 6.15% | 6.08% |  |  | 10165 ms | 4009 ms | | |
| gettysburg-loud-noise.wav | base.en | 268 | 3.36% | 2.99% | 0.74% | 96 / 1720 ms | 2347 ms | 831 ms | 648 MB | 408 MB |
| gettysburg-noise.wav | base.en | 268 | 1.12% | 1.49% | 0.37% | 445 / 3680 ms | 2326 ms | 781 ms | 648 MB | 407 MB |
| gettysburg.wav | base.en | 268 | 0.75% | 0.75% | 0.00% | 323 / 1320 ms | 2317 ms | 786 ms | 648 MB | 407 MB |
| jfk.wav | base.en | 22 | 0.00% | 0.00% | 0.00% | 0 / 0 ms | 381 ms | 220 ms | 639 MB | 347 MB |
| say-fox.wav | base.en | 23 | 0.00% | 0.00% | 0.00% | 10 / 20 ms | 379 ms | 219 ms | 639 MB | 346 MB |
| say-names-fast.wav | base.en | 28 | 28.57% | 28.57% | 0.00% | 2040 / 4080 ms | 441 ms | 236 ms | 639 MB | 346 MB |
| say-names.wav | base.en | 28 | 28.57% | 28.57% | 0.00% | 7 / 20 ms | 485 ms | 251 ms | 639 MB | 346 MB |
| cut.wav | base.en | 395 | 6.08% | 7.09% | 4.42% | 1246 / 3840 ms | 4043 ms | 1171 ms | 660 MB | 421 MB |
| **all** | base.en | 1300 | 4.15% | 4.46% |  |  | 12719 ms | 4495 ms | | |

After the Coax compiler work, 2026-11-01, totals over the same corpus
(`fma` and loop versioning in Coax; ggml's four-lane dot products in the
CPU kernels; each decoder step as one Metal batch of Coax kernels):

| Device | Model | Psionic WER | whisper.cpp WER | Psionic | whisper.cpp |
| --- | --- | ---: | ---: | ---: | ---: |
| CPU | `tiny.en` | 6.15% | 6.31% | 10778 ms | 11739 ms |
| CPU | `base.en` | 4.15% | 4.31% | 15507 ms | 18764 ms |
| GPU | `tiny.en` | 6.15% | 6.08% | 13102 ms | 6643 ms |
| GPU | `base.en` | 4.15% | 4.46% | 15605 ms | 7542 ms |

The CPU rows ran on a quiet machine. The GPU rows ran while other jobs
held the CPU (load average about 50), which slows both backends; on a
quiet machine Psionic's `base.en` run of `gettysburg.wav` takes 1.5 s
(2.3 s before the decoder moved to the GPU).

After the GPU decoder work, 2026-11-01 (a 32-bit integer body for Coax
Metal kernels whose values fit, row views in the device kernels, the beam
rules and top k in less group memory with a parallel join), both on the
GPU, totals over the same corpus. Other jobs held the CPU (load average
70 to 150), which slows both backends:

| Device | Model | Psionic WER | whisper.cpp WER | Psionic | whisper.cpp |
| --- | --- | ---: | ---: | ---: | ---: |
| GPU | `tiny.en` | 6.15% | 6.08% | 5352 ms | 9731 ms |
| GPU | `base.en` | 4.15% | 4.46% | 6391 ms | 11585 ms |

Psionic takes 0.55× whisper.cpp's time on both models. A `base.en` decoder
step takes about 1.5 ms of GPU time, down from about 2.1 ms. Psionic's
word error rate is the same on both devices.

What the numbers say:

- Accuracy. In total, Psionic's word error rate is below whisper.cpp's in
  three of the four runs: 6.15% against 6.31% (`tiny.en`, CPU), 4.15%
  against 4.31% (`base.en`, CPU) and 4.15% against 4.46% (`base.en`,
  GPU). On `tiny.en` on the GPU it is 6.15% against 6.08%. Psionic gives
  the same transcript on both devices; whisper.cpp's Metal run scores
  differently from its own CPU run. Per file, Psionic is behind by one
  word on `gettysburg.wav` (`tiny.en`, CPU) and on
  `gettysburg-loud-noise.wav` (`base.en`, both devices), and on
  `gettysburg-noise.wav` and `say-names-fast.wav` against whisper.cpp's
  Metal run with `tiny.en`. It ties or wins on every other file.
- Names. The names clips score 28 to 39% for both backends because the
  models do not know Psionic, Coax, Nostr or OpenAgents. With
  `--words Psionic,Coax,Nostr,OpenAgents,Coder`, Psionic spells every
  name right on `tiny.en` and `base.en`. `base.en` hears "Psionic" right
  with beam search even without the list.
- Speed on the CPU. Psionic takes 0.92× whisper.cpp's time on `tiny.en`
  and 0.83× on `base.en` (1.26× and 1.22× before the compiler work). Coax
  gained an explicit `fma` and, at native optimization 1 or more, loops
  whose indexes are affine check each range once and run without
  per-element checks, so the four-lane kernels compile to NEON fused
  multiply-adds.
- Speed on the GPU, before the GPU decoder work. Psionic took about 2×
  whisper.cpp's time. A decoder
  step is one command buffer of about 130 launches and takes about 2.1 ms
  on `base.en`; the beam products are most of it. Each product launch is
  short, so its fixed cost (group start, the partial sums in threadgroup
  memory, the barrier) outweighs its reads. Next: fuse the query, key and
  value products, encode independent launches concurrently, and move the
  log-softmax and top-k of each beam onto the GPU.
- Memory. On the CPU, for files longer than one window, Psionic's peak is
  about 100 MB below whisper.cpp's on `tiny.en` and within 15 MB on
  `base.en`. On short files it is 50 to 170 MB higher, because Psionic
  keeps its weights as f32. On the GPU, Psionic's peak is 80 to 290 MB
  higher, because the attention scores also get a buffer of their own.


## Original Parakeet v3

Whisper remains the default. Select `--backend parakeet` for Psionic's own
bounded Parakeet TDT v3 implementation: CPU frontend, Metal FastConformer,
CPU decoder, with model and vocabulary held by a resident native engine.
The [Parakeet guide](parakeet.md) gives pinned downloads, the static C API,
limits and [chapter 153](../book/ch153-transcribe-with-parakeet.md).

```shell
openagents transcribe clip.wav --backend parakeet --model-dir /path/to/parakeet --json
```

Text/JSON and automatic language are supported; forced language, prompt/word
bias and subtitles are refused. The current bound is 320..327520 samples at
16 kHz; long dictation and streaming require a separate policy. Explicit GPU
failure never silently switches engines. All 29 frozen recordings match actual
NeMo token IDs, frame IDs and text, with complete intermediate tensor tolerance
checks and 25-language smoke scores. The [measurement table](verification/parakeet-2026-11-02/reference/product-quality-latency.md)
reports offline timing separately from the Whisper measurements above.

## Checks

```sh
coax/bin/coax test psionic/lib/whisper/segments.coax --library
coax/bin/coax test psionic/lib/whisper/ggml.coax --library
coax/bin/coax test psionic/lib/whisper/tokens.coax --library
coax/bin/coax test psionic/lib/whisper/front.coax --library
coax/bin/coax test psionic/lib/whisper/engine.coax --library
coax/bin/coax test psionic/lib/whisper/score.coax --library
coax/bin/coax test psionic/lib/whisper/search.coax --library
coax/bin/coax test psionic/lib/whisper/wav.coax --library
coax/bin/coax test psionic/lib/whisper/device.coax --library
coax/bin/coax test apps/openagents-cli/transcribe-rules.coax --library
psionic/platform/whisper-build.sh            # psionic/build/whisper/psionic-whisper
psionic/build/whisper/psionic-whisper filters ~/.cache/openagents/models/whisper/ggml-base.en.bin
psionic/build/whisper/psionic-whisper transcribe ~/.cache/openagents/models/whisper/ggml-base.en.bin \
  /opt/homebrew/share/whisper-cpp/jfk.wav --srt
verse/voice-host/test.sh --offline           # includes the psionic provider
```

## Issues

The work is in the openagents tracker and as tasks in
`apps/openagents-web/projects/psionic/work/transcribe.json`:

| Step | Issue |
| --- | --- |
| Tracking | [#12614](https://github.com/OpenAgentsInc/openagents/issues/12614) |
| Audio decode: WAV and a resampler in Coax | [#12615](https://github.com/OpenAgentsInc/openagents/issues/12615) |
| Log-mel parity with whisper.cpp | [#12616](https://github.com/OpenAgentsInc/openagents/issues/12616) |
| Weights: ggml loading, f16, then q8_0 | [#12617](https://github.com/OpenAgentsInc/openagents/issues/12617) |
| Encoder | [#12618](https://github.com/OpenAgentsInc/openagents/issues/12618) |
| Decoder, greedy and beam search | [#12619](https://github.com/OpenAgentsInc/openagents/issues/12619) |
| Timestamps, seek and SRT | [#12620](https://github.com/OpenAgentsInc/openagents/issues/12620) |
| Parity tests against whisper.cpp | [#12621](https://github.com/OpenAgentsInc/openagents/issues/12621) |
| Engine binding for in-game voice | [#12622](https://github.com/OpenAgentsInc/openagents/issues/12622) |
| CLI: Psionic the default backend | [#12623](https://github.com/OpenAgentsInc/openagents/issues/12623) |
