import { Effect } from 'effect'
import { afterEach, describe, expect, test, vi } from 'vitest'

import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAmount,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import type {
  Nip90MarketReceiptStore,
  Nip90MarketSettlementReceiptRecord,
} from './nip90-market-receipts'
import {
  PUBLIC_PYLON_STATS_URL,
  publicPylonStatsFromNexusPayload,
  publicPylonStatsFromRegistrations,
} from './public-pylon-stats'
import { handlePublicPylonStatsApi } from './public-pylon-stats-routes'
import { publicScannerSafeRef } from './public-ref-scanner-safety'
import type { PylonApiRegistrationRecord } from './pylon-api'
import type { TreasuryTransactionRecord } from './treasury-page-routes'
import {
  buildTrainingRunRecord,
  buildTrainingWindowLeaseRecord,
  buildTrainingWindowRecord,
} from './training-run-window-authority'

const nowUnixMs = Date.parse('2026-06-08T14:00:00.000Z')

const registration = (
  input: Partial<PylonApiRegistrationRecord> &
    Pick<PylonApiRegistrationRecord, 'pylonRef'>,
): PylonApiRegistrationRecord => ({
  capabilityRefs: ['capability.public.inference'],
  clientProtocolVersion: '0.2.5',
  clientVersion: 'openagents.pylon@0.2.5',
  createdAt: '2026-06-08T13:30:00.000Z',
  displayName: 'Public Pylon',
  id: `pylon_api_registration_${input.pylonRef}`,
  latestCapacityRefs: ['capacity.public.gpu_available'],
  latestHeartbeatAt: '2026-06-08T13:58:00.000Z',
  latestHeartbeatStatus: 'online',
  latestHealthRefs: ['health.public.ok'],
  latestLoadRefs: ['load.public.low'],
  latestResourceMode: 'balanced',
  ownerAgentCredentialId: 'credential_agent_public',
  ownerAgentTokenPrefix: 'oa_agent_test',
  ownerAgentUserId: 'agent_public',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  resourceMode: 'background_20',
  status: 'active',
  updatedAt: '2026-06-08T13:58:00.000Z',
  walletReady: true,
  walletRef: 'wallet.public.edge',
  ...input,
})

const storeFor = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
) => ({
  listRegistrations: () => Promise.resolve(registrations),
})

const bitcoinSatsAmount = (sats: number): NexusTreasuryPayoutAmount => ({
  amountMinorUnits: sats * 1000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
})

const payoutIntent = (
  input: Readonly<{
    acceptedWorkRefs?: ReadonlyArray<string>
    amountSats: number
    payoutIntentRef: string
  }>,
): NexusTreasuryPayoutIntentRecord => ({
  acceptedWorkRefs: input.acceptedWorkRefs ?? [
    'accepted_work.public.pylon.one',
  ],
  actorRef: 'actor.public.artanis',
  adapterKind: 'hosted_mdk',
  amount: bitcoinSatsAmount(input.amountSats),
  archivedAt: null,
  artanisDispatchRef: 'dispatch.public.artanis.pylon.one',
  assignmentRef: 'assignment.public.pylon.one',
  buyerPaymentRef: null,
  createdAt: '2026-06-08T13:30:00.000Z',
  id: `intent_${input.payoutIntentRef}`,
  idempotencyKeyHash: `idempotency.${input.payoutIntentRef}`,
  metadataRefs: ['metadata.public.pylon.accepted_work'],
  ownerUserId: null,
  payoutIntentRef: input.payoutIntentRef,
  payoutTargetApprovalRef: 'approval.public.pylon.payout_target',
  payoutTargetRef: 'payout_target.public.pylon.one',
  policySnapshotRef: 'policy.public.pylon.spend_cap',
  publicProjectionJson: '{"moneyMovement":"real_bitcoin"}',
  pylonJobRef: 'pylon_job.public.one',
  sourceKind: 'accepted_work',
  spendCap: bitcoinSatsAmount(input.amountSats),
  status: 'settled',
  updatedAt: '2026-06-08T13:58:00.000Z',
})

const payoutAttempt = (
  input: Readonly<{
    payoutAttemptRef: string
    payoutIntentRef: string
  }>,
): NexusTreasuryPayoutAttemptRecord => ({
  adapterAttemptRef: `adapter_attempt.public.${input.payoutAttemptRef}`,
  adapterKind: 'hosted_mdk',
  amount: bitcoinSatsAmount(1),
  archivedAt: null,
  createdAt: '2026-06-08T13:40:00.000Z',
  id: `attempt_${input.payoutAttemptRef}`,
  idempotencyKeyHash: `idempotency.${input.payoutAttemptRef}`,
  metadataRefs: ['metadata.public.pylon.dispatch'],
  payoutAttemptRef: input.payoutAttemptRef,
  payoutIntentRef: input.payoutIntentRef,
  publicProjectionJson: '{"moneyMovement":"real_bitcoin"}',
  redactedDestinationRef: 'destination.public.redacted',
  redactedPaymentRef: 'payment.public.redacted',
  status: 'confirmed',
  updatedAt: '2026-06-08T13:50:00.000Z',
})

const reconciliationEvent = (
  input: Readonly<{
    eventRef: string
    payoutAttemptRef: string
    payoutIntentRef: string
    status?: NexusTreasuryPayoutReconciliationEventRecord['status']
  }>,
): NexusTreasuryPayoutReconciliationEventRecord => ({
  adapterKind: 'hosted_mdk',
  archivedAt: null,
  createdAt: '2026-06-08T13:55:00.000Z',
  eventRef: input.eventRef,
  externalEventRef: `external.public.${input.eventRef}`,
  id: `event_${input.eventRef}`,
  idempotencyKeyHash: `idempotency.${input.eventRef}`,
  metadataRefs: ['metadata.public.pylon.settlement'],
  payoutAttemptRef: input.payoutAttemptRef,
  payoutIntentRef: input.payoutIntentRef,
  providerRef: 'provider.public.mdk',
  publicProjectionJson: '{"moneyMovement":"real_bitcoin"}',
  resultRef: `settlement.public.${input.eventRef}`,
  status: input.status ?? 'matched',
})

const paymentReceipt = (
  input: Readonly<{
    createdAt?: string
    eventRef?: string | null
    payoutAttemptRef?: string | null
    payoutIntentRef: string
    receiptKind?: NexusPaymentAuthorityReceiptRecord['receiptKind']
    receiptRef: string
    simulation?: boolean
  }>,
): NexusPaymentAuthorityReceiptRecord => ({
  archivedAt: null,
  audience: 'public',
  createdAt: input.createdAt ?? '2026-06-08T13:56:00.000Z',
  eventRef: input.eventRef ?? `event.${input.receiptRef}`,
  id: `receipt_${input.receiptRef}`,
  metadataRefs: ['metadata.public.pylon.receipt'],
  payoutAttemptRef:
    input.payoutAttemptRef ?? `attempt.${input.payoutIntentRef}`,
  payoutIntentRef: input.payoutIntentRef,
  publicProjectionJson:
    input.simulation === true
      ? '{"moneyMovement":"simulation"}'
      : '{"moneyMovement":"real_bitcoin"}',
  receiptKind: input.receiptKind ?? 'settlement_recorded',
  receiptRef: input.receiptRef,
})

class MemorySettlementReceiptStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  listPaymentAuthorityReceipts = async (limit: number) =>
    [...this.receipts.values()].slice(0, limit)

  readPayoutAttemptByRef = async (payoutAttemptRef: string) =>
    this.attempts.get(payoutAttemptRef)

  readPayoutIntentByRef = async (payoutIntentRef: string) =>
    this.intents.get(payoutIntentRef)

  readReconciliationEventByRef = async (eventRef: string) =>
    this.events.get(eventRef)
}

const settlementReceiptStore = (
  input: Readonly<{
    attempts?: ReadonlyArray<NexusTreasuryPayoutAttemptRecord>
    events?: ReadonlyArray<NexusTreasuryPayoutReconciliationEventRecord>
    intents?: ReadonlyArray<NexusTreasuryPayoutIntentRecord>
    receipts?: ReadonlyArray<NexusPaymentAuthorityReceiptRecord>
  }>,
) => {
  const store = new MemorySettlementReceiptStore()

  ;(input.intents ?? []).forEach(intent => {
    store.intents.set(intent.payoutIntentRef, intent)
  })
  ;(input.attempts ?? []).forEach(attempt => {
    store.attempts.set(attempt.payoutAttemptRef, attempt)
  })
  ;(input.events ?? []).forEach(event => {
    store.events.set(event.eventRef, event)
  })
  ;(input.receipts ?? []).forEach(receipt => {
    store.receipts.set(receipt.receiptRef, receipt)
  })

  return store
}

const marketReceipt = (
  input: Partial<Nip90MarketSettlementReceiptRecord> &
    Pick<Nip90MarketSettlementReceiptRecord, 'receiptRef' | 'streamKind'>,
): Nip90MarketSettlementReceiptRecord => {
  const { receiptRef, streamKind, ...overrides } = input

  return {
    amountMsats: 2_000,
    createdAt: '2026-06-08T13:45:00.000Z',
    jobRef: `buy_mode_job_${receiptRef}`,
    receiptRef,
    requestEventRef: `event.request.${receiptRef}`,
    resultEventRef: `event.result.${receiptRef}`,
    settledAt: '2026-06-08T13:57:00.000Z',
    state: 'settled',
    streamKind,
    ...overrides,
  }
}

const marketReceiptStore = (
  records: ReadonlyArray<Nip90MarketSettlementReceiptRecord>,
): Nip90MarketReceiptStore => ({
  listSettledMarketReceipts: () => Promise.resolve(records),
  readSettledMarketReceiptByRef: receiptRef =>
    Promise.resolve(
      records.find(record => record.receiptRef === receiptRef) ?? null,
    ),
})

const treasuryPayoutTransaction = (
  input: Readonly<{
    amountSat: number
    createdAt?: string
    id: string
    settledAt?: string | null
    state?: TreasuryTransactionRecord['state']
  }>,
): TreasuryTransactionRecord => ({
  amountSat: input.amountSat,
  bolt11: null,
  createdAt: input.createdAt ?? '2026-06-08T13:57:00.000Z',
  direction: 'out',
  expiresAt: null,
  failureReasonRef: null,
  id: input.id,
  owedRef: null,
  owedSat: null,
  paymentRef: `payment.public.${input.id}`,
  recipientConfirmationRef: null,
  recipientConfirmationState: 'unconfirmed',
  recipientConfirmedAt: null,
  recipientRef: null,
  redactedDestinationRef: null,
  settledAt: input.settledAt ?? '2026-06-08T13:58:00.000Z',
  state: input.state ?? 'settled',
})

const treasuryPayoutStore = (
  transactions: ReadonlyArray<TreasuryTransactionRecord>,
) => ({
  listRecent: () => Promise.resolve(transactions),
})

const trainingContributorStore = (pylonRefs: ReadonlyArray<string>) => {
  const run = buildTrainingRunRecord({
    makeId: () => 'stats_run',
    nowIso: '2026-06-08T13:00:00.000Z',
    request: {
      promiseRef: 'promise.public.training.stats',
      receiptRefs: ['receipt.public.training.stats.run'],
      sourceRefs: ['source.public.training.stats'],
      trainingRunRef: 'run.tassadar.executor.20260615',
    },
  })
  const window = buildTrainingWindowRecord({
    makeId: () => 'stats_window',
    nowIso: '2026-06-08T13:05:00.000Z',
    request: {
      datasetRefs: ['dataset.public.training.stats'],
      receiptRefs: ['receipt.public.training.stats.window'],
      sourceRefs: ['source.public.training.stats.window'],
      trainingRunRef: run.trainingRunRef,
      windowRef: 'window.public.training.stats',
    },
  })
  const leases = pylonRefs.map((pylonRef, index) =>
    buildTrainingWindowLeaseRecord({
      makeId: () => `stats_lease_${index}`,
      nowIso: '2026-06-08T13:10:00.000Z',
      request: {
        pylonRef,
        receiptRefs: [`receipt.public.training.stats.lease.${index}`],
      },
      window,
    }),
  )

  return {
    listVerificationChallengesForRun: () => Promise.resolve([]),
    listWindowLeasesForRun: () => Promise.resolve(leases),
    listWindowsForRun: () => Promise.resolve([window]),
    readRun: (trainingRunRef: string) =>
      Promise.resolve(trainingRunRef === run.trainingRunRef ? run : undefined),
  }
}

describe('public pylon stats', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  test('normalizes legacy Nexus payloads for compatibility fixtures', () => {
    const stats = publicPylonStatsFromNexusPayload({
      as_of_unix_ms: 1_780_000_000_000,
      hosted_nexus_relay_url: 'wss://nexus.openagents.com/',
      nexus_accepted_work_payout_receipt_refs: [
        'receipt.nexus.public.legacy_settlement',
      ],
      nexus_accepted_work_payout_sats_paid_24h: 55,
      nexus_accepted_work_payout_sats_paid_total: 4100,
      nexus_payout_sats_paid_total: 7890,
      pylon_sessions_online_now: 9,
      pylons_online_now: 7,
      pylons_seen_24h: 19,
      recent_pylons: [
        {
          client_version: 'openagents.pylon@0.1.14',
          eligible_product_count: 2,
          last_seen_at_unix_ms: 1_780_000_000_000,
          node_label: 'alpha-mac-mini',
          nostr_pubkey_short: 'npub-alpha',
          products: ['training', 'inference'],
          ready_model: 'gemma4:e4b',
          relay_urls: ['wss://nexus.openagents.com/'],
          runtime_state: 'online',
        },
      ],
      sellable_pylons_online_now: 5,
      training_accepted_contributors: 3,
      training_assigned_contributors: 4,
      training_model_progress_contributors: 2,
    })

    expect(stats).toMatchObject({
      available: true,
      hostedNexusRelayUrl: 'wss://nexus.openagents.com/',
      minimumClientVersion: 'legacy-nexus',
      nexusAcceptedWorkPayoutSatsPaid24h: 55,
      nexusAcceptedWorkPayoutReceiptRefs: [
        'receipt.nexus.public.legacy_settlement',
      ],
      nexusAcceptedWorkPayoutSatsPaidTotal: 4100,
      nexusPayoutSatsPaidTotal: 7890,
      pylonSessionsOnlineNow: 9,
      pylonsOnlineNow: 7,
      pylonsSeen24h: 19,
      recentPylons: [
        {
          clientVersion: 'openagents.pylon@0.1.14',
          eligibleProductCount: 2,
          nodeLabel: 'alpha-mac-mini',
          products: ['training', 'inference'],
          readyModel: 'gemma4:e4b',
          relayUrls: ['wss://nexus.openagents.com/'],
        },
      ],
      sellablePylonsOnlineNow: 5,
      sourceRefs: [
        'nexus.public.stats',
        'https://nexus.openagents.com/api/stats',
      ],
      status: 'live',
      trainingAcceptedContributors: 3,
      trainingAssignedContributors: 4,
      trainingModelProgressContributors: 2,
      earningLaunchGate: {
        blockerRefs: [],
        publicEarningCopyAllowed: true,
        requiredAssignmentReadyPylonsPresent: true,
        requiredOnlinePylonsPresent: true,
        requiredWalletReadyPylonsPresent: true,
        state: 'ready',
      },
    })
  })

  test('does not upgrade legacy aggregate payout totals without receipt refs', () => {
    const stats = publicPylonStatsFromNexusPayload({
      nexus_accepted_work_payout_sats_paid_24h: 20,
      nexus_accepted_work_payout_sats_paid_total: 200,
      nexus_payout_sats_paid_total: 200,
      pylons_online_now: 1,
      sellable_pylons_online_now: 1,
    })

    expect(stats).toMatchObject({
      nexusAcceptedWorkPayoutReceiptRefs: [],
      nexusAcceptedWorkPayoutSatsPaid24h: null,
      nexusAcceptedWorkPayoutSatsPaidTotal: null,
      nexusAcceptedWorkSettlementGate: {
        publicPaidWorkTotalsAllowed: false,
        receiptBackedTotalsAvailable: false,
        state: 'unavailable',
      },
      nexusPayoutSatsPaidTotal: null,
    })
  })

  test('builds Omega-owned v0.2.5+ registration stats without Nexus fetch', () => {
    const stats = publicPylonStatsFromRegistrations(
      [
        registration({
          pylonRef: 'pylon.public.online_wallet_ready',
        }),
        registration({
          clientVersion: 'pylon-v0.2.4',
          pylonRef: 'pylon.public.old_version',
        }),
        registration({
          latestHeartbeatAt: '2026-06-08T13:40:00.000Z',
          pylonRef: 'pylon.public.seen_not_online',
        }),
        registration({
          latestHeartbeatStatus: 'blocked',
          pylonRef: 'pylon.public.blocked_status',
        }),
        registration({
          pylonRef: 'pylon.public.online_wallet_not_ready',
          walletReady: false,
        }),
      ],
      nowUnixMs,
    )

    expect(stats).toMatchObject({
      available: true,
      hostedNexusRelayUrl: null,
      minimumClientVersion: '0.2.5',
      nexusAcceptedWorkPayoutSatsPaid24h: null,
      nexusAcceptedWorkPayoutSatsPaidTotal: null,
      nexusPayoutSatsPaidTotal: null,
      pylonsAssignmentReadyNow: 1,
      pylonsOnlineNow: 2,
      pylonsRegisteredTotal: 4,
      pylonsSeen24h: 4,
      pylonsWalletReadyNow: 1,
      sellablePylonsOnlineNow: 1,
      sourceUrl: PUBLIC_PYLON_STATS_URL,
      status: 'live',
      earningLaunchGate: {
        blockerRefs: [],
        publicEarningCopyAllowed: true,
        requiredAssignmentReadyPylonsPresent: true,
        requiredOnlinePylonsPresent: true,
        requiredWalletReadyPylonsPresent: true,
        state: 'ready',
      },
    })
    expect(stats.nexusAcceptedWorkSettlementGate).toMatchObject({
      publicPaidWorkTotalsAllowed: false,
      receiptBackedTotalsAvailable: false,
      state: 'unavailable',
    })
    expect(stats.pylonsByClientVersion).toEqual({
      'openagents.pylon@0.2.5': 4,
    })
    expect(stats.pylonsByResourceMode).toEqual({ balanced: 4 })
    expect(stats.recentPylons.map(pylon => pylon.nostrPubkeyShort)).toEqual([
      'pylon.public.online_wallet_ready',
      'pylon.public.blocked_status',
      'pylon.public.online_wallet_not_ready',
      'pylon.public.seen_not_online',
    ])
    expect(stats.recentPylons[0]).toMatchObject({
      ownerAgentRef: 'agent:agent_public',
      pylonRef: 'pylon.public.online_wallet_ready',
      tipEndpoint:
        '/api/pylons/pylon.public.online_wallet_ready/tips/ladder',
      tippingAvailable: true,
    })
    expect(JSON.stringify(stats)).not.toMatch(
      /wallet\.secret|lnbc|preimage|\/Users\/|provider_secret|customer@example.com|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('renders scanner-shaped capability refs safely in recent Pylon products', () => {
    const scannerShapedCapabilityRef =
      'edge-pylon-capability-8b378373002501f3e896dcd3'
    const stats = publicPylonStatsFromRegistrations(
      [
        registration({
          capabilityRefs: [
            scannerShapedCapabilityRef,
            'capability.public.inference',
          ],
          pylonRef: 'pylon.public.online_wallet_ready',
        }),
      ],
      nowUnixMs,
    )

    expect(stats.recentPylons[0]?.products).toEqual([
      'capability.public.inference',
      publicScannerSafeRef(
        'capability.public.pylon_stats',
        scannerShapedCapabilityRef,
      ),
    ])
    expect(JSON.stringify(stats)).not.toContain(scannerShapedCapabilityRef)
  })

  test('self-describes counter windows so rows can never contradict counters unlabeled', () => {
    const stats = publicPylonStatsFromRegistrations(
      [
        registration({
          pylonRef: 'pylon.public.fresh_online',
        }),
        registration({
          // Reported "online" 20 minutes ago: inside the 24h sample window,
          // outside the 5-minute online-now window. This is the exact shape
          // from issue #4735 and must now be reconcilable from the JSON.
          latestHeartbeatAt: '2026-06-08T13:40:00.000Z',
          pylonRef: 'pylon.public.stale_reported_online',
        }),
      ],
      nowUnixMs,
    )

    expect(stats.counterWindows).toMatchObject({
      assignmentReadyNowWindowMinutes: 5,
      onlineNowWindowMinutes: 5,
      recentPylonsLimit: 12,
      recentPylonsWindowMinutes: 1440,
      seen24hWindowMinutes: 1440,
      walletReadyNowWindowMinutes: 5,
    })
    expect(stats.counterWindows.onlineHeartbeatStatuses).toContain('online')
    expect(stats.counterWindows.definitionRefs).toContain(
      'definition.public.pylon_stats.runtime_state_is_last_reported_not_live.v1',
    )
    expect(stats.caveatRefs).toContain(
      'caveat.public.recent_pylon_runtime_state_is_last_reported_not_live',
    )

    const fresh = stats.recentPylons.find(
      pylon => pylon.nostrPubkeyShort === 'pylon.public.fresh_online',
    )
    const stale = stats.recentPylons.find(
      pylon => pylon.nostrPubkeyShort === 'pylon.public.stale_reported_online',
    )

    expect(fresh).toMatchObject({
      assignmentReadyNow: true,
      lastHeartbeatAgeSeconds: 120,
      onlineNow: true,
      runtimeState: 'online',
      walletReadyNow: true,
    })
    expect(stale).toMatchObject({
      assignmentReadyNow: false,
      lastHeartbeatAgeSeconds: 1200,
      onlineNow: false,
      runtimeState: 'online',
      walletReadyNow: false,
    })

    const rowsCountedOnline = stats.recentPylons.filter(
      pylon => pylon.onlineNow === true,
    ).length

    expect(stats.pylonsOnlineNow).toBe(rowsCountedOnline)
    expect(stats.pylonsWalletReadyNow).toBe(
      stats.recentPylons.filter(pylon => pylon.walletReadyNow === true).length,
    )
  })

  test('blocks public earning copy and exposes blocker refs when counters are zero', () => {
    const stats = publicPylonStatsFromRegistrations([], nowUnixMs)

    expect(stats).toMatchObject({
      pylonsAssignmentReadyNow: 0,
      pylonsOnlineNow: 0,
      pylonsWalletReadyNow: 0,
      earningLaunchGate: {
        blockedClaimRefs: [
          'blocked_claim.public.pylon.automatic_bitcoin_earning',
          'blocked_claim.public.pylon.self_serve_paid_work',
          'blocked_claim.public.pylon.assignment_ready_payouts',
        ],
        blockerRefs: [
          'blocker.public.pylon.online_now_zero',
          'blocker.public.pylon.wallet_ready_now_zero',
          'blocker.public.pylon.assignment_ready_now_zero',
        ],
        publicEarningCopyAllowed: false,
        requiredAssignmentReadyPylonsPresent: false,
        requiredOnlinePylonsPresent: false,
        requiredWalletReadyPylonsPresent: false,
        state: 'blocked',
      },
    })
  })

  test('keeps wallet-ready and assignment-ready counters separate', () => {
    const stats = publicPylonStatsFromRegistrations(
      [
        registration({
          latestCapacityRefs: [],
          pylonRef: 'pylon.public.wallet_ready_without_capacity',
        }),
      ],
      nowUnixMs,
    )

    expect(stats).toMatchObject({
      pylonsAssignmentReadyNow: 0,
      pylonsOnlineNow: 1,
      pylonsWalletReadyNow: 1,
      sellablePylonsOnlineNow: 0,
      earningLaunchGate: {
        blockerRefs: ['blocker.public.pylon.assignment_ready_now_zero'],
        publicEarningCopyAllowed: false,
        requiredAssignmentReadyPylonsPresent: false,
        requiredOnlinePylonsPresent: true,
        requiredWalletReadyPylonsPresent: true,
        state: 'blocked',
      },
    })
    expect(stats.earningLaunchGate.caveatRefs).toContain(
      'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
    )
  })

  test('expires stale heartbeats from online and earning gate counters', () => {
    const stats = publicPylonStatsFromRegistrations(
      [
        registration({
          latestHeartbeatAt: '2026-06-08T13:50:00.000Z',
          pylonRef: 'pylon.public.stale_wallet_ready',
        }),
      ],
      nowUnixMs,
    )

    expect(stats).toMatchObject({
      pylonsAssignmentReadyNow: 0,
      pylonsOnlineNow: 0,
      pylonsSeen24h: 1,
      pylonsWalletReadyNow: 0,
      earningLaunchGate: {
        blockerRefs: [
          'blocker.public.pylon.online_now_zero',
          'blocker.public.pylon.wallet_ready_now_zero',
          'blocker.public.pylon.assignment_ready_now_zero',
        ],
        publicEarningCopyAllowed: false,
        state: 'blocked',
      },
    })
  })

  test('passes the gate for two fresh wallet and assignment ready Pylons', () => {
    const stats = publicPylonStatsFromRegistrations(
      [
        registration({ pylonRef: 'pylon.public.ready_one' }),
        registration({
          clientVersion: 'openagents.pylon@0.2.6',
          latestHeartbeatAt: '2026-06-08T13:59:00.000Z',
          pylonRef: 'pylon.public.ready_two',
        }),
      ],
      nowUnixMs,
    )

    expect(stats).toMatchObject({
      pylonsAssignmentReadyNow: 2,
      pylonsOnlineNow: 2,
      pylonsWalletReadyNow: 2,
      sellablePylonsOnlineNow: 2,
      earningLaunchGate: {
        blockerRefs: [],
        publicEarningCopyAllowed: true,
        requiredAssignmentReadyPylonsPresent: true,
        requiredOnlinePylonsPresent: true,
        requiredWalletReadyPylonsPresent: true,
        state: 'ready',
      },
    })
  })

  test('aggregates accepted-work payout totals from settled real bitcoin public receipts', async () => {
    const firstIntent = payoutIntent({
      amountSats: 21,
      payoutIntentRef: 'intent.public.first',
    })
    const secondIntent = payoutIntent({
      amountSats: 34,
      payoutIntentRef: 'intent.public.second',
    })
    const firstAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.first',
      payoutIntentRef: firstIntent.payoutIntentRef,
    })
    const secondAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.second',
      payoutIntentRef: secondIntent.payoutIntentRef,
    })
    const firstEvent = reconciliationEvent({
      eventRef: 'event.public.first',
      payoutAttemptRef: firstAttempt.payoutAttemptRef,
      payoutIntentRef: firstIntent.payoutIntentRef,
    })
    const secondEvent = reconciliationEvent({
      eventRef: 'event.public.second',
      payoutAttemptRef: secondAttempt.payoutAttemptRef,
      payoutIntentRef: secondIntent.payoutIntentRef,
    })
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          receiptStore: settlementReceiptStore({
            attempts: [firstAttempt, secondAttempt],
            events: [firstEvent, secondEvent],
            intents: [firstIntent, secondIntent],
            receipts: [
              paymentReceipt({
                eventRef: firstEvent.eventRef,
                payoutAttemptRef: firstAttempt.payoutAttemptRef,
                payoutIntentRef: firstIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.first',
              }),
              paymentReceipt({
                createdAt: '2026-06-06T13:56:00.000Z',
                eventRef: secondEvent.eventRef,
                payoutAttemptRef: secondAttempt.payoutAttemptRef,
                payoutIntentRef: secondIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.second',
              }),
            ],
          }),
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
        },
      ),
    )
    const stats = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(stats).toMatchObject({
      nexusAcceptedWorkPayoutReceiptRefs: [
        'receipt.nexus.public.first',
        'receipt.nexus.public.second',
      ],
      nexusAcceptedWorkPayoutSatsPaid24h: 21,
      nexusAcceptedWorkPayoutSatsPaidTotal: 55,
      nexusAcceptedWorkSettlementGate: {
        publicPaidWorkTotalsAllowed: true,
        receiptBackedTotalsAvailable: true,
        settledReceiptRefs: [
          'receipt.nexus.public.first',
          'receipt.nexus.public.second',
        ],
        state: 'ready',
      },
      nexusPayoutSatsPaidTotal: 55,
    })
    expect(JSON.stringify(stats)).not.toMatch(
      /lnbc|preimage|mnemonic|payment_hash|wallet_secret|privatePayoutDestination|raw_invoice/,
    )
  })

  test('excludes simulations, non-settlement receipts, rejected settlement, and duplicate retries', async () => {
    const countedIntent = payoutIntent({
      amountSats: 13,
      payoutIntentRef: 'intent.public.counted',
    })
    const simulationIntent = payoutIntent({
      amountSats: 100,
      payoutIntentRef: 'intent.public.simulation',
    })
    const verifiedIntent = payoutIntent({
      amountSats: 200,
      payoutIntentRef: 'intent.public.verified',
    })
    const rejectedIntent = payoutIntent({
      amountSats: 300,
      payoutIntentRef: 'intent.public.rejected',
    })
    const countedAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.counted',
      payoutIntentRef: countedIntent.payoutIntentRef,
    })
    const countedEvent = reconciliationEvent({
      eventRef: 'event.public.counted',
      payoutAttemptRef: countedAttempt.payoutAttemptRef,
      payoutIntentRef: countedIntent.payoutIntentRef,
    })
    const simulationAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.simulation',
      payoutIntentRef: simulationIntent.payoutIntentRef,
    })
    const simulationEvent = reconciliationEvent({
      eventRef: 'event.public.simulation',
      payoutAttemptRef: simulationAttempt.payoutAttemptRef,
      payoutIntentRef: simulationIntent.payoutIntentRef,
    })
    const verifiedAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.verified',
      payoutIntentRef: verifiedIntent.payoutIntentRef,
    })
    const verifiedEvent = reconciliationEvent({
      eventRef: 'event.public.verified',
      payoutAttemptRef: verifiedAttempt.payoutAttemptRef,
      payoutIntentRef: verifiedIntent.payoutIntentRef,
    })
    const rejectedAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.rejected',
      payoutIntentRef: rejectedIntent.payoutIntentRef,
    })
    const rejectedEvent = reconciliationEvent({
      eventRef: 'event.public.rejected',
      payoutAttemptRef: rejectedAttempt.payoutAttemptRef,
      payoutIntentRef: rejectedIntent.payoutIntentRef,
      status: 'rejected',
    })
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          receiptStore: settlementReceiptStore({
            attempts: [
              countedAttempt,
              simulationAttempt,
              verifiedAttempt,
              rejectedAttempt,
            ],
            events: [
              countedEvent,
              simulationEvent,
              verifiedEvent,
              rejectedEvent,
            ],
            intents: [
              countedIntent,
              simulationIntent,
              verifiedIntent,
              rejectedIntent,
            ],
            receipts: [
              paymentReceipt({
                eventRef: countedEvent.eventRef,
                payoutAttemptRef: countedAttempt.payoutAttemptRef,
                payoutIntentRef: countedIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.counted',
              }),
              paymentReceipt({
                eventRef: countedEvent.eventRef,
                payoutAttemptRef: countedAttempt.payoutAttemptRef,
                payoutIntentRef: countedIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.counted_retry',
              }),
              paymentReceipt({
                eventRef: simulationEvent.eventRef,
                payoutAttemptRef: simulationAttempt.payoutAttemptRef,
                payoutIntentRef: simulationIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.simulation',
                simulation: true,
              }),
              paymentReceipt({
                eventRef: verifiedEvent.eventRef,
                payoutAttemptRef: verifiedAttempt.payoutAttemptRef,
                payoutIntentRef: verifiedIntent.payoutIntentRef,
                receiptKind: 'verification_recorded',
                receiptRef: 'receipt.nexus.public.verified_only',
              }),
              paymentReceipt({
                eventRef: rejectedEvent.eventRef,
                payoutAttemptRef: rejectedAttempt.payoutAttemptRef,
                payoutIntentRef: rejectedIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.rejected',
              }),
            ],
          }),
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
        },
      ),
    )

    await expect(response.json()).resolves.toMatchObject({
      nexusAcceptedWorkPayoutReceiptRefs: ['receipt.nexus.public.counted'],
      nexusAcceptedWorkPayoutSatsPaid24h: 13,
      nexusAcceptedWorkPayoutSatsPaidTotal: 13,
      publicRealSatsSettled24h: 13,
      publicRealSatsSettledTotal: 13,
      nexusAcceptedWorkSettlementGate: {
        publicPaidWorkTotalsAllowed: true,
        state: 'ready',
      },
    })
  })

  test('distinguishes zero settled receipts from unavailable receipt storage', async () => {
    const zeroResponse = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          receiptStore: settlementReceiptStore({}),
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
        },
      ),
    )
    const unavailableResponse = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          receiptStore: {
            listPaymentAuthorityReceipts: () =>
              Promise.reject(new Error('receipt ledger unavailable')),
            readPayoutAttemptByRef: () => Promise.resolve(undefined),
            readPayoutIntentByRef: () => Promise.resolve(undefined),
            readReconciliationEventByRef: () => Promise.resolve(undefined),
          },
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
        },
      ),
    )

    await expect(zeroResponse.json()).resolves.toMatchObject({
      nexusAcceptedWorkPayoutReceiptRefs: [],
      nexusAcceptedWorkPayoutSatsPaid24h: 0,
      nexusAcceptedWorkPayoutSatsPaidTotal: 0,
      nexusAcceptedWorkSettlementGate: {
        blockerRefs: ['blocker.public.pylon_settlement.settled_receipts_zero'],
        receiptBackedTotalsAvailable: true,
        state: 'blocked',
      },
    })
    await expect(unavailableResponse.json()).resolves.toMatchObject({
      nexusAcceptedWorkPayoutReceiptRefs: [],
      nexusAcceptedWorkPayoutSatsPaid24h: null,
      nexusAcceptedWorkPayoutSatsPaidTotal: null,
      nexusAcceptedWorkSettlementGate: {
        blockerRefs: ['blocker.public.pylon_settlement.receipts_unavailable'],
        receiptBackedTotalsAvailable: false,
        state: 'unavailable',
      },
    })
  })

  test('counts only settled public NIP-90 market receipts by stream', async () => {
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          marketReceiptStore: marketReceiptStore([
            marketReceipt({
              amountMsats: 2_000,
              receiptRef: 'receipt.nip90_market.compute.settled_recent',
              streamKind: 'compute',
            }),
            marketReceipt({
              amountMsats: 3_000,
              receiptRef: 'receipt.nip90_market.data.settled_old',
              settledAt: '2026-06-06T13:57:00.000Z',
              streamKind: 'data',
            }),
            marketReceipt({
              receiptRef: 'receipt.nip90_market.labor.pending',
              state: 'issued',
              streamKind: 'labor',
            }),
            marketReceipt({
              amountMsats: 1_500,
              receiptRef: 'receipt.nip90_market.compute.fractional_msat',
              streamKind: 'compute',
            }),
          ]),
          nowUnixMs: () => nowUnixMs,
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
        },
      ),
    )
    const stats = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(stats.nip90MarketSettlementStats).toMatchObject({
      available: true,
      compute: {
        jobsSettled24h: 1,
        jobsSettledTotal: 1,
        receiptRefs: ['receipt.nip90_market.compute.settled_recent'],
        satsSettled24h: 2,
        satsSettledTotal: 2,
        streamKind: 'compute',
      },
      data: {
        jobsSettled24h: 0,
        jobsSettledTotal: 1,
        receiptRefs: ['receipt.nip90_market.data.settled_old'],
        satsSettled24h: 0,
        satsSettledTotal: 3,
        streamKind: 'data',
      },
      labor: {
        jobsSettled24h: 0,
        jobsSettledTotal: 0,
        receiptRefs: [],
        satsSettled24h: 0,
        satsSettledTotal: 0,
        streamKind: 'labor',
      },
    })
    expect(JSON.stringify(stats.nip90MarketSettlementStats)).not.toMatch(
      /lnbc|bolt11|invoice|preimage|payment_hash|wallet|mnemonic|private_key/,
    )
  })

  test('aggregates real homepage sats across treasury, NIP-90, and accepted-work rails', async () => {
    const acceptedIntent = payoutIntent({
      amountSats: 21,
      payoutIntentRef: 'intent.public.accepted_work',
    })
    const acceptedAttempt = payoutAttempt({
      payoutAttemptRef: 'attempt.public.accepted_work',
      payoutIntentRef: acceptedIntent.payoutIntentRef,
    })
    const acceptedEvent = reconciliationEvent({
      eventRef: 'event.public.accepted_work',
      payoutAttemptRef: acceptedAttempt.payoutAttemptRef,
      payoutIntentRef: acceptedIntent.payoutIntentRef,
    })
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          marketReceiptStore: marketReceiptStore([
            marketReceipt({
              amountMsats: 2_000,
              receiptRef: 'receipt.nip90_market.compute.stats',
              streamKind: 'compute',
            }),
          ]),
          nowUnixMs: () => nowUnixMs,
          receiptStore: settlementReceiptStore({
            attempts: [acceptedAttempt],
            events: [acceptedEvent],
            intents: [acceptedIntent],
            receipts: [
              paymentReceipt({
                eventRef: acceptedEvent.eventRef,
                payoutAttemptRef: acceptedAttempt.payoutAttemptRef,
                payoutIntentRef: acceptedIntent.payoutIntentRef,
                receiptRef: 'receipt.nexus.public.accepted_work',
              }),
            ],
          }),
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
          treasuryPayoutStore: treasuryPayoutStore([
            treasuryPayoutTransaction({
              amountSat: 50_000,
              id: 'treasury_payout_public_recognition',
            }),
            treasuryPayoutTransaction({
              amountSat: 9_999,
              createdAt: '2026-06-06T13:57:00.000Z',
              id: 'treasury_payout_public_old',
              settledAt: '2026-06-06T13:58:00.000Z',
            }),
            treasuryPayoutTransaction({
              amountSat: 123,
              id: 'treasury_payout_public_pending',
              settledAt: null,
              state: 'pending',
            }),
          ]),
        },
      ),
    )
    const stats = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(stats).toMatchObject({
      nexusAcceptedWorkPayoutSatsPaid24h: 21,
      publicRealSatsSettled24h: 50_002,
      publicRealSatsSettledTotal: 60_001,
      treasuryPayoutCount24h: 1,
      treasuryPayoutCountTotal: 2,
      treasuryPayoutSatsPaid24h: 50_000,
      treasuryPayoutSatsPaidTotal: 59_999,
    })
    expect((stats.caveatRefs as ReadonlyArray<string>)).toContain(
      'caveat.public.accepted_work_not_added_on_top_of_treasury_outflows_to_avoid_double_count',
    )
    expect(JSON.stringify(stats)).not.toMatch(
      /lnbc|bolt11|invoice|preimage|payment_hash|wallet_secret|mnemonic|private_key/,
    )
  })

  test('projects training contributors from the live run authority store', async () => {
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          store: storeFor([registration({ pylonRef: 'pylon.public.ready' })]),
          trainingStore: trainingContributorStore([
            'pylon.public.training.alpha',
            'pylon.public.training.alpha',
            'pylon.public.training.beta',
          ]),
        },
      ),
    )
    const stats = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(stats).toMatchObject({
      trainingAcceptedContributors: 0,
      trainingAssignedContributors: 2,
      trainingModelProgressContributors: 2,
    })
    expect((stats.caveatRefs as ReadonlyArray<string>)).toContain(
      'caveat.public.training_contributors_are_live_run_contributor_refs_not_stale_registrations',
    )
  })

  test('serves no-store public stats from the injected Omega store', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch')
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          store: storeFor([
            registration({ pylonRef: 'pylon.public.route_online' }),
          ]),
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(fetchSpy).not.toHaveBeenCalled()
    await expect(response.json()).resolves.toMatchObject({
      available: true,
      pylonsAssignmentReadyNow: 1,
      pylonsOnlineNow: 1,
      sourceUrl: PUBLIC_PYLON_STATS_URL,
    })
  })

  test('marks Omega store failures unavailable', async () => {
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats'),
        {
          nowUnixMs: () => nowUnixMs,
          store: {
            listRegistrations: () =>
              Promise.reject(new Error('D1 unavailable')),
          },
        },
      ),
    )

    await expect(response.json()).resolves.toMatchObject({
      available: false,
      earningLaunchGate: {
        blockerRefs: [
          'blocker.public.pylon.stats_unavailable',
          'blocker.public.pylon.online_now_zero',
          'blocker.public.pylon.wallet_ready_now_zero',
          'blocker.public.pylon.assignment_ready_now_zero',
        ],
        publicEarningCopyAllowed: false,
        state: 'blocked',
      },
      error: 'D1 unavailable',
      pylonsOnlineNow: 0,
      sourceUrl: PUBLIC_PYLON_STATS_URL,
      status: 'unavailable',
    })
  })

  test('rejects public pylon stat mutations', async () => {
    const response = await Effect.runPromise(
      handlePublicPylonStatsApi(
        new Request('https://openagents.com/api/public/pylon-stats', {
          method: 'POST',
        }),
        { store: storeFor([]) },
      ),
    )

    expect(response.status).toBe(405)
  })
})
