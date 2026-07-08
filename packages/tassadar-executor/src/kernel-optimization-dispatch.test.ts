import { describe, expect, test } from "bun:test"

import {
  KERNEL_OPTIMIZATION_ACCEPTED_WORK_RECEIPT_SCHEMA,
  KERNEL_OPTIMIZATION_PROMISE_ID,
  KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA,
  KERNEL_OPTIMIZATION_SETTLEMENT_MODE,
  KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA,
  buildKernelOptimizationAcceptedWorkReceipt,
  buildKernelOptimizationSettlementClaim,
  buildKernelOptimizationWorkRequest,
  type KernelOptimizationJobSpec,
} from "./kernel-optimization-dispatch.js"
import {
  KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
  type KernelOptimizationVerdict,
  type KernelThroughputRecord,
} from "./kernel-optimization-parity.js"

// Anchored on the historical March-2026 Psionic/Qwen 3.5 0.5B result
// (docs/transcripts/217.md): baseline ~328 tok/s, optimized ~523 tok/s.
const baseline: KernelThroughputRecord = {
  device: "cuda",
  graphDigest: "d".repeat(64),
  hardwareRef: "single-machine-demo-gpu",
  kernelRef: "baseline-runtime",
  opRef: "rmsnorm",
  targetModel: "qwen-3.5-0.5b",
  tokensPerSecond: 328,
}

const spec: KernelOptimizationJobSpec = {
  baseline,
  budgetSats: 5000,
  deadlineRef: "deadline.public.kernel_opt.2026-06-30",
  device: "cuda",
  hardwareRef: "single-machine-demo-gpu",
  opRef: "rmsnorm",
  targetModel: "qwen-3.5-0.5b",
  validatorDeviceRef: "validator-device-2",
}

const acceptedVerdict: KernelOptimizationVerdict = {
  baselineTokensPerSecond: 328,
  classId: KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
  optimizedOpRef: "rmsnorm",
  optimizedTokensPerSecond: 523,
  outcome: "accepted",
  parityOutcome: "verified",
  parityValidatorDeviceRef: "validator-device-2",
  rejection: null,
  speedupRatio: 523 / 328,
  target: {
    device: "cuda",
    hardwareRef: "single-machine-demo-gpu",
    targetModel: "qwen-3.5-0.5b",
  },
}

describe("buildKernelOptimizationWorkRequest", () => {
  test("builds a public, market-dispatchable request bound to the named baseline", () => {
    const request = buildKernelOptimizationWorkRequest(spec)
    expect(request.schema).toBe(KERNEL_OPTIMIZATION_WORK_REQUEST_SCHEMA)
    expect(request.promiseId).toBe(KERNEL_OPTIMIZATION_PROMISE_ID)
    expect(request.parityClassId).toBe(KERNEL_OPTIMIZATION_PARITY_CLASS_ID)
    expect(request.target).toEqual({
      device: "cuda",
      hardwareRef: "single-machine-demo-gpu",
      targetModel: "qwen-3.5-0.5b",
    })
    expect(request.namedBaseline.tokensPerSecond).toBe(328)
    expect(request.settlementMode).toBe(KERNEL_OPTIMIZATION_SETTLEMENT_MODE)
    // Both acceptance gates must be named on the dispatched job.
    expect(request.acceptanceCriteriaRefs).toContain(
      "acceptance.kernel_opt.throughput_improvement_vs_named_baseline",
    )
    expect(request.acceptanceCriteriaRefs).toContain(
      "acceptance.kernel_opt.output_parity_independent_replay",
    )
  })

  test("refuses a baseline whose target does not match the job", () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({
        ...spec,
        baseline: { ...baseline, targetModel: "qwen-3.5-4b" },
      }),
    ).toThrow(/baseline target/)
  })

  test("refuses a baseline for a different op", () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({
        ...spec,
        baseline: { ...baseline, opRef: "attention.flash" },
      }),
    ).toThrow(/baseline op/)
  })

  test("refuses a non-positive budget", () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({ ...spec, budgetSats: 0 }),
    ).toThrow(/budgetSats/)
  })

  test("refuses unsafe (credential/path) material in any field", () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({
        ...spec,
        deadlineRef: "/Users/secret/wallet.seed",
      }),
    ).toThrow(/private, payment, credential/)
  })
})

describe("buildKernelOptimizationSettlementClaim", () => {
  test("produces a born-verified claim from an accepted verdict", () => {
    const result = buildKernelOptimizationSettlementClaim(acceptedVerdict)
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected accepted verdict to settle")
    expect(result.claim.schema).toBe(KERNEL_OPTIMIZATION_SETTLEMENT_CLAIM_SCHEMA)
    expect(result.claim.bornVerified).toBe(true)
    expect(result.claim.parityOutcome).toBe("verified")
    expect(result.claim.speedupRatio).toBeCloseTo(523 / 328)
    expect(result.claim.settlementMode).toBe(KERNEL_OPTIMIZATION_SETTLEMENT_MODE)
  })

  test("refuses to settle a rejected verdict (faster-but-wrong never pays)", () => {
    const rejected: KernelOptimizationVerdict = {
      ...acceptedVerdict,
      outcome: "rejected",
      parityOutcome: "rejected",
      rejection: { parityRejection: null, reason: "parity_rejected" },
    }
    const result = buildKernelOptimizationSettlementClaim(rejected)
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("a rejected verdict must not settle")
    expect(result.reason).toBe("verdict_not_accepted")
  })

  test("refuses to settle an accepted verdict missing a speedup ratio", () => {
    const noSpeedup: KernelOptimizationVerdict = {
      ...acceptedVerdict,
      speedupRatio: null,
    }
    const result = buildKernelOptimizationSettlementClaim(noSpeedup)
    expect(result.ok).toBe(false)
  })
})

describe("buildKernelOptimizationAcceptedWorkReceipt", () => {
  test("records an accepted-work receipt bound to request, throughput, parity, and settlement", () => {
    const request = buildKernelOptimizationWorkRequest(spec)
    const result = buildKernelOptimizationAcceptedWorkReceipt({
      acceptedWorkRef: "accepted.kernel_opt.qwen35.rmsnorm.fixture.v1",
      parityVerdictRef: "verdict.kernel_opt.parity.fixture.v1",
      throughputRecordRef: "throughput.kernel_opt.qwen35.rmsnorm.fixture.v1",
      verdict: acceptedVerdict,
      workRequest: request,
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected accepted-work receipt")
    expect(result.receipt.schema).toBe(
      KERNEL_OPTIMIZATION_ACCEPTED_WORK_RECEIPT_SCHEMA,
    )
    expect(result.receipt.receiptState).toBe("accepted_work_proven")
    expect(result.receipt.settlementState).toBe("not_settled")
    expect(result.receipt.payoutClaimAllowed).toBe(false)
    expect(result.receipt.workRequest).toBe(request)
    expect(result.receipt.settlementClaim.speedupRatio).toBeCloseTo(523 / 328)
    expect(result.receipt.marketRailRefs).toEqual([
      "promise:labor.forum_work_requests.v1",
      "promise:labor.nostr_negotiation_market.v1",
    ])
  })

  test("refuses to bind a verdict for a different requested op", () => {
    const request = buildKernelOptimizationWorkRequest({
      ...spec,
      baseline: { ...baseline, opRef: "attention.flash" },
      opRef: "attention.flash",
    })
    const result = buildKernelOptimizationAcceptedWorkReceipt({
      acceptedWorkRef: "accepted.kernel_opt.qwen35.attention.fixture.v1",
      parityVerdictRef: "verdict.kernel_opt.parity.fixture.v1",
      throughputRecordRef: "throughput.kernel_opt.qwen35.rmsnorm.fixture.v1",
      verdict: acceptedVerdict,
      workRequest: request,
    })

    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("op-mismatched receipt must fail")
    expect(result.reason).toBe("request_verdict_mismatch")
  })

  test("refuses to record accepted work for a rejected verdict", () => {
    const request = buildKernelOptimizationWorkRequest(spec)
    const rejected: KernelOptimizationVerdict = {
      ...acceptedVerdict,
      outcome: "rejected",
      parityOutcome: "rejected",
      rejection: { parityRejection: null, reason: "parity_rejected" },
    }
    const result = buildKernelOptimizationAcceptedWorkReceipt({
      acceptedWorkRef: "accepted.kernel_opt.qwen35.rmsnorm.fixture.v1",
      parityVerdictRef: "verdict.kernel_opt.parity.rejected.fixture.v1",
      throughputRecordRef: "throughput.kernel_opt.qwen35.rmsnorm.fixture.v1",
      verdict: rejected,
      workRequest: request,
    })

    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("rejected verdict receipt must fail")
    expect(result.reason).toBe("verdict_not_accepted")
  })

  test("refuses unsafe refs in receipt evidence fields", () => {
    const request = buildKernelOptimizationWorkRequest(spec)
    expect(() =>
      buildKernelOptimizationAcceptedWorkReceipt({
        acceptedWorkRef: "/Users/local/private/result",
        parityVerdictRef: "verdict.kernel_opt.parity.fixture.v1",
        throughputRecordRef: "throughput.kernel_opt.qwen35.rmsnorm.fixture.v1",
        verdict: acceptedVerdict,
        workRequest: request,
      }),
    ).toThrow(/private, payment, credential/)
  })
})
