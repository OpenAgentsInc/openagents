import { describe, expect, test } from "bun:test"
import {
  PYLON_SERVING_CAPABILITY_REF,
  PYLON_SERVING_REAL_GPU_GATED_BLOCKER_REF,
  PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX,
  buildServingCapabilityEvidence,
  estimatedColdStartMs,
  publishableServingCapabilityRefs,
  stripUnreceiptedServingCapability,
  warmResidency,
  type ServingModelResidency,
} from "../src/serving-capability"
import {
  PINNED_SELF_BENCHMARK_WORKLOAD,
  PYLON_SERVING_REAL_GPU_BENCH_ENV,
  fixtureServingBenchmarkAdapter,
  realGpuServingBenchmarkAdapter,
  runServingSelfBenchmark,
  selectServingBenchmarkAdapter,
} from "../src/serving-benchmark"
import {
  buildServingReceipt,
  computeServingVerification,
  runServingReplayChallenge,
} from "../src/serving-receipt"
import { publishableCapabilityRefs } from "../src/tassadar-capability"

const OBSERVED_AT = "2026-06-23T08:00:00.000Z"

function warmResidencyEntry(
  overrides: Partial<ServingModelResidency> = {},
): ServingModelResidency {
  return {
    modelRef: "model.psionic.qwen35.0_8b.q8_0",
    engine: "llama_cpp",
    quantization: "gguf_q8_0",
    residency: "warm",
    coldStart: { gpuProcurementMs: 0, imageLoadMs: 0, weightLoadMs: 0, engineStartupMs: 0 },
    ...overrides,
  }
}

describe("serving capability evidence (typed + precise, not 'GPU online')", () => {
  test("captures usable memory, bandwidth class, interconnect, engines, residency, cold-start posture", () => {
    const bench = runServingSelfBenchmark({
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "llama_cpp@b3000",
    })
    expect(bench.parity.parityPassed).toBe(true)
    expect(bench.receiptRef.startsWith(PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX)).toBe(true)

    const evidence = buildServingCapabilityEvidence({
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      usableGpuMemoryGb: 72,
      totalGpuMemoryGb: 80,
      bandwidthClass: "hbm_high",
      interconnect: "single_gpu",
      engines: ["llama_cpp", "vllm"],
      residency: [warmResidencyEntry()],
      selfBenchmarkReceiptRef: bench.receiptRef,
    })

    // The evidence is typed and precise: each book-required field is present.
    expect(evidence.hardware.usableGpuMemoryGb).toBe(72)
    expect(evidence.hardware.totalGpuMemoryGb).toBe(80)
    expect(evidence.hardware.bandwidthClass).toBe("hbm_high")
    expect(evidence.hardware.interconnect).toBe("single_gpu")
    expect(evidence.engines).toEqual(["llama_cpp", "vllm"])
    expect(evidence.residency[0]?.residency).toBe("warm")
    expect(evidence.selfBenchmarked).toBe(true)
    expect(evidence.realGpuAdapter).toBe(false)
    expect(evidence.blockerRefs).toEqual([])
  })

  test("a serving claim without a self-benchmark receipt is blocked and never publishes", () => {
    const evidence = buildServingCapabilityEvidence({
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.lovelace_l40s",
      usableGpuMemoryGb: 40,
      totalGpuMemoryGb: 48,
      bandwidthClass: "gddr",
      interconnect: "single_gpu",
      engines: ["vllm"],
      residency: [],
      selfBenchmarkReceiptRef: null,
    })
    expect(evidence.selfBenchmarked).toBe(false)
    expect(evidence.blockerRefs).toContain("blocker.pylon.serving.self_benchmark_failed")
    expect(publishableServingCapabilityRefs(evidence)).toEqual([])
  })

  test("rejects impossible usable>total memory and residency engine not supported", () => {
    const evidence = buildServingCapabilityEvidence({
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.unknown",
      usableGpuMemoryGb: 100,
      totalGpuMemoryGb: 80,
      bandwidthClass: "unknown",
      interconnect: "unknown",
      engines: ["llama_cpp"],
      residency: [warmResidencyEntry({ engine: "vllm" })],
      selfBenchmarkReceiptRef: `${PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX}abc`,
    })
    expect(evidence.blockerRefs).toContain("blocker.pylon.serving.usable_memory_exceeds_total")
    expect(evidence.blockerRefs).toContain("blocker.pylon.serving.residency_engine_unsupported")
  })

  test("cold-start posture sums the book's four factors; warm residency is ~0", () => {
    const warm = warmResidencyEntry()
    expect(estimatedColdStartMs(warm)).toBe(0)
    const cold = warmResidencyEntry({
      residency: "cold",
      coldStart: {
        gpuProcurementMs: 30000,
        imageLoadMs: 5000,
        weightLoadMs: 4000,
        engineStartupMs: 2000,
      },
    })
    expect(estimatedColdStartMs(cold)).toBe(41000)
    expect(warmResidency({ ...evidenceWith([warm, cold]) }).length).toBe(1)
  })

  test("publishable serving refs include the capability + its receipt only when proven", () => {
    const receiptRef = `${PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX}proof123`
    const evidence = buildServingCapabilityEvidence({
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      usableGpuMemoryGb: 72,
      totalGpuMemoryGb: 80,
      bandwidthClass: "hbm_high",
      interconnect: "single_gpu",
      engines: ["llama_cpp"],
      residency: [warmResidencyEntry()],
      selfBenchmarkReceiptRef: receiptRef,
    })
    expect(publishableServingCapabilityRefs(evidence)).toEqual([
      PYLON_SERVING_CAPABILITY_REF,
      receiptRef,
    ])
  })
})

// Helper that builds minimal proven evidence around a residency list.
function evidenceWith(residency: ServingModelResidency[]) {
  return buildServingCapabilityEvidence({
    observedAt: OBSERVED_AT,
    gpuClass: "gpu.class.hopper_h100",
    usableGpuMemoryGb: 72,
    totalGpuMemoryGb: 80,
    bandwidthClass: "hbm_high",
    interconnect: "single_gpu",
    engines: ["llama_cpp"],
    residency,
    selfBenchmarkReceiptRef: `${PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX}helper`,
  })
}

describe("worker self-benchmark before registration", () => {
  test("fixture adapter is deterministic: same input -> byte-identical receipt", () => {
    const a = fixtureServingBenchmarkAdapter({
      workload: PINNED_SELF_BENCHMARK_WORKLOAD,
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "llama_cpp@b3000",
    })
    const b = fixtureServingBenchmarkAdapter({
      workload: PINNED_SELF_BENCHMARK_WORKLOAD,
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "llama_cpp@b3000",
    })
    expect(a).toEqual(b)
    expect(a.adapter).toBe("fixture")
    expect(a.parity.parityPassed).toBe(true)
    expect(a.metrics.tokensPerSecond).toBeGreaterThan(0)
    // The receipt discloses engine/version/quant/GPU/warm-cold/parity.
    expect(a.engine).toBe("llama_cpp")
    expect(a.engineVersion).toBe("llama_cpp@b3000")
    expect(a.quantization).toBe("gguf_q8_0")
    expect(a.gpuClass).toBe("gpu.class.hopper_h100")
    expect(a.warmState).toBe("warm")
  })

  test("default adapter selection is the fixture path with no GPU flag set", () => {
    const selected = selectServingBenchmarkAdapter({})
    expect(selected.realGpu).toBe(false)
  })

  test("real-GPU adapter is FLAG-GATED off by default and refuses with a typed blocker", () => {
    const receipt = realGpuServingBenchmarkAdapter({
      workload: PINNED_SELF_BENCHMARK_WORKLOAD,
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "vllm@0.6.0",
      env: {},
    })
    expect(receipt.adapter).toBe("real_gpu")
    expect(receipt.parity.parityPassed).toBe(false)
    expect(receipt.blockerRefs).toContain(PYLON_SERVING_REAL_GPU_GATED_BLOCKER_REF)
  })

  test("even with the gate flag set, the real-GPU path refuses (owner-gated, not implemented)", () => {
    const env = { [PYLON_SERVING_REAL_GPU_BENCH_ENV]: "1" } as NodeJS.ProcessEnv
    const selected = selectServingBenchmarkAdapter(env)
    expect(selected.realGpu).toBe(true)
    const receipt = runServingSelfBenchmark({
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "vllm@0.6.0",
      env,
    })
    expect(receipt.adapter).toBe("real_gpu")
    expect(receipt.blockerRefs).toContain(
      "blocker.pylon.serving.real_gpu_adapter_not_implemented",
    )
    expect(receipt.parity.parityPassed).toBe(false)
  })

  test("quantization is part of product identity: changing it changes the parity digest", () => {
    const base = fixtureServingBenchmarkAdapter({
      workload: PINNED_SELF_BENCHMARK_WORKLOAD,
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "llama_cpp@b3000",
    })
    const fp8Workload = { ...PINNED_SELF_BENCHMARK_WORKLOAD, quantization: "fp8" as const }
    const fp8 = fixtureServingBenchmarkAdapter({
      workload: fp8Workload,
      observedAt: OBSERVED_AT,
      gpuClass: "gpu.class.hopper_h100",
      engineVersion: "llama_cpp@b3000",
    })
    // Different quant -> different measured digest -> parity fails against the
    // q8_0 expected digest (an FP8 model is not the same product).
    expect(fp8.parity.measuredOutputDigest).not.toBe(base.parity.measuredOutputDigest)
    expect(fp8.parity.parityPassed).toBe(false)
  })
})

describe("serving receipt + canary/replay before payout (no money moves)", () => {
  const baseReceiptInput = {
    servedAt: OBSERVED_AT,
    modelRef: "model.psionic.qwen35.0_8b.q8_0",
    engine: "llama_cpp" as const,
    engineVersion: "llama_cpp@b3000",
    quantization: "gguf_q8_0" as const,
    gpuClass: "gpu.class.hopper_h100",
    warmState: "warm" as const,
    residencyAtServe: "warm" as const,
    promptDigest: "prompt-digest-aaa",
    outputDigest: "output-digest-bbb",
    maxNewTokens: 64,
    temperature: 0,
    samplingSeed: 7,
    metrics: {
      ttftMs: 50,
      tokensPerSecond: 40,
      promptTokens: 12,
      completionTokens: 64,
      wallClockMs: 1600,
    },
  }

  test("receipt carries engine/version/quant/GPU/warm-cold/parity fields", () => {
    const verification = computeServingVerification({ parityPassed: true })
    const receipt = buildServingReceipt({ ...baseReceiptInput, verification })
    expect(receipt.engine).toBe("llama_cpp")
    expect(receipt.engineVersion).toBe("llama_cpp@b3000")
    expect(receipt.quantization).toBe("gguf_q8_0")
    expect(receipt.gpuClass).toBe("gpu.class.hopper_h100")
    expect(receipt.warmState).toBe("warm")
    expect(receipt.verification.parityPassed).toBe(true)
    expect(receipt.verification.verified).toBe(true)
    expect(receipt.verification.payoutEligible).toBe(true)
    expect(receipt.blockerRefs).toEqual([])
  })

  test("no parity, no pay: a failed parity is never payout-eligible and is blocked", () => {
    const verification = computeServingVerification({ parityPassed: false })
    const receipt = buildServingReceipt({ ...baseReceiptInput, verification })
    expect(receipt.verification.verified).toBe(false)
    expect(receipt.verification.payoutEligible).toBe(false)
    expect(receipt.blockerRefs).toContain("blocker.pylon.serving.no_parity")
  })

  test("a passing replay challenge keeps the worker payout-eligible", () => {
    const seed = buildServingReceipt({
      ...baseReceiptInput,
      verification: computeServingVerification({ parityPassed: true }),
    })
    const replay = runServingReplayChallenge({
      challengedAt: "2026-06-23T08:05:00.000Z",
      receipt: seed,
      replayedOutputDigest: seed.outputDigest,
      replayEngine: "llama_cpp",
      replayEngineVersion: "llama_cpp@b3000",
      replayQuantization: "gguf_q8_0",
    })
    expect(replay.matched).toBe(true)
    expect(replay.identityMismatch).toBe(false)
    const verification = computeServingVerification({ parityPassed: true, replay })
    expect(verification.verificationClass).toBe("parity+replay")
    expect(verification.payoutEligible).toBe(true)
  })

  test("a mismatched replay challenge fails the payout gate", () => {
    const seed = buildServingReceipt({
      ...baseReceiptInput,
      verification: computeServingVerification({ parityPassed: true }),
    })
    const replay = runServingReplayChallenge({
      challengedAt: "2026-06-23T08:05:00.000Z",
      receipt: seed,
      replayedOutputDigest: "output-digest-DIFFERENT",
      replayEngine: "llama_cpp",
      replayEngineVersion: "llama_cpp@b3000",
      replayQuantization: "gguf_q8_0",
    })
    expect(replay.matched).toBe(false)
    const verification = computeServingVerification({ parityPassed: true, replay })
    expect(verification.payoutEligible).toBe(false)
  })

  test("replay with a different engine/quant is an identity mismatch and fails the gate", () => {
    const seed = buildServingReceipt({
      ...baseReceiptInput,
      verification: computeServingVerification({ parityPassed: true }),
    })
    const replay = runServingReplayChallenge({
      challengedAt: "2026-06-23T08:05:00.000Z",
      receipt: seed,
      replayedOutputDigest: seed.outputDigest,
      replayEngine: "vllm",
      replayEngineVersion: "vllm@0.6.0",
      replayQuantization: "fp8",
    })
    expect(replay.identityMismatch).toBe(true)
    const verification = computeServingVerification({ parityPassed: true, replay })
    expect(verification.payoutEligible).toBe(false)
  })

  test("a failed inline canary fails the payout gate", () => {
    const verification = computeServingVerification({
      parityPassed: true,
      canary: { canaryRef: "canary.known_answer.v1", passed: false },
    })
    const receipt = buildServingReceipt({ ...baseReceiptInput, verification })
    expect(verification.verificationClass).toBe("canary")
    expect(receipt.verification.payoutEligible).toBe(false)
    expect(receipt.blockerRefs).toContain("blocker.pylon.serving.canary_failed")
  })
})

describe("publishable capability refs strip unproven serving claims", () => {
  test("serving ref without its receipt is stripped", () => {
    expect(
      stripUnreceiptedServingCapability([PYLON_SERVING_CAPABILITY_REF, "pylon.other.ref"]),
    ).toEqual(["pylon.other.ref"])
  })

  test("serving ref with its receipt is kept and flows through publishableCapabilityRefs", () => {
    const receiptRef = `${PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX}xyz`
    const refs = publishableCapabilityRefs([PYLON_SERVING_CAPABILITY_REF, receiptRef])
    expect(refs).toContain(PYLON_SERVING_CAPABILITY_REF)
    expect(refs).toContain(receiptRef)
  })
})
