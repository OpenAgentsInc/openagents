# Transcription: Whisper in Psionic

Status, 2026-11-01: Psionic's Whisper transcribes end to end on the CPU
(`openagents transcribe FILE --backend psionic`). With greedy decoding it
writes the same text and timestamps as whisper.cpp on our fixtures, and it
runs about 2× slower than whisper.cpp. Beam search, language detection and
the oracle harness come next. The default backend is still the interim
whisper.cpp one, because whisper.cpp's default beam search hears names
better. Tracking issue:
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

1. **Decode.** Any container to 16 kHz mono samples in [-1, 1]. Today
   ffmpeg does this as one program call. Its firm reason is 6 (no Coax
   decoder for compressed containers yet). A Coax WAV reader and
   resampler replace it for WAV input. The engine already has 16 kHz
   audio, because `verse/voice.coax` resamples the microphone.
2. **Log-mel** (`front.coax`). 30 s of zeros are appended to the audio. Each
   frame is a periodic Hann window of 400 samples with hop 160, centred with
   reflection; the 201-bin power spectrum is multiplied by 80 (128 for
   large-v3) Slaney mel filters with area normalization. Each value becomes
   `log10(max(e, 1e-10))`, clamped below at the file's maximum minus 8, then
   `(v + 4) / 4`. The last frame is dropped. The maximum is over the whole
   file, so `wf_max` makes a first pass and `wf_window` writes any 30 s
   window of 3,000 frames. Memory stays bounded for long files.
3. **Encoder** (`encoder.coax`, next). Two 1-D convolutions of width 3, the
   second with stride 2 (3,000 frames become 1,500), GELU after each, plus
   the stored sinusoidal positions. Then `n_audio_layer` pre-norm
   Transformer blocks: LayerNorm (epsilon 1e-5), multi-head self-attention
   with biases on the query, value and output (none on the key), a residual,
   LayerNorm, an MLP of width 4d with exact (erf) GELU, a residual, and a
   final LayerNorm.
4. **Decoder** (`decoder.coax`, next). Token embedding plus learned
   positions (448), `n_text_layer` blocks of causal self-attention with a key
   and value cache, cross-attention to the encoder output (its keys and
   values computed once per window), and the MLP. Logits are the final
   LayerNorm times the token embedding transposed.
5. **Search** (`search.coax`, next). Greedy first, then beam search (size 5)
   and the temperature fallback (0.0, 0.2, … 1.0 on a compression-ratio or
   log-probability failure), as whisper.cpp's `whisper_full` does.
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
The decoder is checked on all 65,536 bit patterns. q8_0 and q5 come next.

## Coax module layout

| File | Contents | State |
| --- | --- | --- |
| `psionic/lib/whisper/segments.coax` | segments; text, SRT, WebVTT, JSON writers | done, checked |
| `psionic/lib/whisper/ggml.coax` | the model file reader: hyperparameters, filters, vocabulary, tensors | done, checked |
| `psionic/lib/whisper/tokens.coax` | special ids, languages, timestamps, window prompts | done, checked |
| `psionic/lib/whisper/front.coax` | Whisper's log-mel on `spectral.coax`; a frame cache, or two passes in bounded memory | done, checked |
| `psionic/lib/whisper/engine.coax` | f16 weights, tensor names and sizes, the encoder, the decoder and its caches, greedy search with whisper.cpp's logit rules, seek and segments, and the `#export_c` entry points | runs end to end; greedy only |
| `psionic/platform/whisper-face.c` | the host: maps files, owns memory and threads, calls the exports (`psionic-whisper info`, `filters`, `transcribe`) | done |
| `psionic/platform/whisper-build.sh` | builds `psionic-whisper` into `psionic/build/whisper` | done |
| `psionic/lib/whisper/wav.coax` | WAV (PCM 16, float 32, extensible) to mono f32; a resampler | next (#12615) |
| `psionic/lib/whisper/search.coax` | beam search, temperature fallback, language detection, the previous-text prompt | next (#12619, #12620) |
| `psionic/tests/whisper_oracle.c` | the oracle: links libwhisper, writes reference logits and transcripts | next (#12621) |

Coax has no threads yet. The host owns them, as chapter 35's `cores.coax`
does. Each heavy kernel (products, attention, the convolution stem, mel
frames) is a foreign `cxwh_*` call. The host splits it into fixed ranges and
calls the matching `psionic_whisper_*_part` export once per range. Each
output element belongs to one range, so the transcript is byte-identical
for 1, 8 and 16 threads.

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

- `--backend psionic` runs `psionic-whisper`, which the CLI builds from its
  checkout on first use, and again when a source is newer. It decodes
  greedily, so `--language auto` is refused until language detection lands.
- `--backend whispercpp` is the interim default. It runs whisper.cpp's
  `whisper-cli`, and is labelled interim in the status line and the JSON.
  It is removed in the change that makes Psionic the default, once beam
  search matches (issue
  [#12623](https://github.com/OpenAgentsInc/openagents/issues/12623)).

## Engine API for in-game voice

The engine already turns microphone audio into turns. `verse/voice.coax`
resamples it to 16 kHz, detects speech and ends a turn. The voice host
(`verse/voice-host/speech.c`) then sends the turn's audio to
`speech_recognize`. Today that is Google's Chirp 3, or a stand-in
`PROVIDER_LOCAL` sentence.

Psionic becomes a third provider, `PROVIDER_PSIONIC` (#12622):

```c
// verse/voice-host/speech.c
bool speech_recognize(speech_provider p, const int16_t *pcm, size_t n, char *out, size_t cap, char *err, size_t errcap);
//   p == PROVIDER_PSIONIC: the turn's samples as f32, then psionic_whisper_transcribe,
//   with the model mapped and its weights loaded once at startup.
```

Exported from `psionic/lib/whisper/engine.coax` (all views; the host owns
memory):

```text
psionic_whisper_read(file: []u8, tensors: []WgTensor) -> WgModel                 // the file's directory
psionic_whisper_net(file, model, tensors, offsets: []int, ctx: int) -> WeNet     // names, sizes, the work layout
psionic_whisper_load(file, tensors, count, offsets, W: []f32) -> WeStatus        // weights as f32
psionic_whisper_transcribe(net, W, work, file, model, bank, audio: []f32, cache: []f32,
                           language: int, segs: []WsSegment, text: []u8) -> WeRun
psionic_whisper_format(segs, format, model, language, out: []u8) -> int
```

- Turns are at most 30 s, the voice engine's maximum turn length, so a turn
  is one window and needs no seek.
- For latency, a short turn encodes only the frames it has. This is
  whisper.cpp's `audio_ctx`: a 5 s turn uses 250 of the 1,500 positions. The
  parity tests cover both the full and the shortened context.
- Aethyr programs see the same thing as a Coax procedure. They do not call
  the C host.

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
| Speed, in game | under 300 ms for a 5 s turn with `base.en` on the CPU | the voice host's turn log |

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

## Checks

```sh
coax/bin/coax test psionic/lib/whisper/segments.coax --library
coax/bin/coax test psionic/lib/whisper/ggml.coax --library
coax/bin/coax test psionic/lib/whisper/tokens.coax --library
coax/bin/coax test psionic/lib/whisper/front.coax --library
coax/bin/coax test psionic/lib/whisper/engine.coax --library
coax/bin/coax test apps/openagents-cli/transcribe-rules.coax --library
psionic/platform/whisper-build.sh            # psionic/build/whisper/psionic-whisper
psionic/build/whisper/psionic-whisper filters ~/.cache/openagents/models/whisper/ggml-base.en.bin
psionic/build/whisper/psionic-whisper transcribe ~/.cache/openagents/models/whisper/ggml-base.en.bin \
  /opt/homebrew/share/whisper-cpp/jfk.wav --srt
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
| CLI: the Psionic backend; remove whisper.cpp | [#12623](https://github.com/OpenAgentsInc/openagents/issues/12623) |
