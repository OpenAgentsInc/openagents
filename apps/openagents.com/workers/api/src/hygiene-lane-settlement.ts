import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { type DebtReceiptSettlementProjection } from './debt-receipt-policy'
import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusPayoutTargetApprovalRecord,
  NexusTreasuryPayoutAdapterKind,
  type NexusTreasuryPayoutAmount,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import { realSettlementMovementMode } from './tassadar-run-settlement'
import {
  type TassadarRealSettlementGate,
  type TassadarSettlementAdapterDecision,
  resolveTassadarSettlementAdapter,
} from './tassadar-run-settlement-gate'

/**
 * Hygiene-lane Bitcoin settlement (openagents #5372, EPIC #5335).
 *
 * Pays merged, benchmark-verified hygiene PRs (the debt-receipt model, #5340) in
 * real Bitcoin to the contributor's registered Spark payout target, scaled by
 * size/depth and capped at 100 sats (owner-authorized + funded 2026-06-18).
 *
 * This module deliberately REUSES the existing gated, idempotent, receipt-first
 * Tassadar settlement mechanism. It does NOT build a parallel money rail:
 *   - The owner gate (`OPENAGENTS_REAL_SETTLEMENT_GATE`) is already generic over
 *     run refs. Arming the hygiene lane = adding a hygiene run-ref (e.g.
 *     `run.hygiene.lane.20260618`) to the gate's `allowedRunRefs` with its own
 *     `maxPayoutSats` cap. No gate-code change is needed and no amount is
 *     hardcoded in the gate.
 *   - One settlement per `DebtReceiptKey`: a payable hygiene receipt becomes
 *     settleable exactly once. A debt receipt in any non-payable state, or a
 *     duplicate replay, is fail-closed here.
 *   - The amount helper below is `churn_tax.v0.backtest` (#5369): a
 *     deterministic, replayable formula fixture set that pays verified debt
 *     reduction and zeroes duplicate replay / behavior-red / no-debt churn.
 *
 * Pure decision surface. It does not move money, dispatch, read wallets, or
 * write receipts. The live settle endpoint
 * (`POST /api/training/runs/{run}/settlement-receipt`) remains the single rail.
 */

// Per-PR caps for the hygiene lane (#5372, owner-authorized 2026-06-18). The
// average is ~100 sats; tiny hygiene → a few sats; multi-file / deep analysis →
// up to 100. These are the formula ceiling, not the gate's authority: the gate's
// own `maxPayoutSats` is the binding cap at settle time, and these never exceed
// it. Not final numbers.
export const HygieneLaneMaxPayoutSats = 100
export const HygieneLaneMinPayoutSats = 1

// `churn_tax.v0.backtest` (#5369): deterministic, reviewable, and replayed by
// `HygieneLaneChurnTaxBacktestCases` below. This replaces the earlier interim
// size/depth copy while keeping the same owner-authorized 100-sat cap.
export const HygieneLaneChurnTaxBacktestRef =
  'formula.public.hygiene_lane.churn_tax.v0.backtest.5369' as const
export const HygieneLaneSettlementFormulaRef =
  HygieneLaneChurnTaxBacktestRef

// The single real adapter the hygiene lane may use is the proven Spark treasury
// rail — identical to the Tassadar run-settlement gate's allowed adapter.
export const HygieneLaneAllowedAdapterKind = 'spark_treasury' as const

// The hygiene-lane settlement run-ref shape. Arming the lane adds a concrete ref
// of this shape (e.g. `run.hygiene.lane.20260618`) to the gate's allowedRunRefs.
// The date suffix lets the operator rotate the lane scope per UTC day window
// without widening any other run's authority.
const HygieneLaneRunRefPattern = /^run\.hygiene\.lane\.\d{8}$/

export const HygieneLaneRunRef = S.String.check(
  S.isPattern(HygieneLaneRunRefPattern),
).pipe(S.brand('HygieneLaneRunRef'))
export type HygieneLaneRunRef = typeof HygieneLaneRunRef.Type

export const isHygieneLaneRunRef = (
  value: string,
): value is HygieneLaneRunRef => HygieneLaneRunRefPattern.test(value)

// A public-safe request ref: a bounded, non-empty token that carries no
// provider/payment/wallet/secret/raw-timestamp material. Used for every ref
// field on the hygiene settlement request so nothing unsafe enters the rail.
const HygieneRequestRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,260}$/

export const HygieneLaneSettlementRequestRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMaxLength(261),
  S.isPattern(HygieneRequestRefPattern),
)

/**
 * Size/depth signals for one merged hygiene PR. These are the inputs the
 * `churn_tax.v0.backtest` (#5369) formula scores.
 *
 * All counts are non-negative integers (public-safe magnitudes, never raw diffs
 * or file contents). `behaviorReceiptGreen` is the benchmark-as-receipt gate:
 * behavior must be held constant-or-better. `duplicateReplay` is the
 * DebtReceiptKey one-settlement gate surfaced as a denial input.
 */
export const HygieneSizeDepthSignals = S.Struct({
  // Benchmark-as-receipt: tests-green / regenerate-and-diff / perf-constant.
  behaviorReceiptGreen: S.Boolean,
  // Churn-weighted changed lines (added + removed, weighted). Drives size.
  changedWeightedLines: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  // Reviewer/settlement-authority public refs that explain intentional
  // conflict/debt overrides. Empty by default; refs are projected but never
  // treated as worker authority.
  conflictOverrideRefs: S.optionalKey(S.Array(S.String)),
  // Measured debt reduction (weighted units). The signal that earns sats; a
  // diff that reduces no measured debt is behavior-neutral churn and pays the
  // floor (or zero when not even behavior-green).
  debtReducedWeightedUnits: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
  ),
  // Duplicate-replay of an already-retired DebtReceiptKey is never payable.
  duplicateReplay: S.Boolean,
  // Distinct files touched. Drives the depth/breadth bonus.
  filesTouched: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  // New debt the change introduces (weighted units). Penalizes regressions.
  newDebtWeightedUnits: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
})
export type HygieneSizeDepthSignals = typeof HygieneSizeDepthSignals.Type

export class HygieneLaneSettlementUnsafe extends S.TaggedErrorClass<HygieneLaneSettlementUnsafe>()(
  'HygieneLaneSettlementUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeSignals = S.decodeUnknownSync(HygieneSizeDepthSignals)

export type HygieneLaneAmountDenialReason =
  | 'behavior_receipt_not_green'
  | 'duplicate_replay'
  | 'no_measured_debt_reduction'

export type HygieneLaneAmount = Readonly<{
  // Why a payable amount was denied (amount 0), or null when payable.
  denialReason: HygieneLaneAmountDenialReason | null
  // The churn-tax formula ref used to produce this amount.
  formulaRef: typeof HygieneLaneSettlementFormulaRef
  // The deterministic payout multiplier in basis points of
  // `HygieneLaneMaxPayoutSats` (0..10000). This is the reviewable multiplier
  // #5369 requires; `payoutSats` is the rounded capped amount.
  payoutMultiplierBps: number
  // The settleable amount in sats, in [0, HygieneLaneMaxPayoutSats]. 0 means
  // "do not settle" (a denial), never "settle zero".
  payoutSats: number
}>

const clampToHygieneCap = (value: number): number =>
  Math.max(
    HygieneLaneMinPayoutSats,
    Math.min(HygieneLaneMaxPayoutSats, Math.round(value)),
  )

const multiplierBpsForPayout = (payoutSats: number): number =>
  Math.round((payoutSats / HygieneLaneMaxPayoutSats) * 10_000)

const payableAmount = (payoutSats: number): HygieneLaneAmount => ({
  denialReason: null,
  formulaRef: HygieneLaneSettlementFormulaRef,
  payoutMultiplierBps: multiplierBpsForPayout(payoutSats),
  payoutSats,
})

/**
 * The churn-tax payout formula (#5369/#5372). Deterministic and pure.
 *
 * Denials (return 0 sats, with a typed reason):
 *   - a duplicate replay of an already-settled DebtReceiptKey,
 *   - a change whose behavior receipt is not green (behavior not held), or
 *   - a change that reduced no measured debt (behavior-neutral / format churn).
 *
 * Otherwise the amount scales with measured debt reduced, with a bounded size
 * and breadth bonus, minus a new-debt penalty, then clamped into [1, 100]. The
 * shape — debt-reduction-led, churn alone pays nothing — matches the #5369
 * acceptance: good large deletion pays; good small targeted simplification pays;
 * large churn with no measured debt reduction zeroes.
 */
export const computeHygieneLaneSettlementSats = (
  rawSignals: HygieneSizeDepthSignals,
): HygieneLaneAmount => {
  const signals = decodeSignals(rawSignals)
  const deny = (
    denialReason: HygieneLaneAmountDenialReason,
  ): HygieneLaneAmount => ({
    denialReason,
    formulaRef: HygieneLaneSettlementFormulaRef,
    payoutMultiplierBps: 0,
    payoutSats: 0,
  })

  if (signals.duplicateReplay) {
    return deny('duplicate_replay')
  }

  if (!signals.behaviorReceiptGreen) {
    return deny('behavior_receipt_not_green')
  }

  if (signals.debtReducedWeightedUnits === 0) {
    return deny('no_measured_debt_reduction')
  }

  // Debt-reduction-led base: each weighted unit of measured debt removed is
  // worth a few sats. A new-debt penalty subtracts first so a regression-heavy
  // change cannot buy size.
  const netDebtReduced = Math.max(
    0,
    signals.debtReducedWeightedUnits - signals.newDebtWeightedUnits,
  )

  if (netDebtReduced === 0) {
    // All measured reduction was offset by new debt: pay only the floor.
    return payableAmount(HygieneLaneMinPayoutSats)
  }

  const debtComponent = netDebtReduced * 6
  // Bounded size bonus: large, real moves earn more, but size alone is capped
  // so pure churn can never dominate the score.
  const sizeBonus = Math.min(30, Math.floor(signals.changedWeightedLines / 40))
  // Bounded breadth/depth bonus: touching more files (a cross-cutting refactor)
  // earns a little more, also capped.
  const breadthBonus = Math.min(20, signals.filesTouched * 3)

  return payableAmount(
    clampToHygieneCap(debtComponent + sizeBonus + breadthBonus),
  )
}

export type HygieneLaneChurnTaxBacktestCase = Readonly<{
  caseRef: string
  expectedDenialReason: HygieneLaneAmountDenialReason | null
  expectedPayoutMultiplierBps: number
  expectedPayoutSats: number
  signals: HygieneSizeDepthSignals
}>

export const HygieneLaneChurnTaxBacktestCases: ReadonlyArray<HygieneLaneChurnTaxBacktestCase> =
  [
    {
      caseRef: 'case.public.hygiene_lane.churn_tax.large_generation_dedup_pays',
      expectedDenialReason: null,
      expectedPayoutMultiplierBps: 10_000,
      expectedPayoutSats: 100,
      signals: {
        behaviorReceiptGreen: true,
        changedWeightedLines: 4_000,
        debtReducedWeightedUnits: 40,
        duplicateReplay: false,
        filesTouched: 12,
        newDebtWeightedUnits: 0,
      },
    },
    {
      caseRef:
        'case.public.hygiene_lane.churn_tax.small_targeted_simplification_pays',
      expectedDenialReason: null,
      expectedPayoutMultiplierBps: 900,
      expectedPayoutSats: 9,
      signals: {
        behaviorReceiptGreen: true,
        changedWeightedLines: 12,
        debtReducedWeightedUnits: 1,
        duplicateReplay: false,
        filesTouched: 1,
        newDebtWeightedUnits: 0,
      },
    },
    {
      caseRef: 'case.public.hygiene_lane.churn_tax.large_churn_no_debt_zeroed',
      expectedDenialReason: 'no_measured_debt_reduction',
      expectedPayoutMultiplierBps: 0,
      expectedPayoutSats: 0,
      signals: {
        behaviorReceiptGreen: true,
        changedWeightedLines: 50_000,
        debtReducedWeightedUnits: 0,
        duplicateReplay: false,
        filesTouched: 80,
        newDebtWeightedUnits: 0,
      },
    },
    {
      caseRef: 'case.public.hygiene_lane.churn_tax.behavior_red_zeroed',
      expectedDenialReason: 'behavior_receipt_not_green',
      expectedPayoutMultiplierBps: 0,
      expectedPayoutSats: 0,
      signals: {
        behaviorReceiptGreen: false,
        changedWeightedLines: 500,
        debtReducedWeightedUnits: 10,
        duplicateReplay: false,
        filesTouched: 4,
        newDebtWeightedUnits: 0,
      },
    },
    {
      caseRef: 'case.public.hygiene_lane.churn_tax.duplicate_replay_zeroed',
      expectedDenialReason: 'duplicate_replay',
      expectedPayoutMultiplierBps: 0,
      expectedPayoutSats: 0,
      signals: {
        behaviorReceiptGreen: true,
        changedWeightedLines: 1_000,
        debtReducedWeightedUnits: 20,
        duplicateReplay: true,
        filesTouched: 6,
        newDebtWeightedUnits: 0,
      },
    },
  ]

export type HygieneLaneChurnTaxBacktestResult = Readonly<{
  caseRef: string
  expectedDenialReason: HygieneLaneAmountDenialReason | null
  expectedPayoutMultiplierBps: number
  expectedPayoutSats: number
  formulaRef: typeof HygieneLaneSettlementFormulaRef
  passed: boolean
  projectedDenialReason: HygieneLaneAmountDenialReason | null
  projectedPayoutMultiplierBps: number
  projectedPayoutSats: number
}>

export const replayHygieneLaneChurnTaxBacktest = (
  cases: ReadonlyArray<HygieneLaneChurnTaxBacktestCase> =
    HygieneLaneChurnTaxBacktestCases,
): ReadonlyArray<HygieneLaneChurnTaxBacktestResult> =>
  cases.map(testCase => {
    const projected = computeHygieneLaneSettlementSats(testCase.signals)

    return {
      caseRef: testCase.caseRef,
      expectedDenialReason: testCase.expectedDenialReason,
      expectedPayoutMultiplierBps: testCase.expectedPayoutMultiplierBps,
      expectedPayoutSats: testCase.expectedPayoutSats,
      formulaRef: projected.formulaRef,
      passed:
        projected.denialReason === testCase.expectedDenialReason &&
        projected.payoutMultiplierBps ===
          testCase.expectedPayoutMultiplierBps &&
        projected.payoutSats === testCase.expectedPayoutSats,
      projectedDenialReason: projected.denialReason,
      projectedPayoutMultiplierBps: projected.payoutMultiplierBps,
      projectedPayoutSats: projected.payoutSats,
    }
  })

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,260}$/
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const refIsPublicSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !containsProviderSecretMaterial(ref) &&
  !rawTimestampPattern.test(ref)

export type HygieneLaneSettlementDecisionInput = Readonly<{
  // The contributor (Pylon ref) whose registered Spark target is paid.
  contributorRef: string
  // The benchmark-verified debt-receipt projection for this PR (#5340). Only a
  // `payable` debt receipt is settleable; everything else is fail-closed.
  debtReceiptProjection: DebtReceiptSettlementProjection
  // The owner gate, read from env at the boundary.
  gate: TassadarRealSettlementGate
  // The contributor's resolved adapter request (always the Spark treasury rail
  // for a real hygiene settlement; defaults to simulation).
  requestedAdapterKind?: typeof NexusTreasuryPayoutAdapterKind.Type | undefined
  // The size/depth signals for this merged PR.
  signals: HygieneSizeDepthSignals
  // The hygiene-lane run-ref the gate must allowlist (e.g. run.hygiene.lane.…).
  trainingRunRef: string
}>

export type HygieneLaneSettlementBlockedReason =
  | 'amount_denied'
  | 'debt_receipt_not_payable'
  | 'duplicate_replay'
  | 'gate_decision_blocked'
  | 'not_hygiene_lane_run_ref'

export type HygieneLaneSettlementDecision = Readonly<{
  // The amount the churn-tax formula produced (sats + denial reason).
  amount: HygieneLaneAmount
  // Why settlement is not authorized, or null when authorized.
  blockedReason: HygieneLaneSettlementBlockedReason | null
  // The DebtReceiptKey ref this settlement retires (public-safe string), or null
  // when the projection carries none.
  debtReceiptKey: string | null
  // The gate decision (real-vs-simulation, typed blockedReason), or null when we
  // fail closed before the gate (e.g. not payable, not a hygiene run-ref).
  gateDecision: TassadarSettlementAdapterDecision | null
  // Public-safe refs only. Asserted secret-free by construction.
  publicProjectionRefs: ReadonlyArray<string>
  // Whether a REAL Bitcoin settlement is authorized (gate real-authorized AND
  // the debt receipt is payable AND the amount is payable). False keeps the lane
  // on the simulation chain, honestly.
  realAuthorized: boolean
  // The single rail that would settle this — for operator docs only.
  settlementRail: 'POST /api/training/runs/{run}/settlement-receipt'
}>

/**
 * Decide whether one merged hygiene PR may settle, and at what amount, through
 * the SAME owner-gated rail. Pure and fail-closed:
 *   - the run-ref must be a hygiene-lane run-ref,
 *   - the debt-receipt projection must be `payable` (not blocked/duplicate/etc.),
 *   - the churn-tax amount must be payable (> 0; not a denial),
 *   - the owner gate must authorize the real branch for this run + contributor +
 *     amount (the gate's own per-payout and daily caps still bind).
 * Any failing condition yields `realAuthorized: false` with a typed reason; the
 * caller then either records the honest simulation chain or skips.
 */
export const decideHygieneLaneSettlement = (
  input: HygieneLaneSettlementDecisionInput,
): HygieneLaneSettlementDecision => {
  const settlementRail =
    'POST /api/training/runs/{run}/settlement-receipt' as const
  const amount = computeHygieneLaneSettlementSats(input.signals)
  const debtReceiptKey = input.debtReceiptProjection.debtReceiptKey ?? null
  const conflictOverrideRefs = input.signals.conflictOverrideRefs ?? []

  const publicProjectionRefs = [
    amount.formulaRef,
    ...(debtReceiptKey === null ? [] : [debtReceiptKey]),
    ...conflictOverrideRefs,
    ...(amount.denialReason === null
      ? []
      : [`denial.public.hygiene_lane.${amount.denialReason}`]),
  ]
  const unsafe = publicProjectionRefs.find(ref => !refIsPublicSafe(ref))

  if (unsafe !== undefined) {
    throw new HygieneLaneSettlementUnsafe({
      reason:
        'Hygiene-lane settlement projection refs must be public-safe (no provider, payment, wallet, secret, or raw-timestamp material).',
    })
  }

  const blocked = (
    blockedReason: HygieneLaneSettlementBlockedReason,
    gateDecision: TassadarSettlementAdapterDecision | null,
  ): HygieneLaneSettlementDecision => ({
    amount,
    blockedReason,
    debtReceiptKey,
    gateDecision,
    publicProjectionRefs,
    realAuthorized: false,
    settlementRail,
  })

  if (!isHygieneLaneRunRef(input.trainingRunRef)) {
    return blocked('not_hygiene_lane_run_ref', null)
  }

  // Duplicate replay of a retired DebtReceiptKey is never payable (one
  // settlement per receipt, #5340). The projection already encodes this.
  if (input.debtReceiptProjection.duplicateReplay) {
    return blocked('duplicate_replay', null)
  }

  // Only a `payable` debt receipt is settleable: it has the verified
  // benchmark/hygiene delta, settlement approval, and a positive payable amount,
  // and is not retired/quarantined. Everything else fails closed.
  if (input.debtReceiptProjection.state !== 'payable') {
    return blocked('debt_receipt_not_payable', null)
  }

  if (amount.payoutSats <= 0 || amount.denialReason !== null) {
    return blocked('amount_denied', null)
  }

  // Now route through the SAME owner gate the Tassadar run settlement uses. The
  // gate is the single authority for the real branch; arming = adding this
  // hygiene run-ref to allowedRunRefs with a `maxPayoutSats` cap.
  const gateDecision = resolveTassadarSettlementAdapter({
    amountSats: amount.payoutSats,
    contributorRef: input.contributorRef,
    gate: input.gate,
    requestedAdapterKind: input.requestedAdapterKind ?? 'simulation',
    trainingRunRef: input.trainingRunRef,
  })

  if (!gateDecision.realAuthorized) {
    return blocked('gate_decision_blocked', gateDecision)
  }

  return {
    amount,
    blockedReason: null,
    debtReceiptKey,
    gateDecision,
    publicProjectionRefs,
    realAuthorized: true,
    settlementRail,
  }
}

/**
 * The honest verification basis for a hygiene-lane settlement (#5372).
 *
 * Hygiene PRs are verified by tests + reviewer acceptance + the merged debt
 * receipt — NOT by exact trace replay. This module therefore NEVER emits an
 * `exact_trace_replay` verdict for hygiene work. The settlement receipt instead
 * states this `hygiene_merged_reviewed` basis, so the public projection is
 * truthful about WHY the money moved. The money rail is identical to the proven
 * Tassadar Spark treasury payout; only the verification class differs.
 */
export const HygieneLaneVerificationClass = 'hygiene_merged_reviewed' as const

export const HygieneLaneSettlementPolicySnapshotRef =
  'policy.public.hygiene_lane.merged_reviewed_small_sats.v1' as const

export const HygieneLaneSettlementPayoutTargetApprovalPolicyRef =
  'policy.public.hygiene_lane.settlement_payout' as const

/**
 * Admin request to settle ONE merged, benchmark-verified hygiene debt receipt
 * (#5372, EPIC #5335). Every field is a public-safe ref or a bounded integer.
 * No raw diffs, PR bodies, wallet/payment material, or addresses ever enter the
 * request — the contributor's private Spark destination is resolved server-side
 * from `contributorRef` via `resolveSparkPayoutDestination` (fail-closed).
 */
export const HygieneLaneSettlementRequest = S.Struct({
  // The single allowed real adapter; defaults to simulation (honest fail-closed).
  adapterKind: S.optionalKey(NexusTreasuryPayoutAdapterKind),
  // The contributor (Pylon ref) whose registered Spark target is paid.
  contributorRef: HygieneLaneSettlementRequestRef,
  // The merged-PR / DebtReceiptKey ref this settlement retires (#5340).
  debtReceiptKeyRef: HygieneLaneSettlementRequestRef,
  // The merged-PR ref (e.g. github PR ref). Public-safe ref only.
  mergedPrRef: HygieneLaneSettlementRequestRef,
  // Idempotency ref: a retry with the SAME ref pays at most once.
  idempotencyRef: HygieneLaneSettlementRequestRef,
  // Operator approval ref (the human who authorized this settle).
  operatorApprovalRef: HygieneLaneSettlementRequestRef,
  // Payout-target approval ref + payout-target ref (owner-scoped). The raw
  // destination is NEVER in the request; only these refs are.
  payoutTargetApprovalRef: HygieneLaneSettlementRequestRef,
  payoutTargetRef: HygieneLaneSettlementRequestRef,
  // Reviewer acceptance ref (the human/maintainer who accepted the PR).
  reviewerAcceptanceRef: HygieneLaneSettlementRequestRef,
  // The size/depth signals for this merged PR (drive the interim amount).
  signals: HygieneSizeDepthSignals,
  // The hygiene-lane run-ref the gate must allowlist (run.hygiene.lane.YYYYMMDD).
  trainingRunRef: HygieneLaneSettlementRequestRef,
})
export type HygieneLaneSettlementRequest =
  typeof HygieneLaneSettlementRequest.Type

export type HygieneLaneSettlementRecords = Readonly<{
  amountSats: number
  attempt: NexusTreasuryPayoutAttemptRecord
  contributorRef: string
  intent: NexusTreasuryPayoutIntentRecord
  reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord
  settlementReceipt: NexusPaymentAuthorityReceiptRecord
  settlementReceiptRef: string
  targetApproval: NexusPayoutTargetApprovalRecord
}>

const stableSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const bitcoinAmount = (sats: number): NexusTreasuryPayoutAmount => ({
  amountMinorUnits: sats * 1000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
})

/**
 * Build the settlement ledger chain for ONE merged, benchmark-verified hygiene
 * debt receipt (#5372). Structurally identical to the Tassadar run-settlement
 * builder — same intent -> attempt -> reconciliation -> settlement_recorded
 * chain, same deterministic idempotency hashes, same redacted destination ref —
 * so it drives the SAME `dispatchRealRunSettlementCore` Spark rail. The ONLY
 * differences are honest:
 *   - the verification basis is `hygiene_merged_reviewed` (merged PR + reviewer
 *     acceptance + debt receipt), NEVER `exact_trace_replay`; and
 *   - the receipt projection cites the merged-PR/debt-receipt refs as its basis
 *     instead of a fabricated verification-challenge ref.
 *
 * `amountSats` is the gate-bound, formula-computed amount (≤ 100). The caller
 * (`decideHygieneLaneSettlement`) has already confirmed the debt receipt is
 * payable, the amount is payable, and the gate authorizes this run/contributor/
 * amount before this builder is reached on the real branch.
 */
export const buildHygieneLaneSettlement = (
  input: Readonly<{
    adapterKind: typeof NexusTreasuryPayoutAdapterKind.Type
    amountSats: number
    contributorRef: string
    debtReceiptKeyRef: string
    idempotencyDigestHex: string
    mergedPrRef: string
    nowIso: string
    operatorApprovalRef: string
    payoutTargetApprovalRef: string
    payoutTargetRef: string
    reviewerAcceptanceRef: string
    trainingRunRef: string
  }>,
): HygieneLaneSettlementRecords => {
  const adapterKind = input.adapterKind
  const moneyMovement = realSettlementMovementMode(adapterKind)
  const suffix = stableSuffix(`sha256_${input.idempotencyDigestHex}`)
  const contributorRef = input.contributorRef.trim()
  const amount = bitcoinAmount(input.amountSats)
  const idempotencyHash = (stage: string): string =>
    `hash.hygiene_lane_settlement.${stage}.${input.idempotencyDigestHex}`
  // The accepted-work basis is the HONEST hygiene evidence: merged PR + reviewer
  // acceptance + the debt receipt. No verification-challenge ref appears.
  const acceptedWorkRefs = uniqueRefs([
    input.debtReceiptKeyRef,
    input.mergedPrRef,
    input.reviewerAcceptanceRef,
  ])
  const metadataRefs = uniqueRefs([
    input.trainingRunRef,
    input.debtReceiptKeyRef,
    input.mergedPrRef,
    input.reviewerAcceptanceRef,
    input.operatorApprovalRef,
    `verification.public.hygiene_lane.${HygieneLaneVerificationClass}`,
    'metadata.hygiene_lane.settlement.merged_reviewed',
  ])
  const redactedDestinationRef = `destination.redacted.hygiene_lane_settlement.${suffix}`

  const targetApproval: NexusPayoutTargetApprovalRecord = {
    agentRef: 'agent.artanis',
    approvalPolicyRef: HygieneLaneSettlementPayoutTargetApprovalPolicyRef,
    approvalRef: input.payoutTargetApprovalRef,
    approvedByRef: 'operator.openagents.hygiene_lane_settlement',
    archivedAt: null,
    createdAt: input.nowIso,
    expiresAt: null,
    id: `nexus_payout_target_approval_hygiene_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('approval'),
    ownerUserId: 'user_openagents_operator',
    payoutTargetRef: input.payoutTargetRef,
    publicProjectionJson: JSON.stringify({
      debtReceiptKeyRef: input.debtReceiptKeyRef,
      pylonRef: contributorRef,
      state: 'active',
      trainingRunRef: input.trainingRunRef,
    }),
    pylonRef: contributorRef,
    redactedDestinationRef,
    scopeRefs: uniqueRefs([
      input.trainingRunRef,
      input.debtReceiptKeyRef,
      input.mergedPrRef,
    ]),
    status: 'active',
    updatedAt: input.nowIso,
  }

  const intent: NexusTreasuryPayoutIntentRecord = {
    acceptedWorkRefs,
    actorRef: 'agent.artanis',
    adapterKind,
    amount,
    archivedAt: null,
    artanisDispatchRef: `artanis_dispatch.hygiene_lane_settlement.${suffix}`,
    assignmentRef: null,
    buyerPaymentRef: null,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_intent_hygiene_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('intent'),
    metadataRefs,
    ownerUserId: null,
    payoutIntentRef: `payout_intent.hygiene_lane_settlement.${suffix}`,
    payoutTargetApprovalRef: input.payoutTargetApprovalRef,
    payoutTargetRef: input.payoutTargetRef,
    policySnapshotRef: HygieneLaneSettlementPolicySnapshotRef,
    publicProjectionJson: JSON.stringify({
      acceptedWork: true,
      adapter: adapterKind,
      amountSats: input.amountSats,
      moneyMovement,
      operatorApproved: true,
      state: 'approved',
      trainingRunRef: input.trainingRunRef,
      verificationBasis: HygieneLaneVerificationClass,
    }),
    pylonJobRef: null,
    sourceKind: 'accepted_work',
    spendCap: amount,
    status: 'approved',
    updatedAt: input.nowIso,
  }

  const attempt: NexusTreasuryPayoutAttemptRecord = {
    adapterAttemptRef: `adapter_attempt.hygiene_lane_settlement.${adapterKind}.${suffix}`,
    adapterKind,
    amount,
    archivedAt: null,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_attempt_hygiene_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('attempt'),
    metadataRefs,
    payoutAttemptRef: `payout_attempt.hygiene_lane_settlement.${suffix}`,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: input.amountSats,
      moneyMovement,
      trainingRunRef: input.trainingRunRef,
      verificationBasis: HygieneLaneVerificationClass,
    }),
    redactedDestinationRef,
    redactedPaymentRef: null,
    status: 'confirmed',
    updatedAt: input.nowIso,
  }

  const reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord = {
    adapterKind,
    archivedAt: null,
    createdAt: input.nowIso,
    eventRef: `reconciliation.hygiene_lane_settlement.${suffix}`,
    externalEventRef: `external_event.hygiene_lane_settlement.${adapterKind}.${suffix}`,
    id: `nexus_treasury_reconciliation_hygiene_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('reconciliation'),
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    providerRef: `provider.${adapterKind}`,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: input.amountSats,
      moneyMovement,
      trainingRunRef: input.trainingRunRef,
      verificationBasis: HygieneLaneVerificationClass,
    }),
    resultRef: `result.hygiene_lane_settlement.${suffix}`,
    status: 'matched',
  }

  const settlementReceiptRef = `receipt.nexus.hygiene_lane_settlement.${suffix}`
  const settlementReceipt: NexusPaymentAuthorityReceiptRecord = {
    archivedAt: null,
    audience: 'public',
    createdAt: input.nowIso,
    eventRef: reconciliationEvent.eventRef,
    id: `nexus_payment_authority_receipt_hygiene_lane_${suffix}`,
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: input.amountSats,
      asset: 'bitcoin',
      contributorRef,
      // HONEST basis. The receipt names the merged-PR + reviewer acceptance +
      // debt receipt as the reason money moved — NOT trace replay. There is no
      // `verificationChallengeRef` and no `exact_trace_replay` verdict here.
      debtReceiptKeyRef: input.debtReceiptKeyRef,
      mergedPrRef: input.mergedPrRef,
      moneyMovement,
      reviewerAcceptanceRef: input.reviewerAcceptanceRef,
      state: 'settled',
      trainingRunRef: input.trainingRunRef,
      verificationBasis: HygieneLaneVerificationClass,
    }),
    receiptKind: 'settlement_recorded',
    receiptRef: settlementReceiptRef,
  }

  return {
    amountSats: input.amountSats,
    attempt,
    contributorRef,
    intent,
    reconciliationEvent,
    settlementReceipt,
    settlementReceiptRef,
    targetApproval,
  }
}
