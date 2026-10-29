# Resident original GPT-OSS decode speed — 2026-10-29

Change: use admitted unsigned-32 tensor offsets in single-token BF16/MXFP4
products, hoist expert-row/input-slot bases, preserve view checks and every
coefficient/lane/fold/reduction operation. The backend and numerical profile
remain `gpt-oss-cuda-f32-v1`. No conversion, CPU fallback, handwritten CUDA or
served BLAS library is introduced. The Responses socket also sets TCP_NODELAY
so small semantic SSE writes do not wait for Nagle coalescing.

## Qualification

- Coax checks compare varied signed coefficients/inputs against frozen old
  lane programs, including ragged group counts, shared versus slot-specific
  inputs and both 8-/64-iteration fold boundaries.
- Focused device checks pass in debug/release with native plain/ASan/UBSan,
  independent owned tiny-model references, failed-step atomicity and book output.
- Focused Responses HTTP checks pass in debug/release, plain/ASan/UBSan after
  the socket effect change. No full mutation roots or original CPU inference.
- Original-shaped GPU microbenchmarks alternate both modules and flush 80 MiB
  before each timed launch. Constant coefficients give exact partial-sum
  oracles across selected expert IDs and lanes.
- Full original 20b A/B uses one authenticated resident weight allocation,
  two Coax-generated CUDA modules, alternating order, one warm-up and five
  measured repeats per variant. Each decode token runs greedy selection and
  waits/reads control, matching service synchronization. Complete caches,
  logits, residual traces and expert IDs compare bit for bit after the fixed
  workload; actual greedy generated IDs and stops agree on every trial.

Hardware: owned `coderos-4080`, RTX 4080 (16 GiB), NVRTC 13.2 and the
dynamically loaded CUDA driver.
Model: original 20b revision `6cee5e81ee83917806bbde320786a8fb61efebee`,
2048 context capacity. Original model/device accuracy qualification remains
[45a–45c](psionic-gptoss-device-2026-10-29.md).

## Measurements

The selected-expert up product fell from about 0.401 ms to 0.195 ms; down fell
from about 0.215 ms to 0.105 ms, on owned original-shaped slices. The full head
also improved modestly. Product-only timings are distinct from full decoding.

| Full-model workload | Before, median decode | After, median decode | Gain |
| --- | ---: | ---: | ---: |
| 1024 input / 128 forced continuation IDs | 29.193 tokens/s | 39.244 tokens/s | 34.4% |
| 70 input / 128 actual greedy response IDs | 36.611 tokens/s | 53.843 tokens/s | 47.1% |

The [retained benchmark JSON](../../psionic/fixtures/gptoss/qualification/decode-speed-2026-10-29/model-speed.json)
records every sample and the paired summary. Median 1024-token prefill stays
3.220119 s before / 3.222130 s after; short prefill stays about 0.239 s.
The changed decode program remains bit-identical across every trial.

The fixed
workload uses 1024 varied input IDs then 128 forced continuation IDs. A second
workload actually generates up to 128 greedy IDs for a frozen list-of-integers
prompt. Its decode rate excludes the first ID selected by prefill. Prefill is
measured separately and its implementation is unchanged.

An alternative bit-built coefficient scaler was bit-exact but slower in the
microbenchmark; explicit shape specialization also added no useful speedup.
Neither is shipped. Only the measured address transformation is retained.

The optimized Responses endpoint's [five measured SSE runs](../../psionic/fixtures/gptoss/qualification/decode-speed-2026-10-29/responses-speed.json)
use the same 70-token prompt and 128-output limit. Median full request time is
**2.622076 s**, or **48.816 output IDs/s** including rendering/reset, prefill,
decode, serialization and SSH-forwarded transport. The first semantic delta
arrives at a median **0.310010 s**. This is an absolute after-change API smoke
measurement, not a separate HTTP before/after claim. Its output-state digest
agrees across all trials.

Real original-model SDK create/stream accumulation, function handoff and typed
result replay pass, along with baseline Responses schemas, explicit refusals,
busy admission and disconnect/reuse. The test listener and ephemeral key are
removed afterward; no public service is deployed.

[Build identities](../../psionic/fixtures/gptoss/qualification/decode-speed-2026-10-29/build.json)
retain the actual executable, generated host/CUDA and compiler hashes, native
flags and touched source identities. The previous CUDA module is the retained
qualified baseline. Neither benchmark swaps code in the production API.

## Reproduce

`psionic/tests/gptoss_decode_bench.c` takes before/after generated CUDA sources
and a block size. `psionic/tests/gptoss_speed.c` is an opt-in native harness
linked with the generated host C and platform effects; it takes original cache,
before CUDA, after CUDA and Harmony table paths. Private session access and
module switching exist only inside that harness; the production ABI is unchanged.
`psionic/tests/gptoss_speed_report.py LOG OUT.json` checks the parity gates and
summarizes paired runs. `psionic/tests/responses_speed.py URL --out OUT.json`
measures a running owned endpoint using `PSIONIC_API_KEY`; it logs only timing,
counts and a digest, including no prompt/response content. End-to-end SSE rate
includes rendering, prefill, decoding, serialization and transport.

Five repeats give a smoke measurement, not production capacity or a packaged
engine comparison. Concurrency remains one; prefill below 2 seconds, cooperative
matrix products, graph replay and continuous batching remain open.
