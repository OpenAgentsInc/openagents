# 71. Throughput is a gate

> **Status: unimplemented — design draft.** Nothing here runs yet.

A correct decoder that is too slow is not useful inference. This chapter
turns gpt-oss-20b throughput on the RTX 4080 into a pass or fail gate on a
fixed workload, and then adds bounded continuous batching.

## What this chapter covers

- A fixed workload: 1,024 input tokens and 128 output tokens, seeded
  requests, warm-up and at least five repeats.
- Two goals:
  - median single-stream decode of at least 25 tokens/s;
  - 1,024-token warm prefill in at most 2 s.
- The plan records earlier measurements: a first SIMT baseline of 28.454
  tokens/s and 4.325127 s prefill, then 29.586 tokens/s and 3.219239 s after
  attention and launch tuning. By those records, decode meets its goal and
  prefill does not yet.
- Reported per run: P50 and P95, allocation and peak device memory,
  transfers and phase times.
- A comparison with an installed packaged reference engine on the same
  device.
- Bounded continuous batching and KV allocation, with aggregate and
  per-request rates at concurrency 1, 4 and 8.

## Dependencies

- [Chapter 66](ch066-a-complete-cuda-decoder.md),
  [chapter 67](ch067-prefill-together.md) and
  [chapter 70](ch070-replay-the-device-program.md): the device decoder,
  prefill and replay.
- [Chapter 68](ch068-coax-kernels-cooperate.md) and
  [chapter 69](ch069-original-mxfp4-products-use-matrix-hardware.md): the
  faster products the prefill goal needs.
- [Chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md): the
  comparison method.

## Done means

- Both goals pass on the fixed workload with the stated repeats, and the
  record names the build, device and clocks.
- Concurrency 1, 4 and 8 report aggregate and per-request rates, and each
  request's output matches its single-request output.
- The comparator run uses the same file, prompt shapes and settings.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 8.
- [Decode qualification](../../docs/verification/psionic-decode-speed-2026-10-29.md).
- [Making Psionic serving faster](../docs/serving/performance.md).
