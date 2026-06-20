import { describe, expect, test } from "bun:test"

import {
  KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
  type KernelThroughputRecord,
  verifyKernelOptimizationParity,
} from "./kernel-optimization-parity.js"
import {
  TASSADAR_TS_REPLAY_CLASS_ID,
  type TassadarReplayVerdict,
} from "./replay.js"

// Anchored on the historical March-2026 Psionic/Qwen 3.5 0.5B result
// (docs/transcripts/217.md): baseline runtime ~328 tok/s, optimized ~523 tok/s.
const baseline: KernelThroughputRecord = {
  device: "cuda",
  hardwareRef: "single-machine-demo-gpu",
  kernelRef: "baseline-runtime",
  opRef: "rmsnorm",
  targetModel: "qwen-3.5-0.5b",
  tokensPerSecond: 328,
}
const optimized: KernelThroughputRecord = {
  device: "cuda",
  hardwareRef: "single-machine-demo-gpu",
  kernelRef: "psionic-custom-cuda",
  opRef: "rmsnorm",
  targetModel: "qwen-3.5-0.5b",
  tokensPerSecond: 523,
}

const verifiedParity: TassadarReplayVerdict = {
  claimedTraceDigest: "a".repeat(64),
  classId: TASSADAR_TS_REPLAY_CLASS_ID,
  comparedSteps: 8,
  graphDigest: "b".repeat(64),
  outcome: "verified",
  rejection: null,
  replayedSteps: 8,
  replayedTraceDigest: "a".repeat(64),
  validatorDeviceRef: "validator-device-2",
}
const rejectedParity: TassadarReplayVerdict = {
  claimedTraceDigest: "a".repeat(64),
  classId: TASSADAR_TS_REPLAY_CLASS_ID,
  comparedSteps: 8,
  graphDigest: "b".repeat(64),
  outcome: "rejected",
  rejection: { actual: "c".repeat(64), reason: "trace_digest_mismatch" },
  replayedSteps: 8,
  replayedTraceDigest: "c".repeat(64),
  validatorDeviceRef: "validator-device-2",
}

describe("kernel-optimization throughput-parity verdict", () => {
  test("accepts a faster kernel that passes independent-device parity", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.classId).toBe(KERNEL_OPTIMIZATION_PARITY_CLASS_ID)
    expect(verdict.outcome).toBe("accepted")
    expect(verdict.rejection).toBeNull()
    expect(verdict.parityOutcome).toBe("verified")
    expect(verdict.parityValidatorDeviceRef).toBe("validator-device-2")
    expect(verdict.speedupRatio).toBeCloseTo(523 / 328, 6)
    // The op being optimized is surfaced (trimmed) for per-op settlement binding.
    expect(verdict.optimizedOpRef).toBe("rmsnorm")
  })

  test("surfaces the trimmed op ref so settlements can bind by op", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline: { ...baseline, opRef: "attention.flash" },
      optimized: { ...optimized, opRef: "attention.flash" },
      optimizedOpRef: "  attention.flash  ",
      parityVerdict: verifiedParity,
    })
    expect(verdict.optimizedOpRef).toBe("attention.flash")
  })

  test("accepts when record op provenance matches case-insensitively", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline: { ...baseline, opRef: "RMSNorm" },
      optimized: { ...optimized, opRef: " rmsnorm " },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("accepted")
  })

  test("rejects when a throughput record measures a different op", () => {
    // Apples-to-oranges: optimized record is for a different op than claimed,
    // so its (higher) tok/s is not a speedup of the claimed op.
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, opRef: "attention.flash" },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("op_mismatch")
  })

  test("rejects when the claimed op disagrees with both records", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized,
      optimizedOpRef: "attention.flash",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("op_mismatch")
  })

  test("rejects a blank claimed op (no provenance to bind a settlement to)", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized,
      optimizedOpRef: "   ",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("op_mismatch")
  })

  test("op-mismatch wins over a faster-but-wrong parity verdict", () => {
    // Op provenance is structural and checked before the parity gate.
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, opRef: "attention.flash" },
      optimizedOpRef: "rmsnorm",
      parityVerdict: rejectedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("op_mismatch")
  })

  test("rejects when baseline and optimized name the same kernel impl", () => {
    // No new kernel delivered: the tok/s delta is remeasurement / cherry-picking
    // the same implementation, not an optimization. The deliverable gate catches
    // it even though parity is verified and the optimized number is faster.
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, kernelRef: "baseline-runtime" },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("kernel_not_optimized")
  })

  test("same-kernel detection is trim/case-insensitive", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline: { ...baseline, kernelRef: "Baseline-Runtime" },
      optimized: { ...optimized, kernelRef: "  baseline-runtime  " },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("kernel_not_optimized")
  })

  test("rejects a blank optimized kernel ref (no deliverable to attest)", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, kernelRef: "   " },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("kernel_not_optimized")
  })

  test("deliverable gate is structural: it wins over a faster-but-wrong parity verdict", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, kernelRef: "baseline-runtime" },
      optimizedOpRef: "rmsnorm",
      parityVerdict: rejectedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("kernel_not_optimized")
  })

  test("speed never overrides correctness: faster-but-wrong is rejected", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: rejectedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("parity_rejected")
    if (verdict.rejection?.reason === "parity_rejected") {
      expect(verdict.rejection.parityRejection?.reason).toBe(
        "trace_digest_mismatch",
      )
    }
  })

  test("rejects a parity-verified kernel with no throughput improvement", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, tokensPerSecond: 328 },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("no_throughput_improvement")
  })

  test("rejects mismatched targets (cannot compare across devices)", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, device: "metal" },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("target_mismatch")
  })

  test("rejects non-positive or non-finite throughput records", () => {
    const zero = verifyKernelOptimizationParity({
      baseline: { ...baseline, tokensPerSecond: 0 },
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(zero.outcome).toBe("rejected")
    expect(zero.rejection?.reason).toBe("invalid_throughput")

    const nan = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, tokensPerSecond: Number.NaN },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(nan.outcome).toBe("rejected")
    expect(nan.rejection?.reason).toBe("invalid_throughput")
    expect(nan.speedupRatio).toBeNull()
  })
})
