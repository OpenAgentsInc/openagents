# Original-weight incremental ASR, Linux CUDA, 2026-11-03

Issue #12673 is qualified for the separate `psionic-nemotron-streaming-en-v1`
profile and its explicit file-backed Aethyr consumer. This is English RNNT
streaming with persistent state, not independent offline TDT windows. Chapter
153's multilingual profile, implementation and evidence are unchanged.

## Implementation and ownership

The [manifest](../../../psionic/fixtures/parakeet-streaming/manifest.json) pins
NVIDIA `nemotron-speech-streaming-en-0.6b` at
`ebe59e5a817142986528bbbee5dba8db7b38ed50`. The native loader checks the complete
2,472,413,604-byte safetensors and 400,216-byte tokenizer SHA-256 values in
Coax before using fixed original tensor offsets. The five original files were
acquired as data; no pickle is loaded. Weights remain outside Git.

[Coax sources](../../../psionic/lib/incremental/) own frontend, arithmetic,
launch scheduling, model identity, cache movement, RNNT greedy decisions and
stream/voice admission. C owns mmap/allocation/mutex/CUDA transport effects.
The build requires the host C compiler; NVRTC compiles Coax-generated CUDA at
runtime through the existing CUDA effect layer. Python/Torch/Transformers are
independent qualification dependencies and are absent from the product.

At 16 kHz, preemphasis .97 and centred 512-FFT/400-Hann/160-hop features retain
sample overlap and bounded right context. F32 preemphasis/tables feed an F64
FFT and compensated mel reductions, rounded to F32 features. Features have 128 mels and no
utterance-wide normalization. The first encoder chunk has nine frames, later
chunks sixteen. Each chunk produces two 80 ms rows. Finalization zero-pads the
remaining feature chunk, without adding captured PCM. Three causal subsampling
stages carry their last input row. Each of 24 width-1024 layers retains 70 K/V
rows and eight convolution rows. Relative-position projections are prepared
once. The two-layer width-640 RNNT carries hidden/cell and last prediction
state, with blank 1024 and at most ten symbols per encoded row.

The [native API](../../../psionic/platform/psionic-incremental.h) serializes one
resident engine. Coax validates the whole push before consumption: finite F32
samples in [-1,1], at most 16,384 samples, exact sequence, current nonzero local
epoch and at most 960,000 samples per stream. Zero samples are final-only.
Malformed audio/order/extent refuses unchanged. Reset requires a newer epoch;
cancel clears host/work/mutable device state. Device/backend failure invalidates
all prior text/inspection histories until reset. Fixed 4,096-token/8,000-byte
text exhaustion fails the epoch. Snapshot capacity/stale/cancel refusals happen
before writing caller text. Immutable mapped original files are a caller
obligation. Close frees model/device state. This is not forensic erasure.

The [actual Coax consumer](../../../psionic/examples/transcribe-incremental.coax)
feeds public PCM in 320-sample pushes, prints real append-only partials and
offers exactly one final through Aethyr's current question/media-epoch replay
gate. Partials never become realm answers. Native cancel and Aethyr barge-in
revoke the local ticket and realm epoch; a late answer refuses. Existing wire
phase numbers and the initial media epoch zero are preserved. The realm's
existing 600-byte answer capacity still applies. No microphone was opened.

## Independent original-weight reference

The oracle is Hugging Face Transformers at
`9719c27f4e04e8d10ed1f908bdb3712d16fef73c`, using
`NemotronAsrStreamingForRNNT` and its original feature extractor, original F32
weights and one-lookahead-token profile. Its model source hashes, input PCM
hashes, per-chunk texts, token IDs/positions and environment are in the archived
`original/record.json` and `expanded/record.json`. Torch was 2.12.0+cu126,
Transformers 5.23.0.dev0, with deterministic CUDA math and TF32 disabled.

The pinned generic eager-mask factory returns additive 0/negative-limit masks;
the streaming module expects boolean allowed masks in both attention and
padding-row detection. Unadapted first-chunk inference was nonfinite. An
attention-only adaptation still incorrectly zeroed convolution input and
emitted all blanks. The final independent harness asserts that every mask
entry is 0 or <= -1e30, then converts `mask == 0` **at the factory**, before
both consumers. This changes representation only, preserving every allowed
edge, numerical weight and model state transition. The initial all-blank
reference records remain outside Git at
`/tmp/incremental-asr-ref/reference-before-factory-adapter`. This qualification
is against that explicitly adapted pinned Transformers reference; it does not
claim execution or equivalence of an unmodified NeMo streaming model.

The expanded reference additionally offloads original parameters between
layers to fit a GPU shared with another workload. All reference arithmetic
still runs on CUDA. The Coax product separately qualifies full resident weights
and a bounded prefix/transfer mode, preserving F32 math in both.

[reference-stages.tar.gz](reference-stages.tar.gz) retains all thirteen cases'
actual raw little-endian F32 feature/encoder/projected/cache artifacts and
reference JSON, without weights. [reference-files.json](reference-files.json)
checks every member. The four initial public recordings and human transcripts
are in `psionic/fixtures/parakeet/`. The expanded nine inputs and labels were
frozen before inference in [expanded-corpus.json](expanded-corpus.json): clean
and other LibriSpeech speakers, two short utterances, a 30.46-second continuous
concatenation, one second of silence and deterministic low-level noise (seed
12673, integer samples uniform [-327,327]). Source split/row IDs, hashes and
human labels are retained; speech is the public CC BY 4.0 LibriSpeech corpus.
The long input joins the existing evaluation's clean rows 120/121/122 without
introducing segment resets.

## Results

All thirteen cases match exact final tokens, encoded-row emission positions
and final text. Features, encoded rows, encoder projections and selected layer
0/23 K/convolution cache snapshots pass elementwise
`abs(product-reference) <= .001 + .001*abs(reference)` with finite outputs.
The expanded nine also match every chunk's partial text. Cache snapshots at
chunks 1, 2 and 36 exercise startup and the saturated 70-row history; the long
case continues through 191 chunks. [Comparison receipts](comparison-final.json)
retain observed worst fractions of the allowed tolerance, not raw errors.
The separate original four-case replay is [here](comparison-160.json).

Partition checks cover 160-sample pushes, 320-sample pushes, 2,560-sample
(160 ms) pushes, ragged 137-sample
pushes and single-sample pushes. Each compares the same original reference;
no model or decoder reset is inserted between chunks. Exact partials are
checked in the single-sample and expanded comparisons. Both reference records retain their adapter and source-file hash metadata.

The expanded speech slice has **7 word errors / 205 words = 3.4146% WER** and
**9 character errors / 1,099 characters = 0.8189% CER**. Normalization is Unicode
NFKC, casefold, letter/number/mark/apostrophe retention and whitespace collapse;
characters include normalized spaces. First words match in 6/7 cases, last
words in 7/7. The first-word difference is “I'm” against “I am”, not missing
initial speech. The long case's two differences are “forests”/“forest” and
“travellers”/“travelers”. Silence/noise both produce empty text (0/2 false
positives). The initial four recordings are parity fixtures, not included in
this separate quality denominator. This finite English slice establishes no
population accuracy, microphone quality or multilingual claim.

[measurements.json](measurements.json) summarizes the timings on coderos-4080,
RTX 4080, with another Psionic process using about 12 GiB of the 16 GiB GPU.
These are unpaced file replays. Each chunk time measures a chunk-producing
native push, excluding subsequent oracle snapshot copies. Whole-replay and
first-partial wall times include those inspections and Python loop overhead.
The final-call timer measures the actual empty final push, including any
remaining feature padding/encoder/RNNT work. Percentiles use the recorded
sorted nearest index, round((N-1)*p).

| Observation | Full resident, final artifact | Bounded prefix, busier GPU |
| --- | ---: | ---: |
| Chunk-producing calls | 460 | 460 |
| Chunk p50 / p99 | 61.19 / 61.80 ms | 168.57 / 182.07 ms |
| First partial replay wall p50 / p99 (7 speech cases) | 84.92 / 146.57 ms | 759.89 / 1,123.12 ms |
| Audio received at first partial p50 / p99 | 0.90 / 1.38 s | 0.90 / 1.38 s |
| Final call p50 / p99 (9 cases) | 28.83 / 61.36 ms | not separately timed |
| 30.46 s long replay, including inspections | 10.94 s | 32.05 s |
| Model open, including hash/JIT/preparation | 8.50 s | 8.63 s |

The low-memory branch is qualified for correctness and bounded storage; the
busy run was slower than real time. No hard real-time promise follows from
these finite measurements. In particular, the unpaced first-partial wall
number does not include waiting for its 0.90..1.38 seconds of audio. No actual
mic-stop-to-composer or paced UI latency is claimed.

Device allocations remain fixed from open through the long stream:

- Full resident: 2,472,339,460 bytes original weights, 0 transfer buffers,
  3,200,000 work and 28,726,464 state bytes.
- Bounded prefix: 1,073,741,824 bytes original weights, 40,000,000 transfer
  buffers, the same work and state. A smaller prefix is permitted if needed;
  it changes transport only and has no independent latency qualification.
- Host work/state and stream buffers are fixed. The 2.47 GB mapped file,
  operating-system page cache and CUDA context/JIT allocations are separate
  from these explicit arenas. Constant process RSS/address space is not claimed.

## Checks and reproduction

Run from a worktree. Set `STREAMING_MODEL` to the authenticated original data
directory; on this host it is
`~/.cache/openagents/models/parakeet/nemotron-streaming-en-0.6b-ebe59e5a`.

```sh
coax/web/tool.sh psionic/platform/incremental-build.coax
psionic/build/incremental/psionic-incremental "$STREAMING_MODEL" \
  psionic/fixtures/parakeet/6930-75918-0011.pcm
coax/bin/coax test psionic/lib/incremental/runtime.coax
coax/bin/coax test psionic/lib/incremental/kernels.coax --library
coax/bin/coax mutate psionic/lib/incremental/policy.coax --library --root-only
coax/bin/coax mutate psionic/lib/incremental/voice.coax --library --root-only
```

The initial complete runtime check was 250/250; later size-admission,
snapshot and backend-loss checks were selected separately (253 total checks
now). Six hand-worked kernel checks pass. Both authority policies are registered
in `psionic/check.sh` for subsequent debug/release and mutation runs. The final
mutation receipts kill **58/58** stream/model-admission and **33/33** voice
mutants, with zero survivors; passing unchanged mutation results were reused.
Release authority checks pass 4/4 and 2/2. Actual model-stage reference checks
exercise CUDA rather than substituting seeded model outputs.

The normal consumer checks API extent/order/NaN atomicity, stale snapshot and
cancel/rearm/final refusal, real partials, one Aethyr final and stale/barge-in
refusal. ASan/UBSan uses checked generated Coax C and the native effects,
with leak detection disabled for CUDA; these instruments cover host code,
not CUDA device memory. Its exact invocation and result are in the retained
qualification record. Default ASan shadow-gap protection made `cuInit` fail
with CUDA error 2; `protect_shadow_gap=0` passed the minimal CUDA probe.
NVRTC also required its companion library directory on `LD_LIBRARY_PATH`.
Both failed environment runs are retained separately, rather than counted as
model or sanitizer passes.

```sh
coax/web/tool.sh psionic/platform/incremental-build.coax \
  psionic/build/incremental-sanitized --sanitize
ASAN_OPTIONS=detect_leaks=0:protect_shadow_gap=0 \
UBSAN_OPTIONS=halt_on_error=1 \
LD_LIBRARY_PATH="$HOME/cuda-13.2/lib:/run/opengl-driver/lib" \
  psionic/build/incremental-sanitized/psionic-incremental "$STREAMING_MODEL" \
  psionic/fixtures/parakeet/6930-75918-0011.pcm
```

For independent reproduction, review the pinned Transformers source and use
[the reference harness](../../../psionic/tests/incremental_asr_reference.py)
in an isolated oracle environment. Extract the retained reference archive
outside Git, retaining or mapping the recorded immutable input paths. The
[comparison harness](../../../psionic/tests/incremental_asr_compare.py) takes
`LIBRARY MODEL REFERENCE_DIR OUTPUT [--partition N] [--corpus JSON]`, verifies
input PCM hashes and compares actual native stages and partials. On this
NixOS host its existing Torch environment needs the GCC shared library path
recorded in `qualification.json`; this is an oracle dependency only.

Mac/Metal streaming, live microphone/UI partials, default CLI/TUI/Verse voice
provider adoption, vocabulary rescoring and other lookahead/language models
remain separate engineering/qualification work. No physical owner acceptance
step is added for a UI this change does not implement. Existing owner-only
microphone/TCC/device acceptance stays in `NEEDS_OWNER.md`. The prior offline
profile/evidence and default consumers remain as previously qualified.

The required tracked-tool check includes both new oracle files and reports
only the existing unrelated missing-header/port-list problems; see
[tools-check-final.log](tools-check-final.log). No new tool lacks its firm reason.

After rebasing onto `eac8e9be11`, both global book commands stop at an unrelated
stale `coax.wgsl.current` evidence hash for `verse/everglade/gpu.bindings`
(upstream changed its uniform extent from 575 to 608). See
[the rebased Psionic log](book-psionic-rebased.log) and
[Aethyr log](book-aethyr-rebased.log). The prior structure/source-listing/link
checks passed before rebase; every recorded ASR source hash is unchanged.
The current [tool audit](tools-check-rebased.log) reports six unrelated missing
headers, with both new oracle headers accepted. These repository-wide gates
are recorded as blocked/failed, not counted as passes or silently rebound.
