import { Effect, Schema as S } from 'effect'
import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import { exampleArtanisApprovalGateLedger } from './artanis-approval-gates'
import {
  ARTANIS_NEXUS_PYLON_ADMIN_ADAPTER_SURFACES,
  ARTANIS_NEXUS_PYLON_ADMIN_NO_LIVE_AUTHORITY,
  ArtanisNexusPylonAdapterUnsafe,
  ArtanisNexusPylonAdminAdapterProjection,
  ArtanisNexusPylonDispatchRecord,
  artanisNexusPylonFleetSnapshotFromStats,
  artanisNexusPylonProjectionHasPrivateMaterial,
  createFakeArtanisNexusPylonDispatchAdapter,
  dispatchApprovedArtanisNexusPylonRecord,
  exampleArtanisNexusPylonAdminAdapterLedger,
  projectArtanisNexusPylonAdminAdapter,
  runArtanisNexusPylonPaymentBackedDispatch,
} from './artanis-nexus-pylon-adapters'
import {
  readArtanisPersistedRecord,
  saveArtanisNexusPylonAdapterDispatch,
} from './artanis-persistence'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  PublicPylonAcceptedWorkSettlementGate,
  PublicPylonEarningLaunchGate,
  publicPylonStatsCounterWindows,
  PublicPylonStats,
  emptyUnavailableMarketSettlementTotals,
} from './public-pylon-stats'
import { examplePylonMarketplaceLedger } from './pylon-marketplace-jobs'
import { buildPylonMarketplacePayoutFlowRecords } from './pylon-marketplace-payout-flow'
import {
  ArtanisPersistenceTestStore,
  artanisPersistenceTestDb,
} from './test/artanis-persistence-fixture'
import { makeTreasuryPaymentAuthority } from './treasury-payment-authority'
import { makeTreasuryPaymentSimulationAdapter } from './treasury-payment-simulation-adapter'

const nowIso = '2026-06-07T06:05:00.000Z'
const approvalNowIso = '2026-06-07T04:30:00.000Z'

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

const approvedDispatch = (): ArtanisNexusPylonDispatchRecord => {
  const record =
    exampleArtanisNexusPylonAdminAdapterLedger().dispatchRecords[0]!

  return new ArtanisNexusPylonDispatchRecord({
    ...record,
    createdAtIso: '2026-06-07T04:20:00.000Z',
    updatedAtIso: '2026-06-07T04:21:00.000Z',
  })
}

const payoutFlowRefs = {
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

const payoutAmounts = {
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

const payoutFlow = () =>
  buildPylonMarketplacePayoutFlowRecords({
    amounts: payoutAmounts,
    assignment: examplePylonMarketplaceLedger().assignmentRecords[0]!,
    createdAtIso: approvalNowIso,
    refs: payoutFlowRefs,
    updatedAtIso: nowIso,
  })

describe('Artanis Nexus/Pylon admin adapters', () => {
  test('projects public-safe fleet status and approval-gated dispatch boundaries', () => {
    const publicProjection = projectArtanisNexusPylonAdminAdapter(
      exampleArtanisNexusPylonAdminAdapterLedger(),
      'public_artanis',
      nowIso,
    )
    const operatorProjection = projectArtanisNexusPylonAdminAdapter(
      exampleArtanisNexusPylonAdminAdapterLedger(),
      'operator',
      nowIso,
    )

    expect(
      S.decodeUnknownSync(ArtanisNexusPylonAdminAdapterProjection)(
        publicProjection,
      ),
    ).toEqual(publicProjection)
    expect(publicProjection.authority).toEqual(
      ARTANIS_NEXUS_PYLON_ADMIN_NO_LIVE_AUTHORITY,
    )
    expect(publicProjection.fleetSnapshots[0]).toMatchObject({
      caveatRefs: [
        'caveat.public.legacy_nexus_fixture',
        'caveat.public.no_sensitive_material',
        'caveat.public.online_not_assignment_paid_or_settled',
        'caveat.public.pylon_stats_are_read_only',
      ],
      fleetState: 'live',
      pylonsOnlineNow: 3,
      sellablePylonsOnlineNow: 2,
      sourceRefs: [
        'https://nexus.openagents.com/api/stats',
        'nexus.public.stats',
        'omega.public.pylon_api.registrations',
      ],
      surfaces: ARTANIS_NEXUS_PYLON_ADMIN_ADAPTER_SURFACES,
    })
    expect(publicProjection.dispatchRecords[0]).toMatchObject({
      adapterMode: 'fake',
      fakeDispatchReceiptRecorded: false,
      jobKind: 'inference',
      liveDispatchClaimAllowed: false,
      state: 'approved',
    })
    expect(publicProjection.dispatchRecords[1]).toMatchObject({
      adapterMode: 'live',
      blockerRefs: ['blocker.public.live_dispatch_not_enabled'],
      liveDispatchClaimAllowed: false,
      state: 'blocked',
    })
    expect(operatorProjection.dispatchRecords[0]?.operatorDetailRefs).toEqual([
      'operator.artanis.nexus_pylon.dispatch_review',
    ])
    expect(operatorProjection.dispatchRecords[0]?.privateEvidenceRefs).toEqual([
      'evidence.private.artanis.nexus_pylon.review',
    ])
    expect(publicProjection.dispatchRecords[0]?.operatorDetailRefs).toEqual([])
    expect(publicProjection.dispatchRecords[0]?.privateEvidenceRefs).toEqual([])
    expect(
      artanisNexusPylonProjectionHasPrivateMaterial(publicProjection),
    ).toBe(false)
    expect(JSON.stringify(publicProjection)).not.toContain('operator.artanis')
    expect(JSON.stringify(publicProjection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('summarizes unavailable Nexus/Pylon stats as public blockers', () => {
    const snapshot = artanisNexusPylonFleetSnapshotFromStats(
      new PublicPylonStats({
        asOfLabel: null,
        asOfUnixMs: null,
        available: false,
        error: 'Omega public Pylon stats are unavailable.',
        hostedNexusRelayUrl: null,
        minimumClientVersion: '0.2.5',
        nexusAcceptedWorkPayoutReceiptRefs: [],
        nexusAcceptedWorkPayoutSatsPaid24h: null,
        nexusAcceptedWorkPayoutSatsPaidTotal: null,
        nexusAcceptedWorkSettlementGate:
          new PublicPylonAcceptedWorkSettlementGate({
            blockerRefs: [
              'blocker.public.pylon_settlement.receipts_unavailable',
            ],
            caveatRefs: [
              'caveat.public.pylon_settlement.simulation_receipts_do_not_count',
              'caveat.public.pylon_settlement.payment_receipt_without_settlement_does_not_count',
              'caveat.public.pylon_settlement.duplicate_retries_count_once',
              'caveat.public.no_private_payment_material',
            ],
            gateRef: 'gate.public.pylon.accepted_work_settlement_receipts.v1',
            publicPaidWorkTotalsAllowed: false,
            receiptBackedTotalsAvailable: false,
            settledReceiptRefs: [],
            sourceRefs: [
              'gate.public.pylon.accepted_work_settlement_receipts.v1',
              'route:/api/public/pylon-stats',
            ],
            state: 'unavailable',
            stateLabel:
              'Accepted-work settlement totals unavailable: Omega public Pylon stats are unavailable.',
          }),
        nip90MarketSettlementStats: emptyUnavailableMarketSettlementTotals(
          'NIP-90 market receipt store unavailable.',
        ),
        nexusPayoutSatsPaidTotal: null,
        publicRealSatsSettled24h: null,
        publicRealSatsSettledTotal: null,
        pylonSessionsOnlineNow: 0,
        pylonsAssignmentReadyNow: 0,
        pylonsByClientVersion: {},
        pylonsByResourceMode: {},
        pylonsOnlineNow: 0,
        pylonsRegisteredTotal: 0,
        pylonsSeen24h: 0,
        pylonsWalletReadyNow: 0,
        recentPylons: [],
        sellablePylonsOnlineNow: 0,
        sourceUrl: 'https://openagents.com/api/public/pylon-stats',
        sourceRefs: ['route:/api/public/pylon-stats'],
        status: 'unavailable',
        trainingAcceptedContributors: 0,
        trainingAssignedContributors: 0,
        trainingModelProgressContributors: 0,
        treasuryPayoutCount24h: null,
        treasuryPayoutCountTotal: null,
        treasuryPayoutSatsPaid24h: null,
        treasuryPayoutSatsPaidTotal: null,
        counterWindows: publicPylonStatsCounterWindows(),
        earningLaunchGate: new PublicPylonEarningLaunchGate({
          blockedClaimRefs: [
            'blocked_claim.public.pylon.automatic_bitcoin_earning',
            'blocked_claim.public.pylon.self_serve_paid_work',
            'blocked_claim.public.pylon.assignment_ready_payouts',
          ],
          blockerRefs: [
            'blocker.public.pylon.stats_unavailable',
            'blocker.public.pylon.online_now_zero',
            'blocker.public.pylon.wallet_ready_now_zero',
            'blocker.public.pylon.assignment_ready_now_zero',
          ],
          caveatRefs: [
            'caveat.public.pylon_online_is_not_paid_work',
            'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
            'caveat.public.assignment_ready_is_not_acceptance_or_settlement',
            'caveat.public.no_unconditional_earning_promise',
          ],
          gateRef: 'gate.public.pylon.earning_network_counters.v1',
          publicEarningCopyAllowed: false,
          requiredAssignmentReadyPylonsPresent: false,
          requiredOnlinePylonsPresent: false,
          requiredWalletReadyPylonsPresent: false,
          sourceRefs: ['route:/api/public/pylon-stats'],
          state: 'blocked',
          stateLabel: 'Blocked before public earning copy',
        }),
        caveatRefs: ['caveat.public.pylon_stats_unavailable'],
      }),
      nowIso,
    )

    expect(snapshot.fleetState).toBe('unavailable')
    expect(snapshot.blockerRefs).toEqual([
      'blocker.public.omega_pylon_stats_unavailable',
    ])
    expect(snapshot.sourceRefs).toEqual([
      'https://openagents.com/api/public/pylon-stats',
      'omega.public.pylon_api.registrations',
      'route:/api/public/pylon-stats',
    ])
  })

  test('calls the intended fake adapter route and persists the dispatch receipt', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)
    const { adapter, calls } = createFakeArtanisNexusPylonDispatchAdapter()
    const approvalGate = exampleArtanisApprovalGateLedger.gates[0]!
    const dispatched = dispatchApprovedArtanisNexusPylonRecord({
      adapter,
      approvalGate,
      nowIso: approvalNowIso,
      record: approvedDispatch(),
    })

    expect(calls).toEqual([
      {
        adapterMode: 'fake',
        dispatchRef:
          'dispatch.public.artanis.nexus_pylon.pylon_inference_fake_dispatch',
        idempotencyKey:
          'artanis-nexus-pylon-dispatch:pylon_inference_fake_dispatch:v1',
        nexusRouteRef: 'nexus.route.public.pylon.assign_job.v1',
        proposalRef: 'work.public.artanis.pylon_inference_accepted',
        pylonRouteRef: 'pylon.route.public.assignment.receive.v1',
      },
    ])
    expect(dispatched.receipt).toMatchObject({
      receiptRef:
        'receipt.public.artanis.nexus_pylon_dispatch.dispatch_public_artanis_nexus_pylon_pylon_inference_fake_dispatch',
      runStatusRef:
        'run.public.artanis.nexus_pylon_dispatch.dispatch_public_artanis_nexus_pylon_pylon_inference_fake_dispatch.queued',
    })
    expect(dispatched.record.state).toBe('dispatch_recorded')
    expect(dispatched.record.receiptRefs).toEqual([
      dispatched.receipt.receiptRef,
    ])

    const inserted = await Effect.runPromise(
      saveArtanisNexusPylonAdapterDispatch(
        db,
        dispatched.record,
        approvalNowIso,
      ),
    )
    const retried = await Effect.runPromise(
      saveArtanisNexusPylonAdapterDispatch(
        db,
        dispatched.record,
        approvalNowIso,
      ),
    )
    const stored = await Effect.runPromise(
      readArtanisPersistedRecord(
        db,
        'nexus_pylon_adapter_dispatch',
        dispatched.record.dispatchRef,
      ),
    )

    expect(inserted).toMatchObject({
      executableAuthority: false,
      kind: 'nexus_pylon_adapter_dispatch',
      state: 'inserted',
    })
    expect(retried).toMatchObject({
      idempotent: true,
      kind: 'nexus_pylon_adapter_dispatch',
      state: 'retried',
    })
    expect(stored?.recordRef).toBe(dispatched.record.dispatchRef)
    expect(store.rows('artanis_nexus_pylon_adapter_dispatches')).toHaveLength(1)
  })

  test('runs a simulated payment-backed Pylon dispatch after authority gates pass', async () => {
    const flow = payoutFlow()
    const ledger = new MemoryLedgerStore()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [makeTreasuryPaymentSimulationAdapter()],
      ledgerStore: ledger,
    })

    const result = await Effect.runPromise(
      runArtanisNexusPylonPaymentBackedDispatch({
        attempt: flow.attempt,
        intent: flow.intent,
        nowIso,
        paymentAuthority: authority,
        record: approvedDispatch(),
        settlementBridgeRefs: [flow.bridgeTimeline.at(-1)!.id],
        walletReadiness: 'ready',
      }),
    )
    const operatorProjection = projectArtanisNexusPylonAdminAdapter(
      {
        ...exampleArtanisNexusPylonAdminAdapterLedger(),
        dispatchRecords: [result.record],
        updatedAtIso: nowIso,
      },
      'operator',
      nowIso,
    )
    const publicProjection = projectArtanisNexusPylonAdminAdapter(
      {
        ...exampleArtanisNexusPylonAdminAdapterLedger(),
        dispatchRecords: [result.record],
        updatedAtIso: nowIso,
      },
      'public_artanis',
      nowIso,
    )

    expect(result.blockedReason).toBeNull()
    expect(result.preview?.dispatchAllowed).toBe(true)
    expect(result.creation?.intent.payoutIntentRef).toBe(
      flow.intent.payoutIntentRef,
    )
    expect(result.dispatch?.attempt.status).toBe('dispatched')
    expect(result.record).toMatchObject({
      paymentAuthorityState: 'dispatch_authorized',
      state: 'dispatch_recorded',
    })
    expect(result.record.acceptedWorkRefs).toEqual(flow.intent.acceptedWorkRefs)
    expect(result.record.payoutIntentRefs).toContain(
      flow.intent.payoutIntentRef,
    )
    expect(result.record.payoutAttemptRefs).toContain(
      flow.attempt.payoutAttemptRef,
    )
    expect(result.record.payoutTargetApprovalRefs).toContain(
      payoutFlowRefs.payoutTargetApprovalRef,
    )
    expect(ledger.intents.get(flow.intent.payoutIntentRef)).toEqual(flow.intent)
    expect(ledger.attempts.get(flow.attempt.payoutAttemptRef)?.status).toBe(
      'dispatched',
    )
    expect(operatorProjection.dispatchRecords[0]).toMatchObject({
      paymentAuthorityGatePassed: true,
      paymentAuthorityState: 'dispatch_authorized',
      paymentAuthorityStateLabel: 'Dispatch authorized',
      payoutAttemptRefs: expect.arrayContaining([
        flow.attempt.payoutAttemptRef,
      ]),
    })
    expect(publicProjection.dispatchRecords[0]).toMatchObject({
      paymentAuthorityGatePassed: true,
      paymentAuthorityRefs: [],
      payoutAttemptRefs: [],
    })
    expect(
      artanisNexusPylonProjectionHasPrivateMaterial(publicProjection),
    ).toBe(false)
  })

  test('blocks payment-backed Pylon dispatch before accepted work, payout approval, fresh wallet readiness, or idempotency', async () => {
    const flow = payoutFlow()

    for (const scenario of [
      {
        intent: {
          ...flow.intent,
          acceptedWorkRefs: [],
          idempotencyKeyHash: `${flow.intent.idempotencyKeyHash}.missing_work`,
          payoutIntentRef: `${flow.intent.payoutIntentRef}.missing_work`,
        },
        reason: 'missing_accepted_work_ref',
        walletReadiness: 'ready',
      },
      {
        intent: {
          ...flow.intent,
          idempotencyKeyHash: `${flow.intent.idempotencyKeyHash}.missing_approval`,
          payoutIntentRef: `${flow.intent.payoutIntentRef}.missing_approval`,
          payoutTargetApprovalRef: null,
        },
        reason: 'missing_payout_target_approval',
        walletReadiness: 'ready',
      },
      {
        intent: {
          ...flow.intent,
          idempotencyKeyHash: `${flow.intent.idempotencyKeyHash}.stale_wallet`,
          payoutIntentRef: `${flow.intent.payoutIntentRef}.stale_wallet`,
        },
        reason: 'stale_or_absent_wallet_readiness',
        walletReadiness: 'stale',
      },
    ] as const) {
      const ledger = new MemoryLedgerStore()
      const authority = makeTreasuryPaymentAuthority({
        adapters: [makeTreasuryPaymentSimulationAdapter()],
        ledgerStore: ledger,
      })
      const result = await Effect.runPromise(
        runArtanisNexusPylonPaymentBackedDispatch({
          attempt: {
            ...flow.attempt,
            idempotencyKeyHash: `${flow.attempt.idempotencyKeyHash}.${scenario.reason}`,
            payoutAttemptRef: `${flow.attempt.payoutAttemptRef}.${scenario.reason}`,
            payoutIntentRef: scenario.intent.payoutIntentRef,
          },
          intent: scenario.intent,
          nowIso,
          paymentAuthority: authority,
          record: approvedDispatch(),
          walletReadiness: scenario.walletReadiness,
        }),
      )

      expect(result.blockedReason).toBe(scenario.reason)
      expect(result.creation).toBeNull()
      expect(result.dispatch).toBeNull()
      expect(result.record).toMatchObject({
        paymentAuthorityState: 'dispatch_blocked',
        state: 'blocked',
      })
      expect(result.record.blockerRefs).toContain(
        `blocker.public.payment_authority.${scenario.reason}`,
      )
      expect(result.record.payoutAttemptRefs).toEqual([])
      expect(ledger.intents.size).toBe(0)
      expect(ledger.attempts.size).toBe(0)
    }

    const ledger = new MemoryLedgerStore()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [makeTreasuryPaymentSimulationAdapter()],
      ledgerStore: ledger,
    })
    const first = await Effect.runPromise(
      runArtanisNexusPylonPaymentBackedDispatch({
        attempt: flow.attempt,
        intent: flow.intent,
        nowIso,
        paymentAuthority: authority,
        record: approvedDispatch(),
        walletReadiness: 'ready',
      }),
    )
    const replay = await Effect.runPromise(
      runArtanisNexusPylonPaymentBackedDispatch({
        attempt: flow.attempt,
        intent: flow.intent,
        nowIso,
        paymentAuthority: authority,
        record: approvedDispatch(),
        walletReadiness: 'ready',
      }),
    )

    expect(first.blockedReason).toBeNull()
    expect(replay.blockedReason).toBe('replayed_idempotency_key')
    expect(replay.record.blockerRefs).toContain(
      'blocker.public.payment_authority.replayed_idempotency_key',
    )
    expect(ledger.intents.size).toBe(1)
    expect(ledger.attempts.size).toBe(1)
  })

  test('requires effective approval, eligibility, and fake-adapter mode before dispatch', () => {
    const { adapter } = createFakeArtanisNexusPylonDispatchAdapter()
    const approvalGate = exampleArtanisApprovalGateLedger.gates[0]!
    const noAuthority = new ArtanisNexusPylonDispatchRecord({
      ...approvedDispatch(),
      authorityReceiptRefs: [],
      providerEligibilityRefs: [],
    })
    const liveDispatch = new ArtanisNexusPylonDispatchRecord({
      ...approvedDispatch(),
      adapterMode: 'live',
    })
    const expiredGateTime = '2026-06-07T05:30:00.000Z'

    expect(() =>
      dispatchApprovedArtanisNexusPylonRecord({
        adapter,
        approvalGate,
        nowIso: approvalNowIso,
        record: noAuthority,
      }),
    ).toThrow(ArtanisNexusPylonAdapterUnsafe)
    expect(() =>
      dispatchApprovedArtanisNexusPylonRecord({
        adapter,
        approvalGate,
        nowIso: approvalNowIso,
        record: liveDispatch,
      }),
    ).toThrow(ArtanisNexusPylonAdapterUnsafe)
    expect(() =>
      dispatchApprovedArtanisNexusPylonRecord({
        adapter,
        approvalGate,
        nowIso: expiredGateTime,
        record: approvedDispatch(),
      }),
    ).toThrow(ArtanisNexusPylonAdapterUnsafe)
  })

  test('rejects unsafe private/provider/wallet/payment/raw material and migration drift', () => {
    const unsafe = exampleArtanisNexusPylonAdminAdapterLedger()
    const unsafeDispatch = new ArtanisNexusPylonDispatchRecord({
      ...unsafe.dispatchRecords[0]!,
      privateEvidenceRefs: ['provider_token.local_pylon'],
    })
    const migration = readFileSync(
      new URL(
        '../migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql',
        import.meta.url,
      ),
      'utf8',
    )

    expect(() =>
      projectArtanisNexusPylonAdminAdapter(
        {
          ...unsafe,
          dispatchRecords: [unsafeDispatch],
        },
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisNexusPylonAdapterUnsafe)
    expect(migration).toContain(
      'CREATE TABLE IF NOT EXISTS artanis_nexus_pylon_adapter_dispatches',
    )
    expect(migration).toContain('idempotency_key TEXT NOT NULL UNIQUE')
    expect(migration).toContain('public_projection_json TEXT NOT NULL')
  })
})
