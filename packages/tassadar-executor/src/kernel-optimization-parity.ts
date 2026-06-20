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
  /**
   * The op this record's kernel implements (e.g. "rmsnorm",
   * "attention.flash"). Distinct from `kernelRef`, which names the *kernel
   * implementation* of that op (baseline vs optimized intentionally differ).
   * Both the baseline and optimized record must declare the SAME op as the one
   * the job claims to optimize, otherwise the tok/s numbers are not comparable
   * (a "rmsnorm" baseline vs an "attention.flash" optimized is apples-to-
   * oranges and must never be accepted as a speedup).
   */
  opRef: string
  /** Kernel/op identifier this record measures. */
  kernelRef: string
  /**
   * The model-graph digest the tok/s number was measured on — the same
   * `graph_digest` the exact-trace-replay engine reports as `graphDigest` on its
   * verdict. This is what BINDS the measured kernel to the artifact the
   * independent validator actually replayed: for the optimized record it must
   * equal the parity verdict's `graphDigest`, otherwise the tok/s number and the
   * correctness proof describe different artifacts (a "verified" parity for one
   * graph paired with a tok/s record for another, unverified kernel). Hex digest,
   * compared trim + case-insensitively.
   */
  graphDigest: string
  /** Measured throughput in tokens per second (must be finite and > 0). */
  tokensPerSecond: number
}>

export type KernelOptimizationRejection =
  | Readonly<{ reason: "target_mismatch"; detail: string }>
  | Readonly<{ reason: "op_mismatch"; detail: string }>
  | Readonly<{ reason: "kernel_not_optimized"; detail: string }>
  | Readonly<{ reason: "parity_trace_unbound"; detail: string }>
  | Readonly<{ reason: "output_parity_mismatch"; detail: string }>
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
 * Gate order is intentional: structural validity (same target, same op
 * provenance, a genuinely distinct delivered kernel, valid numbers) first, then
 * the parity correctness gate, then throughput. Op provenance is checked before
 * throughput because comparing tok/s across different ops is meaningless — a
 * faster number for a different op is not a speedup of the claimed op. The
 * delivered-kernel gate is also checked before throughput because if the
 * optimized record names the SAME kernel implementation as the baseline, no new
 * kernel was delivered and the tok/s delta is remeasurement (or a cherry-picked
 * run), not an optimization. The trace-binding gate is also checked before
 * throughput AND before the parity outcome: if the optimized record's graph
 * digest does not match the graph the validator actually replayed, the parity
 * proof does not pertain to the measured kernel, so even a "verified" verdict is
 * meaningless for it. Parity is checked before throughput so that "faster but
 * wrong" can never be accepted.
 *
 * Output-parity (cross-graph equivalence): the optimized kernel's own replay
 * verdict (`parityVerdict`) only proves the optimized graph is self-consistent
 * (the worker did not lie about its OWN outputs). The work definition's actual
 * correctness anchor is stronger — "the optimized kernel must produce IDENTICAL
 * outputs to the baseline". That is a TWO-graph claim, and the bounded-workload
 * replay engine re-executes ONE graph, so it is established by replaying the
 * baseline kernel on the SAME validator device (`baselineParityVerdict`) to get
 * the reference output, then requiring the independently-recomputed optimized
 * output trace to equal that reference byte-for-byte. A faster kernel that
 * computes different outputs than the baseline — even one whose own replay is
 * self-consistent — is wrong and is rejected (`output_parity_mismatch`), before
 * the throughput gate.
 */
export const verifyKernelOptimizationParity = (
  input: Readonly<{
    /** The op being optimized (e.g. "rmsnorm"); the dispatched job's kernelRef. */
    optimizedOpRef: string
    baseline: KernelThroughputRecord
    optimized: KernelThroughputRecord
    /** The OPTIMIZED kernel replayed on the independent validator device. */
    parityVerdict: TassadarReplayVerdict
    /**
     * The BASELINE kernel replayed on the SAME validator device, establishing
     * the reference output the optimized kernel must reproduce. Without it the
     * verifier can prove the optimized kernel is self-consistent but NOT that it
     * computes the same thing as the baseline (cross-graph output equivalence).
     */
    baselineParityVerdict: TassadarReplayVerdict
  }>,
): KernelOptimizationVerdict => {
  const { baseline, baselineParityVerdict, optimized, parityVerdict } = input
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

  const normalizeOp = (value: string): string => value.trim().toLowerCase()
  const claimedOp = normalizeOp(input.optimizedOpRef)
  const baselineOp = normalizeOp(baseline.opRef)
  const optimizedOp = normalizeOp(optimized.opRef)
  if (claimedOp.length === 0) {
    return rejected({
      detail: "optimizedOpRef must name a non-empty op",
      reason: "op_mismatch",
    })
  }
  if (baselineOp !== claimedOp || optimizedOp !== claimedOp) {
    return rejected({
      detail: `op provenance mismatch: claimed "${input.optimizedOpRef.trim()}" but baseline record op "${baseline.opRef.trim()}", optimized record op "${optimized.opRef.trim()}"`,
      reason: "op_mismatch",
    })
  }

  // Deliverable gate: the work definition requires an agent-authored optimized
  // kernel DISTINCT from the baseline kernel. If the optimized record names the
  // same kernel implementation as the baseline (or no implementation at all),
  // no new kernel was delivered, so any tok/s delta is remeasurement noise or a
  // cherry-picked run, not an optimization — never an accepted speedup.
  const normalizeKernel = (value: string): string => value.trim().toLowerCase()
  const baselineKernel = normalizeKernel(baseline.kernelRef)
  const optimizedKernel = normalizeKernel(optimized.kernelRef)
  if (optimizedKernel.length === 0) {
    return rejected({
      detail:
        "optimized record must name a non-empty kernel implementation (the delivered kernel)",
      reason: "kernel_not_optimized",
    })
  }
  if (optimizedKernel === baselineKernel) {
    return rejected({
      detail: `optimized record names the same kernel implementation as the baseline ("${optimized.kernelRef.trim()}"); no new kernel was delivered, so the tok/s delta is remeasurement, not an optimization`,
      reason: "kernel_not_optimized",
    })
  }

  // Trace-binding gate: the optimized tok/s record must have been measured on the
  // SAME model graph the independent validator replayed for parity. The parity
  // engine reports that graph as `parityVerdict.graphDigest`; the optimized
  // record carries the graph its tok/s was measured on. If they differ (or the
  // record names none), the throughput number and the correctness proof describe
  // different artifacts — a "verified" parity for one graph paired with a tok/s
  // record for another, unverified kernel — so the proof does not pertain to the
  // measured kernel and acceptance must be refused. Structural and checked before
  // the parity outcome, since an unbound parity proof says nothing about THIS
  // kernel even when its own verdict reads "verified".
  const normalizeDigest = (value: string): string => value.trim().toLowerCase()
  const optimizedGraph = normalizeDigest(optimized.graphDigest)
  const replayedGraph = normalizeDigest(parityVerdict.graphDigest)
  if (optimizedGraph.length === 0) {
    return rejected({
      detail:
        "optimized record must name the graph digest its tok/s was measured on (to bind it to the replayed parity trace)",
      reason: "parity_trace_unbound",
    })
  }
  if (optimizedGraph !== replayedGraph) {
    return rejected({
      detail: `optimized record graph "${optimized.graphDigest.trim()}" != replayed parity-trace graph "${parityVerdict.graphDigest.trim()}"; the tok/s record and the correctness proof describe different artifacts`,
      reason: "parity_trace_unbound",
    })
  }

  // Symmetric baseline trace-binding: the baseline reference output is only
  // trustworthy if it was replayed on the graph the baseline record actually
  // names. Otherwise the "reference" the optimized output is compared against
  // could be some other graph's outputs.
  const baselineGraph = normalizeDigest(baseline.graphDigest)
  const baselineReplayedGraph = normalizeDigest(baselineParityVerdict.graphDigest)
  if (baselineGraph.length === 0) {
    return rejected({
      detail:
        "baseline record must name the graph digest its reference output was measured on (to bind it to the baseline replay)",
      reason: "parity_trace_unbound",
    })
  }
  if (baselineGraph !== baselineReplayedGraph) {
    return rejected({
      detail: `baseline record graph "${baseline.graphDigest.trim()}" != baseline reference-replay graph "${baselineParityVerdict.graphDigest.trim()}"; the reference output does not pertain to the named baseline kernel`,
      reason: "parity_trace_unbound",
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

  // Output-parity gate (cross-graph output equivalence). The optimized parity
  // verdict above only proves the optimized kernel is self-consistent. The work
  // definition requires it to produce IDENTICAL outputs to the baseline, so the
  // baseline reference output (itself an independent replay on the SAME device)
  // must equal the optimized kernel's recomputed output trace. Checked before
  // throughput: a faster kernel that computes different outputs is wrong.
  if (baselineParityVerdict.outcome !== "verified") {
    return rejected({
      detail:
        "baseline reference output is not itself reproducible on the validator device, so there is no trustworthy reference to compare the optimized output against",
      reason: "output_parity_mismatch",
    })
  }
  const optimizedValidator = parityVerdict.validatorDeviceRef.trim()
  const baselineValidator = baselineParityVerdict.validatorDeviceRef.trim()
  if (optimizedValidator !== baselineValidator) {
    return rejected({
      detail: `baseline reference recomputed on a different validator device ("${baselineValidator}") than the optimized kernel ("${optimizedValidator}"); a byte-for-byte output comparison is only meaningful on one device`,
      reason: "output_parity_mismatch",
    })
  }
  const optimizedOutput = normalizeDigest(parityVerdict.replayedTraceDigest ?? "")
  const baselineOutput = normalizeDigest(
    baselineParityVerdict.replayedTraceDigest ?? "",
  )
  if (optimizedOutput.length === 0 || baselineOutput.length === 0) {
    return rejected({
      detail:
        "missing a recomputed output trace digest on one side; cannot prove the optimized kernel reproduces the baseline output",
      reason: "output_parity_mismatch",
    })
  }
  if (optimizedOutput !== baselineOutput) {
    return rejected({
      detail: `optimized output trace "${(parityVerdict.replayedTraceDigest ?? "").trim()}" != baseline reference output trace "${(baselineParityVerdict.replayedTraceDigest ?? "").trim()}"; the optimized kernel produces different outputs than the baseline (faster but wrong)`,
      reason: "output_parity_mismatch",
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
