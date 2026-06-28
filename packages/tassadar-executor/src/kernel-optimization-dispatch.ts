/**
 * Kernel-optimization market dispatch: bind the throughput-parity acceptance
 * verdict to the verified-work labor rail.
 *
 * Advances `compute.agentic_kernel_optimization_at_scale.v1` blocker
 * `agentic_kernel_optimization_market_dispatch_missing` by making mechanically
 * checkable (not paper-only) the two market boundaries the work definition
 * (docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md)
 * describes:
 *
 *   1. DISPATCH — turn a named target + named-baseline tok/s record into a
 *      public, public-safe kernel-optimization WORK REQUEST shaped like the
 *      green labor market (`labor.forum_work_requests.v1`): it names the target
 *      model+device, the baseline throughput record, the required parity
 *      capability, and the dual acceptance criteria (throughput improvement AND
 *      output parity via exact-trace-replay on an independent validator device).
 *   2. SETTLE — turn the kernel-optimization acceptance verdict
 *      (`KernelOptimizationVerdict` from ./kernel-optimization-parity) into a
 *      BORN-VERIFIED settlement claim that can clear through the verified-work
 *      rail (`labor.nostr_negotiation_market.v1`). A claim is ONLY ever produced
 *      for an `accepted` verdict — a faster-but-wrong kernel (parity rejected) or
 *      a no-speedup kernel never yields a payable claim, so correctness and
 *      throughput dominate at the money boundary, not just at verification.
 *
 * This module moves no money and creates no serving claim. It produces the
 * public request body a dispatcher would post, and the settlement claim a
 * verified-work clear would consume; live posting, network execution, and real
 * receipts remain out of scope (and the promise stays red).
 */
import type {
  KernelOptimizationVerdict,
  KernelThroughputRecord,
} from "./kernel-optimization-parity.js"
import { KERNEL_OPTIMIZATION_PARITY_CLASS_ID } from "./kernel-optimization-parity.js"
import { TASSADAR_TS_REPLAY_CLASS_ID } from "./replay.js"
import { TASSADAR_EXECUTOR_CAPABILITY_REF } from "./lane.js"

export const KERNEL_OPTIMIZATION_PROMISE_ID =
  "compute.agentic_kernel_optimization_at_scale.v1"

export const KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA =
  "openagents.kernel_optimization_work_request.v1"

export const KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA =
  "openagents.kernel_optimization_settlement_claim.v1"

export const KERNEL_OPTIMIZATION_ACCEPTED_WORK_RECEIPT_SCHEMA =
  "openagents.kernel_optimization_accepted_work_receipt.v1"

/**
 * Same posture as the green labor rail: a worker payout only ever follows
 * accepted work, never on dispatch. Surfaced on both the request (the promise to
 * the worker) and the claim (the realized accepted-work settlement).
 */
export const KERNEL_OPTIMIZATION_SETTLEMENT_MODE =
  "no_worker_payout_until_accepted_work" as const

/**
 * Mirrors the public-safety guard the green work-requester enforces
 * (apps/pylon/src/work-requester.ts): a public kernel-optimization request must
 * never embed a local path, credential, wallet, payment, or raw secret material.
 */
const unsafeRequestPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|seed)|xprv)/i

const assertPublicSafe = (value: string, field: string): void => {
  if (unsafeRequestPattern.test(value)) {
    throw new Error(
      `kernel-optimization ${field} contains private, payment, credential, wallet, or raw secret material`,
    )
  }
}

const assertNonEmpty = (value: string, field: string): string => {
  const trimmed = value.trim()
  if (trimmed.length === 0) {
    throw new Error(`kernel-optimization ${field} must be a non-empty value`)
  }
  assertPublicSafe(trimmed, field)
  return trimmed
}

const isPositiveFinite = (value: number): boolean =>
  Number.isFinite(value) && value > 0

/** Inputs to dispatch one kernel-optimization job to the market. */
export type KernelOptimizationJobSpec = Readonly<{
  /** Named open model, e.g. "qwen-3.5-0.5b". */
  targetModel: string
  /** Named device class, e.g. "cuda", "metal", "webgpu". */
  device: string
  /** Declared hardware the baseline tok/s was measured on. */
  hardwareRef: string
  /** The op to optimize, e.g. "rmsnorm", "attention.flash". */
  opRef: string
  /**
   * The named-baseline throughput record (the prior-best/unoptimized kernel on
   * this exact target). Its target fields must match the job target, and its op
   * must match `opRef`, otherwise the speedup the job advertises is not
   * comparable.
   */
  baseline: KernelThroughputRecord
  /**
   * The independent validator device the parity verdict must be recomputed on.
   * Named at dispatch so acceptance is auditable against the dispatched job.
   */
  validatorDeviceRef: string
  /** Worker budget in sats (integer, > 0). */
  budgetSats: number
  /** Public deadline ref, e.g. "deadline.public.kernel_opt.2026-06-30". */
  deadlineRef: string
}>

/**
 * A public, market-dispatchable kernel-optimization work request — the body a
 * dispatcher posts to the labor rail. Every field is public-safe and binds the
 * job to a named target, a named baseline, the parity capability, and the dual
 * acceptance criteria.
 */
export type KernelOptimizationWorkRequest = Readonly<{
  schema: typeof KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA
  promiseId: typeof KERNEL_OPTIMIZATION_PROMISE_ID
  target: Readonly<{ targetModel: string; device: string; hardwareRef: string }>
  opRef: string
  /** The named-baseline record the optimized kernel must beat (correctly). */
  namedBaseline: Readonly<{
    kernelRef: string
    tokensPerSecond: number
    graphDigest: string
  }>
  /** Dual acceptance criteria, mechanically checked at settlement. */
  acceptanceCriteriaRefs: ReadonlyArray<string>
  /** Capabilities a worker/validator must hold to take + verify this job. */
  requiredCapabilityRefs: ReadonlyArray<string>
  /** The parity class the optimized kernel is verified under. */
  parityClassId: typeof KERNEL_OPTIMIZATION_PARITY_CLASS_ID
  /** The exact-trace-replay class the parity verdict is recomputed under. */
  replayClassId: typeof TASSADAR_TS_REPLAY_CLASS_ID
  /** The independent validator device parity must be recomputed on. */
  validatorDeviceRef: string
  budgetSats: number
  deadlineRef: string
  settlementMode: typeof KERNEL_OPTIMIZATION_SETTLEMENT_MODE
}>

/**
 * Build the public work-request body for one kernel-optimization job.
 *
 * Throws on a structurally invalid or unsafe spec (target/op mismatch between
 * the job and its named baseline, non-positive baseline tok/s, non-integer or
 * non-positive budget, or any embedded secret/path/credential material), so a
 * malformed or unsafe job is never dispatched.
 */
export const buildKernelOptimizationWorkRequest = (
  spec: KernelOptimizationJobSpec,
): KernelOptimizationWorkRequest => {
  const targetModel = assertNonEmpty(spec.targetModel, "targetModel")
  const device = assertNonEmpty(spec.device, "device")
  const hardwareRef = assertNonEmpty(spec.hardwareRef, "hardwareRef")
  const opRef = assertNonEmpty(spec.opRef, "opRef")
  const validatorDeviceRef = assertNonEmpty(
    spec.validatorDeviceRef,
    "validatorDeviceRef",
  )
  const deadlineRef = assertNonEmpty(spec.deadlineRef, "deadlineRef")
  const baselineKernelRef = assertNonEmpty(
    spec.baseline.kernelRef,
    "baseline.kernelRef",
  )
  const baselineGraphDigest = assertNonEmpty(
    spec.baseline.graphDigest,
    "baseline.graphDigest",
  )

  if (
    spec.baseline.targetModel !== spec.targetModel ||
    spec.baseline.device !== spec.device ||
    spec.baseline.hardwareRef !== spec.hardwareRef
  ) {
    throw new Error(
      `kernel-optimization baseline target ${spec.baseline.targetModel}/${spec.baseline.device}/${spec.baseline.hardwareRef} != job target ${spec.targetModel}/${spec.device}/${spec.hardwareRef}; the advertised speedup would not be comparable`,
    )
  }
  if (
    spec.baseline.opRef.trim().toLowerCase() !== opRef.toLowerCase()
  ) {
    throw new Error(
      `kernel-optimization baseline op "${spec.baseline.opRef.trim()}" != job op "${opRef}"; a baseline for a different op cannot anchor this job's speedup`,
    )
  }
  if (!isPositiveFinite(spec.baseline.tokensPerSecond)) {
    throw new Error(
      `kernel-optimization baseline tok/s must be finite and > 0 (got ${spec.baseline.tokensPerSecond})`,
    )
  }
  if (!Number.isInteger(spec.budgetSats) || spec.budgetSats <= 0) {
    throw new Error(
      `kernel-optimization budgetSats must be a positive integer sat amount (got ${spec.budgetSats})`,
    )
  }

  return {
    acceptanceCriteriaRefs: [
      "acceptance.kernel_opt.throughput_improvement_vs_named_baseline",
      "acceptance.kernel_opt.output_parity_independent_replay",
    ],
    budgetSats: spec.budgetSats,
    deadlineRef,
    namedBaseline: {
      graphDigest: baselineGraphDigest,
      kernelRef: baselineKernelRef,
      tokensPerSecond: spec.baseline.tokensPerSecond,
    },
    opRef,
    parityClassId: KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
    promiseId: KERNEL_OPTIMIZATION_PROMISE_ID,
    replayClassId: TASSADAR_TS_REPLAY_CLASS_ID,
    requiredCapabilityRefs: [
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      "capability.kernel_opt.independent_validator_replay",
    ],
    schema: KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA,
    settlementMode: KERNEL_OPTIMIZATION_SETTLEMENT_MODE,
    target: { device, hardwareRef, targetModel },
    validatorDeviceRef,
  }
}

/**
 * A born-verified kernel-optimization settlement claim — the body a
 * verified-work clear consumes. Produced ONLY from an accepted verdict; carries
 * the proven speedup and parity provenance so settlement never re-litigates
 * correctness.
 */
export type KernelOptimizationSettlementClaim = Readonly<{
  schema: typeof KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA
  promiseId: typeof KERNEL_OPTIMIZATION_PROMISE_ID
  parityClassId: typeof KERNEL_OPTIMIZATION_PARITY_CLASS_ID
  target: Readonly<{ targetModel: string; device: string; hardwareRef: string }>
  optimizedOpRef: string
  baselineTokensPerSecond: number
  optimizedTokensPerSecond: number
  speedupRatio: number
  parityOutcome: "verified"
  parityValidatorDeviceRef: string
  bornVerified: true
  settlementMode: typeof KERNEL_OPTIMIZATION_SETTLEMENT_MODE
}>

export type KernelOptimizationSettlementResult =
  | Readonly<{ ok: true; claim: KernelOptimizationSettlementClaim }>
  | Readonly<{ ok: false; reason: "verdict_not_accepted"; detail: string }>

/**
 * Convert a kernel-optimization acceptance verdict into a born-verified
 * settlement claim that can clear through the verified-work rail.
 *
 * A claim is ONLY produced for an `accepted` verdict with a verified parity
 * outcome and a finite positive speedup ratio — the same dual gate the verdict
 * itself enforces, re-asserted at the money boundary so a rejected verdict (or a
 * tampered one with a missing speedup) can never settle. This is the dispatch
 * counterpart to verification: speed never overrides correctness, and a verdict
 * that did not pass cannot be paid.
 */
export const buildKernelOptimizationSettlementClaim = (
  verdict: KernelOptimizationVerdict,
): KernelOptimizationSettlementResult => {
  if (verdict.outcome !== "accepted") {
    return {
      detail: `verdict outcome is "${verdict.outcome}"${verdict.rejection ? ` (${verdict.rejection.reason})` : ""}; only an accepted verdict settles`,
      ok: false,
      reason: "verdict_not_accepted",
    }
  }
  if (verdict.parityOutcome !== "verified") {
    return {
      detail: `accepted verdict carries parity outcome "${verdict.parityOutcome}"; a settlement requires a verified parity proof`,
      ok: false,
      reason: "verdict_not_accepted",
    }
  }
  if (verdict.speedupRatio === null || !isPositiveFinite(verdict.speedupRatio)) {
    return {
      detail: `accepted verdict has no finite positive speedup ratio (${verdict.speedupRatio}); cannot settle a non-improving kernel`,
      ok: false,
      reason: "verdict_not_accepted",
    }
  }
  return {
    claim: {
      baselineTokensPerSecond: verdict.baselineTokensPerSecond,
      bornVerified: true,
      optimizedOpRef: verdict.optimizedOpRef,
      optimizedTokensPerSecond: verdict.optimizedTokensPerSecond,
      parityClassId: KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
      parityOutcome: "verified",
      parityValidatorDeviceRef: verdict.parityValidatorDeviceRef,
      promiseId: KERNEL_OPTIMIZATION_PROMISE_ID,
      schema: KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA,
      settlementMode: KERNEL_OPTIMIZATION_SETTLEMENT_MODE,
      speedupRatio: verdict.speedupRatio,
      target: verdict.target,
    },
    ok: true,
  }
}

export type KernelOptimizationAcceptedWorkReceipt = Readonly<{
  schema: typeof KERNEL_OPTIMIZATION_ACCEPTED_WORK_RECEIPT_SCHEMA
  promiseId: typeof KERNEL_OPTIMIZATION_PROMISE_ID
  workRequestSchema: typeof KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA
  settlementClaimSchema: typeof KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA
  acceptedWorkRef: string
  throughputRecordRef: string
  parityVerdictRef: string
  receiptState: "accepted_work_proven"
  marketRailRefs: ReadonlyArray<string>
  workRequest: KernelOptimizationWorkRequest
  settlementClaim: KernelOptimizationSettlementClaim
  settlementState: "not_settled"
  payoutClaimAllowed: false
}>

export type KernelOptimizationAcceptedWorkReceiptResult =
  | Readonly<{ ok: true; receipt: KernelOptimizationAcceptedWorkReceipt }>
  | Readonly<{
      ok: false
      reason: "request_verdict_mismatch" | "verdict_not_accepted"
      detail: string
    }>

const sameTarget = (
  request: KernelOptimizationWorkRequest,
  verdict: KernelOptimizationVerdict,
): boolean =>
  request.target.targetModel === verdict.target.targetModel &&
  request.target.device === verdict.target.device &&
  request.target.hardwareRef === verdict.target.hardwareRef

/**
 * Bind a dispatched kernel-optimization request to the accepted throughput +
 * parity verdict and the born-verified settlement claim.
 *
 * This is the public-safe receipt shape the verified-work market can record for
 * an accepted kernel optimization. It is deliberately not a payout or settled
 * payment receipt: it proves the work is accepted and settlement-ready, while
 * keeping actual payout/settlement authority on the existing verified-work rail.
 */
export const buildKernelOptimizationAcceptedWorkReceipt = (
  input: Readonly<{
    workRequest: KernelOptimizationWorkRequest
    verdict: KernelOptimizationVerdict
    acceptedWorkRef: string
    throughputRecordRef: string
    parityVerdictRef: string
  }>,
): KernelOptimizationAcceptedWorkReceiptResult => {
  const acceptedWorkRef = assertNonEmpty(
    input.acceptedWorkRef,
    "acceptedWorkRef",
  )
  const throughputRecordRef = assertNonEmpty(
    input.throughputRecordRef,
    "throughputRecordRef",
  )
  const parityVerdictRef = assertNonEmpty(
    input.parityVerdictRef,
    "parityVerdictRef",
  )

  if (!sameTarget(input.workRequest, input.verdict)) {
    return {
      detail: `request target ${input.workRequest.target.targetModel}/${input.workRequest.target.device}/${input.workRequest.target.hardwareRef} != verdict target ${input.verdict.target.targetModel}/${input.verdict.target.device}/${input.verdict.target.hardwareRef}`,
      ok: false,
      reason: "request_verdict_mismatch",
    }
  }
  if (
    input.workRequest.opRef.trim().toLowerCase() !==
    input.verdict.optimizedOpRef.trim().toLowerCase()
  ) {
    return {
      detail: `request op "${input.workRequest.opRef.trim()}" != verdict op "${input.verdict.optimizedOpRef.trim()}"`,
      ok: false,
      reason: "request_verdict_mismatch",
    }
  }
  if (
    input.workRequest.namedBaseline.tokensPerSecond !==
    input.verdict.baselineTokensPerSecond
  ) {
    return {
      detail: `request baseline tok/s ${input.workRequest.namedBaseline.tokensPerSecond} != verdict baseline tok/s ${input.verdict.baselineTokensPerSecond}`,
      ok: false,
      reason: "request_verdict_mismatch",
    }
  }
  if (
    input.workRequest.validatorDeviceRef !== input.verdict.parityValidatorDeviceRef
  ) {
    return {
      detail: `request validator ${input.workRequest.validatorDeviceRef} != verdict validator ${input.verdict.parityValidatorDeviceRef}`,
      ok: false,
      reason: "request_verdict_mismatch",
    }
  }

  const settlement = buildKernelOptimizationSettlementClaim(input.verdict)
  if (!settlement.ok) {
    return {
      detail: settlement.detail,
      ok: false,
      reason: "verdict_not_accepted",
    }
  }

  return {
    ok: true,
    receipt: {
      acceptedWorkRef,
      marketRailRefs: [
        "promise:labor.forum_work_requests.v1",
        "promise:labor.nostr_negotiation_market.v1",
      ],
      parityVerdictRef,
      payoutClaimAllowed: false,
      promiseId: KERNEL_OPTIMIZATION_PROMISE_ID,
      receiptState: "accepted_work_proven",
      schema: KERNEL_OPTIMIZATION_ACCEPTED_WORK_RECEIPT_SCHEMA,
      settlementClaim: settlement.claim,
      settlementClaimSchema: KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA,
      settlementState: "not_settled",
      throughputRecordRef,
      workRequest: input.workRequest,
      workRequestSchema: KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA,
    },
  }
}
