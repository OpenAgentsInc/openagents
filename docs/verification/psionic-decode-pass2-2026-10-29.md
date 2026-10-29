# Original GPT-OSS performance, second pass — 2026-10-29

The [performance notebook](../../psionic/docs/serving/performance.md) documents
our process, the first pass, rejected experiments and a prioritized optimization
backlog across kernels, submission, transport, prefill, concurrency and startup.
This record measures the second pass against the already optimized first pass;
[first-pass samples](psionic-decode-speed-2026-10-29.md) remain unchanged.

## Changes and reasoning

A bounded instrumented decode step at position 1024 identified approximately
6.4 ms in sequential softmax, 1.9 ms in scores and 1.6 ms in context. Event
instrumentation synchronizes and affects scheduling; these are hotspot clues,
not throughput measurements. The uninstrumented paired runs decide speed.

- Decode scores/context reuse the existing admitted 64-query/8-KV prefill
  layout helpers and unsigned-32 tensor offsets, with generic fallbacks.
- Softmax retains its exact maximum, exponentials, sink and sequential f64
  denominator. A separate ordered kernel performs each original f64 division
  in parallel. No reciprocal approximation or reduction-tree change is used.
- The per-head denominator reuses the 64-f64 norm scratch after its prior
  lifetime ends. Workspace stays 527933440 bytes at the original 20b/2048 plan.
  The generated module gains two entry points (45 total). Coax owns both stages
  and their order; C still only performs effects.

`gpt-oss-cuda-f32-v1` and `psionic-gptoss-responses-v1` retain their arithmetic
and wire behavior. Original coefficient bytes and all model choices stay fixed.

## Correctness and measurement

- Authored checks compare frozen decode attention with reused layouts across
  full/wrapped history, head/slot endpoints and pending rows, and compare all
  split-softmax values and untouched tails bit for bit with the original row.
- The plan checks denominator extent/type, stage adjacency, work count and
  lifetime. Focused device integration checks pass in debug/release and native
  plain/ASan/UBSan, including independent tiny-model references and failure
  publication boundaries. The book transcript records the new entry count.
- A full original 20b/2048 paired run authenticates once, alternates variants,
  warms up and retains five measured repeats. Baseline and candidate use their
  actual Coax launch programs. A test-only shared baseline translation supplies
  the old program; compatibility exports are loaded but never launched by it.
  Production has no program/library switch. No fake extra baseline launches
  are included in the before timing.
- Entire caches, logits, residual traces and expert IDs agree bit for bit after
  each fixed trial. Actual greedy generated IDs and derived stops agree on every
  trial. Per-token selection, synchronization and control reads match serving.
- Owned softmax slices compare the old row and new two-stage implementation on
  the same module/input, with exact active-value and untouched-tail agreement.
- Existing unchanged wire/fixture behavior uses the first-pass passing results;
  the actual GPU-backed API is tested again because its backend changed.
  No full mutation roots or original-model CPU inference are run.

The [retained folder](../../psionic/fixtures/gptoss/qualification/decode-pass2-2026-10-29/)
holds the instrumented baseline profile, raw paired samples, parsed summary,
component slice and actual API timing/build identities. Hardware is the owned
RTX 4080 (16 GiB), NVRTC 13.2, original 20b revision
`6cee5e81ee83917806bbde320786a8fb61efebee`, context capacity 2048.

## Results

| Paired core workload | First pass | Second pass | Improvement |
| --- | ---: | ---: | ---: |
| 1024 input / 128 forced continuation IDs | 39.218 decode IDs/s | 53.157 decode IDs/s | 35.5% |
| 70 input / 128 actual greedy response IDs | 53.974 decode IDs/s | 62.453 decode IDs/s | 15.7% |

These are current paired baselines, rather than ratios against historical
samples from a different run. Greedy decode excludes the first ID selected by
prefill. Core rates exclude tokenization, HTTP and serialization. Prefill's
implementation is unchanged and its long-prompt bottleneck remains open.

The owned 64-head softmax slice, including both new launches, falls from
0.064922 to 0.028093 ms at 128 visible positions and from 0.527142 to
0.206195 ms at 1025 positions (medians). It compares the original row against
the two stages on the same module and inputs, including untouched tails.

The actual optimized endpoint's five measured 70-input/128-output SSE requests
complete in a median **2.349427 s**, or **54.481 output IDs/s** end to end.
Its output-state digest is identical to the first pass. Median first semantic
delta is **0.334685 s** and first visible text is **0.647178 s**. The earlier
separate HTTP sample was 2.622076 s / 48.816 IDs/s, with 0.310010 s to its first
semantic delta and 0.616299 s to visible text (computed from retained rows).
This is not a freshly paired HTTP baseline: no first-delta or first-visible
latency improvement is claimed. Core prefill remains about 0.239 s for 70 IDs
and 3.21 s for 1024 IDs. The current gain is decode and total response time.

Real GPU-backed Responses text/stream accumulation, function handoff and typed
replay, cancellation/reuse, busy admission and supported Open Responses schemas
pass on the final backend. The temporary listener and key are removed afterward.
The [build record](../../psionic/fixtures/gptoss/qualification/decode-pass2-2026-10-29/build.json)
records the actual native/modules/host translations and the profiling snapshot
limitation. The paired and serving identities bind the actual generated programs, native
executables, headers and touched sources.

## Reproduce and scope

The paired tool accepts `--baseline-plan ./baseline-host.so` when the candidate
changes dispatch. Build that trusted shared Coax translation from the previous
source, using matching public struct layouts and local symbol binding. The
baseline CUDA adds only unused compatibility entry points for lookup. Record
both binaries/modules and the baseline shared translation. The default tool
continues comparing modules under one current program when dispatch is unchanged.

`gptoss_speed.c --profile` (after its four path arguments, compiled with
`PSIONIC_DEVICE_TEST`) collects a bounded single-step kernel profile.
`gptoss_softmax_bench.c` takes the candidate CUDA path and compares the retained
old complete-row kernel with the two new stages. The report tool accepts
`--before-label` / `--after-label` so new experiments do not inherit old labels.
`responses_speed.py` reports both first semantic delta and first visible text;
neither creation time nor a reasoning delta is the visible answer.

Five repeats are a smoke benchmark, not production capacity qualification.
Concurrency, tensor-core prefill, graph replay, prefix reuse and remaining
items in the backlog are not claimed implemented by this pass.
