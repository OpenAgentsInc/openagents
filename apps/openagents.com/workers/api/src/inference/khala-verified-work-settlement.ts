// Khala M3 — verified-work -> Bitcoin/Spark settlement (EPIC #6017, #6011).
//
// PAYMENTS DIRECTION (owner, 2026-06-22): Bitcoin-only, SPARK as the PRIMARY
// payout method (Lightning as the rail). No Stripe, no card funding. MDK is
// checkout-only and not used for payouts. This module settles sats to a real
// contributor (the guinea-pig Pylon first) over the SAME proven Spark treasury
// rail the tip/treasury/Tassadar tests already exercise.
//
// WHAT THIS BRIDGES. `serving-node-payout.ts` already computes the PURE, gated
// per-stage payout DECISION + the internal PayIn-shaped credit-ledger legs from a
// `ServingReceipt`, but it never dispatches a real Bitcoin send. This module is
// the missing leg: it turns one ARMED, parity-verified serving payout into a real
// Spark dispatch and a dereferenceable `realBitcoinMoved`-shaped settlement
// receipt, reusing the exact owner gate (`resolveTassadarSettlementAdapter`),
// daily-budget ceiling (`decideTassadarDailyBudget`), Spark treasury adapter, and
// `Nexus*` ledger record shapes the Tassadar auto-settlement path proved out — so
// there is NO parallel money path and NO new payout authority surface.
//
// SAFETY (real money — be conservative):
//   - FAIL-CLOSED gates, in order: PARITY (born-verified exact-greedy parity),
//     RL-3 ASSET BOUNDARY (only Bitcoin revenue funds a withdrawable Bitcoin
//     share), the OWNER-ARMED real-settlement gate (default OFF everywhere ->
//     every leg falls back to `gate_not_authorized`), the per-payout cap, the
//     cumulative daily-budget ceiling, and a registered payout destination.
//   - IDEMPOTENT: the settlement-receipt ref is derived from the serving-run +
//     node, so a replay pays AT MOST ONCE per run per node.
//   - FAIL-SOFT: never throws into the caller. A blocked/failed settlement
//     returns a structured outcome and is logged public-safe (refs + amount-sats
//     + neutral blocker only — never an address, invoice, preimage, or wallet
//     material). The metering hook fires this fire-and-forget.
//   - The first real dispatched payout stays OWNER-ARMED. With the gate OFF the
//     module is fully inert; arming real money beyond the existing bounded test
//     path is a NEEDS-OWNER step, never an agent workaround.

import { Effect } from 'effect'

import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusPayoutTargetApprovalRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from '../nexus-treasury-payout-ledger'
import { workerLogEntry } from '../observability'
import { currentIsoTimestamp } from '../runtime-primitives'
import { realSettlementMovementMode } from '../tassadar-run-settlement'
import {
  type TassadarRealSettlementGate,
  decideTassadarDailyBudget,
  resolveTassadarSettlementAdapter,
  tassadarRealSettledSatsForDay,
  tassadarRealSettlementUtcDayKey,
} from '../tassadar-run-settlement-gate'
import { type ServingReceipt } from './openagents-network-adapter'
import {
  type ServingNodePayoutDecision,
  type ServingPayoutShare,
} from './serving-node-payout'

// The single records bundle one Spark settlement leg writes to the ledger. The
// shape mirrors `TassadarRunSettlementRecords` so the existing idempotent
// dispatch + daily-budget reader treat a Khala settlement identically.
export type KhalaSettlementRecords = Readonly<{
  amountSats: number
  contributorRef: string
  intent: NexusTreasuryPayoutIntentRecord
  attempt: NexusTreasuryPayoutAttemptRecord
  reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord
  settlementReceipt: NexusPaymentAuthorityReceiptRecord
  settlementReceiptRef: string
  // The payout-target approval record. Carried so these records are directly
  // dispatchable through the SAME proven `dispatchRealRunSettlementCore` the
  // Tassadar autostream uses (it requires a `targetApproval`) — no parallel path.
  targetApproval: NexusPayoutTargetApprovalRecord
}>

// Public-safe ref helpers (neutral; never payment material).
const stableSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 180)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const bitcoinAmount = (sats: number) => ({
  amountMinorUnits: sats * 1000,
  asset: 'bitcoin' as const,
  denomination: 'bitcoin_millisatoshi' as const,
})

// The policy / approval-policy refs stamped on a Khala serving settlement.
export const KHALA_SETTLEMENT_POLICY_SNAPSHOT_REF =
  'policy.khala_verified_work_settlement.v1'
export const KHALA_SETTLEMENT_PAYOUT_TARGET_APPROVAL_POLICY_REF =
  'policy.khala_verified_work_settlement.payout_target_approval.v1'

// Build the full `Nexus*` settlement record chain for ONE serving payout share
// (one node) of a verified serving run. PURE. The `adapterKind` decides the
// public `moneyMovement` label: `spark_treasury` => `real_bitcoin` (the only
// label `realBitcoinMoved` and the daily-budget reader key on); `simulation` =>
// `none` (unpaid smoke). No raw address / invoice / preimage enters any record.
export const buildKhalaSettlementRecords = (
  input: Readonly<{
    adapterKind: 'simulation' | 'spark_treasury'
    nowIso: string
    servedModel: string
    servingRunRef: string
    share: ServingPayoutShare
  }>,
): KhalaSettlementRecords => {
  const { adapterKind, nowIso, servedModel, servingRunRef, share } = input
  const amountSats = share.amountMsat // caller passes a sat-denominated share
  const contributorRef = share.nodeRef.trim()
  const suffix = stableSuffix(`${servingRunRef}.${contributorRef}`)
  const moneyMovement = realSettlementMovementMode(adapterKind)
  const amount = bitcoinAmount(amountSats)
  const redactedDestinationRef = `destination.redacted.khala_serving_settlement.${suffix}`
  const acceptedWorkRefs = uniqueRefs([
    `serving_run.${servingRunRef}`,
    `served_model.${stableSuffix(servedModel)}`,
  ])
  const metadataRefs = uniqueRefs([
    `serving_run.${servingRunRef}`,
    `served_model.${stableSuffix(servedModel)}`,
    'metadata.khala.verified_work_settlement.accepted_work',
  ])

  const targetApproval: NexusPayoutTargetApprovalRecord = {
    agentRef: 'agent.artanis',
    approvalPolicyRef: KHALA_SETTLEMENT_PAYOUT_TARGET_APPROVAL_POLICY_REF,
    approvalRef: `payout_target_approval.khala_serving_settlement.${suffix}`,
    approvedByRef: 'operator.openagents.khala_serving_settlement',
    archivedAt: null,
    createdAt: nowIso,
    expiresAt: null,
    id: `nexus_payout_target_approval_khala_serving_${suffix}`,
    idempotencyKeyHash: `hash.khala_serving_settlement.approval.${suffix}`,
    ownerUserId: 'user_openagents_operator',
    payoutTargetRef: `payout_target.khala_serving_settlement.${suffix}`,
    publicProjectionJson: JSON.stringify({
      contributorRef,
      servingRunRef,
      state: 'active',
    }),
    pylonRef: contributorRef,
    redactedDestinationRef,
    scopeRefs: acceptedWorkRefs,
    status: 'active' as const,
    updatedAt: nowIso,
  }

  const intent: NexusTreasuryPayoutIntentRecord = {
    acceptedWorkRefs,
    actorRef: 'agent.artanis',
    adapterKind,
    amount,
    archivedAt: null,
    artanisDispatchRef: `artanis_dispatch.khala_serving_settlement.${suffix}`,
    assignmentRef: null,
    buyerPaymentRef: null,
    createdAt: nowIso,
    id: `nexus_treasury_payout_intent_khala_serving_${suffix}`,
    idempotencyKeyHash: `hash.khala_serving_settlement.intent.${suffix}`,
    metadataRefs,
    ownerUserId: null,
    payoutIntentRef: `payout_intent.khala_serving_settlement.${suffix}`,
    payoutTargetApprovalRef: targetApproval.approvalRef,
    payoutTargetRef: targetApproval.payoutTargetRef,
    policySnapshotRef: KHALA_SETTLEMENT_POLICY_SNAPSHOT_REF,
    publicProjectionJson: JSON.stringify({
      acceptedWork: true,
      adapter: adapterKind,
      amountSats,
      moneyMovement,
      operatorApproved: true,
      servingRunRef,
      state: 'approved',
    }),
    pylonJobRef: null,
    sourceKind: 'accepted_work',
    spendCap: amount,
    status: 'approved',
    updatedAt: nowIso,
  }

  const attempt: NexusTreasuryPayoutAttemptRecord = {
    adapterAttemptRef: `adapter_attempt.khala_serving_settlement.${adapterKind}.${suffix}`,
    adapterKind,
    amount,
    archivedAt: null,
    createdAt: nowIso,
    id: `nexus_treasury_payout_attempt_khala_serving_${suffix}`,
    idempotencyKeyHash: `hash.khala_serving_settlement.attempt.${suffix}`,
    metadataRefs,
    payoutAttemptRef: `payout_attempt.khala_serving_settlement.${suffix}`,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats,
      moneyMovement,
      servingRunRef,
    }),
    redactedDestinationRef,
    redactedPaymentRef: null,
    status: 'confirmed',
    updatedAt: nowIso,
  }

  const reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord = {
    adapterKind,
    archivedAt: null,
    createdAt: nowIso,
    eventRef: `reconciliation.khala_serving_settlement.${suffix}`,
    externalEventRef: `external_event.khala_serving_settlement.${adapterKind}.${suffix}`,
    id: `nexus_treasury_reconciliation_khala_serving_${suffix}`,
    idempotencyKeyHash: `hash.khala_serving_settlement.reconciliation.${suffix}`,
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    providerRef: `provider.${adapterKind}`,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats,
      moneyMovement,
      servingRunRef,
    }),
    resultRef: `result.khala_serving_settlement.${suffix}`,
    status: 'matched',
  }

  const settlementReceiptRef = `receipt.nexus.khala_serving_settlement.${suffix}`
  const settlementReceipt: NexusPaymentAuthorityReceiptRecord = {
    archivedAt: null,
    audience: 'public',
    createdAt: nowIso,
    eventRef: reconciliationEvent.eventRef,
    id: `nexus_payment_authority_receipt_khala_serving_${suffix}`,
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats,
      asset: 'bitcoin',
      contributorRef,
      moneyMovement,
      servedModel,
      servingRunRef,
      // realBitcoinMoved-shaped: the public activity timeline + daily-budget
      // reader treat `state:'settled' && moneyMovement:'real_bitcoin'` as a real
      // Bitcoin movement.
      state: 'settled',
    }),
    receiptKind: 'settlement_recorded',
    receiptRef: settlementReceiptRef,
  }

  return {
    amountSats,
    attempt,
    contributorRef,
    intent,
    reconciliationEvent,
    settlementReceipt,
    settlementReceiptRef,
    targetApproval,
  }
}

export type KhalaSettlementLegSkipReason =
  | 'amount_not_positive'
  | 'daily_budget_exhausted'
  | 'gate_not_authorized'
  | 'no_payout_destination'
  | 'parity_unverified'
  | 'settlement_failed'

export type KhalaSettlementLegOutcome = Readonly<{
  amountSats: number
  contributorRef: string
  eligibilitySource: 'allowlisted' | 'run_scoped_streaming' | null
  mode: 'real_bitcoin' | 'unpaid_smoke_simulation' | null
  party: 'serving_node'
  realBitcoinMoved: boolean
  remainingDailyBudgetSats: number | null
  settled: boolean
  settlementReceiptRef: string | null
  skipped: KhalaSettlementLegSkipReason | null
}>

export type KhalaSettlementOutcome = Readonly<{
  servingRunRef: string
  legs: ReadonlyArray<KhalaSettlementLegOutcome>
}>

// Injected surface. `dispatchRealSettlement` is the proven receipt-first,
// idempotent Spark dispatch (the same `dispatchRealRunSettlementCore` the admin
// + Tassadar paths use); it must fail-soft for the caller. `resolvePayoutDestination`
// resolves a contributor ref to its registered Spark target (the guinea-pig
// Pylon's address is read from `.secrets/khala-test-payout.env` at the wiring
// layer, NEVER hard-coded here). `readGate` reads the owner real-settlement gate
// (default disabled). `run` carries the run ref the gate allowlists.
export type KhalaSettlementDeps = Readonly<{
  ledger: NexusTreasuryPayoutLedgerStore
  resolvePayoutDestination: (contributorRef: string) => Promise<string | undefined>
  dispatchRealSettlement: (input: {
    contributorRef: string
    settlement: KhalaSettlementRecords
  }) => Effect.Effect<void, unknown>
  readGate: () => TassadarRealSettlementGate
  // The serving run's allowlist key for the owner gate (`allowedRunRefs`). The
  // serving run is keyed under this ref so the owner can enroll the Khala lane
  // explicitly, exactly like a training run.
  settlementRunRef: string
  nowIso?: string | undefined
}>

// Settle one serving-node share for real Bitcoin over Spark, fail-soft +
// idempotent. Threads `alreadySettledTodaySats` so multiple shares in one run
// share the daily budget window. Returns the leg outcome plus the sats actually
// settled (0 when skipped) so the caller advances the running daily total.
const settleShare = (
  deps: KhalaSettlementDeps,
  input: Readonly<{
    alreadySettledTodaySats: number
    nowIso: string
    parityVerified: boolean
    servedModel: string
    servingRunRef: string
    share: ServingPayoutShare
  }>,
): Effect.Effect<
  Readonly<{ outcome: KhalaSettlementLegOutcome; settledSats: number }>
> =>
  Effect.gen(function* () {
    const amountSats = input.share.amountMsat
    const contributorRef = input.share.nodeRef.trim()

    const base = {
      amountSats,
      contributorRef,
      eligibilitySource: null,
      mode: null,
      party: 'serving_node' as const,
      realBitcoinMoved: false,
      remainingDailyBudgetSats: null,
      settled: false,
      settlementReceiptRef: null,
    } satisfies Omit<KhalaSettlementLegOutcome, 'skipped'>

    // GATE 1 — parity (born-verified). Pay only against a checkable outcome.
    if (!input.parityVerified) {
      return {
        outcome: { ...base, skipped: 'parity_unverified' },
        settledSats: 0,
      }
    }

    // GATE 2 — positive amount + a contributor.
    if (!(Number.isInteger(amountSats) && amountSats > 0) || contributorRef === '') {
      return {
        outcome: { ...base, skipped: 'amount_not_positive' },
        settledSats: 0,
      }
    }

    // GATE 3 — owner-armed real-settlement gate (default OFF => not authorized).
    const gate = deps.readGate()
    const decision = resolveTassadarSettlementAdapter({
      amountSats,
      contributorRef,
      gate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: deps.settlementRunRef,
    })

    if (!decision.realAuthorized) {
      return {
        outcome: { ...base, skipped: 'gate_not_authorized' },
        settledSats: 0,
      }
    }

    // GATE 4 — cumulative daily budget (fail-closed).
    const budget = decideTassadarDailyBudget({
      alreadySettledTodaySats: input.alreadySettledTodaySats,
      amountSats,
      gate,
    })

    if (!budget.authorized) {
      return {
        outcome: {
          ...base,
          eligibilitySource: decision.eligibilitySource,
          remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
          skipped: 'daily_budget_exhausted',
        },
        settledSats: 0,
      }
    }

    // GATE 5 — a registered Spark payout destination (the guinea-pig Pylon's
    // Spark address, resolved at the wiring layer). Absent target => skip clean.
    const destination = yield* Effect.tryPromise({
      catch: () => undefined,
      try: () => deps.resolvePayoutDestination(contributorRef),
    }).pipe(Effect.orElseSucceed(() => undefined))

    if (destination === undefined || destination.trim() === '') {
      return {
        outcome: {
          ...base,
          eligibilitySource: decision.eligibilitySource,
          remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
          skipped: 'no_payout_destination',
        },
        settledSats: 0,
      }
    }

    const settlement = buildKhalaSettlementRecords({
      adapterKind: 'spark_treasury',
      nowIso: input.nowIso,
      servedModel: input.servedModel,
      servingRunRef: input.servingRunRef,
      share: input.share,
    })

    // Receipt-first idempotent dispatch. Any failure is caught here so the
    // caller (metering/heartbeat) is never broken.
    const dispatched = yield* deps
      .dispatchRealSettlement({ contributorRef, settlement })
      .pipe(
        Effect.as(true),
        Effect.orElseSucceed(() => false),
      )

    if (!dispatched) {
      return {
        outcome: {
          ...base,
          eligibilitySource: decision.eligibilitySource,
          remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
          skipped: 'settlement_failed',
        },
        settledSats: 0,
      }
    }

    return {
      outcome: {
        ...base,
        eligibilitySource: decision.eligibilitySource,
        mode: 'real_bitcoin',
        realBitcoinMoved: true,
        remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
        settled: true,
        settlementReceiptRef: settlement.settlementReceiptRef,
        skipped: null,
      },
      settledSats: amountSats,
    }
  })

// Settle an ARMED serving-node payout DECISION for real Bitcoin over Spark.
//
// The decision is produced by `decideServingNodePayout` (#5484): it already ran
// the parity, no-resale, asset-boundary, owner-armed, and positive-amount gates
// PURELY and computed the per-stage split. This entrypoint takes that decision,
// settles each share's sats to its node (the guinea-pig Pylon's share first, by
// the order the split presents stages), and produces a real `realBitcoinMoved`
// settlement receipt per share. Fail-soft + idempotent throughout: this never
// fails into the caller. The owner real-settlement gate (default OFF) keeps every
// leg `gate_not_authorized` until armed, so the metering hook can call this
// fire-and-forget with no live money risk by default.
//
// NOTE: the decision's split is computed in MSAT; this settlement path converts
// the per-share cut to whole sats (floor), and a share that rounds below 1 sat
// skips as `amount_not_positive` (dust never moves). Keep the contributor cut
// small + treasury-bounded (the gate's per-payout + daily caps are the ceiling).
export const settleVerifiedServingPayout = (
  deps: KhalaSettlementDeps,
  input: Readonly<{
    decision: ServingNodePayoutDecision
    servedModel: string
    parityVerified: boolean
  }>,
): Effect.Effect<KhalaSettlementOutcome> =>
  Effect.gen(function* () {
    const nowIso = deps.nowIso ?? currentIsoTimestamp()
    const servingRunRef = input.decision.servingRunRef

    // Read today's already-settled real total from the receipt ledger (the
    // receipt-first source of truth for the daily budget). Fail-soft to 0.
    const utcDayKey = tassadarRealSettlementUtcDayKey(nowIso)
    const receipts = yield* Effect.tryPromise({
      catch: () => [],
      try: () => deps.ledger.listPaymentAuthorityReceipts(5000),
    }).pipe(Effect.orElseSucceed(() => []))
    const dayStartSettledSats = tassadarRealSettledSatsForDay(
      receipts,
      utcDayKey,
    )

    // Convert each msat share to a whole-sat share, preserving stage order so the
    // guinea-pig Pylon (the first/only stage of a whole-model serve) is paid
    // first. A share that rounds below 1 sat is kept (it will skip as
    // amount_not_positive) so the outcome is honest about every stage.
    const satShares: ReadonlyArray<ServingPayoutShare> =
      input.decision.split.shares.map(share => ({
        ...share,
        amountMsat: Math.floor(share.amountMsat / 1000),
      }))

    type Acc = Readonly<{
      legs: ReadonlyArray<KhalaSettlementLegOutcome>
      runningSettledSats: number
    }>

    let acc: Acc = { legs: [], runningSettledSats: dayStartSettledSats }
    for (const share of satShares) {
      const result = yield* settleShare(deps, {
        alreadySettledTodaySats: acc.runningSettledSats,
        nowIso,
        parityVerified: input.parityVerified,
        servedModel: input.servedModel,
        servingRunRef,
        share,
      })
      acc = {
        legs: [...acc.legs, result.outcome],
        runningSettledSats: acc.runningSettledSats + result.settledSats,
      }
    }

    // Public-safe diagnostic only: run ref + per-leg settled flag + amount sats +
    // neutral skip reason. Never an address, invoice, preimage, or wallet material.
    yield* Effect.logInfo(
      workerLogEntry('inference.khala_settlement.decided', {
        legCount: acc.legs.length,
        servingRunRef,
        settledLegs: acc.legs.filter(l => l.settled).length,
        skips: acc.legs
          .map(l => l.skipped)
          .filter((s): s is KhalaSettlementLegSkipReason => s !== null)
          .join(','),
        totalSettledSats: acc.legs.reduce(
          (sum, l) => sum + (l.settled ? l.amountSats : 0),
          0,
        ),
      }),
    )

    return { legs: acc.legs, servingRunRef }
  })

// Bridge: build the `recordServingPayout` sink the metering hook
// (`metering-hook.ts` `LedgerMeteringDeps.recordServingPayout`) forwards an ARMED
// serving-payout decision to. This is the wiring point that turns the dormant
// #5484 seam into a real Bitcoin/Spark settlement for the served node(s). The
// metering hook only forwards when the decision is already `armed` (its own
// owner-armed MDK gate passed), and `settleVerifiedServingPayout` independently
// re-checks the owner REAL-SETTLEMENT gate (default OFF) + the per-payout/daily
// caps + a registered destination — so this stays fully inert until the owner
// arms real settlement, and never moves money beyond the bounded test path.
//
// Returns `Effect<void>` (fail-soft) so the metering hook can forward it
// fire-and-forget without ever failing the customer's inference response.
export const makeKhalaServingSettlementSink = (
  deps: KhalaSettlementDeps,
): ((
  decision: ServingNodePayoutDecision,
  receipt: ServingReceipt,
) => Effect.Effect<void>) =>
  (decision, receipt) =>
    settleVerifiedServingPayout(deps, {
      decision,
      parityVerified: receipt.parityVerified,
      servedModel: receipt.servedModel,
    }).pipe(Effect.asVoid)
