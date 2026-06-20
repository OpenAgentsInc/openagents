/**
 * Kernel-optimization throughput-parity acceptance verdict.
 *
 * Implements (mechanically, not on paper) the acceptance rule from
 * docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md
 * for `compute.agentic_kernel_optimization_at_scale.v1`: an agent-authored
 * optimized kernel is ACCEPTED iff BOTH gates pass on the SAME declared target:
 *
 *   1. Output parity — the optimized kernel reproduces identical outputs,
 *      proven by the exact-trace-replay verdict on an independent validator
 *      device (`TassadarReplayVerdict` from ./replay, the green correctness
 *      anchor under compute.tassadar_executor_poc.v1).
 *   2. Throughput improvement — a public tok/s record that beats the named
 *      baseline on the declared hardware.
 *
 * Correctness dominates speed: a faster-but-wrong kernel (parity rejected)
 * is REJECTED regardless of its throughput. This verdict moves no money and
 * creates no serving claim; it only informs verified-work acceptance, which
 * then settles through the labor market rail.
 */
import type { TassadarReplayRejection, TassadarReplayVerdict } from "./replay.js"

export const KERNEL_OPTIMIZATION_PARITY_CLASS_ID =
  "kernel_optimization_throughput_parity.v1"

/** A public throughput record for one kernel on one declared target. */
export type KernelThroughputRecord = Readonly<{
  /** Named open model, e.g. "qwen-3.5-0.5b". */
  targetModel: string
  /** Named device class, e.g. "cuda", "metal", "webgpu". */
  device: string
  /** Declared hardware the tok/s number was measured on. */
  hardwareRef: string
  /** Kernel/op identifier this record measures. */
  kernelRef: string
  /** Measured throughput in tokens per second (must be finite and > 0). */
  tokensPerSecond: number
}>

export type KernelOptimizationRejection =
  | Readonly<{ reason: "target_mismatch"; detail: string }>
  | Readonly<{ reason: "invalid_throughput"; detail: string }>
  | Readonly<{
      reason: "parity_rejected"
      parityRejection: TassadarReplayRejection | null
    }>
  | Readonly<{
      reason: "no_throughput_improvement"
      baselineTokensPerSecond: number
      optimizedTokensPerSecond: number
    }>

export type KernelOptimizationVerdict = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_PARITY_CLASS_ID
  outcome: "accepted" | "rejected"
  rejection: KernelOptimizationRejection | null
  target: Readonly<{ targetModel: string; device: string; hardwareRef: string }>
  /**
   * The op this optimization job targets (e.g. "rmsnorm", "attention.flash"),
   * matching the dispatched job's `kernelRef`. Distinct from the throughput
   * records' `kernelRef` (which name the baseline vs optimized *implementations*
   * of this op). Surfaced so a settlement can be bound back to the specific
   * dispatched job by OP, not just by model/device/hardware: two jobs can share
   * a target but optimize different ops, and only this field tells them apart.
   */
  optimizedOpRef: string
  baselineTokensPerSecond: number
  optimizedTokensPerSecond: number
  /** optimized / baseline tok/s when both are valid, else null. */
  speedupRatio: number | null
  parityOutcome: "verified" | "rejected"
  parityValidatorDeviceRef: string
}>

const isPositiveFinite = (value: number): boolean =>
  Number.isFinite(value) && value > 0

/**
 * Combine a named-baseline throughput record, the optimized-kernel throughput
 * record, and the independent-device exact-trace-replay parity verdict into a
 * single kernel-optimization acceptance verdict.
 *
 * Gate order is intentional: structural validity (same target, valid numbers)
 * first, then the parity correctness gate, then throughput. Parity is checked
 * before throughput so that "faster but wrong" can never be accepted.
 */
export const verifyKernelOptimizationParity = (
  input: Readonly<{
    /** The op being optimized (e.g. "rmsnorm"); the dispatched job's kernelRef. */
    optimizedOpRef: string
    baseline: KernelThroughputRecord
    optimized: KernelThroughputRecord
    parityVerdict: TassadarReplayVerdict
  }>,
): KernelOptimizationVerdict => {
  const { baseline, optimized, parityVerdict } = input
  const speedupRatio =
    isPositiveFinite(baseline.tokensPerSecond) &&
    isPositiveFinite(optimized.tokensPerSecond)
      ? optimized.tokensPerSecond / baseline.tokensPerSecond
      : null
  const base = {
    baselineTokensPerSecond: baseline.tokensPerSecond,
    classId: KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
    optimizedOpRef: input.optimizedOpRef.trim(),
    optimizedTokensPerSecond: optimized.tokensPerSecond,
    parityOutcome: parityVerdict.outcome,
    parityValidatorDeviceRef: parityVerdict.validatorDeviceRef,
    speedupRatio,
    target: {
      device: baseline.device,
      hardwareRef: baseline.hardwareRef,
      targetModel: baseline.targetModel,
    },
  } as const

  const rejected = (
    rejection: KernelOptimizationRejection,
  ): KernelOptimizationVerdict => ({ ...base, outcome: "rejected", rejection })

  if (
    baseline.targetModel !== optimized.targetModel ||
    baseline.device !== optimized.device ||
    baseline.hardwareRef !== optimized.hardwareRef
  ) {
    return rejected({
      detail: `baseline ${baseline.targetModel}/${baseline.device}/${baseline.hardwareRef} != optimized ${optimized.targetModel}/${optimized.device}/${optimized.hardwareRef}`,
      reason: "target_mismatch",
    })
  }

  if (
    !isPositiveFinite(baseline.tokensPerSecond) ||
    !isPositiveFinite(optimized.tokensPerSecond)
  ) {
    return rejected({
      detail: `tok/s must be finite and > 0 (baseline=${baseline.tokensPerSecond}, optimized=${optimized.tokensPerSecond})`,
      reason: "invalid_throughput",
    })
  }

  if (parityVerdict.outcome !== "verified") {
    return rejected({
      parityRejection: parityVerdict.rejection,
      reason: "parity_rejected",
    })
  }

  if (optimized.tokensPerSecond <= baseline.tokensPerSecond) {
    return rejected({
      baselineTokensPerSecond: baseline.tokensPerSecond,
      optimizedTokensPerSecond: optimized.tokensPerSecond,
      reason: "no_throughput_improvement",
    })
  }

  return { ...base, outcome: "accepted", rejection: null }
}
