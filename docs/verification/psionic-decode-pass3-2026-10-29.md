# Original GPT-OSS performance, third pass — 2026-10-29

This pass implements synchronous CUDA graph replay without changing numerical
or API profiles. The measured core gain is small: **0.8%** on the long-prefix
case and **1.3%** on actual short greedy decode. Coefficient and arithmetic
execution remain the larger throughput target.

The [performance notebook](../../psionic/docs/serving/performance.md) retains
our process and backlog. [Chapter 70](../../psionic/book/ch070-replay-the-device-program.md)
explains the implementation and checks. The
[retained folder](../../psionic/fixtures/gptoss/qualification/decode-pass3-2026-10-29/)
holds raw samples, parsed summaries, measured harness source and build identities.
Earlier dated records and their samples are unchanged.

## Change

Coax still admits a complete `gd_program` each step. Its new replay policy
compares semantic descriptors, lists changed nodes, and requests a rebuild on
kernel-sequence/argument-count changes. Capacity refusal leaves the update list
untouched. Token, position, history extent and output limit remain checked.

C constructs an explicit linear graph: each kernel node depends on its
predecessor. It retains original node handles, the original graph and its
executable. Only listed nodes get new parameters before one replay launch.
Immutable coefficient/workspace bindings survive resets with their session.
This does not depend on unspecified graph enumeration order.

Replay is restricted to synchronous GPU steps without per-kernel profiling.
CPU tiny emulation, asynchronous steps and instrumented profiling use the
ordinary stream path. The ordered control download's wait completes the step;
the old extra wait before enqueueing that download is removed. Driver failures
poison the session. Closing waits and destroys graphs before buffers/modules.
Publication, numeric flags, stops and cancellation boundaries are unchanged.

The CUDA module is byte-identical to pass 2: 45 entry points, original
coefficient bytes and `gpt-oss-cuda-f32-v1` arithmetic. Original 20b revision is
`6cee5e81ee83917806bbde320786a8fb61efebee`; capacity 2048; concurrency one;
owned RTX 4080 16 GiB; GCC 15.2.0 and NVRTC 13.2. The
[build record](../../psionic/fixtures/gptoss/qualification/decode-pass3-2026-10-29/build.json)
binds actual binaries, generated translations, flags and touched sources.

## Core measurements

One authentication/upload; the same module/program for both variants; one
warm-up and five measured paired trials with alternating order. Variant 0 uses
ordinary launches; variant 1 uses graph replay. Both variants already use one
ordered control download/wait, so this comparison isolates replay rather than
claiming the removed duplicate wait as an additional measured gain.

| Core workload | Stream launches | Graph replay | Improvement |
| --- | ---: | ---: | ---: |
| 1024 input / 128 forced continuation IDs | 53.789 decode IDs/s | 54.236 decode IDs/s | 0.8% |
| 70 input / 128 actual greedy response IDs | 63.174 decode IDs/s | 64.007 decode IDs/s | 1.3% |

Both cases select on device and read control per token. Core timing excludes
HTTP/tokenization; greedy timing excludes the first ID selected by prefill.
Every fixed pair has bitwise-equal complete cache, logits, traces and routes.
Every greedy pair has identical generated IDs and stops. Prefill remains about
3.20 s for 1024 IDs and 0.238 s for 70 IDs; its kernels are unchanged.

The exact measured harness is retained. After that run, the final harness also
disables replay in its other module-comparison modes and refuses builds without
`PSIONIC_DEVICE_TEST`, preventing a benchmark from reusing bindings from a
replaced test module. The measured `--host-replay` path is unchanged. The final
harness compile passes; the build record distinguishes the snapshots.

## Endpoint and correctness

The graph-backed Responses endpoint serves the frozen 70-input/128-output
request in a median **2.255627 s**, or **56.747 output IDs/s**, including reset,
tokenization, prefill, decoding, serialization and transport. Median first
semantic delta is **0.309761 s**; first visible text **0.568112 s**. Five
measured requests follow a warm-up. Its output-state SHA-256 remains
`3d49e04347f243cbbe1deeea20975ca8134e835775fdbb3e900feb351e41f126`.

A fresh batch using the prior qualified pass-2 executable on the same GPU
measures **2.262329 s / 56.579 IDs/s**, **0.306640 s** to the first semantic
delta and **0.571821 s** to visible text. Candidate ran first, baseline second;
these are sequential warm batches, not alternating HTTP pairs. The candidate's
HTTP rate is only 0.3% higher, which is negligible in this small sample. No
meaningful endpoint speedup or first-visible latency improvement is claimed.
This replaces a tempting comparison against the older 54.481 IDs/s sample;
that historical difference mostly does not survive a fresh baseline.

Focused Coax checks pass in debug/release (136/136 for the replay root).
Existing device checks pass with owned tiny CPU execution in plain and
ASan/UBSan modes, debug/release. New GPU graph/stream checks compare the full
public state across resets, a wrapped sliding cache, selection topology changes
and an asynchronous interleave. Existing GPU tiny checks retain independent
vector bounds and prove late arithmetic refusal preserves public cache/logits.
The GPU ASan attempt could not initialize CUDA (`cuInit` error 2, out of memory);
no GPU sanitizer pass is claimed. Ordinary GPU and native CPU sanitizers pass.

Actual original-model Responses checks pass: HTTP text, SSE accumulation, typed
replay, function handoff and SDK continuation, busy admission, limits/errors,
cancellation/reuse and supported Open Responses schemas. The temporary listener,
forwarding connection and API key are removed after measurement. The idle GPU
switch supervisor was paused for the benchmark and restored afterward.
No full mutation roots or original-model CPU inference ran.

## Rejected work and next target

Before replay, bounded owned slices tested a 32-bit FP4 nibble lookup, input
subviews, quarter-word extraction and neighboring-row tiles. Lookup/extraction
and subviews did not yield a convincing gain; pairing/quadrupling expert rows
was neutral or slower. Those kernel changes were discarded. Exploratory logs
are retained, but discarded generated experimental modules are not; no shipped
speed claim depends on these slices.

The next substantial throughput work remains Coax matrix operations and
coefficient reuse, dense prefill, pointwise fusion and measured attention work.
Keeping dynamic token/position controls on device could reduce graph-node
updates, but these results put a low ceiling on submission-only gains for this
workload. Graphing prefill, continuous batching and speculative device
verification retain separate gates. Five repetitions on one worker do not
establish production capacity or tail latency.
