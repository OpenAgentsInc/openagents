import { describe, expect, test } from 'vitest'

import { deriveDebtReceiptKey } from './debt-receipt-key'
import {
  type DebtReceiptSettlementInput,
  projectDebtReceiptSettlement,
} from './debt-receipt-policy'
import {
  HygieneLaneChurnTaxBacktestCases,
  HygieneLaneChurnTaxBacktestRef,
  HygieneLaneMaxPayoutSats,
  HygieneLaneMinPayoutSats,
  HygieneLaneSettlementUnsafe,
  type HygieneSizeDepthSignals,
  computeHygieneLaneSettlementSats,
  decideHygieneLaneSettlement,
  isHygieneLaneRunRef,
  replayHygieneLaneChurnTaxBacktest,
} from './hygiene-lane-settlement'
import {
  type TassadarRealSettlementGate,
  disabledTassadarRealSettlementGate,
} from './tassadar-run-settlement-gate'

const HYGIENE_RUN_REF = 'run.hygiene.lane.20260618'
const CONTRIBUTOR_REF = 'pylon.public.contributor.trigger'

const baseSignals: HygieneSizeDepthSignals = {
  behaviorReceiptGreen: true,
  changedWeightedLines: 0,
  debtReducedWeightedUnits: 0,
  duplicateReplay: false,
  filesTouched: 0,
  newDebtWeightedUnits: 0,
}

// A `payable` debt-receipt projection: defined → funded → verified → payable,
// settlement-approved but not yet settled. Built from the live policy so the
// hygiene bridge always tracks the real state machine.
const payableDebtReceiptInput: DebtReceiptSettlementInput = {
  acceptedWorkRefs: ['accepted_work.public.debt_receipt.5334.fixture_dedup'],
  baselineMetricRefs: ['metric.public.debt_receipt.5334.baseline'],
  budgetCapSats: 100,
  debtReceiptKeyInput: {
    debtReceiptRef: 'receipt.public.debt.5334',
    objectiveDigest: 'objective.public.debt_receipt.5334.dual_format_to_zero',
    repoBaselineRef: 'baseline.public.commit.c43992567',
    scopeDigest: 'scope.public.debt_receipt.5334.tassadar_fixture_pairs',
  },
  fundingApprovalRefs: ['approval.public.debt_receipt.5334.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  hygieneDeltaRefs: ['delta.public.debt_receipt.5334.dual_format_removed'],
  noNewEqualOrWorseDebtRefs: ['check.public.debt_receipt.5334.no_worse_debt'],
  payableSats: 80,
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewDecisionRefs: ['review.public.debt_receipt.5334.accepted'],
  reviewerActorRef: 'actor.public.reviewer.trigger',
  scopeRefs: ['scope.public.debt_receipt.5334.tassadar_fixture_pairs'],
  settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5334'],
  stopConditionRefs: ['stop.public.debt_receipt.5334.retire_once'],
  targetMetricRefs: ['metric.public.debt_receipt.5334.target.churn_0'],
  verificationCommandRefs: ['command.public.debt_receipt.5334.regen_and_diff'],
  workerActorRef: 'actor.public.worker.codex_loop',
}

const payableProjection = projectDebtReceiptSettlement(payableDebtReceiptInput)

// An armed gate that allowlists the hygiene run-ref under the per-payout cap of
// 100 sats (the #5372 owner-authorized shape), via run-scoped streaming so any
// contributor on the lane with a registered Spark target is eligible.
const armedHygieneGate: TassadarRealSettlementGate = {
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: [HYGIENE_RUN_REF],
  enabled: true,
  maxDailyPayoutSats: 5_000,
  maxPayoutSats: 100,
  runScopedStreaming: true,
}

describe('hygiene-lane run-ref shape', () => {
  test('accepts the dated hygiene lane ref and rejects others', () => {
    expect(isHygieneLaneRunRef('run.hygiene.lane.20260618')).toBe(true)
    expect(isHygieneLaneRunRef('run.hygiene.lane.2026')).toBe(false)
    expect(isHygieneLaneRunRef('run.cs336.a1.demo')).toBe(false)
    expect(isHygieneLaneRunRef('run.hygiene.lane.')).toBe(false)
  })
})

describe('computeHygieneLaneSettlementSats (churn_tax.v0.backtest, #5369/#5372)', () => {
  test('a tiny targeted simplification pays a few sats (well under the cap)', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      changedWeightedLines: 12,
      debtReducedWeightedUnits: 1,
      filesTouched: 1,
    })

    expect(result.denialReason).toBeNull()
    expect(result.formulaRef).toBe(HygieneLaneChurnTaxBacktestRef)
    expect(result.payoutMultiplierBps).toBe(900)
    expect(result.payoutSats).toBe(9)
    expect(result.payoutSats).toBeGreaterThanOrEqual(HygieneLaneMinPayoutSats)
    expect(result.payoutSats).toBeLessThan(20)
  })

  test('a deep multi-file debt-reducing move scales up to the 100-sat cap', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      changedWeightedLines: 4_000,
      debtReducedWeightedUnits: 40,
      filesTouched: 12,
    })

    expect(result.denialReason).toBeNull()
    expect(result.formulaRef).toBe(HygieneLaneChurnTaxBacktestRef)
    expect(result.payoutMultiplierBps).toBe(10_000)
    expect(result.payoutSats).toBe(HygieneLaneMaxPayoutSats)
  })

  test('replays the public churn-tax fixture set', () => {
    const results = replayHygieneLaneChurnTaxBacktest()

    expect(HygieneLaneChurnTaxBacktestCases.length).toBeGreaterThanOrEqual(3)
    expect(results.every(result => result.passed)).toBe(true)
    expect(results.map(result => result.caseRef)).toEqual(
      expect.arrayContaining([
        'case.public.hygiene_lane.churn_tax.large_generation_dedup_pays',
        'case.public.hygiene_lane.churn_tax.small_targeted_simplification_pays',
        'case.public.hygiene_lane.churn_tax.large_churn_no_debt_zeroed',
      ]),
    )
    expect(results).toContainEqual(
      expect.objectContaining({
        caseRef: 'case.public.hygiene_lane.churn_tax.large_generation_dedup_pays',
        expectedPayoutMultiplierBps: 10_000,
        expectedPayoutSats: 100,
        formulaRef: HygieneLaneChurnTaxBacktestRef,
        projectedDenialReason: null,
        projectedPayoutMultiplierBps: 10_000,
        projectedPayoutSats: 100,
      }),
    )
  })

  test('is deterministic', () => {
    const signals: HygieneSizeDepthSignals = {
      ...baseSignals,
      changedWeightedLines: 320,
      debtReducedWeightedUnits: 6,
      filesTouched: 3,
    }
    expect(computeHygieneLaneSettlementSats(signals)).toEqual(
      computeHygieneLaneSettlementSats(signals),
    )
  })

  test('large churn with no measured debt reduction is zeroed (denied)', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      changedWeightedLines: 50_000,
      debtReducedWeightedUnits: 0,
      filesTouched: 80,
    })

    expect(result.payoutSats).toBe(0)
    expect(result.payoutMultiplierBps).toBe(0)
    expect(result.denialReason).toBe('no_measured_debt_reduction')
  })

  test('behavior receipt not green is denied (no payout for unverified behavior)', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      behaviorReceiptGreen: false,
      changedWeightedLines: 500,
      debtReducedWeightedUnits: 10,
      filesTouched: 4,
    })

    expect(result.payoutSats).toBe(0)
    expect(result.payoutMultiplierBps).toBe(0)
    expect(result.denialReason).toBe('behavior_receipt_not_green')
  })

  test('duplicate replay is denied regardless of size', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      changedWeightedLines: 1_000,
      debtReducedWeightedUnits: 20,
      duplicateReplay: true,
      filesTouched: 6,
    })

    expect(result.payoutSats).toBe(0)
    expect(result.payoutMultiplierBps).toBe(0)
    expect(result.denialReason).toBe('duplicate_replay')
  })

  test('new debt fully offsetting measured reduction pays only the floor', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      changedWeightedLines: 2_000,
      debtReducedWeightedUnits: 10,
      filesTouched: 8,
      newDebtWeightedUnits: 10,
    })

    expect(result.denialReason).toBeNull()
    expect(result.payoutMultiplierBps).toBe(100)
    expect(result.payoutSats).toBe(HygieneLaneMinPayoutSats)
  })

  test('never exceeds the 100-sat cap for any input', () => {
    const result = computeHygieneLaneSettlementSats({
      ...baseSignals,
      changedWeightedLines: 1_000_000,
      debtReducedWeightedUnits: 1_000,
      filesTouched: 1_000,
    })
    expect(result.payoutSats).toBeLessThanOrEqual(HygieneLaneMaxPayoutSats)
    expect(result.payoutMultiplierBps).toBeLessThanOrEqual(10_000)
  })
})

describe('decideHygieneLaneSettlement (gate + debt-receipt bridge)', () => {
  const goodSignals: HygieneSizeDepthSignals = {
    ...baseSignals,
    changedWeightedLines: 1_200,
    debtReducedWeightedUnits: 8,
    filesTouched: 4,
  }

  test('disabled gate fails closed to simulation', () => {
    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: payableProjection,
      gate: disabledTassadarRealSettlementGate,
      requestedAdapterKind: 'spark_treasury',
      signals: goodSignals,
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('gate_decision_blocked')
    expect(decision.gateDecision?.blockedReason).toBe('gate_disabled')
  })

  test('armed gate + payable receipt + payable amount authorizes the real branch', () => {
    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: payableProjection,
      gate: armedHygieneGate,
      requestedAdapterKind: 'spark_treasury',
      signals: goodSignals,
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.realAuthorized).toBe(true)
    expect(decision.blockedReason).toBeNull()
    expect(decision.gateDecision?.adapterKind).toBe('spark_treasury')
    expect(decision.amount.payoutSats).toBeGreaterThan(0)
    expect(decision.amount.payoutSats).toBeLessThanOrEqual(
      HygieneLaneMaxPayoutSats,
    )
    expect(decision.debtReceiptKey).toBe(payableProjection.debtReceiptKey)
  })

  test('a non-hygiene run-ref fails closed before the gate', () => {
    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: payableProjection,
      gate: armedHygieneGate,
      requestedAdapterKind: 'spark_treasury',
      signals: goodSignals,
      trainingRunRef: 'run.cs336.a1.demo',
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('not_hygiene_lane_run_ref')
    expect(decision.gateDecision).toBeNull()
  })

  test('a non-payable (blocked) debt receipt fails closed before the gate', () => {
    const blockedProjection = projectDebtReceiptSettlement({
      budgetCapSats: 100,
      scopeRefs: ['scope.public.debt_receipt.5334.tassadar_fixture_pairs'],
      sourceRefs: ['issue.public.github.openagentsinc_openagents.5334'],
    })
    expect(blockedProjection.state).not.toBe('payable')

    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: blockedProjection,
      gate: armedHygieneGate,
      requestedAdapterKind: 'spark_treasury',
      signals: goodSignals,
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('debt_receipt_not_payable')
    expect(decision.gateDecision).toBeNull()
  })

  test('a duplicate-replay debt receipt is never payable', () => {
    const retiredKey = deriveDebtReceiptKey(
      payableDebtReceiptInput.debtReceiptKeyInput!,
    )
    expect(payableProjection.debtReceiptKey).toBe(retiredKey)

    const duplicateProjection = projectDebtReceiptSettlement({
      ...payableDebtReceiptInput,
      retiredDebtReceiptKeys: [retiredKey],
    })
    expect(duplicateProjection.duplicateReplay).toBe(true)

    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: duplicateProjection,
      gate: armedHygieneGate,
      requestedAdapterKind: 'spark_treasury',
      signals: goodSignals,
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('duplicate_replay')
  })

  test('a denied amount (no measured debt reduction) fails closed', () => {
    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: payableProjection,
      gate: armedHygieneGate,
      requestedAdapterKind: 'spark_treasury',
      signals: { ...baseSignals, changedWeightedLines: 9_000 },
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('amount_denied')
    expect(decision.amount.denialReason).toBe('no_measured_debt_reduction')
  })

  test('an amount over the gate cap fails closed (gate, not the formula, binds)', () => {
    const tightGate: TassadarRealSettlementGate = {
      ...armedHygieneGate,
      maxPayoutSats: 5,
    }
    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: payableProjection,
      gate: tightGate,
      requestedAdapterKind: 'spark_treasury',
      signals: {
        ...baseSignals,
        changedWeightedLines: 4_000,
        debtReducedWeightedUnits: 40,
        filesTouched: 12,
      },
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.amount.payoutSats).toBeGreaterThan(5)
    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('gate_decision_blocked')
    expect(decision.gateDecision?.blockedReason).toBe('amount_over_gate_cap')
  })

  test('public projection refs are secret-free and include formula + override refs', () => {
    const conflictOverrideRef = 'override.public.hygiene_lane.conflict_reviewed'
    const decision = decideHygieneLaneSettlement({
      contributorRef: CONTRIBUTOR_REF,
      debtReceiptProjection: payableProjection,
      gate: armedHygieneGate,
      requestedAdapterKind: 'spark_treasury',
      signals: {
        ...goodSignals,
        conflictOverrideRefs: [conflictOverrideRef],
      },
      trainingRunRef: HYGIENE_RUN_REF,
    })

    expect(decision.publicProjectionRefs).toContain(HygieneLaneChurnTaxBacktestRef)
    expect(decision.publicProjectionRefs).toContain(conflictOverrideRef)
    for (const ref of decision.publicProjectionRefs) {
      expect(ref).not.toMatch(/spark1|lnbc|preimage|mnemonic|sk-|bearer/i)
      expect(ref).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/)
    }
  })

  test('unsafe conflict override refs fail the public projection guard', () => {
    expect(() =>
      decideHygieneLaneSettlement({
        contributorRef: CONTRIBUTOR_REF,
        debtReceiptProjection: payableProjection,
        gate: armedHygieneGate,
        requestedAdapterKind: 'spark_treasury',
        signals: {
          ...goodSignals,
          conflictOverrideRefs: [
            'override.public.hygiene_lane.2026-06-18T18:00:00',
          ],
        },
        trainingRunRef: HYGIENE_RUN_REF,
      }),
    ).toThrow(HygieneLaneSettlementUnsafe)
  })
})
