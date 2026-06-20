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
// The parity verdict below replayed graph "b".repeat(64); the optimized record's
// graphDigest must match it for the tok/s number to be bound to that proof.
const baseline: KernelThroughputRecord = {
  device: "cuda",
  graphDigest: "d".repeat(64),
  hardwareRef: "single-machine-demo-gpu",
  kernelRef: "baseline-runtime",
  opRef: "rmsnorm",
  targetModel: "qwen-3.5-0.5b",
  tokensPerSecond: 328,
}
const optimized: KernelThroughputRecord = {
  device: "cuda",
  graphDigest: "b".repeat(64),
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
// The baseline kernel replayed on the SAME validator device, establishing the
// reference output the optimized kernel must reproduce. It replays the baseline
// graph ("d".repeat(64) == baseline.graphDigest) and yields the same output
// trace ("a".repeat(64)) as the verified optimized replay above — i.e. the
// optimized kernel computes identical outputs to the baseline (cross-graph
// equivalence), the actual correctness anchor.
const baselineParity: TassadarReplayVerdict = {
  claimedTraceDigest: "a".repeat(64),
  classId: TASSADAR_TS_REPLAY_CLASS_ID,
  comparedSteps: 8,
  graphDigest: "d".repeat(64),
  outcome: "verified",
  rejection: null,
  replayedSteps: 8,
  replayedTraceDigest: "a".repeat(64),
  validatorDeviceRef: "validator-device-2",
}

describe("kernel-optimization throughput-parity verdict", () => {
  test("accepts a faster kernel that passes independent-device parity", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
      baseline: { ...baseline, opRef: "attention.flash" },
      optimized: { ...optimized, opRef: "attention.flash" },
      optimizedOpRef: "  attention.flash  ",
      parityVerdict: verifiedParity,
    })
    expect(verdict.optimizedOpRef).toBe("attention.flash")
  })

  test("accepts when record op provenance matches case-insensitively", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
      baseline,
      optimized: { ...optimized, kernelRef: "baseline-runtime" },
      optimizedOpRef: "rmsnorm",
      parityVerdict: rejectedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("kernel_not_optimized")
  })

  test("binds the optimized tok/s record to the replayed parity-trace graph", () => {
    // The optimized record's graph matches verifiedParity.graphDigest, so the
    // tok/s number is bound to the artifact the validator actually replayed.
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("accepted")
  })

  test("trace binding is trim/case-insensitive on the graph digest", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
      baseline,
      optimized: { ...optimized, graphDigest: `  ${"B".repeat(64)}  ` },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("accepted")
  })

  test("rejects a tok/s record measured on a different graph than was replayed", () => {
    // The optimized number was measured on graph "e..." but the validator
    // replayed graph "b..."; the proof does not pertain to the measured kernel.
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
      baseline,
      optimized: { ...optimized, graphDigest: "e".repeat(64) },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("parity_trace_unbound")
  })

  test("rejects a blank optimized graph digest (nothing to bind the proof to)", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
      baseline,
      optimized: { ...optimized, graphDigest: "   " },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("parity_trace_unbound")
  })

  test("trace binding is structural: an unbound graph wins over a verified-but-irrelevant parity", () => {
    // Even though this parity verdict reads "verified", it replayed a different
    // graph than the optimized kernel was measured on, so it proves nothing about
    // that kernel — binding is checked before the parity outcome gate.
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
      baseline,
      optimized: { ...optimized, graphDigest: "e".repeat(64) },
      optimizedOpRef: "rmsnorm",
      parityVerdict: { ...verifiedParity, graphDigest: "f".repeat(64) },
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("parity_trace_unbound")
  })

  test("accepts when the optimized kernel reproduces the baseline reference output", () => {
    // Cross-graph equivalence: baseline graph "d..." and optimized graph "b..."
    // differ (different kernel impls) but both yield output trace "a..." on the
    // same validator device, so the optimized kernel computes identical outputs.
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("accepted")
  })

  test("rejects a faster kernel that computes DIFFERENT outputs than the baseline", () => {
    // The optimized kernel's own replay is verified (self-consistent), but its
    // output trace ("a...") differs from the baseline reference ("9..."), so it
    // is faster but wrong across graphs.
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: {
        ...baselineParity,
        claimedTraceDigest: "9".repeat(64),
        replayedTraceDigest: "9".repeat(64),
      },
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("output_parity_mismatch")
  })

  test("rejects when the baseline reference output is not itself reproducible", () => {
    const verdict = verifyKernelOptimizationParity({
      // Baseline replay ran on the right graph ("d...") but did not verify, so
      // there is no trustworthy reference output to compare against.
      baselineParityVerdict: {
        ...rejectedParity,
        graphDigest: "d".repeat(64),
      },
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("output_parity_mismatch")
  })

  test("rejects when the baseline reference ran on a different validator device", () => {
    // A byte-for-byte output comparison is only meaningful on one device.
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: {
        ...baselineParity,
        validatorDeviceRef: "validator-device-7",
      },
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("output_parity_mismatch")
  })

  test("rejects when the baseline reference replayed a different graph than the baseline record names", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: { ...baselineParity, graphDigest: "f".repeat(64) },
      baseline,
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("parity_trace_unbound")
  })

  test("output parity is checked before throughput: faster-but-different is rejected on parity", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: {
        ...baselineParity,
        claimedTraceDigest: "9".repeat(64),
        replayedTraceDigest: "9".repeat(64),
      },
      baseline,
      // Much faster, but different outputs than the baseline.
      optimized: { ...optimized, tokensPerSecond: 9999 },
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(verdict.outcome).toBe("rejected")
    expect(verdict.rejection?.reason).toBe("output_parity_mismatch")
  })

  test("speed never overrides correctness: faster-but-wrong is rejected", () => {
    const verdict = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
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
      baselineParityVerdict: baselineParity,
      baseline: { ...baseline, tokensPerSecond: 0 },
      optimized,
      optimizedOpRef: "rmsnorm",
      parityVerdict: verifiedParity,
    })
    expect(zero.outcome).toBe("rejected")
    expect(zero.rejection?.reason).toBe("invalid_throughput")

    const nan = verifyKernelOptimizationParity({
      baselineParityVerdict: baselineParity,
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
