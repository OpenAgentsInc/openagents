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
  targetModel: "qwen-3.5-0.5b",
  tokensPerSecond: 328,
}
const optimized: KernelThroughputRecord = {
  device: "cuda",
  hardwareRef: "single-machine-demo-gpu",
  kernelRef: "psionic-custom-cuda",
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
      parityVerdict: verifiedParity,
    })
    expect(verdict.classId).toBe(KERNEL_OPTIMIZATION_PARITY_CLASS_ID)
    expect(verdict.outcome).toBe("accepted")
    expect(verdict.rejection).toBeNull()
    expect(verdict.parityOutcome).toBe("verified")
    expect(verdict.parityValidatorDeviceRef).toBe("validator-device-2")
    expect(verdict.speedupRatio).toBeCloseTo(523 / 328, 6)
  })

  test("speed never overrides correctness: faster-but-wrong is rejected", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized,
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
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("no_throughput_improvement")
  })

  test("rejects mismatched targets (cannot compare across devices)", () => {
    const verdict = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, device: "metal" },
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("target_mismatch")
  })

  test("rejects non-positive or non-finite throughput records", () => {
    const zero = verifyKernelOptimizationParity({
      baseline: { ...baseline, tokensPerSecond: 0 },
      optimized,
      parityVerdict: verifiedParity,
    })
    expect(zero.outcome).toBe("rejected")
    expect(zero.rejection?.reason).toBe("invalid_throughput")

    const nan = verifyKernelOptimizationParity({
      baseline,
      optimized: { ...optimized, tokensPerSecond: Number.NaN },
      parityVerdict: verifiedParity,
    })
    expect(nan.outcome).toBe("rejected")
    expect(nan.rejection?.reason).toBe("invalid_throughput")
    expect(nan.speedupRatio).toBeNull()
  })
})
