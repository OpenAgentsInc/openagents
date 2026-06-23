# Pylon Proven-Engine Serving Capability Evidence

Status: schema + self-benchmark + receipt machinery landed (book P1-6,
openagents#6089). The real vLLM/SGLang GPU serving path and the real-GPU
benchmark adapter are **compute/owner-gated and not implemented** — see the
flag below. Nothing here is a green product promise; it is the supply-side
evidence shape Khala's marketplace can verify before it pays anyone.

## Why this exists (from the book)

The inference-engineering book's near-term advice for whole-small-model Pylon
serving: don't write a bespoke engine — use a proven runtime (vLLM/SGLang/
TensorRT-LLM/llama.cpp) and make Pylon capability evidence **precise**. Two
ideas drive the schema here:

- **GPUs are not fungible.** "GPU online" is not capability evidence. Usable GPU
  memory, memory-bandwidth class (decode is bandwidth-bound), and interconnect
  posture (single GPU vs NVLink node vs InfiniBand multi-node) all change what a
  node can actually serve.
- **A served model id is not a product** unless engine, version, quantization
  mode, GPU class, and warm/cold state travel with it. An FP8/MXFP8 serve is a
  different product than an unqualified model id, so the precision/backend are
  disclosed in every receipt and scoped into parity.

## What landed (buildable now, no GPU/network)

- `src/serving-capability.ts` — typed `PylonServingCapabilityEvidence`: engines,
  usable/total GPU memory, bandwidth class, interconnect posture, per-model
  residency (warm/paged/cold) with the book's four cold-start factors
  (GPU procurement, image load, weight load, engine startup/compile). A serving
  claim is published only when a self-benchmark receipt accompanies it
  (`publishableServingCapabilityRefs`); `stripUnreceiptedServingCapability` is
  wired into the presence path's `publishableCapabilityRefs`, so an unproven
  serving claim never leaves the device — the same no-overclaim rule as the
  Tassadar executor capability.
- `src/serving-benchmark.ts` — worker **self-benchmark before registration**.
  The default adapter is deterministic and fixture-backed: a pinned,
  digest-known workload produces a stable, public-safe self-benchmark receipt
  (same input → byte-identical metrics + parity digest), touching no GPU and no
  network. Quantization is part of the parity digest, so changing the quant mode
  changes the digest and fails parity against the original — the book's "FP8 is
  a different product" rule expressed mechanically.
- `src/serving-receipt.ts` — per-serve `PylonServingReceipt` carrying
  engine/version/quantization/GPU class/warm-cold/parity, plus the
  **canary + replay-challenge** shape needed to verify a worker **before**
  payout. `computeServingVerification` centralizes "no parity, no pay":
  `payoutEligible` is only ever true when parity passed and no attached canary
  or replay challenge failed (an identity mismatch on replay also fails the
  gate). **This module computes eligibility; it never moves money.** A product
  surface owns the payout decision.

## Compute/owner-gated split (honest bounds)

- The **real-GPU benchmark adapter** (`realGpuServingBenchmarkAdapter`) is
  reachable only when `PYLON_SERVING_REAL_GPU_BENCH=1`. Until then it refuses
  with `blocker.pylon.serving.real_gpu_adapter_gated`. Even with the flag set it
  currently refuses with `blocker.pylon.serving.real_gpu_adapter_not_implemented`
  rather than fabricating a measurement — wiring a real vLLM/SGLang serve +
  measured benchmark is owner/compute-gated work, out of scope for this change.
- Tests and CI always run the fixture path (`realGpuAdapter: false`).

## Where this plugs in next (not in this change)

The fabric supply adapter behind `InferenceProviderAdapter`
(`apps/openagents.com/workers/api/src/inference/provider-adapter.ts`, P1-5 lane)
is the consumer that would canary/replay a registered Pylon's serving receipt
before routing paid traffic to it. That wiring, real serving, and any payout are
deliberately not in this change.
