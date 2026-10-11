# Activation-weighted Q4 preparation

Status: design only. No model, GPU, build, statistics collection, or artifact
regeneration has run for this experiment. The original BF16 acceptance suite
and its thresholds remain unchanged.

## Why use the existing BF16 MPS path

The successful official text resume took 143.47 seconds and peaked at
58,125,060,040 bytes of sampled group memory. The clean BF16-MPS trunk with
CPU-f32 head took 219.44 seconds, peaked at 60,152,859,888 bytes, and kept at
least 24,476,598,272 bytes available. Both used the same 72 GiB group cap,
84 GiB starting availability, 12 GiB available-memory floor, and 60 GiB MPS
allocation limit. These are observed bounds, not a hard guarantee for a new
workload. The older two-case head capture failed its functional checks;
its guard did not reject memory (57,915,852,552-byte sampled peak).

Reuse the pinned Cloudflare/clef BF16 checkpoint, Torch 2.9.1, Transformers
5.10.2, eager attention, batch size one, no cache, and two CPU threads. Load
once with the same reviewed low-memory loader. Invoke only the official text
backbone. No MPS joint head or lexical gather is needed for calibration.
Keep the 60 GiB MPS limit and fail closed if that limit or the external guard
rejects the run; do not raise bounds automatically.

## Corpus and separation

Create and freeze a new authored corpus before collecting any activations or
model answers: 96 calibration records from eight scenario families and 24
development records from four different families. Each request contains
self-contained state and narrow typed questions. Use varied text lengths,
structured state, code fragments, alternative counts, and question types.
No acceptance examples, recorded logits, known failures, or answers enter the
generator. Do not create labels by consulting the acceptance suite.

Freeze the generator, complete request bytes, family-level split, seed, and
encoded token IDs in a manifest. Each request must encode to at most 1,024
tokens; refuse oversized records instead of silently truncating. Calibration
and development families are disjoint. A separate overlap checker may read
acceptance request bytes solely to reject exact request/state duplicates;
it emits counts and hashes and never exposes acceptance outputs to the
collector or quantizer. Similarity checks are diagnostic, not permission to
rewrite the corpus around held-out examples.

The held-out development set selects whether to advance the single declared
candidate. Its activations never enter imatrix statistics. Keep development
BF16 references immutable. Do not use the original 104-answer acceptance set
until the recipe, candidate artifact, and development report are frozen.

## Statistics collection

Register forward-pre-hooks on the 64 layers' gate_proj, up_proj, and down_proj
modules (192 entries). The weight-to-GGUF mapping is explicit:

- gate_proj -> blk.N.ffn_gate.weight, 5,120 input columns.
- up_proj -> blk.N.ffn_up.weight, 5,120 input columns.
- down_proj -> blk.N.ffn_down.weight, 17,408 input columns.

For every real token row, accumulate sum(x_j squared) and a row count for each
input column. Dense upstream imatrix collection does exactly this before
normalizing by count. Do not use absolute means, output gradients, acceptance
losses, or per-output-row weights. Gate and up see the same normalized input;
check their input statistics match instead of assuming a tensor naming rule.

Copy each hook's BF16 input to CPU with synchronization and an immutable byte
snapshot, then process at most 64 rows at a time in NumPy float64. Do not retain
activation tensors or autograd graphs. Accumulate float64 sums on CPU and
publish finite nonnegative float32 mean-square vectors only after completion.
There are 1,769,472 entries: 14,155,776 bytes for float64 sums or 7,077,888
bytes for exported float32 means. The largest input snapshot at 1,024 tokens
is 35,651,584 bytes; the row-block conversion and reduction remain bounded.
This adds synchronization and is an accuracy collection run, never a latency
measurement.

Keep each request's statistics pending until its full backbone call succeeds,
all 192 hooks fire exactly once with the expected token count/shape, input IDs
and masks remain unchanged, and final hidden rows are finite. Only then merge
into committed sums. Check checkpoint identity before and after every request.
Record each completed request hash, exact token count, moments checksum,
module name/width, count, and collection time. Store raw sums and counts as
well as normalized vectors so the normalization can be audited.

Before a full run, validate the accumulator on synthetic CPU arrays and the
hook transport on one small BF16 tensor; then run a two-record model pilot
under the same bounds. The pilot selects the shortest and longest frozen calibration records and is part of the frozen calibration set and is
not a basis for editing it. It must confirm exact hook coverage, input
immutability, and a practical runtime estimate before a complete collection.

## Quantizer recipe

Use the already verified llama.cpp revision
62a6f74bab748b1755f4cd8dfc0871caf865c6c8. Extend only the scratch wrapper to pass
one finite, nonnegative mean-square vector of length input_columns to
`ggml_quantize_chunk(GGML_TYPE_Q4_K, ..., imatrix)`.

Re-quantize all 192 FFN matrices directly from original BF16 rows. Keep every
other tensor, metadata entry, descriptor ordering, and padding byte unchanged.
All FFN tensors remain Q4_K, so the file size stays 19,232,219,200 bytes and
runtime placement stays the existing q4k-on-demand path. This tests
activation-weighted Q4 at the same storage cost; it does not combine another
precision change. Do not add smoothing, clipping, rescaling, selective layers,
or per-case overrides unless a separate experiment is proposed and frozen.

The upstream Q4_K implementation multiplies the supplied importance values
by its own weight-dependent factor. Pass mean(x squared) directly, with no
square root or extra weighting. Keep all-zero columns as zero; require at
least one positive entry per tensor. Synthetic tests must cover sparse/zero
importance blocks and reject nonfinite/negative vectors before generation.

Use the same stream-and-hash artifact writer pattern already reviewed:
original SHA verified while reading, selected BF16 payloads read in 64-row
blocks, all copied/replaced payload hashes, exclusive partial file, exact
source-state checks, and final output hash. Validate the actual loaded
quantizer image with dladdr and pinned hashes. Do not re-quantize the existing
Q4 values. No model runs occur during generation.

## Development and final acceptance

Generate a fresh BF16 reference for all 24 development records, then evaluate
the baseline and the single weighted-Q4 candidate on those identical bytes.
Report every question's logits, probability differences, top agreement, and
token/record identity; show all records, not only improvements. Advance only
if the candidate reduces both mean absolute probability error and maximum
probability error without lowering top agreement on this independent set.
This is a development selection rule, not a replacement acceptance threshold.

After freezing the development report and exact candidate identity, rerun the
original 104 text answers and 14 images against unchanged BF16 goldens. The
existing M1/M4 text limits still apply. Failure remains failure. Then run the
existing chunk, prefix, batch, and speed checks before claiming M4 completion.
No finite corpus can guarantee that weighted Q4 meets all BF16 tolerances.

## Proposed resource windows

No window is authorized by this document alone.

1. CPU accumulator and source checks: at most 2 GiB, 60 seconds; no GPU.
2. Two-record BF16 MPS pilot: GPU + memory72 + disk2 leases; 72 GiB group cap,
   start84/floor12 GiB, MPS60 GiB, two threads, max1,024 tokens, 300 seconds.
3. Full96-record collection: same resources, 1,800-second deadline. Expected
   runtime is minutes based on prior40-record references, but hook overhead
   must be measured by the pilot. No other model or timing workload overlaps.
4. Quantizer wrapper build: build + memory4 + disk2, two jobs, 600 seconds.
5. Streaming regeneration: CPU only, memory2 + disk20, start14/floor12 GiB,
   start45/free-disk-floor25 GiB, one quantizer thread, 1,800 seconds.
6. Development references and native evaluation: separate reviewed model
   windows using existing guards; no simultaneous BF16 and native residency.

Every guard stops only its new child process group. Never request a quiet
lease, change power or permissions, or control desktop apps or system services.

Primary source references: pinned tools/imatrix/imatrix.cpp (dense collection
and count normalization), ggml/src/ggml-quants.c (Q4_K weighted quantization),
and src/llama-quant.cpp (per-input-column importance passed for dense matrices).
Question design follows https://docs.typesafe.ai/concepts/state and
https://docs.typesafe.ai/primitives/choice; these guide corpus structure only,
not labels or API calls.
