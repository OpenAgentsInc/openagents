# Making Psionic serving faster

This is the working performance notebook and optimization backlog for our
resident original GPT-OSS engine. Dated records retain measurements; this page
explains the method, what changed, unsuccessful experiments, and what to try next.
Ideas below are hypotheses for our implementation, not promised speedups.

## What the first decode pass changed

The first pass shipped in `8e6304adf0`. Its
[record](../../../docs/verification/psionic-decode-speed-2026-10-29.md) and
[raw samples/build identities](../../fixtures/gptoss/qualification/decode-speed-2026-10-29/)
are retained in Git.

The packed BF16 and MXFP4 single-token products still did checked signed-64
index arithmetic inside their coefficient loops. Our memory plan already proves
the relevant tensor-relative offsets fit unsigned 32 bits. We changed those
address calculations, hoisted expert-row/input-slot bases and retained every
view check, original coefficient byte, lane assignment, f32 fold and ordered f64
reduction. The arithmetic profile did not change. TCP_NODELAY also avoids
coalescing delays for small SSE writes.

On the owned RTX 4080, the expert-up slice fell from roughly 0.401 to 0.195 ms;
expert-down from 0.215 to 0.105 ms. Full-model rates were:

| Workload | Before | After |
| --- | ---: | ---: |
| 1024 input IDs, 128 forced continuation IDs | 29.193 decode IDs/s | 39.244 decode IDs/s |
| 70 input IDs, 128 actual greedy response IDs | 36.611 decode IDs/s | 53.843 decode IDs/s |
| Optimized Responses SSE, including prefill and transport | No paired HTTP baseline | 48.816 output IDs/s |

The 128-ID HTTP request took a median 2.622076 s, with its first semantic delta
at 0.310010 s. The 1024-ID prefill remained about 3.22 s. A bit-built coefficient
scaler matched values but ran slower; explicit expert-shape specialization added
no useful gain. Both experiments were discarded.

## The repeatable process

1. **Name the target.** Separate cold startup, tokenization, reset, prefill,
   first selected ID, first reasoning delta, first visible text, steady decode,
   total response latency and aggregate throughput. Improving one does not prove
   another improved. Count framing/reasoning IDs honestly.
2. **Freeze identity and work.** Record source/compiler/generated-code/executable
   hashes, native flags, driver/device, original artifact revision, numerical
   profile, context capacity, prompt/ID workload, output limit and concurrency.
   Use one GPU experiment at a time. Keep weights resident across requests.
3. **Locate the cost.** Collect a bounded profile by kernel class and host phase.
   CUDA event instrumentation adds launches/synchronization; its measurements
   locate costs, while uninstrumented runs decide end-to-end speed.
4. **Start small.** Owned original-shaped slices have independent expected
   answers. Alternate before/after modules and displace cache when studying
   streaming coefficients. Check occupancy, registers, spills, instruction and
   memory traffic when a plausible change disappoints.
5. **Prove behavior.** Preserve exact outputs for transformations of the current
   arithmetic. Compare partials, full caches, logits, traces, expert IDs, chosen
   IDs and stops, including wrapped windows and ragged work. A different numerical
   scheme gets a new named profile and frozen tolerances before measurement.
6. **Measure the complete engine.** The paired harness authenticates once and
   alternates modules over one resident weight allocation. Use a warm-up and five
   measured repetitions, alternating order. Separate prefill/decode. One workload
   forces IDs to control work; another follows real greedy choices. Include the
   per-token selection/wait/control-read used by serving.
7. **Measure the actual API.** Run the same frozen text over SSE, retain all
   samples and a content digest, and check SDK/tool continuation, cancellation,
   admission and error behavior. Do not log reasoning or user content.
8. **Keep or reject.** Retain raw samples, min/max/median and exact build identities.
   Explain limitations and rejected ideas. Push only a measured improvement with
   passing focused checks. Never call a microbenchmark gain a full-service gain.

The tools are [decode slices](../../tests/gptoss_decode_bench.c),
[paired full model](../../tests/gptoss_speed.c),
[paired report](../../tests/gptoss_speed_report.py),
[HTTP timing](../../tests/responses_speed.py), and
[API conformance](../../tests/responses_api.py). Default checks use owned small
fixtures. No routine original-model CPU inference or full mutation runs.

This follows an iterative measurement approach; NVIDIA's
[best-practices guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/index.html)
and [graph documentation](https://docs.nvidia.com/cuda/cuda-programming-guide/04-special-topics/cuda-graphs.html)
are primary references for investigating device memory and submission overhead.
All shipped arithmetic and scheduling decisions remain Coax.

## A concrete Linux/CUDA experiment

Generate the candidate host ABI and CUDA from the same trusted Coax revision.
Use a private temporary directory for output:

```shell
bench_dir=$(mktemp -d /tmp/psionic-bench.XXXXXX)
coax/bin/coax c psionic/lib/responses/server-host.coax --library --release \
  --header "$bench_dir/device-host.h" > "$bench_dir/device-host.c"
coax/bin/coax cuda psionic/lib/gptoss/device-kernels.coax > "$bench_dir/after.cu"
python3 psionic/tests/gptoss_device_bindings.py "$bench_dir/device-host.h" \
  > "$bench_dir/gptoss-device-cpu.h"
cc -std=c11 -O3 -g -ffp-contract=off -fno-fast-math \
  -DPSIONIC_DEVICE_TEST -I"$bench_dir" -Ipsionic/platform psionic/tests/gptoss_speed.c \
  "$bench_dir/device-host.c" psionic/platform/cuda-host.c -ldl -lm \
  -o "$bench_dir/paired"
```

Supply trusted before/after CUDA modules and an authenticated original cache.
When dispatch changes, build a matching baseline Coax shared translation, with
its own symbols bound locally (`-shared -fPIC -Wl,-Bsymbolic` on this worker),
and load it only in the test harness. Lookup compatibility exports in the before
module must remain unused by its original program. Verify this instead of
adding fake launches to baseline timing. The second-pass build record captures
all these identities.

```shell
"$bench_dir/paired" ORIGINAL_CACHE BEFORE.cu "$bench_dir/after.cu" HARMONY.hst \
  --baseline-plan ./baseline-host.so > "$bench_dir/paired.log"
python3 psionic/tests/gptoss_speed_report.py \
  "$bench_dir/paired.log" "$bench_dir/paired.json" \
  --before-label 'qualified baseline' --after-label 'candidate'
```

Omit `--baseline-plan` when both modules use the same Coax dispatch program.
For submission-only comparisons, pass the same module twice and use
`--host-replay` instead. This explicitly enables graph replay only in variant 1;
other harness modes disable replay so replacing test kernel tables cannot reuse
stale graph bindings. The benchmark requires `PSIONIC_DEVICE_TEST`; production
sessions retain their immutable module and expose no such switch.
Then check actual HTTP on a dedicated test instance with its key already in the
environment; the benchmark sends six bounded requests and checks a state digest:

```shell
python3 psionic/tests/responses_speed.py http://127.0.0.1:8080 \
  --out "$bench_dir/responses.json"
```

Record native flags, exact baseline source/layout compatibility, binary/module
hashes and every sample before removing temporary files. Retain published dated
records as history; a later pass gets its own folder and comparison labels.

## Optimization backlog

P0 means measure/try next on the current exact profile. P1 means a larger exact
engineering change. P2 requires a separate numerical, scheduling or capability
qualification. Startup and multi-user improvements have their own metrics.

### Kernel execution and memory

| Priority | Candidate | Intended target and gate |
| --- | --- | --- |
| Done, pass 2 | Apply admitted 32-bit indexing to decode attention | Scores/context still have signed-64 offsets and repeated modulo. Reuse qualified prefill helpers where layouts agree; exact wrapped/full-cache parity. |
| P0 | Remove repeated cache/address work | Hoist head/row/slot bases, use admitted power-of-two ring masks, and create bounded subviews outside inner loops. Keep generic-shape coverage. |
| P0 | Profile norm, softmax, routing and head separately | The largest cost can move after expert products improve. Optimize measured total contribution. |
| P0 | Tune block size and row/lane mapping per kernel | Sweep actual original shapes, register count and spills; retain the smallest measured policy, not a universal block size. |
| Done, pass 3 | Avoid duplicate stream waits | Queue the control download behind kernels on the same ordered stream and synchronize once. Check failure propagation and poisoned-session behavior. |
| P0 | Remove bounds/division work proven redundant by admission | Keep views checked; specialize safe constants only when code size, compilation and actual runtime improve. |
| P1 | Compute norm denominators once | Current output lanes repeat the same ordered reduction. Share the exact f64 denominator; account for added launches/barriers. |
| Done, pass 2 | Parallelize softmax normalization divisions | Preserve the original ordered denominator and each f64 division; avoid reciprocal replacement. |
| P1 | Parallelize softmax exponentials | Preserve sink mass and the ordered denominator, stage row maxima/exponentials, then normalize. Compare long and window attention. |
| P1 | Fuse compatible pointwise operations | Norm scale, bias, residual, activation, flags and trace copies may share passes. Preserve atomic publication and diagnostics. |
| P1 | Tile attention over cached keys/values | Reuse queries, probabilities and GQA KV heads across lanes. Measure memory traffic and retained reduction order. |
| P1 | Reduce partial-buffer traffic | Use registers/shared storage and cooperative reductions where the language can express/test their participation rules. |
| P1 | Improve coefficient-load coalescing | Adjacent lanes, packed width, vector loads, alignment and cache policy; measure bytes transferred per useful coefficient. |
| P1 | Repack/interleave resident rows from authenticated originals | Measure coalescing and cache reuse; prove the layout mapping and retain original artifact identity. |
| P1 | Reuse selected coefficients across requests/tokens | Expert grouping, tiling and shared-memory staging; keep original packed bytes and explicit workspace budgets. |
| P1 | Tune expert grouping and sparse dispatch | Compact only actual selected work; reduce padded tiles, grouping scans and idle experts, preserving tie order. |
| P1 | Optimize vocabulary projection and argmax | Test row tiling, fused partial reduction/top-choice and deterministic lowest-ID ties. Full logits remain available for required consumers. |
| P1 | Avoid unnecessary diagnostic copies | Make retained trace/readout needs explicit, with a separately checked serving execution plan. Keep conformance mode complete. |
| P1 | Avoid clearing scratch overwritten before read | Prove write-before-read per buffer; retain fresh KV/control state and failed-step atomicity. |
| P2 | Cooperative BF16/FP16 matrix operations for dense prefill | Coax-authored tiles, capability/participation admission, independent scalar references and frozen error bounds. |
| P2 | Matrix-hardware products from original MXFP4 | Decode bounded tiles, split activations into limbs if needed, measure accuracy/route margins before enabling a new profile. |
| P2 | Fused/online attention with a different reduction tree | Separately name numerical behavior; preserve sinks, YaRN, causality and sliding history. |
| P2 | Alternative cache precision or layout | Measure bandwidth, memory capacity and error across long histories; keep the current f32 reference. |
| P2 | Prove per-binding nonalias hints | Emit compiler hints only for admitted disjoint spans, retaining legal in-place kernels and raw ABI semantics. |
| P2 | Additional lower-precision execution profiles | Quantized activations/caches, TF32/FP8 or other formats require device capability checks, frozen error/route gates and separate profile names. |
| P2 | Profile-guided code generation | Unrolling, strength reduction, vectorization and register caps should improve measured workloads without excess compile time or spills. |

### Host submission and API latency

| Priority | Candidate | Intended target and gate |
| --- | --- | --- |
| P0 | Attribute host plan construction and launch cost | Measure Coax plan admission, native argument packing, driver enqueue and waiting separately from GPU work. |
| P0 | Reuse bounded host buffers | Remove per-token/request allocation and repeated setup where exclusive ownership is explicit. |
| P0 | Incremental Harmony parsing | Current snapshots reparse the bounded continuation. Retain typed header/body/UTF-8 state and identical event boundaries. |
| P0 | Serialize only newly required SSE bytes | Avoid recopying closed items and redundant JSON construction; preserve event order, stable IDs and backpressure. |
| P0 | Measure first visible text separately from reasoning | Early creation/reasoning events are not the visible answer; report both distributions. |
| P1 | Dedicated model thread and overlapped output work | Keep the CUDA context and worker on one owned thread; move parsing/sending off its critical path under explicit bounded queues. |
| P1 | Cache immutable launch bindings | Pre-admit coefficients/views and patch bounded token/position scalars in Coax; preserve all dynamic checks. |
| Done for synchronous decode, pass 3 | CUDA graph replay | Coax compares admitted descriptors; update only changed nodes in a retained ordered graph. Device-side control scalars and batched-prefill graphs remain future experiments. |
| P1 | Bounded asynchronous decode/lookahead | Overlap host parsing/sending with GPU work; avoid computing past a return/handoff or cancellation without an explicit rollback rule. |
| P1 | Pinned control-transfer memory and event completion | Measure pageable-copy overhead and per-token host wakeups; keep lifetimes and failure handling explicit. |
| P1 | Batch harmless transport writes | Balance syscall count against first-delta/tail latency; respect slow clients and never merge semantic items. TCP_NODELAY is already enabled. |
| P1 | HTTP connection reuse | Bounded keepalive and framing tests could reduce request setup; no ambiguous bodies or pipelining surprises. |
| P1 | Tokenization and rendering caches | Cache immutable system/tool templates or exact tokenized prefixes under their tokenizer/template identities. |
| P1 | Faster tokenization/JSON admission | Profile first; improve lookup/allocation and scanning while keeping duplicate-key, UTF-8 and size checks. |

### Prefill, concurrency and model strategy

| Priority | Candidate | Intended target and gate |
| --- | --- | --- |
| P0 | Tune prefill chunk/token/row tiles | Measure 1/32/128/512/1024-token prompts and cancellation delay; the fixed 1024-token prefill still exceeds the 2 s goal. |
| P1 | Skip redundant final heads during prompt chunks | Only request choices/readouts when needed; preserve all arithmetic/refusal guarantees. |
| P1 | Exact prefix KV reuse | Key by original model, template, tokens and numerical profile; isolate tenants and invalidate safely. Measure hit/miss latency. |
| P1 | Chunked prefill scheduling | Interleave long prefills with decode so one prompt does not stall every active stream. |
| P1 | Continuous batching | Coax owns slot admission and per-request stops/deadlines. Measure aggregate and per-request latency at concurrency 1/4/8. |
| P1 | Paged/bounded KV allocation | Admission, fragmentation, prefix sharing, eviction and request isolation precede increased concurrency. |
| P1 | Separate prefill/decode queues and budgets | Avoid head-of-line blocking; report fairness and tail latency alongside throughput. |
| P2 | Device speculative verification | Batch target verification, preserve complete snapshots and correct rejection law; require a net measured speedup. |
| P2 | Prompt-lookup or smaller-model drafts | Measure acceptance and draft cost on realistic tasks; preserve greedy equivalence or the stated sampling law. |
| P2 | Adaptive batching or speculation policy | Base decisions on measured load/acceptance with authored checks, not output-quality shortcuts. |
| P2 | Length-aware request packing | Reduce padding and select batch shapes under fair scheduling; measure short-request tail latency. |
| P2 | Multi-GPU partitioning | Tensor/pipeline/expert placement, communication overlap and topology-aware budgets; qualify actual available hardware. |
| P2 | Additional GPU/Metal backends | Capability/numerical parity and actual device measurements; not inferred from memory capacity alone. |

### Startup, operations and measurement

| Priority | Candidate | Intended target and gate |
| --- | --- | --- |
| Startup | Parallel artifact authentication and buffered hashing | Independent files can hash in parallel; each SHA-256 file retains its standard chaining and complete byte coverage. No unchecked result cache. |
| Startup | Coax-defined SHA acceleration | Exact hardware/vector lowering with scalar oracles, preserving original pinned digests; measure cold admission separately. |
| Startup | Cache generated/compiled modules by exact identity | Include Coax/compiler/NVRTC/device/profile/flags; authenticate cache contents before use. |
| Startup | Overlap authenticated reads and upload | Coax owns what bytes may publish; bounded staging and cancellation. |
| Startup | Warm admitted workers and model pools | Separate cold admission time from request latency; budget memory and avoid repeated uploads. |
| Operations | Higher-bandwidth or matrix-capable hardware experiments | Qualify available devices and actual price/performance; buying or reconfiguring hardware remains an owner decision. |
| Operations | GPU clocks, thermals and contention records | Log observations; compare paired runs on the same owned device, without changing owner hardware policy. |
| Operations | Dedicated worker/process and CPU affinity experiments | Measure launch jitter and tail latency; avoid disturbing other agents or workloads. |
| Measurement | GPU events plus bounded Nsight traces | Explain instrumentation overhead; collect registers, spills, occupancy, memory throughput and launch gaps. |
| Measurement | A representative prompt/context suite | Short chat, long context, tools, structured data and coding; frozen outputs/limits and warm/cold cases. |
| Measurement | Packaged-engine comparison on the same device | Benchmark-only reference, exact artifact/context/precision identities; never hide an external runtime in Psionic. |
| Measurement | Bounded automatic performance regression checks | Use our own idle workers, frozen identities and variance budgets; never GitHub Actions or an uncontrolled benchmark as a release verdict. |
| Measurement | Latency/load curves and regression budgets | P50/P95/P99 with enough samples, concurrency, power/memory and cost; distinguish throughput gains from worse individual latency. |

## Current second pass

The initial bounded profile identified about 6.4 ms in sequential softmax,
1.9 ms in scores and 1.6 ms in context for one 1024-prefix decode step.
Instrumentation changes scheduling, so those are hotspot clues rather than
end-to-end timing. Preserve the ordered denominator, parallelize its independent
normalization divisions, and reuse existing 32-bit attention layouts. The first
pass is the new baseline. Record the second pass separately; do not overwrite prior samples or
reuse its old before/after labels for a new experiment.

The [second-pass record](../../../docs/verification/psionic-decode-pass2-2026-10-29.md)
now retains the profile, component slice, complete-state/ID parity, matched Coax
program A/B and actual API samples. On current paired runs, long-prefix decode
improves 39.218 → 53.157 IDs/s and short greedy decode 53.974 → 62.453 IDs/s.
The endpoint measures 54.481 output IDs/s including prefill and transport.
First-visible latency is explicitly recorded and is not claimed improved;
further phase/transport attribution remains a next measurement.

## Third pass: ordered graph replay

[Chapter 45f](../../book/ch045f-replay-the-device-program.md) now implements
Coax descriptor deltas and synchronous decode replay. Kernel arithmetic and
generated CUDA are unchanged. The adapter retains an ordered node chain,
updates changing scalar/view bindings, and submits one graph. CPU emulation,
asynchronous work and per-kernel profiling keep ordinary launches. A single
ordered control download/wait replaces the old redundant pre-download wait.

The [third-pass record](../../../docs/verification/psionic-decode-pass3-2026-10-29.md)
and [raw samples](../../fixtures/gptoss/qualification/decode-pass3-2026-10-29/)
retain all results. Both paired core variants use the same single download/wait;
the measured comparison therefore isolates graph replay:

| Workload | Ordinary launches | Graph replay | Change |
| --- | ---: | ---: | ---: |
| 1024 input / 128 forced IDs | 53.789 decode IDs/s | 54.236 decode IDs/s | +0.8% |
| 70 input / 128 greedy IDs | 63.174 decode IDs/s | 64.007 decode IDs/s | +1.3% |
| Fresh HTTP batches, including all request work | 56.579 output IDs/s | 56.747 output IDs/s | Negligible (+0.3%) |

Complete state and output IDs remain bitwise equal. Five HTTP samples follow
a warm-up per variant; candidate and baseline batches run sequentially, so this
is a weaker comparison than alternating core pairs. Candidate response time is
2.255627 s, first semantic delta 0.309761 s and first visible text 0.568112 s.
The fresh baseline is 2.262329 / 0.306640 / 0.571821 s respectively. No
meaningful API latency gain is claimed. Compare with a fresh baseline before
attributing a historical timing difference to an optimization.

Rejected owned-slice experiments included a smaller FP4 nibble lookup,
quarter-word extraction, hoisted input subviews and two/four-row expert tiles.
None produced a convincing useful win; row tiles were neutral or slower.
They are absent from the shipped kernels. The retained exploratory logs do not
constitute a qualified alternative backend.

Next, prioritize coefficient/matrix execution and prefill rather than expecting
large submission-only gains. Device-side controls could remove graph updates;
measure that against this small ceiling. Keep matrix instructions, fusion,
prefill replay and concurrency as separate behavior/performance gates.
