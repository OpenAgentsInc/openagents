# Clef M3 and M4 validation evidence

This package records correctness checks on a shared Mac M5 and capacity checks
on an RTX 4080. The official Clef 27B BF16 reference completed all 40 text
requests (104 answers) and six image requests (14 answers). Native 27B passed
the image checks, but the latest Stage 5 build failed the strict text gate:
93 of 104 top answers agreed, with a maximum probability difference of 0.217196.
An independent BF16-backbone/CPU-f32-head diagnostic confirms the large native
gap. Both earlier Flash HTTP modes passed
against the existing CPU reference. Standalone direct-solve DeltaNet passed its
Flash weighted check, but the combined optimization mode failed. One native
27B CPU diagnostic agreed closely with Metal while both differed from BF16.
Full-model prefix reuse passes for Flash and 27B. Flash continuous batching
passes the HTTP comparison. A shared-machine latency comparison favors fused
attention and shape-selected GEMM, but misses the absolute M3 targets. These
files do not establish completion of M3 or M4.

## Stage 5 normalization, synchronization, and 27B projections

The [source manifest](stage5/source-manifest.json) freezes 2,236 files based on
commit `452ef01c90d4d704b22568c47181271e1629f2c7`. This build corrects DeltaNet
query/key normalization to `x / sqrt(sum(x*x) + 1e-6)`, combines each serial
chunk's layer stack and head-input preparation in one command buffer, and adds
the eight observed 27B projection shapes to the opt-in GEMM planner. Profiling
and layer observers retain their intermediate waits.

The [build and test results](stage5/checks.json) pass: the release server,
72 Clef serving tests, the new CPU normalization oracle, and 23 backend tests.
The [explicit GPU checks](stage5/small-gpu-checks.json) pass all 18 backend
tests and six serving tests. They include independent additive-epsilon oracles
for Flash and 27B head layouts, all eight 27B GEMM shapes with tails and output
guards, buffer lifetime checks, and prefix and batch scratch checks.
The [build guard](stage5/guard.json) records successful cleanup and no memory
rejection. GPU test binary hashes are in [test-binaries.json](stage5/test-binaries.json).

The fresh server has SHA-256
`e8bb9510ada3d29651e3197ee372d90b3db86ecc46a981b68f14a66a2e58b168`.
The [27B run](native27b-stage5-default/run.json) uses staged attention,
fixed-64 GEMM, sequential DeltaNet, on-demand Q4 FFN, a 512-token chunk, and
the official external BF16 head. All six image cases, 14 top answers, exact
token counts, and ten refusal checks pass; maximum image probability difference
is `0.0017859468247664045`. The [text comparison](native27b-stage5-default/1-text.json)
fails: 93 of 104 top answers match, maximum probability difference is
`0.21719610918470517`, and 64 questions exceed `0.02`. The normalization fix
does not resolve this gap. The run overlaps compilation and makes no latency
claim; its owned server stops without a guard or cleanup failure.

## Independent CPU-head reference and MPS diagnostics

Bounded head-only experiments found instability in PyTorch 2.9.1 MPS on this
machine. The [first frozen probe bundle](head-copy-publication-v1/README.md)
records copy/conversion failures and successful isolated lexical gathers.
The [second bundle](head-copy-publication-v2/README.md) preserves 16 official
head calls with exact input and weight bytes: all eight case-014 calls agree,
but case 034 produces four nonfinite and four differing results. Cloning views
does not reliably fix the failure. The exact runtime or operator cause remains
unknown. These probes neither invalidate every earlier answer nor establish
native acceptance.

To check the native gap independently, a new reference runs the official
BF16 MPS backbone with the official head on CPU in f32. It reads the untied
lexical output rows directly from the pinned BF16 checkpoint and verifies
immutable IDs, masks, records, hidden rows, and lexical rows. The
[complete reference artifact](reference27b/cpu-head-stage1/results/bench-artifacts.json)
contains all 40 cases and 104 answers, with every immutable-input check passing.
The 1.43 MB reference is stored by hash in the repository's benchmark bucket;
restore it with `scripts/bench-artifacts.py restore` followed by the path to
this report directory. The comparison and run receipts remain in Git.
Its precision is explicitly `trunk:bfloat16,head:float32`; it is a diagnostic,
not a relabeled replacement for the original BF16 acceptance goldens.

The [offline comparison](reference27b/cpu-head-stage1/diagnostic-comparison.json)
finds 103 of 104 matching top answers between this reference and the original
BF16 goldens, with maximum probability difference `0.0043` and none above
`0.02`. The Stage 5 native output matches only 94 of 104 against the independent
reference, with maximum difference `0.21609610918470518` and 63 questions above
`0.02`. Moving the reference head to CPU therefore confirms the large native
gap. It does not identify whether quantized weights or native execution causes it.

The [reference guard](reference27b/cpu-head-stage1/guard.json) records completion
in 219.44 seconds, peak accounted memory of 60,152,859,888 bytes, a 72 GiB cap,
and successful owned-process cleanup. Model tensors and saved intermediate
tensor payloads remain outside Git; the reference records their hashes.

The [same-input head isolation](cpu27b-diagnostic/stage5-head-isolation-clean/run/results/isolation.json)
then compares cases 014 and 034 on CPU with four combinations of native and
reference hidden and lexical rows. With identical native inputs, Rust and the
official head differ by at most `9.5367431640625e-7` in logits. Reference hidden
rows with native lexical rows restore every top answer and differ by at most
`0.000343262` and `0.000187645` in probability. Native hidden rows with reference
lexical rows still differ by `0.092048511` and `0.216365304`, respectively.
These two cases locate the large difference in the backbone output rather than
head arithmetic or lexical gathering. They do not distinguish quantization
from backbone execution. The [guard](cpu27b-diagnostic/stage5-head-isolation-clean/run/guard.json)
records a CPU-only run with a 4 GiB cap and successful cleanup.

The [Q4 weight comparison](q4k-weight-diagnostic/result.json) samples 144 rows
from 18 FFN tensors across six layers against the official BF16 checkpoint.
The [actual native Rust decoder](q4k-weight-diagnostic/native-run-lto/receipt.json)
matches all 1,327,104 independently decoded values bitwise. This rules out a
Q4 block-decoding mismatch in that sample; it does not establish why the model
differs from BF16. The first link failure remains recorded separately.

Stage 5 also repeats the weighted block-DeltaNet check. The
[standalone configuration](stage5/weighted-direct-delta/output.log) passes
34 logits at maximum difference `8.87095928e-4`. The
[combined configuration](stage5/weighted-direct-combined-delta/output.log)
still fails at `1.40523911e-3` for the 2,032-token case, chunk 257, question 4,
option 5. Block DeltaNet remains experimental and disabled by default; neither
this failure nor the strict `1e-3` bound is removed.

## Official 27B reference

| Evidence | Result |
| --- | --- |
| [Text goldens](reference27b/text-mps-bf16.json) | Complete: 40 requests, 104 answers, 155–1,090 tokens per request. |
| [Image goldens](reference27b/images-mps-bf16.json) | Complete: six requests, 14 answers, 334–388 tokens per request. All answers match the synthetic image expectations. |
| [Checkpoint verification](checkpoint/verified.json) | Pinned public checkpoint files, sizes, and SHA-256 hashes. No weights are included. |
| [License and template verification](checkpoint/extra-verified.json) | Additional public checkpoint files verified against the same revision. |

The reference is `Cloudflare/clef` at revision
`0b331204bb13fbd2ca93a64df1956f5b55478ce5`, with its released BF16 head
(`a010ac04f078e699988e4049cbea5e62c962393f59fec366640b64e8d69a4953`).
Execution used PyTorch 2.9.1, Transformers 5.10.2, Accelerate 1.15.0, MPS,
BF16, eager attention, and two CPU threads. The source, tokenizer, processor,
head, and model-index hashes are in each golden file.

The goldens retain every input token ID, raw logit, probability, answer, case
identity, and source-request hash. The public text inputs are in
[the text corpus](../../e2e/requests.jsonl); the synthetic image inputs are
generated by [the reference runner](../../tools/image_parity.py). Text cases
have no independent expected-answer labels: `all_match_expected` in the text
file means that no case failed its optional expectation check, not 100% accuracy.
These are reference outputs for a later native comparison.

The run history includes failures and retries:

| Run | Outcome and evidence |
| --- | --- |
| First text pilot | The original RSS monitor timed out during loading and stopped its owned process group. [Guard receipt](reference27b/text-pilot-20261011T024216-64396/guard.json), [log](reference27b/text-pilot-20261011T024216-64396/reference.log). |
| Text pilot after guard changes | `text-006` completed. [Guard receipt](reference27b/text-pilot-20261011T031536-38110/guard.json), [log](reference27b/text-pilot-20261011T031536-38110/reference.log). |
| First text continuation | Token validation failed at `text-034`; that attempt did not add the case to the accepted goldens. The cause was not established. [Guard receipt](reference27b/text-resume-20261011T031811-43459/guard.json), [failure log](reference27b/text-resume-20261011T031811-43459/reference.log). |
| Isolated `text-034` retry | Strict token checks passed after adding owned pre-forward and post-forward captures. The runner now preserves diagnostics on any mismatch. [Guard receipt](reference27b/text-pilot-20261011T033125-31319/guard.json), [log](reference27b/text-pilot-20261011T033125-31319/reference.log). |
| Final text continuation | Completed the remaining cases; the final file has all 40 case IDs exactly once. [Guard receipt](reference27b/text-resume-20261011T033437-40165/guard.json), [log](reference27b/text-resume-20261011T033437-40165/reference.log). |
| Image run | Completed all six cases. [Guard receipt](reference27b/image-resume-20261011T032211-7326/guard.json), [log](reference27b/image-resume-20261011T032211-7326/reference.log). |

These runs used GPU and memory leases, with a 72 GiB sampled owned-process
memory cap, an 84 GiB available-memory start requirement, a 12 GiB remaining
available-memory floor, and a 60 GiB MPS allocator setting. The monitor records
RSS and physical footprint separately and caps the sum of each owned process's
larger reading. It is a sampled guard, not a hard allocation limit. Receipt
durations include loading and shared-machine conditions; they are not latency
benchmarks. No quiet lease was requested.

## Stage 4 and HTTP checks

The [frozen source manifest](stage4/source-manifest.json) records 2,236 source
files based on commit `9c1b755d5914848cbc19e97286a70ca966871bdf`, including
uncommitted changes. The [build results](stage4/checks.json) and
[build guard](stage4/guard.json) record a successful server build, nine CLI
tests, 72 serving tests with eight ignored, and 22 backend tests with 16 ignored.
The [first build attempt](stage4/preflight-refusal.log) was refused before
starting Cargo because available memory was below 36 GiB; its failure was not
discarded. The later build completed under the configured memory guard.

Both HTTP checks used server binary SHA-256
`bb097ec7fb4c18134377b3877b0d66d2c6fc68812436c3bf10ecd9240b08a0d2`.
The [source review](http-source-review.json) confirms that all 2,236 source
hashes still matched at publication review and distinguishes this server from
the older Stage 2 test binary.

The subsequent [small GPU checks](stage4/small-gpu-checks.json) passed all
[16 backend tests](stage4/metal-gpu/output.log) and
[six serving tests](stage4/serve-gpu/output.log). The new direct-solve DeltaNet
test's correlated-key cases differed from the oracle by at most `2.90572643e-7`.
The subsequent weighted checks below cover the same direct-solve kernel on
Clef Flash.

| HTTP check | Comparison and outcome |
| --- | --- |
| [Flash default](flash-e2e-stage2/default.jsonl) | 40 requests, 104/104 top answers agree with the committed Flash CPU Q4_K_M reference; maximum probability difference `0.0007844769703932997`. Passed. |
| [Flash fused attention and shape-selected GEMM](flash-e2e-stage2/fused-shapes.jsonl) | 40 requests, 104/104 top answers agree with the same CPU reference; maximum probability difference `0.0009549690061149141`. Passed. |
| [Native 27B images](native27b-image-text-parity/0-images.json) | Six requests, 14/14 top answers agree with the official BF16 reference, exact input-token counts, and all ten HTTP refusals passed. Maximum probability difference `0.001797753043799255` is diagnostic; the image gate requires top-answer and token agreement plus refusals. Passed. |
| [Native 27B text](native27b-image-text-parity/1-text.json) | 40 requests, 94/104 top answers agree with the official BF16 reference; input-token counts agree. Maximum probability difference `0.2199534834774498`. M4's stated BF16 gate applies the original M1 bounds: 100% top-answer agreement and maximum difference at most `0.02`. Failed. |

The [Flash summary](flash-e2e-stage2/summary.json) and
[request and server records](flash-e2e-stage2/records.jsonl) retain both modes,
uncached responses, effective settings, startup, shutdown, and memory checks.
This comparator is the existing
[Flash CPU Q4_K_M corpus](../../e2e/psionic-cpu-q4_k_m.jsonl), not the new
official 27B BF16 reference. Both modes used a 2,048-token chunk and disabled
prefix reuse. The successful checks do not establish prefix or batch parity.

The [native 27B run receipt](native27b-image-text-parity/run.json) preserves the
overall failed status. It identifies the Q4_K_M trunk, external official BF16
head, vision manifest, 512-token chunk, and `q4k-on-demand` FFN placement.
The run stayed below its 52 GiB sampled memory cap, with no memory rejection or
cleanup failure. The image success does not override the text failure. All
per-question mismatches, probability differences, decision types, and top-two
margins remain in the text result. This comparison alone does not identify the
cause of the difference from the BF16 reference.

For reproduction, [the exact Flash harness](flash_e2e_check.py) and
[its configuration](flash-latency-config.json) are included. Copy them together
to a fresh task scratch directory, replace the artifact and binary path labels
with local paths, and run from the repository root with the required GPU and
memory leases. The harness uses the 40-request corpus; the latency configuration's
`rounds`, `runs`, `requests`, and `output_dir` do not select its corpus or output.
It creates `flash-e2e-stage2` beside itself and refuses to reuse an existing
directory. The [native configuration](native27b-image-text-config.json) is an
archived input to the repository's
[guarded native runner](../../tools/native_image_parity.py); its path labels
also require local replacements. Shared-machine request timings are retained
for diagnostics and are not a latency claim.
The reusable performance harness subsequently added an explicit external-head
option so future 27B timing runs can use the same official head; its
[self-test passed](stage4/harness-head-selftest.log). This Python-only change
does not change the tested Rust server binary.

## Stage 4 weighted DeltaNet checks

These runs used the Stage 4 serving test binary with SHA-256
`ef60f2f3842ab8d2ef5bbbecb6cbebe0395688b4baa2dc75ee449659b63997ca`,
built from the same frozen source snapshot as the `bb097ec7` HTTP server.
The [runner](stage4/gpu_checks.py),
[GPU lease receipt](stage4/weighted-gpu-lease.json), and
[memory lease receipt](stage4/weighted-memory-lease.json) record shared-machine
correctness checks, with a 36 GiB sampled memory cap and 12 GiB available-memory
floor. Both owned test groups stopped successfully, without guard failures.

| Configuration | Result |
| --- | --- |
| [Direct-solve DeltaNet](stage4/weighted-direct-delta/output.log), staged attention, fixed-64 GEMM, resident FFN | All 34 compared logits passed; maximum absolute difference `9.53912735e-4`. [Receipt](stage4/weighted-direct-delta/receipt.json). |
| [Direct-solve DeltaNet with fused attention, shape-selected GEMM, and on-demand Q4 FFN](stage4/weighted-direct-combined-delta/output.log) | Failed with maximum absolute difference `1.64407492e-3`. Three coordinates exceeded the test bound. [Receipt](stage4/weighted-direct-combined-delta/receipt.json). |

The combined failure reported question 0, option 0 at 155 tokens with chunk
sizes 155 and 257 (`-1.1325965` versus `-1.133729`, difference
`1.13248825e-3`), and question 4, option 5 at 2,032 tokens with chunk size 257
(`-0.6216057` versus `-0.62324977`, difference `1.64407492e-3`). The
[combined run record](stage4/gpu-retry-checks.json) preserves both outcomes.
The standalone pass does not validate the combined mode or native 27B DeltaNet.

## Native 27B CPU diagnostic

The [CPU result](cpu27b-diagnostic/text-014.json) uses the same Q4_K_M artifact,
official BF16 head, and `bb097ec7` server as the Metal HTTP comparison.
For `text-014`, both native CPU and Metal selected `export`, with a maximum
probability difference of `0.00023842464503166794`; the official BF16 reference
selected `downgrade_1`. CPU versus BF16 differed by at most
`0.09196533972951138`. All three reported 321 input tokens.

The request took 538.706 seconds with two configured CPU threads on the shared
machine. That duration is diagnostic, not a performance measurement. The
[run receipt](cpu27b-diagnostic/run.json) remains `partial_cancelled`:
[`text-034` was cancelled](cpu27b-diagnostic/cancellation.json) because its
estimated runtime would exceed the remaining deadline. Only the diagnostic's
owned processes were stopped. The receipt retains the interruption, completed
answer, cleanup observation, and confirmation that the owned group stopped;
the [memory lease receipt](cpu27b-diagnostic/lease.json) retains its nonzero exit.
There is no completed CPU result for `text-034`.

This one case shows that its BF16 disagreement also occurs on native CPU;
it does not establish the cause or explain the other text mismatches. The
[exact diagnostic script](cpu27b-diagnostic/run.py),
[selected requests](cpu27b-diagnostic/requests.jsonl), and
[server log](cpu27b-diagnostic/server.log) are included. As with the other
archived scripts, replace the path labels and select a fresh output directory
before reproduction. The script requires a memory lease and checks the pinned
server, model, and head hashes.

These weighted and CPU results precede the later normalization correction,
serial Metal wait removal, and expanded 27B GEMM routing. They do not validate
those changes. The earlier source manifests and review records remain unchanged.

## Stage 4 shared-machine latency

The [completed comparison](flash-latency-stage4/summary.json) ran two rounds
with five measured uncached requests per workload and variant in each round.
Variant order reversed between rounds. Warmups are excluded. Both variants used the
same `bb097ec7` server and Flash Q4_K_M artifact, a 2,048-token chunk, resident
FFN weights, sequential DeltaNet, and disabled prefix reuse.

| Actual input tokens | Default median | Fused attention and weight-shape GEMM median | Relative speed |
| --- | --- | --- | --- |
| 1,075–1,082 | 0.818956 s | 0.708045 s | 1.16× |
| 3,915–3,920 | 2.664879 s | 2.438215 s | 1.09× |
| 15,509–15,513 | 11.097903 s | 10.552463 s | 1.05× |

These are shared-machine measurements, with other applications running and
no quiet lease. The task ran no concurrent build or second model. System load
varied during the run. The results miss the absolute M3 targets of 0.35 s at
1k and 7.5 s at 16k; they cannot establish idle-machine acceptance.
[Request records](flash-latency-stage4/records.jsonl) retain real token counts,
queue and model timing, cache checks, paired nonces, and process cleanup.
[Metadata](flash-latency-stage4/metadata.json),
[configuration](flash-latency-stage4/config.json), and the
[GPU](flash-latency-stage4/gpu-lease.json) and
[memory](flash-latency-stage4/memory-lease.json) lease receipts identify the run.
This comparison precedes the next serial-wait reduction and normalization fix.

## Stage 4 Flash HTTP batching

The [batch check](flash-batch-stage4/summary.json) passes for two and four
concurrent request slots. Each mode compares 15 successful requests with the
same server's serial responses: every top answer agrees and the maximum
probability difference is zero. Cohort metadata confirms shared FFN work and
maximum groups of two and four. Each saturation burst receives nine successful
responses and three `busy` refusals; requests after a disconnected client and
after saturation succeed.

The [request records](flash-batch-stage4/records.jsonl) include 57 HTTP outcomes,
two disconnect attempts, exact question and option identities, token counts,
queue/model timing, artifact identity, and cleanup. The
[synthetic inputs](flash-batch-stage4/requests.json),
[configuration](flash-batch-stage4/config.json), and
[metadata](flash-batch-stage4/metadata.json) identify the 256-token chunk,
disabled prefix reuse, and frozen `bb097ec7` server. The run overlaps the
Stage 5 CPU compilation. Its 1.05× and 1.01× relative throughput values are
diagnostic only; it establishes correctness and recovery, not the planned
throughput improvement. It does not validate 27B batching.

## Stage 4 prefix reuse

Five changed-schema requests per model compare the same synthetic state with
prefix reuse disabled and enabled. Every compared raw logit is identical.
Each model has 21 question comparisons, including repeated exports from the
eviction and restoration checks. Whole-answer caching is disabled by raw-logit
export; the responses report the actual reused token counts.

| Model and evidence | Disabled median | Reuse median | Relative speed |
| --- | --- | --- | --- |
| [Flash](flash-prefix-stage4/summary.json) | 2.761632 s | 0.404329 s | 6.83× |
| [27B](native27b-prefix-stage4/summary.json) | 10.603304 s | 1.125043 s | 9.42× |

Both pass the stated `1e-3` logit and 5× reuse bounds on the shared machine.
These runs overlap CPU compilation and use raw-logit export; they do not
establish uncached or idle-machine latency. Both use the frozen `bb097ec7`
server and a 512 MiB cache limit. Flash uses the default kernels; 27B uses
fused attention, shape-selected GEMM, on-demand Q4 FFN weights, and the external
official BF16 head. State changes miss the cache, returning to an evicted state
misses, and the next identical-state request hits.

The [Flash requests and responses](flash-prefix-stage4/records.jsonl) and
[27B requests and responses](native27b-prefix-stage4/records.jsonl) retain
cleanup and memory receipts. Each directory also contains the input pairs,
configuration, binary/artifact identities, lease receipts, and raw logits under
`disabled/rows.jsonl` and `reuse/rows.jsonl`.

## DeltaNet normalization correction

The [executed reference source](delta-epsilon/reference-identity.json) normalizes
queries and keys as `x / sqrt(sum(x*x) + 1e-6)`. The native CPU, CUDA prefill,
and Metal prefill implementations instead clamped the norm after its square
root. The correction matches the reference for small norms. Its effect on
full-model accuracy remains unmeasured at this checkpoint.

The [RTX 4080 oracle check](delta-epsilon/cuda/summary.json) executes the changed
CUDA kernel in 12 synthetic cases, covering Flash and 27B head counts, zero and
small norms, dimension tails, guards, and repeatability. All 367,488 comparisons
pass an independent f64 oracle. The largest gate error is `1.003e-7`, and the
largest normalized-key error is `1.118e-8`. This run loads no model. The
[oracle source](delta-epsilon/cuda/oracle.cu),
[guard](delta-epsilon/cuda/guard.py), and
[retained hashes](delta-epsilon/cuda/retained-hashes.json) record reproduction
inputs and bounded execution.

A [sampled artifact comparison](delta-epsilon/static-weight-check.json) finds
compatible 27B projection rows, exact converted normalization and convolution
weights, and the expected value-head permutation. This supports compatible
weights and layout; the GGUF metadata does not bind an HF source revision.
Tiny individual convolution channels do not establish tiny complete query or
key norms. Activation evidence is still needed before attributing the text
accuracy difference to normalization or quantization.

## Earlier Flash and Metal checks

The Stage 2 serving test binary has SHA-256
`89f9f6f09cc19cdf0ad74a197ca641081edef78285b2856c8e7c14aa8523b073`.
The weighted tests below used Clef Flash. Their historical names mention CUDA
even when the selected device is Metal; the receipts and environment identify
the actual configuration.

| Check | Result |
| --- | --- |
| [Default weighted check](stage2/weighted-default/output.log) | Passed chunk comparisons and CPU comparison; maximum reported CPU logit difference `2.79e-4`, probability difference `1.35e-5`. [Receipt](stage2/weighted-default/receipt.json). |
| [Fused attention and shape-selected GEMM](stage2/weighted-fused-shapes/output.log) | Passed; maximum reported CPU logit difference `1.31e-3`, probability difference `5.55e-5`. [Receipt](stage2/weighted-fused-shapes/receipt.json). |
| [On-demand Q4 FFN](stage2/weighted-q4-ffn/output.log) | All 34 compared logits were identical to resident FFN weights. [Receipt](stage2/weighted-q4-ffn/receipt.json). |
| [Earlier block DeltaNet](stage2/weighted-block-delta/output.log) | Numerical test passed with maximum logit difference `8.33928585e-4`, but the guard reported an exit-race failure. The overall run failed. [Failed receipt](stage2/weighted-block-delta/receipt.json). |
| [Earlier block DeltaNet retry](stage2/weighted-block-delta-retry/output.log) | Numerical test and guard passed after bounded exit handling was added. [Receipt](stage2/weighted-block-delta-retry/receipt.json). |
| [Earlier combined DeltaNet configuration](stage2/weighted-combined-delta/output.log) | Failed: `-1.1321846` versus `-1.133729` at 155 tokens exceeded the test bound. [Receipt](stage2/weighted-combined-delta/receipt.json). |
| [Synthetic backend checks](stage2/metal-gpu-retry.log) | 15 tests passed, covering buffers, kernels, real GEMM shapes, and attention boundaries. |
| [Synthetic serving checks](stage2/serve-gpu.log) | Six tests passed. [Receipt](stage2/serve-gpu.json). |

Both historical DeltaNet results precede the direct-solve correction. Neither
the standalone pass nor the combined failure validates that correction; the
Stage 4 weighted results above evaluate it separately. These earlier Flash
results do not validate native 27B answers, prefix reuse across full requests,
or continuous-batch request parity; the later HTTP comparisons are separate
evidence above.

## Image preprocessing failures and correction

The [Stage 2 CPU test log](stage2/serve.log) failed because the pinned JPEG
decoders differed by three RGB levels. The
[decoder diagnostic](jpeg-diagnostic/comparison.json) separated decoding from
bicubic resizing: resizing identical decoded RGB matched the reference exactly.
The JPEG-only allowance was corrected without relaxing PNG or lossless WebP
checks.

The [Stage 3 CPU test log](stage3/serve.log) then failed on the EXIF-tagged JPEG:
Rust reported grid `[1,20,18]`, while the actual base64 reference reported
`[1,18,20]` (70 tests passed, one failed, eight were ignored).
[The decoding diagnostic](image-exif-diagnostic/reference-decoding.json) showed
that the separate PIL loader applied EXIF rotation, but the released
Torchvision base64 path did not. The generator had mixed those entry points.
Rust and the generator now both preserve the byte decoder's stored pixel order;
the generator also checks its intermediates against the complete processor.

After that fix, a [small Rust wrapper](image-exif-diagnostic/all_image_tests.rs)
included the actual production JSON, encoding, media, and resize modules and
linked the existing model library. All
[14 actual tests passed](image-exif-diagnostic/rust-all-image-tests.log), including
every assertion across the eight image fixtures, normalized patch hashes,
grids, token placement, and three-axis positions. The
[build receipt](image-exif-diagnostic/build-lease-lld.json) records the bounded
build. The earlier failed full serving runs remain recorded. The later
[Stage 4 serving rerun](stage4/serve.log) passed all 72 selected tests, with
eight hardware tests ignored in that CPU invocation.

## Guard and RTX 4080 evidence

The [original guard self-test failure](stage2/guard-self-test.log) and
[physical-footprint cleanup diagnostic](stage2/guard-diagnostic-physical_footprint_limit.json)
are retained. The monitor now uses scoped owned-group sampling and bounded
exit confirmation. The
[final full guard self-test](stage2/guard-full-selftest-afterexitgrace.log)
passed after the exit-race correction. The injected monitor, timeout, and
host-memory-floor diagnostic receipts are also included; their forced rejection
is expected test behavior.

The [RTX 4080 summary](cuda-4080/summary.json) records two successful capacity
checks: explicit `cuda` and `auto` both refused the 27B artifact before weight
upload. Required memory was 20,927,797,248 bytes, versus 5,549,588,480 free bytes
and 16,711,352,320 total bytes reported by CUDA. Neither run started serving or
fell back to CPU. Existing GPU process counts and aggregate memory readings
were unchanged. These were startup checks with zero inference requests.
The [source comparison](cuda-4080/admission-source-match.json) identifies the
tested admission code and later unrelated source changes.

## Publication and integrity

[manifest.json](manifest.json) maps every copied file's original SHA-256 and
byte count to its published path, SHA-256, and byte count. Private path prefixes
become `repo`, `run-artifacts`, `cargo-target`, `reference-python-env`, or
`user-home`. The checkpoint path becomes the public artifact label. These labels
describe relocated evidence; they are not executable local paths.

The two complete golden files differ from their originals only in the checkpoint
label. All numerical fields and complete case contents are preserved. The
[4080 sanitization record](cuda-4080/sanitization.json) separately links its
summaries to raw inputs and explains the removal of unrelated GPU process names
and commands. No model weights, binaries, private process command lines, or
large memory timelines are included. References to `memory.jsonl` inside receipts
name retained local evidence that is deliberately not copied here.

The manifest's initial source snapshot identifies reviewed files, while the
separate Stage 4 source manifest and build records identify the later tested
server. Neither substitutes for the source of an earlier test binary. Add later
validation as new evidence, retaining these failures and their original hashes.
The [follow-up publication check](stage4-followup-publication-check.json)
verifies the added weighted and CPU records, recomputes the CPU comparison,
and confirms that all earlier copied files and manifest entries stayed intact.
