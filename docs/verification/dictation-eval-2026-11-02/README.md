# Parakeet dictation: release latency, boundaries and accuracy — 2026-11-02

GitHub #12665 (part of #12614). This record measures local Parakeet dictation
on Linux with the CUDA profile from #12672 (67aadf9e2c): how long the text
takes after you stop, whether the first and last words survive the key
timing, how accurate it is on a frozen public corpus, what silence and noise
do, and what the 60-second segmentation costs at its seams. It sets gates
that fail when crossed, and records two optimizations with before and after.

Every dictation ran through the real `openagents dictate` code with recorded
audio. No microphone was opened, no sound was played, and no one's own voice
was used. The Mac (Metal) side and the Coder composer are owner measurements
(`NEEDS_OWNER.md`).

## Summary

- **Release to text (Linux CLI, RTX 4080):** after you let go, the controller
  waits 200 ms for the tail, then the text is ready in 82-120 ms (p50) and
  302-467 ms (p99) for one-call dictations, depending on machine load. So key
  release to final text is about 280-320 ms at p50. Measured back to back
  under the same load, this change took stop-to-text from 186 ms (p50) and
  642 ms (p99) to 120 ms and 467 ms.
- **Model open:** 19.1 s before and 9.0 s after (p50, new process, back to
  back at load average about 40); 3.5 s after at load 20, against 27-41 s
  before at load 45-55. Every byte of the 2.5 GB model still checked against pinned
  SHA-256 digests.
- **Accuracy:** 3.23% WER on 40 LibriSpeech test-clean speakers plus chapter
  153's four clips, 3.78% on 33 test-other speakers, 3.69% at 10 dB SNR and
  10.3% at 0 dB. Synthetic code-term speech: 27.6% WER and 59 of 80 terms.
- **Boundaries:** no last word was dropped when releasing 0 to +200 ms after
  speech ends, and no first word when pressing up to +300 ms after speech
  starts (one alignment artifact, below). Releasing 300 ms early keeps the
  last word exactly in 88% of clips.
- **No speech:** 1 of 21 held no-speech dictations produced text ("Uh" from
  white noise at -40 dBFS).
- **Seams:** a hard cut at 20.47 s can make Parakeet drop most or all of the
  next segment. One 31 s dictation lost its whole second segment (10.7 s of
  speech). This is the largest quality defect found; see "Seams".
- **Optimizations:** (1) the clear after each recognition wiped 148 MB a byte
  at a time (70-350 ms per call, on the release path); it now uses `memset`
  kept by a compiler barrier. (2) The model's SHA-256 ran on one thread; the
  file is now checked as 38 pinned 64 MiB pieces on up to 16 threads.
  Transcripts are byte-identical before and after on all 202 clips and 6
  long dictations, and chapter 153's parity holds (29/29 NeMo clips exact).

## Results

The quality, boundary and noise tables come from the first run (before the
optimizations); the optimizations change no arithmetic, and the transcripts
of all 202 clips and the 6 long dictations are byte-identical before and
after, so they were not rerun. Raw records: [raw/](raw/) (`final/` scored
below; `before2/` and `after2/` the back-to-back timing pair). Each line is
one dictation: kind, clip, variant, press and release offsets, status,
samples, segments, stop-to-text µs, calls, engine µs, encoder µs, decoder
µs, speech onset and offset ms, listen µs, text, segment texts, clear µs.

### Quality (held 300 ms before speech to 200 ms after; GPU)

| Set | Clips | Words | WER % | CER % | Exact clips |
| --- | ---: | ---: | ---: | ---: | ---: |
| LibriSpeech test-clean (40 speakers) + ch.153 (4) | 44 | 928 | 3.23 | 2.22 | 29 |
| LibriSpeech test-other (33 speakers) | 33 | 661 | 3.78 | 1.48 | 20 |
| short utterances (<= 3 words) | 8 | 20 | 15.00 | 5.34 | 6 |
| FLEURS en (1) | 1 | 19 | 5.26 | 1.01 | 0 |
| numbers (LibriSpeech, tagged) | 26 | 783 | 4.09 | 3.75 | 16 |
| synthetic code terms (espeak-ng, 2 voices) | 40 | 322 | 27.64 | 20.36 | 8 |
| synthetic short (repo fixtures) | 4 | 31 | 0.00 | 0.00 | 4 |
| seam rows, each alone | 48 | 1163 | 2.67 | 1.43 | 32 |

Synthetic term recall: 59 of 80 terms spelled exactly (normalized).

### Speech in white noise (10 test-clean clips)

| Set | Clips | Words | WER % | CER % | Exact clips |
| --- | ---: | ---: | ---: | ---: | ---: |
| SNR 10 dB | 10 | 271 | 3.69 | 1.70 | 4 |
| SNR 0 dB | 10 | 271 | 10.33 | 5.23 | 1 |

### No speech (3 s held; 7 signals x 3 seeds)

Through the controller: 21 dictations, 12 reached the model, 1 produced text. Direct model calls without admission: 0 of 21 produced text. white-40dBFS#3: "Uh"; 

### Release offsets (pressed 300 ms before speech)

| Release vs end of speech | Clips | Last word exact | % | Last word dropped | No speech |
| ---: | ---: | ---: | ---: | ---: | ---: |
| -300 ms | 112 | 99 | 88.39 | 0 | 0 |
| -200 ms | 112 | 110 | 98.21 | 0 | 0 |
| -100 ms | 112 | 111 | 99.11 | 0 | 0 |
| 0 ms | 112 | 111 | 99.11 | 0 | 0 |
| 100 ms | 112 | 109 | 97.32 | 0 | 0 |
| 200 ms | 112 | 112 | 100.00 | 0 | 0 |

### Late presses (released at the end of speech)

| Press vs start of speech | Clips | First word exact | % | First word dropped | No speech |
| ---: | ---: | ---: | ---: | ---: | ---: |
| +0 ms | 118 | 116 | 98.31 | 0 | 0 |
| +100 ms | 118 | 116 | 98.31 | 0 | 0 |
| +200 ms | 118 | 117 | 99.15 | 0 | 0 |
| +300 ms | 118 | 115 | 97.46 | 1 | 0 |
| +450 ms | 118 | 112 | 94.92 | 4 | 1 |
| +600 ms | 118 | 76 | 64.41 | 30 | 3 |

Words lost inside the supported timing (release 0..+200 ms, press up to +300 ms):

```
changed release +100 ms 4507-16021-0000: "CHAPTER IRIGING"
changed release +0 ms s01-en-us: "Open the file voice.cox and run the text."
changed release +100 ms s01-en-us: "Open the file voice.cox and run the text."
changed release +100 ms s02-en-gb-scotland: "Sionic runs parakeet on the DPU."
DROPPED press +300 ms 7127-75947-0000: "Everyone could observe his agitation and prostration, a prostration which was, indeed, the more remarkable since people were not accustomed to see him, with his arms hanging listlessly by his side, his head bewildered, and his eyes with all their bright intelligence bedimmed."
changed press +300 ms 1284-1181-0000: "Oju examined this curious contrivance with wonder."
changed press +100 ms 260-123288-0014: "Han stirs nod."
changed press +0 ms s03-en-gb-scotland: "Cole C D submit after the draft is ready."
changed press +0 ms s16-en-us: "CUBA kernels run on the RTX for the AD."
changed press +200 ms s16-en-us: "CUBA kernels run on the RTX for the A B."
changed press +300 ms s16-en-us: "CUBA kernels run on the RTX for the A B."
changed press +100 ms s17-en-gb-scotland: "Kicks keeps a 450m preroll."
```

### Seams (independent segments, not full-context recognition)

| Dictation | Audio s | Segments | WER % joined | WER % pause-split | Errors near seams | Pause-split near seams |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| seam-6930-75918 | 46 | 3 | 0.79 | 7.94 | 1 | 0 |
| seam-6930-76324 | 31 | 2 | 4.12 | 5.15 | 0 | 0 |
| seam-6930-81414 | 31 | 2 | 22.37 | 1.32 | 3 | 0 |
| seam-1320-122617 | 36 | 2 | 1.94 | 1.94 | 1 | 0 |
| seam-1320-122612 | 31 | 2 | 53.93 | 0.00 | 3 | 0 |
| seam-5639-40744 | 37 | 2 | 3.77 | 2.83 | 1 | 0 |

Seams: 7; errors within 3 reference words of a seam beyond the pause-split result: 9 (deletions near seams 6, insertions 2).

Lost or changed boundary words inside the supported timing are mostly
recognition changes, not truncation: "checks" heard as "text", "CUDA" as
"CUBA", "Psionic" as "Sionic". The one "dropped" first word is an alignment
artifact: the reference reads "every one", the model writes "Everyone".
Releasing 300 ms before speech ends still keeps the last word exactly in 88%
of clips (the 200 ms tail and the words' own decay), and pressing up to 300 ms
after speech starts loses no first word (the 450 ms pre-roll). Past the
pre-roll, at +600 ms, 30 of 118 first words are gone, as expected.

### Timings

Before and after the optimizations, run back to back under the same load
(load average 38-50; `before2/` and `after2/` in [raw/](raw/)). Milliseconds;
p99 is nearest-rank, so with fewer than 100 values it is the largest.

| Measure | n | Before p50 | Before p99 | After p50 | After p99 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Stop to final text, all clips (one call; 26 s at most) | 201 | 186 | 642 | **120** | **467** |
| — clips under 5 s | 90 | 146 | 368 | 95 | 165 |
| — clips 5-10 s | 58 | 172 | 466 | 130 | 306 |
| — clips over 10 s | 53 | 252 | 1,049 | 242 | 737 |
| The clear after each call | 201 | 87 | 327 | 29 | 98 |
| Engine call (frontend, encoder, decoder, text) | 201 | 79 | 357 | 86 | 371 |
| — CPU frontend and text | 201 | 32 | 234 | 35 | 198 |
| — CUDA encoder | 201 | 45 | 158 | 47 | 185 |
| — CUDA TDT decoder | 201 | 3.4 | 17.6 | 4.3 | 30.2 |
| Model open, first in a new process | 3 | 19,071 | 26,841 | **9,043** | 9,210 |
| Model open, again in the same process | 6 | 16,481 | 27,480 | 8,211 | 10,365 |
| — `pkrt_open` (whole-file SHA-256 before; layout and vocabulary after) | 9 | 14,974 | 23,513 | 54 | 76 |
| — piece hashing on threads, weight copy, CUDA start | 9 | 1,050 | 4,515 | 6,139 | 7,783 |
| — NVRTC compile | 9 | 47 | 165 | 130 | 193 |
| — weight upload | 9 | 1,139 | 5,979 | 2,686 | 5,460 |
| Model close | 9 | 438 | 1,065 | 617 | 1,017 |

On a quieter machine (load 14-20, the first after run): stop to final text
82 ms p50 and 302 ms p99 (66 / 148 ms for clips under 5 s); the clear
15 / 84 ms; the CPU frontend 24 / 132 ms; model open in a new process 3.5 s
p50 (2.4-3.7 s over 9 opens). The 30-58 s dictations (2-3 calls) take 354 ms
p50 and 570 ms p99. The CPU frontend is the step most hurt by load: the
#12672 record has about 9 ms on a quieter machine.

Key release to final text on Linux is the 200 ms tail plus the stop-to-text
time: about 280 ms (quiet) to 320 ms (loaded) at p50. The proposal's 300 ms
target for a 5 s clip is met at p50 on a quiet machine, not at p99. Capture
adds at most one 20 ms `pw-record` buffer; the controller's work when the
clip freezes is under 4 ms per dictation.

### The voice host (turn end to transcript)

`voice-host --recognizer parakeet` on CUDA, its four fixtures ten times each
through `voice-client --hold --aethyr --exact`, the client's own
`latency final_ms`. Every one of the 80 turns came back exact. The host
clears once per turn before it delivers the text, so the wipe change applies.

| Build | Load average | n | p50 | p99 |
| --- | ---: | ---: | ---: | ---: |
| before (HEAD's Parakeet sources) | 125 | 40 | 250 ms | 620 ms |
| after | 71 | 40 | **125 ms** | **342 ms** |

The two runs were back to back but the machine's load was not the same, so
part of the difference is load. #12672 measured 76-121 ms on four turns at a
lower load.

## The corpus (dictation-eval-v1)

Frozen in [corpus/corpus.tsv](corpus/corpus.tsv) before any model ran: 202
clips, 1,444 s of audio, each with its SHA-256, reference and source. Large
audio stays outside Git under `~/.cache/openagents/corpus/dictation-12665/`
(34 MB); the manifest pins it. `dictate-score.coax corpus` checks every hash
(202 of 202 matched).

| Set | Clips | Audio | Source and selection | Covers |
| --- | ---: | ---: | --- | --- |
| `clean-speaker` | 40 | 343 s | LibriSpeech test-clean: the first utterance of each of its 40 speakers, in dataset order | names, numbers, long sentences, many voices |
| `other-speaker` | 33 | 242 s | LibriSpeech test-other: the first utterance of each of its 33 speakers | harder recordings and accents |
| `short` | 8 | 17 s | test-clean: the first 8 utterances of at most 3 words not already chosen | short holds |
| `seam-*` | 48 | 414 s | test-clean: the first 8 utterances of the first 6 (speaker, chapter) pairs; joined in order into 30-58 s dictations | segmentation seams |
| `nemo-librispeech` | 4 | 18 s | chapter 153's set `parakeet-clips-v1` (in Git) | the qualification clips |
| `nemo-fleurs` | 25 | 281 s | chapter 153's 25-language FLEURS slice (in Git) | languages |
| `synthetic-code` | 40 | 112 s | [synthetic-script.tsv](corpus/synthetic-script.tsv), 20 sentences x 2 voices (espeak-ng 1.52.0.1, `en-us` and `en-gb-scotland`) | code terms, product names, numbers; **synthetic, scored apart** |
| `synthetic-short` | 4 | 17 s | the voice host's fixtures (`verse/voice-host/fixtures/`) | short holds; synthetic |

Generated by the runner, not stored: 21 no-speech signals (3 s each: digital
silence, white noise at -70, -50, -40 and -30 dBFS, a 60/120 Hz hum at -30
dBFS, clicks at -20 dBFS; three seeds each) and 20 noisy copies of the first
10 `clean-speaker` clips (white noise at 10 dB and 0 dB SNR).

Provenance: LibriSpeech (Panayotov et al., openslr.org/12), CC BY 4.0, from
the Hugging Face dataset `openslr/librispeech_asr` at revision
`71cacbfb7e2354c4226d01e70d77d5fca3d04ba1`, fetched as FLAC through its rows
API (the manifest names config, split and row) and converted losslessly to
16-bit WAV with ffmpeg. FLEURS is CC BY 4.0 (chapter 153's manifest). The
synthetic clips are espeak-ng formant synthesis, never a person's voice.
Limits: LibriSpeech references are upper-case read speech without labelled
names; accents are only LibriSpeech's speakers, test-other and one synthetic
Scottish voice; languages other than English are covered by chapter 153's
existing per-language scores (unchanged: the transcripts here are
byte-identical), not rescored.

## How it runs

- [`apps/openagents-cli/tests/dictate-eval.coax`](../../../apps/openagents-cli/tests/dictate-eval.coax)
  loads `dictate.coax` unchanged and calls its `dict_listen` and
  `dict_recognize` on the recorded-audio backend (`--capture fixture`'s
  virtual clock). Every replay goes through the shared controller
  (`apps/shared/dictation.coax`: 450 ms pre-roll, 300 ms minimum hold, 200 ms
  tail, the Verse resampler and VAD, silence admission, exact 20.47 s
  segments) and the CLI's model worker thread, with one resident model on
  the GPU. Each clip gets 1 s of silence before it; presses and releases are
  placed relative to the speech onset and offset (the first and last 20 ms
  frames within 35 dB of the loudest, above -55 dBFS).
- [`dictate-eval-native.c`](../../../apps/openagents-cli/tests/dictate-eval-native.c)
  includes `dictate-native.c` unchanged and adds only measurement effects: a
  microsecond clock, the engine's per-call timing and CUDA encoder and
  decoder times read before the CLI's clear, the clear's own time, each
  segment's text, and the open's split (`--wrap=pkrt_open`).
- [`dictate-score.coax`](../../../apps/openagents-cli/tests/dictate-score.coax)
  verifies the corpus, builds and runs the runner one step per process (each
  under 14 minutes), runs the voice host and its client, scores, prints the
  tables and gates, and exits 1 when a gate is crossed. Word and character
  error rates use one normalization for references and hypotheses (ASCII
  lower case; letters, digits, apostrophes and non-ASCII bytes kept; anything
  else separates words). Numbers are not spelled out, so "25" against
  "twenty five" counts as errors; this inflates the `number` and synthetic
  rows.

"Stop to final text" is the CLI's own path after the clip is frozen: copying
the clip, every segment's model call on the worker thread, the clear after
each call, the controller's ticketed join and the final copy. On a real
microphone the controller first waits the 200 ms tail (plus at most one
20 ms recorder buffer), so key release to final text is the tail plus this.
Printing is not included. The voice host's number is the client's own
`latency final_ms` (turn end to transcript over a local WebSocket).

Machine: coderos-4080, Intel i7-14700K (28 threads), NVIDIA GeForce RTX 4080
(16 GiB, driver 595.71.05), NVRTC 13.2 (`~/cuda-13.2`), GCC 15.2.0, Linux
6.18.49. Model `nvidia/parakeet-tdt-0.6b-v3` at
`541d1f99c6b0c3cd0b11a95167540bb8edefd82b`; 8 decoder threads (unused on the
CUDA path). Coax compiler SHA-256 `35c11e8f30459fb23fb29d02855faf119d10fe0d4b26614fd299dc8a55bd4c42`
(provenance fingerprint `8a14ccfe…`, a current development build; this tree's
own bootstrap was stopped at its 15-minute limit). The machine was shared
with other agents: load average 14-65 during the runs, noted per step below.
Timings are therefore noisy; the gates leave room for it.

Load average at the start of each step: first run `gen` 45-55, `release`
45, `press` 65, `noise`, `seam` and `snr` 17-24; after run `gen` 14, `seam`
15, `startup` 20; the back-to-back pair 38-50; the voice host pair 125
(before) and 71 (after).

## Seams

The controller splits a dictation longer than 20.47 s into at most three
independent model calls at fixed sample offsets, without looking at the
audio. Each joined dictation is compared with its utterances recognized one
by one (cut at their natural pauses). This is not a claim about full-context
recognition, which the bounded model cannot do.

| Dictation | Audio | Segments | WER joined | WER pause-split | Errors within 3 words of a seam | Pause-split, same words |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| seam-6930-75918 | 46 s | 3 | 0.79% | 7.94% | 1 | 0 |
| seam-6930-76324 | 31 s | 2 | 4.12% | 5.15% | 0 | 0 |
| seam-6930-81414 | 31 s | 2 | **22.37%** | 1.32% | 3 | 0 |
| seam-1320-122617 | 36 s | 2 | 1.94% | 1.94% | 1 | 0 |
| seam-1320-122612 | 31 s | 2 | **53.93%** | 0.00% | 3 | 0 |
| seam-5639-40744 | 37 s | 2 | 3.77% | 2.83% | 1 | 0 |

Seven seams add 9 word errors within three reference words of a cut (6
deletions, 2 insertions). That window understates the two bad cases:

- `seam-1320-122612`: the second segment (10.7 s of speech starting
  mid-word) comes back **empty**. The same audio recognized on its own gives
  the same empty result on CUDA and on the exact CPU path (`cpu-exact`,
  which matches NeMo token for token on chapter 153's clips), so this is the
  model's greedy TDT decoding of an abrupt start, not a CUDA defect. Trimming
  20, 100 or 300 ms from the start still gives nothing; trimming 1 s gives
  the rest of the text. Putting 300 ms of silence before the same segment
  recovers almost all of it ("in the forest when the travelers resumed their
  journey. After proceeding a few miles, the progress of Hawkeye ...").
- `seam-6930-81414`: the second segment loses its first seven words ("his
  black eyes shining while in his") and starts at "hand he held a gleaming
  knife".

The segmentation lives in the shared controller (`apps/shared/dictation-policy.coax`,
`dgp_segment`, `#authority`), which Coder for Mac also links, so it is not
changed here. Two fixes are worth measuring with this harness: cut at the
quietest 20 ms frame in the last 2 s before the 20.47 s limit, and give each
later segment a few hundred milliseconds of silence (or overlap) before its
first sample. Both keep the exact-coverage rule if the padding is added to
the model call, not to the clip. The seam gate is set at today's 9 so that
it fails on anything worse; a fix should lower it.

## Gates

The constants at the top of `dictate-score.coax`, set from these
measurements with room for this shared machine's noise. `score` exits 1
when one is crossed.

The final record ([raw/final/](raw/final/): quality, boundary, noise and SNR
from the first run, `gen` and `startup` from the loaded after run, `seam`
from the quiet after run, the voice host's after run) scores:

```
pass  WER test-clean speakers + ch.153 (%): 3.23 <= 4.00
pass  WER test-other speakers (%): 3.78 <= 5.00
pass  WER at 10 dB SNR (%): 3.69 <= 5.00
pass  no-speech dictations that produce text: 1.00 <= 1.00
pass  last word exact, released 0..+200 ms (%): 98.81 >= 97.00
pass  last words dropped, released 0..+200 ms: 0.00 <= 0.00
pass  first word exact, pressed 0..+300 ms late (%): 98.31 >= 97.00
pass  first words dropped, pressed 0..+300 ms late: 1.00 <= 1.00
pass  seam errors beyond pause-split, all seams: 9.00 <= 9.00
pass  stop to final text p99, clips <= 20.47 s (ms): 467.38 <= 600.00
pass  stop to final text p99, 30-58 s (ms): 569.92 <= 1000.00
pass  model open p50, new process (ms): 9043.12 <= 12000.00
pass  voice host turn end to transcript p99 (ms): 342.00 <= 400.00
```

The before build crosses the model-open gate (19.1 s) and, on the first run,
the stop-to-text gate (700 ms p99). Three gates sit exactly at a known
defect so that anything worse fails: the "Uh" from white noise (a filler
word that a text rule such as the proposal's D3 filler filter would drop),
the "every one"/"Everyone" alignment, and the seams.

## Optimizations

Both are on measured paths and change no arithmetic.

1. **The clear after each recognition** (`psionic_parakeet_clear`, which the
   CLI and the voice host run before they hand over the text) wiped the
   resident work, a 145 MB f32 arena, a 2.4 MB f64 arena and smaller
   buffers, with a `volatile` byte-at-a-time loop on Linux: 87 ms p50 and
   327 ms p99 per call under load, more than the recognition itself. It now
   uses `memset` followed by an empty `asm` that may read the memory, which
   keeps the stores (`explicit_bzero`'s method), in
   `psionic/platform/parakeet-face.c`, `parakeet-cuda.c` and the CLI's
   `dictate-native.c`. Apple keeps `memset_s`. After: 29 ms p50, 98 ms p99.
2. **The model open** hashed the whole 2.5 GB `model.safetensors` with
   SHA-256 on one thread: 15-24 s under load, most of the open.
   [`model-chunks.coax`](../../../psionic/lib/parakeet/model-chunks.coax)
   (`#authority`, 61 of 61 mutants killed, registered as the Psionic unit
   `parakeet/model-chunks`) pins the file as 38 pieces of 64 MiB, each with
   its own SHA-256, computed from the copy whose whole-file SHA-256 is the
   pinned `3a2026…5e11` (checked again while computing them). The engine runs
   share *i* of *n* (pieces *i*, *i+n*, ...) on up to 16 threads, and
   `pkrt_open` admits the file only when every share answers yes, so every
   byte is still checked against a pinned digest. The tokenizer keeps its
   whole-file digest, and an empty share list still hashes the whole file on
   one thread. After: 9.0 s p50 under the same load, where the weight copy
   and upload now dominate and grow with load; 3.5 s at load 20.

Parity: chapter 153's 29 NeMo clips through the public API with the changed
library pass `parakeet_reference_compare.py` (29 of 29: every token, frame
and text byte equals NeMo's, every value inside the frozen tolerance;
[raw/parity-comparison.json](raw/parity-comparison.json)), against the
recorded reference on the GPU path. The transcripts of the 202 clips and 6
long dictations are byte-identical before and after.

Next costs, not changed here: the CPU frontend (24-35 ms p50 under load),
the weight upload (0.3-5 s), and the wipe, which could cover only the bytes a
call used instead of the whole arena.

## Reproduce

```shell
apps/openagents-cli/build.sh                     # psionic/build/parakeet and the CLI
verse/voice-host/build.sh                        # for the voice host step
coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax corpus
D=/tmp/dictation-eval
for s in gen release press noise seam snr startup voicehost; do
  coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax run $D $s
done
coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax score $D
```

Fetching the corpus: for each `corpus:librispeech/CONFIG-ROW.wav` line, get
the row from
`https://datasets-server.huggingface.co/rows?dataset=openslr/librispeech_asr&config=CONFIG&split=test&offset=ROW&length=1`,
download its `audio[0].src` FLAC and convert it with
`ffmpeg -i IN.flac -map_metadata -1 -fflags +bitexact -flags:a +bitexact -c:a pcm_s16le -ar 16000 -ac 1 OUT.wav`.
The synthetic clips are
`espeak-ng -v VOICE -w TMP.wav -- "SENTENCE"` then the same ffmpeg line.
`corpus` refuses any file whose SHA-256 differs.
