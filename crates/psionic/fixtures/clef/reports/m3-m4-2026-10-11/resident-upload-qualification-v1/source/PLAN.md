# Positional resident upload qualification

This is a source-frozen correctness and memory qualification, not a latency test.
No build or model run is authorized by this document. The parent assigns each
workload window after source review. There are no quiet leases, system changes,
or controls over other processes. A failed attempt stops the sequence; preserve
its evidence without a retry or a higher limit.

## Fixed candidate and baseline

Use the original 19,232,219,200-byte Clef Q4_K_M GGUF, SHA-256
`9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`, and
the official external head. The previously verified file snapshot must match;
this preparation does not reread the 19 GB payload. The candidate uses
`resident-f16` and `PSIONIC_CLEF_METAL_WEIGHT_UPLOAD=positional`, fused TensorOps
attention, weight-shapes GEMM, fused-fields head, sequential Delta, chunk512,
prefix cache0, batch requests1, and two CPU threads.

The baseline is the passed `attention-qk128-key32-integration` server
`f4def388b4b6622fef9023c2616639c215829d7a519359eb64d1dbca9fdfa31f` and
source descriptor `843d7861b221b650d17f4a4902c2297117765e583a2d6159dce6ae797da70a09`.
Its saved 42 requests and exports cover40 text records/104 questions plus the
fixed4k and16k records/6 questions. It used `q4k-on-demand`, not resident weights.
That placement difference is intentional: both paths dequantize the same bytes
into the same f16 weights. Exact equivalence is required, not assumed.
The full candidate source manifest permits only the five reviewed upload/test
files to differ in product code. Encoder, head, backend arithmetic, input bytes,
and relevant metadata must match. HTTP does not expose token IDs; the source
identity and exact serialized requests establish the tokenization basis, with
counts and exact pooled bytes checked separately.

## Build and CPU checks

Use Rust1.97.1, release profile, two jobs, and the existing target-agent1 directory.
The single owned build group has a24 GiB sampled max(RSS,physical footprint) cap,
36 GiB available at start,12 GiB available floor, and1800-second deadline.
Declare `OPENAGENTS_BUILD_DISK_GB=2` plus a4 GiB disk lease (6 GiB total), require
31 GiB free before starting and25 GiB throughout and after cleanup. All new
qualification outputs combined must stay within2 GiB. Never delete a target or
raise a bound after refusal. Incremental build size is uncertain; disk monitoring
is authoritative. An earlier server build took701 seconds, which informs but
does not guarantee this1800-second budget.

Compile catalog, models, and serve library test executables. Run only the four
`positional_reads_` tests, the models forwarding test, the serve `clef::` CPU
filter, and `qwen35::clef_metal::` CPU filter. GPU/model tests remain ignored.
Then build the release server and retain its binary plus the serve test executable.
The latter contains the new ignored positional-upload GPU fixture for a later,
separately assigned hardware window. The proposed small fixture guard uses GPU,
memory4, and disk2 leases;4 GiB cap/start16/floor12 and180 seconds. It requires
exactly one passing ignored fixture and clean shutdown. No GPU test executes in
the build script.

The guard hashes sources before and after work, validates cleanup and late memory
monitor errors, then writes the candidate identity. Model checks refuse any
binary whose build guard, harness identity, or source descriptor differs.

Planned invocation, only after the parent grants the window:

```sh
OPENAGENTS_BUILD_DISK_GB=2 CARGO_TARGET_DIR=@@CLEF_TARGET@@ openagents lease build --keep-target-dir --receipt build-lease.json -- openagents lease memory --amount 24 --receipt build-memory-lease.json -- openagents lease disk --amount 4 --receipt build-disk-lease.json -- python3 guard_build.py
```

Run from this scratch directory only if the parent supplies the intended worktree
as lease context; the child Cargo command explicitly uses that worktree. Each
receipt name is new. The persistent target is never replaced or removed.

## Text and image checks

After build checks and the small GPU fixture pass under separately reviewed
hardware bounds, run one resident text server with52 GiB max(RSS,physical
footprint),64 GiB available at start,12 GiB available floor,600 seconds startup,
300 seconds per request, and1800 seconds for its entire lifetime. Hold GPU,
memory52, and disk2 leases. Require27 GiB free at start and25 GiB during checks.
These are sampled fail-closed bounds, not a hard allocation reservation.

Send the saved request bytes without reserialization. Require all110 f32 raw
logits to match at the bit level and all indexed pooled f16 bytes to match exactly.
Also require matching tokens, model/head metadata, answer identities/tops, and
zero cache use. Positive or malformed cache fields fail. Export readers reject
missing/duplicate rows, options, offsets, tails, and nonfinite logits. A mismatch
is a failed qualification even when all top answers happen to match.

Only if text passes, start a separate resident image server under the same limits.
Run the existing six image records/14 answers and ten HTTP refusals. Preserve
that existing BF16 image top-answer/token/expected-content gate; probability
differences remain diagnostic. Do not add a new image tolerance. The manifest
matches the passed baseline; current333 vision tensor file snapshots are frozen.
This preparation does not independently hash all vision payloads.

The source, inputs, baseline receipts, candidate build chain, process receipts,
all comparisons, and failures remain in this directory. No raw weights, pooled
arrays, binaries, or unrelated process metadata enter the publication bundle.
Execution completion and acceptance are separate. This qualification does not
alter the unresolved independent Torch same-Q4 text gate or prove the52 GiB
capacity hypothesis before a passing run.
