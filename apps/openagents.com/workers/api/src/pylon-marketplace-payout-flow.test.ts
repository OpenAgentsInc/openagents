import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutLedgerStore,
  type NexusTreasuryPayoutReconciliationEventRecord,
  nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial,
  projectNexusTreasuryPayoutLedgerRecord,
} from './nexus-treasury-payout-ledger'
import { projectPylonAcceptedWorkPayoutRow } from './pylon-accepted-work-payout-rows'
import {
  PylonMarketplaceAssignmentRecord,
  PylonMarketplaceLedgerRecord,
  examplePylonMarketplaceLedger,
  projectPylonMarketplaceLedger,
} from './pylon-marketplace-jobs'
import {
  PylonMarketplacePayoutFlowUnsafe,
  buildPylonMarketplacePayoutFlowRecords,
  buildPylonMarketplaceSettlementBridgePauseRecord,
  buildPylonMarketplaceSettlementBridgeTimeline,
} from './pylon-marketplace-payout-flow'
import { projectOpenAgentsPylonSettlementBridge } from './pylon-settlement-bridge'
import { makeTreasuryPaymentAuthority } from './treasury-payment-authority'
import {
  buildTreasuryPaymentSimulationReceipts,
  makeTreasuryPaymentSimulationAdapter,
} from './treasury-payment-simulation-adapter'

const createdAtIso = '2026-06-07T10:00:00.000Z'
const updatedAtIso = '2026-06-07T10:10:00.000Z'

class MemoryLedgerStore implements NexusTreasuryPayoutLedgerStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  attemptsByIdempotency = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  createPayoutAttempt = async (record: NexusTreasuryPayoutAttemptRecord) => {
    if (!this.intents.has(record.payoutIntentRef)) {
      throw new Error('intent missing')
    }

    this.attempts.set(record.payoutAttemptRef, record)
    this.attemptsByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createPayoutIntent = async (record: NexusTreasuryPayoutIntentRecord) => {
    this.intents.set(record.payoutIntentRef, record)
    this.intentsByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createPayoutTargetApproval = async () => {}

  createPaymentAuthorityReceipt = async (
    record: NexusPaymentAuthorityReceiptRecord,
  ) => {
    this.receipts.set(record.receiptRef, record)
  }

  createReconciliationEvent = async (
    record: NexusTreasuryPayoutReconciliationEventRecord,
  ) => {
    this.events.set(record.eventRef, record)
  }

  createReleaseGate = async () => {}

  readPayoutAttemptByRef = async (payoutAttemptRef: string) =>
    this.attempts.get(payoutAttemptRef)

  readPayoutAttemptByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.attemptsByIdempotency.get(idempotencyKeyHash)

  readPayoutIntentByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.intentsByIdempotency.get(idempotencyKeyHash)

  readPayoutIntentByBuyerPaymentRef = async (buyerPaymentRef: string) =>
    [...this.intents.values()].find(
      intent => intent.buyerPaymentRef === buyerPaymentRef,
    )

  readPayoutIntentByRef = async (payoutIntentRef: string) =>
    this.intents.get(payoutIntentRef)
  listPaymentAuthorityReceipts = async (limit: number) =>
    [...this.receipts.values()].slice(0, limit)

  readPaymentAuthorityReceiptByRef = async (receiptRef: string) =>
    this.receipts.get(receiptRef)

  readReconciliationEventByRef = async (eventRef: string) =>
    this.events.get(eventRef)
}

const flowRefs = {
  artanisDispatchRef: 'artanis.dispatch.pylon_marketplace.gepa_autopilot_001',
  buyerPaymentEvidenceRef:
    'buyer_payment_evidence.public.pylon_marketplace.gepa_autopilot_001',
  idempotencyRef: 'gepa_autopilot_001',
  ownerUserId: 'user_openagents_operator',
  payoutTargetApprovalRef:
    'approval.nexus_payout_target.pylon_marketplace.gepa_autopilot_001',
  payoutTargetRef: 'payout_target.pylon_marketplace.gepa_autopilot_001',
  policySnapshotRef: 'policy_snapshot.nexus.pylon_marketplace.spend_cap_001',
  providerRef: 'provider.public.pylon_demo_runner',
} as const

const amounts = {
  amount: {
    amountMinorUnits: 1_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  spendCap: {
    amountMinorUnits: 2_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
} as const

describe('Pylon marketplace payout flow', () => {
  test('moves accepted marketplace work through simulated payout intent and settlement receipt', async () => {
    const marketplace = examplePylonMarketplaceLedger()
    const intake = marketplace.intakeRecords[0]!
    const assignment = marketplace.assignmentRecords[0]!
    const flow = buildPylonMarketplacePayoutFlowRecords({
      amounts,
      assignment,
      createdAtIso,
      refs: flowRefs,
      updatedAtIso,
    })
    const ledger = new MemoryLedgerStore()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [makeTreasuryPaymentSimulationAdapter()],
      ledgerStore: ledger,
    })

    const creation = await Effect.runPromise(
      authority.createPayoutIntent({
        intent: flow.intent,
        walletReadiness: 'ready',
      }),
    )
    const dispatch = await Effect.runPromise(
      authority.dispatchPayout({
        attempt: flow.attempt,
        payoutIntentRef: flow.intent.payoutIntentRef,
      }),
    )
    const reconciliation = await Effect.runPromise(
      authority.reconcilePayout({
        event: flow.reconciliationEvent,
      }),
    )
    const receipts = buildTreasuryPaymentSimulationReceipts({
      attempt: dispatch.attempt,
      createdAt: updatedAtIso,
      event: reconciliation.event,
      intent: flow.intent,
    })
    const bridgeTimeline = buildPylonMarketplaceSettlementBridgeTimeline({
      acceptedWork: flow.acceptedWork,
      attempt: dispatch.attempt,
      createdAtIso,
      intent: flow.intent,
      providerRef: flowRefs.providerRef,
      reconciliationEvent: reconciliation.event,
      receipts: [flow.intentCreatedReceipt, ...receipts],
      refs: flowRefs,
      updatedAtIso,
    })

    await ledger.createPaymentAuthorityReceipt(flow.intentCreatedReceipt)
    for (const receipt of receipts) {
      await ledger.createPaymentAuthorityReceipt(receipt)
    }

    const settledBridge = bridgeTimeline.at(-1)!
    const operatorBridge = projectOpenAgentsPylonSettlementBridge(
      settledBridge,
      'operator',
      updatedAtIso,
    )
    const publicBridge = projectOpenAgentsPylonSettlementBridge(
      settledBridge,
      'public',
      updatedAtIso,
    )
    const settlementReceipt = receipts.find(
      receipt => receipt.receiptKind === 'settlement_recorded',
    )!
    const publicReceiptProjection = projectNexusTreasuryPayoutLedgerRecord(
      'receipt',
      settlementReceipt,
      'public',
    )
    const payoutRowProjection = projectPylonAcceptedWorkPayoutRow(
      flow.payoutRow,
      'public',
      updatedAtIso,
    )
    const settledAssignment = new PylonMarketplaceAssignmentRecord({
      ...assignment,
      payoutState: 'settled',
      treasuryReceiptRefs: [
        ...assignment.treasuryReceiptRefs,
        flow.intentCreatedReceipt.receiptRef,
        ...receipts.map(receipt => receipt.receiptRef),
      ],
      updatedAtIso,
    })
    const settledMarketplaceProjection = projectPylonMarketplaceLedger(
      new PylonMarketplaceLedgerRecord({
        ...marketplace,
        assignmentRecords: [settledAssignment],
        intakeRecords: [intake],
        updatedAtIso,
      }),
      'public',
      updatedAtIso,
    )

    expect(
      S.decodeUnknownSync(PylonMarketplaceAssignmentRecord)(settledAssignment),
    ).toEqual(settledAssignment)
    expect(creation.intent.payoutIntentRef).toBe(flow.intent.payoutIntentRef)
    expect(dispatch.attempt.status).toBe('dispatched')
    expect(reconciliation.event.status).toBe('matched')
    expect(ledger.intents.get(flow.intent.payoutIntentRef)).toEqual(flow.intent)
    expect(ledger.attempts.get(dispatch.attempt.payoutAttemptRef)).toEqual(
      dispatch.attempt,
    )
    expect(ledger.events.get(reconciliation.event.eventRef)).toEqual(
      reconciliation.event,
    )
    expect(
      [...ledger.receipts.values()].map(receipt => receipt.receiptKind),
    ).toEqual([
      'intent_created',
      'dispatch_recorded',
      'confirmation_recorded',
      'verification_recorded',
      'settlement_recorded',
    ])
    expect(bridgeTimeline.map(record => record.state)).toEqual([
      'reward_intent',
      'payout_eligible',
      'payout_dispatched',
      'payout_confirmed',
      'payout_verified',
      'settled',
    ])
    expect(operatorBridge.providerJobRefs).toContain(assignment.jobRef)
    expect(operatorBridge.providerAssignmentRefs).toContain(
      assignment.assignmentRef,
    )
    expect(operatorBridge.evidenceRefs).toEqual(
      expect.arrayContaining([
        flowRefs.artanisDispatchRef,
        flow.intent.payoutIntentRef,
        dispatch.attempt.adapterAttemptRef,
      ]),
    )
    expect(operatorBridge.payoutDispatchRefs).toContain(
      dispatch.attempt.payoutAttemptRef,
    )
    expect(publicBridge.settlementClaimAllowed).toBe(true)
    expect(publicBridge.payoutDispatchRefs).toEqual([])
    expect(publicBridge.payoutVerificationRefs).toEqual([])
    expect(publicReceiptProjection.receiptRef).toBe(
      settlementReceipt.receiptRef,
    )
    expect(
      nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial(
        publicReceiptProjection,
      ),
    ).toBe(false)
    expect(payoutRowProjection.settlementClaimAllowed).toBe(true)
    expect(settledMarketplaceProjection.assignmentRecords[0]).toMatchObject({
      payoutState: 'settled',
      settlementClaimAllowed: true,
    })
  })

  test('rejects payout intent creation when accepted-work evidence is missing', () => {
    const assignment = examplePylonMarketplaceLedger().assignmentRecords[0]!

    expect(() =>
      buildPylonMarketplacePayoutFlowRecords({
        amounts,
        assignment: {
          ...assignment,
          acceptedWorkRefs: [],
        },
        createdAtIso,
        refs: flowRefs,
        updatedAtIso,
      }),
    ).toThrow(PylonMarketplacePayoutFlowUnsafe)
    expect(() =>
      buildPylonMarketplacePayoutFlowRecords({
        amounts,
        assignment: {
          ...assignment,
          state: 'running',
        },
        createdAtIso,
        refs: flowRefs,
        updatedAtIso,
      }),
    ).toThrow(PylonMarketplacePayoutFlowUnsafe)
  })

  test('models failed or paused settlement as blocked evidence-only bridge records', () => {
    const assignment = examplePylonMarketplaceLedger().assignmentRecords[0]!
    const flow = buildPylonMarketplacePayoutFlowRecords({
      amounts,
      assignment,
      createdAtIso,
      refs: flowRefs,
      updatedAtIso,
    })
    const paused = buildPylonMarketplaceSettlementBridgePauseRecord({
      acceptedWork: flow.acceptedWork,
      attempt: flow.attempt,
      blockerRefs: ['blocker.public.pylon_marketplace.provider_paused'],
      createdAtIso,
      intent: flow.intent,
      providerRef: flowRefs.providerRef,
      refs: flowRefs,
      updatedAtIso,
    })
    const projection = projectOpenAgentsPylonSettlementBridge(
      paused,
      'operator',
      updatedAtIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'blocker.public.pylon_marketplace.provider_paused',
    ])
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
  })
})
