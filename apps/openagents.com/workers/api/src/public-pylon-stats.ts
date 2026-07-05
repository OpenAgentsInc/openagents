import { Effect, Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  arrayFromUnknown,
  isRecord,
  optionalInteger,
  optionalString,
  parseJsonUnknown,
  stringArrayFromUnknown,
} from './json-boundary'
import { nexusPylonPublicReceiptDetailFromLedger } from './nexus-pylon-visibility'
import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type Nip90MarketReceiptStore,
  type Nip90MarketStreamKind,
  type PublicNip90MarketSettlementReceipt,
  publicNip90MarketReceiptFromRecord,
} from './nip90-market-receipts'
import { publicScannerSafeRefs } from './public-ref-scanner-safety'
import {
  type PylonApiRegistrationRecord,
  pylonApiStoreErrorFromUnknown,
  pylonClientVersionMeetsMinimum,
} from './pylon-api'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'
import {
  PublicProjectionStalenessContract,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'
import type { TrainingAuthorityStore } from './training-run-window-authority'
import { publicTrainingRunSummary } from './training-run-window-authority'
import type { TreasuryTransactionStore } from './treasury-page-routes'

export const PUBLIC_NEXUS_STATS_URL = 'https://nexus.openagents.com/api/stats'
export const PUBLIC_PYLON_STATS_URL =
  'https://openagents.com/api/public/pylon-stats'
export const PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION = '0.2.5'
export const PUBLIC_PYLON_STATS_STALENESS = rebuiltOnTransitionStaleness(4, [
  'pylon_registry_registration_changed',
  'pylon_heartbeat_recorded',
  'pylon_wallet_readiness_changed',
  'pylon_assignment_lifecycle_changed',
  'nexus_treasury_payout_ledger_changed',
  'nip90_market_receipt_settled',
  'treasury_transaction_changed',
  'training_window_lifecycle_changed',
])

const ONLINE_WINDOW_MS = 5 * 60 * 1000
const SEEN_24H_WINDOW_MS = 24 * 60 * 60 * 1000
const MAX_RECENT_PYLONS = 12
const PUBLIC_PYLON_EARNING_LAUNCH_GATE_REF =
  'gate.public.pylon.earning_network_counters.v1'
const PUBLIC_PYLON_SETTLEMENT_TOTALS_GATE_REF =
  'gate.public.pylon.accepted_work_settlement_receipts.v1'
const OPENAGENTS_PUBLIC_APP_URL = 'https://openagents.com'
const DEFAULT_STATS_TRAINING_RUN_REF = 'run.tassadar.executor.20260615'
const onlineHeartbeatStatuses = new Set([
  'available',
  'healthy',
  'idle',
  'online',
  'ready',
])

export type PublicPylonStatsStore = Readonly<{
  listRegistrations: (
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiRegistrationRecord>>
}>

export type PublicPylonSettlementReceiptStore = Readonly<{
  listPaymentAuthorityReceipts: (
    limit: number,
  ) => Promise<ReadonlyArray<NexusPaymentAuthorityReceiptRecord>>
  readPayoutAttemptByRef: (
    payoutAttemptRef: string,
  ) => Promise<NexusTreasuryPayoutAttemptRecord | undefined>
  readPayoutIntentByRef: (
    payoutIntentRef: string,
  ) => Promise<NexusTreasuryPayoutIntentRecord | undefined>
  readReconciliationEventByRef: (
    eventRef: string,
  ) => Promise<NexusTreasuryPayoutReconciliationEventRecord | undefined>
}>

export type PublicTreasuryPayoutStatsStore = Pick<
  TreasuryTransactionStore,
  'listRecent'
>
export type PublicTrainingContributorStatsStore = Pick<
  TrainingAuthorityStore,
  | 'listVerificationChallengesForRun'
  | 'listWindowLeasesForRun'
  | 'listWindowsForRun'
  | 'readRun'
>

export class PublicRecentPylon extends S.Class<PublicRecentPylon>(
  'PublicRecentPylon',
)({
  pylonRef: S.NullOr(S.String),
  ownerAgentRef: S.NullOr(S.String),
  nodeLabel: S.NullOr(S.String),
  nostrPubkeyShort: S.String,
  clientVersion: S.NullOr(S.String),
  readyModel: S.NullOr(S.String),
  runtimeState: S.NullOr(S.String),
  lastSeenAtUnixMs: S.NullOr(S.Int),
  lastSeenAtLabel: S.NullOr(S.String),
  lastHeartbeatAgeSeconds: S.NullOr(S.Int),
  onlineNow: S.NullOr(S.Boolean),
  walletReadyNow: S.NullOr(S.Boolean),
  assignmentReadyNow: S.NullOr(S.Boolean),
  cumulativeSettledSats: S.Int,
  tippingAvailable: S.NullOr(S.Boolean),
  tipEndpoint: S.NullOr(S.String),
  eligibleProductCount: S.Int,
  relayUrls: S.Array(S.String),
  products: S.Array(S.String),
}) {}

export class PublicPylonStatsCounterWindows extends S.Class<PublicPylonStatsCounterWindows>(
  'PublicPylonStatsCounterWindows',
)({
  onlineNowWindowMinutes: S.Int,
  walletReadyNowWindowMinutes: S.Int,
  assignmentReadyNowWindowMinutes: S.Int,
  seen24hWindowMinutes: S.Int,
  recentPylonsWindowMinutes: S.Int,
  recentPylonsLimit: S.Int,
  onlineHeartbeatStatuses: S.Array(S.String),
  definitionRefs: S.Array(S.String),
}) {}

export class PublicPylonEarningLaunchGate extends S.Class<PublicPylonEarningLaunchGate>(
  'PublicPylonEarningLaunchGate',
)({
  gateRef: S.String,
  state: S.Literals(['blocked', 'ready']),
  stateLabel: S.String,
  publicEarningCopyAllowed: S.Boolean,
  requiredOnlinePylonsPresent: S.Boolean,
  requiredWalletReadyPylonsPresent: S.Boolean,
  requiredAssignmentReadyPylonsPresent: S.Boolean,
  blockerRefs: S.Array(S.String),
  blockedClaimRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
}) {}

export class PublicPylonAcceptedWorkSettlementGate extends S.Class<PublicPylonAcceptedWorkSettlementGate>(
  'PublicPylonAcceptedWorkSettlementGate',
)({
  gateRef: S.String,
  state: S.Literals(['blocked', 'ready', 'unavailable']),
  stateLabel: S.String,
  publicPaidWorkTotalsAllowed: S.Boolean,
  receiptBackedTotalsAvailable: S.Boolean,
  settledReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
}) {}

export class PublicNip90MarketStreamStats extends S.Class<PublicNip90MarketStreamStats>(
  'PublicNip90MarketStreamStats',
)({
  jobsSettled24h: S.Int,
  jobsSettledTotal: S.Int,
  receiptRefs: S.Array(S.String),
  satsSettled24h: S.Number,
  satsSettledTotal: S.Number,
  streamKind: S.Literals(['compute', 'data', 'labor']),
}) {}

export class PublicNip90MarketSettlementStats extends S.Class<PublicNip90MarketSettlementStats>(
  'PublicNip90MarketSettlementStats',
)({
  available: S.Boolean,
  caveatRefs: S.Array(S.String),
  compute: PublicNip90MarketStreamStats,
  data: PublicNip90MarketStreamStats,
  error: S.NullOr(S.String),
  labor: PublicNip90MarketStreamStats,
  sourceRefs: S.Array(S.String),
}) {}

export class PublicPylonStats extends S.Class<PublicPylonStats>(
  'PublicPylonStats',
)({
  available: S.Boolean,
  status: S.Literals(['live', 'unavailable']),
  error: S.NullOr(S.String),
  sourceUrl: S.String,
  hostedNexusRelayUrl: S.NullOr(S.String),
  generatedAtUnixMs: S.Int,
  asOfUnixMs: S.NullOr(S.Int),
  asOfLabel: S.NullOr(S.String),
  staleness: PublicProjectionStalenessContract,
  minimumClientVersion: S.String,
  pylonsOnlineNow: S.Int,
  pylonsSeen24h: S.Int,
  pylonsRegisteredTotal: S.Int,
  pylonsWalletReadyNow: S.Int,
  pylonsAssignmentReadyNow: S.Int,
  pylonsByResourceMode: S.Record(S.String, S.Int),
  pylonsByClientVersion: S.Record(S.String, S.Int),
  pylonSessionsOnlineNow: S.Int,
  sellablePylonsOnlineNow: S.Int,
  nexusPayoutSatsPaidTotal: S.NullOr(S.Int),
  nexusAcceptedWorkPayoutSatsPaidTotal: S.NullOr(S.Int),
  nexusAcceptedWorkPayoutSatsPaid24h: S.NullOr(S.Int),
  nexusAcceptedWorkPayoutReceiptRefs: S.Array(S.String),
  nexusAcceptedWorkSettlementGate: PublicPylonAcceptedWorkSettlementGate,
  nip90MarketSettlementStats: PublicNip90MarketSettlementStats,
  treasuryPayoutSatsPaidTotal: S.NullOr(S.Int),
  treasuryPayoutSatsPaid24h: S.NullOr(S.Int),
  treasuryPayoutCountTotal: S.NullOr(S.Int),
  treasuryPayoutCount24h: S.NullOr(S.Int),
  publicRealSatsSettledTotal: S.NullOr(S.Int),
  publicRealSatsSettled24h: S.NullOr(S.Int),
  trainingAssignedContributors: S.Int,
  trainingAcceptedContributors: S.Int,
  trainingModelProgressContributors: S.Int,
  counterWindows: PublicPylonStatsCounterWindows,
  recentPylons: S.Array(PublicRecentPylon),
  earningLaunchGate: PublicPylonEarningLaunchGate,
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
}) {}

export const publicPylonStatsCounterWindows =
  (): PublicPylonStatsCounterWindows =>
    new PublicPylonStatsCounterWindows({
      assignmentReadyNowWindowMinutes: ONLINE_WINDOW_MS / 60_000,
      definitionRefs: [
        'definition.public.pylon_stats.online_now.v1',
        'definition.public.pylon_stats.wallet_ready_now.v1',
        'definition.public.pylon_stats.assignment_ready_now.v1',
        'definition.public.pylon_stats.recent_pylons.v1',
        'definition.public.pylon_stats.runtime_state_is_last_reported_not_live.v1',
      ],
      onlineHeartbeatStatuses: [...onlineHeartbeatStatuses].sort(),
      onlineNowWindowMinutes: ONLINE_WINDOW_MS / 60_000,
      recentPylonsLimit: MAX_RECENT_PYLONS,
      recentPylonsWindowMinutes: SEEN_24H_WINDOW_MS / 60_000,
      seen24hWindowMinutes: SEEN_24H_WINDOW_MS / 60_000,
      walletReadyNowWindowMinutes: ONLINE_WINDOW_MS / 60_000,
    })

class PublicPylonStatsSnapshotError extends S.TaggedErrorClass<PublicPylonStatsSnapshotError>()(
  'PublicPylonStatsSnapshotError',
  {
    reason: S.String,
  },
) {}

const intValue = (value: unknown): number => optionalInteger(value) ?? 0

const nullableInt = (value: unknown): number | null =>
  optionalInteger(value) ?? null

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const fallbackNullableInt = (
  value: unknown,
  fallback: number | null,
): number | null => nullableInt(value) ?? fallback

const timestampLabel = (unixMs: number | null): string | null =>
  unixMs === null ? null : epochMillisToIsoTimestamp(unixMs)

const friendlyTimestampLabel = (
  unixMs: number | null,
  nowUnixMs: number,
): string | null =>
  unixMs === null
    ? null
    : friendlyBlueprintMissionBriefingTime(
        epochMillisToIsoTimestamp(unixMs),
        epochMillisToIsoTimestamp(nowUnixMs),
      )

const recentPylonFromUnknown = (value: unknown): PublicRecentPylon | null => {
  if (!isRecord(value)) {
    return null
  }

  const lastSeenAtUnixMs = nullableInt(value.last_seen_at_unix_ms)

  return new PublicRecentPylon({
    pylonRef: optionalString(value.pylon_ref) ?? null,
    ownerAgentRef: optionalString(value.owner_agent_ref) ?? null,
    nodeLabel: optionalString(value.node_label) ?? null,
    nostrPubkeyShort: optionalString(value.nostr_pubkey_short) ?? 'unknown',
    clientVersion: optionalString(value.client_version) ?? null,
    readyModel: optionalString(value.ready_model) ?? null,
    runtimeState: optionalString(value.runtime_state) ?? null,
    lastSeenAtUnixMs,
    lastSeenAtLabel: timestampLabel(lastSeenAtUnixMs),
    // Legacy Nexus payloads carry no eligibility evidence, so the
    // counter-reconciliation fields stay null instead of guessing.
    lastHeartbeatAgeSeconds: null,
    onlineNow: null,
    walletReadyNow: null,
    assignmentReadyNow: null,
    cumulativeSettledSats: intValue(
      value.cumulative_settled_sats ?? value.cumulativeSettledSats,
    ),
    tippingAvailable: null,
    tipEndpoint: null,
    eligibleProductCount: intValue(value.eligible_product_count),
    relayUrls: stringArrayFromUnknown(value.relay_urls),
    products: stringArrayFromUnknown(value.products),
  })
}

const recentPylonsFromUnknown = (
  value: unknown,
): ReadonlyArray<PublicRecentPylon> =>
  (arrayFromUnknown(value) ?? [])
    .map(recentPylonFromUnknown)
    .filter((row): row is PublicRecentPylon => row !== null)

const incrementCount = (
  counts: Record<string, number>,
  key: string | null | undefined,
): void => {
  const normalized = key?.trim()

  if (normalized === undefined || normalized === '') {
    return
  }

  counts[normalized] = (counts[normalized] ?? 0) + 1
}

const pylonHeartbeatUnixMs = (
  registration: PylonApiRegistrationRecord,
): number | null => {
  if (registration.latestHeartbeatAt === null) {
    return null
  }

  const value = Date.parse(registration.latestHeartbeatAt)

  return Number.isFinite(value) ? value : null
}

const isVersionEligible = (registration: PylonApiRegistrationRecord): boolean =>
  pylonClientVersionMeetsMinimum(
    registration.clientVersion,
    PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
  )

const isActiveEligibleRegistration = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  registration.status === 'active' && isVersionEligible(registration)

const hasOnlineHeartbeatStatus = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  onlineHeartbeatStatuses.has(
    (registration.latestHeartbeatStatus ?? '').trim().toLowerCase(),
  )

const isHeartbeatWithin = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
  windowMs: number,
): boolean => {
  const heartbeatUnixMs = pylonHeartbeatUnixMs(registration)

  return heartbeatUnixMs !== null && nowUnixMs - heartbeatUnixMs <= windowMs
}

const isOnlineNow = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
): boolean =>
  isActiveEligibleRegistration(registration) &&
  hasOnlineHeartbeatStatus(registration) &&
  isHeartbeatWithin(registration, nowUnixMs, ONLINE_WINDOW_MS)

const isSeen24h = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
): boolean =>
  isActiveEligibleRegistration(registration) &&
  isHeartbeatWithin(registration, nowUnixMs, SEEN_24H_WINDOW_MS)

const hasAssignmentReadinessEvidence = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  registration.capabilityRefs.length > 0 &&
  registration.latestCapacityRefs.length > 0 &&
  registration.latestHealthRefs.length > 0 &&
  registration.latestLoadRefs.length > 0

const isAssignmentReady = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
): boolean =>
  isOnlineNow(registration, nowUnixMs) &&
  registration.walletReady &&
  hasAssignmentReadinessEvidence(registration) &&
  registration.status !== 'blocked'

const publicPylonEarningLaunchGate = (input: {
  available: boolean
  pylonsOnlineNow: number
  pylonsWalletReadyNow: number
  pylonsAssignmentReadyNow: number
  sourceRefs: ReadonlyArray<string>
}): PublicPylonEarningLaunchGate => {
  const requiredOnlinePylonsPresent =
    input.available && input.pylonsOnlineNow > 0
  const requiredWalletReadyPylonsPresent =
    input.available && input.pylonsWalletReadyNow > 0
  const requiredAssignmentReadyPylonsPresent =
    input.available && input.pylonsAssignmentReadyNow > 0
  const blockerRefs = [
    ...(input.available ? [] : ['blocker.public.pylon.stats_unavailable']),
    ...(requiredOnlinePylonsPresent
      ? []
      : ['blocker.public.pylon.online_now_zero']),
    ...(requiredWalletReadyPylonsPresent
      ? []
      : ['blocker.public.pylon.wallet_ready_now_zero']),
    ...(requiredAssignmentReadyPylonsPresent
      ? []
      : ['blocker.public.pylon.assignment_ready_now_zero']),
  ]
  const publicEarningCopyAllowed = blockerRefs.length === 0

  return new PublicPylonEarningLaunchGate({
    blockerRefs,
    blockedClaimRefs: publicEarningCopyAllowed
      ? []
      : [
          'blocked_claim.public.pylon.automatic_bitcoin_earning',
          'blocked_claim.public.pylon.self_serve_paid_work',
          'blocked_claim.public.pylon.assignment_ready_payouts',
        ],
    caveatRefs: [
      'caveat.public.pylon_online_is_not_paid_work',
      'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
      'caveat.public.assignment_ready_is_not_acceptance_or_settlement',
      'caveat.public.no_unconditional_earning_promise',
    ],
    gateRef: PUBLIC_PYLON_EARNING_LAUNCH_GATE_REF,
    publicEarningCopyAllowed,
    requiredAssignmentReadyPylonsPresent,
    requiredOnlinePylonsPresent,
    requiredWalletReadyPylonsPresent,
    sourceRefs: [
      ...new Set(['route:/api/public/pylon-stats', ...input.sourceRefs]),
    ],
    state: publicEarningCopyAllowed ? 'ready' : 'blocked',
    stateLabel: publicEarningCopyAllowed
      ? 'Ready for bounded public earning copy'
      : 'Blocked before public earning copy',
  })
}

type PublicPylonSettlementTotals = Readonly<{
  available: boolean
  error: string | null
  receiptRefs: ReadonlyArray<string>
  satsPaid24h: number | null
  satsPaidTotal: number | null
  sourceRefs: ReadonlyArray<string>
}>

type PublicNip90MarketSettlementTotals = PublicNip90MarketSettlementStats

type PublicTreasuryPayoutTotals = Readonly<{
  available: boolean
  error: string | null
  payoutCount24h: number | null
  payoutCountTotal: number | null
  satsPaid24h: number | null
  satsPaidTotal: number | null
  sourceRefs: ReadonlyArray<string>
}>

type PublicTrainingContributorTotals = Readonly<{
  acceptedContributors: number
  assignedContributors: number
  available: boolean
  error: string | null
  modelProgressContributors: number
  sourceRefs: ReadonlyArray<string>
}>

const emptyUnavailableSettlementTotals = (
  error: string,
): PublicPylonSettlementTotals => ({
  available: false,
  error,
  receiptRefs: [],
  satsPaid24h: null,
  satsPaidTotal: null,
  sourceRefs: ['route:/api/public/pylon-stats'],
})

const emptyMarketStreamStats = (
  streamKind: Nip90MarketStreamKind,
): PublicNip90MarketStreamStats =>
  new PublicNip90MarketStreamStats({
    jobsSettled24h: 0,
    jobsSettledTotal: 0,
    receiptRefs: [],
    satsSettled24h: 0,
    satsSettledTotal: 0,
    streamKind,
  })

export const emptyUnavailableMarketSettlementTotals = (
  error: string,
): PublicNip90MarketSettlementTotals =>
  new PublicNip90MarketSettlementStats({
    available: false,
    caveatRefs: [
      'caveat.public.nip90_market.settled_receipts_only',
      'caveat.public.no_private_settlement_material',
    ],
    compute: emptyMarketStreamStats('compute'),
    data: emptyMarketStreamStats('data'),
    error,
    labor: emptyMarketStreamStats('labor'),
    sourceRefs: ['route:/api/public/pylon-stats'],
  })

const emptyAvailableMarketSettlementTotals =
  (): PublicNip90MarketSettlementTotals =>
    new PublicNip90MarketSettlementStats({
      available: true,
      caveatRefs: [
        'caveat.public.nip90_market.settled_receipts_only',
        'caveat.public.nip90_market.pending_records_excluded',
        'caveat.public.no_private_settlement_material',
      ],
      compute: emptyMarketStreamStats('compute'),
      data: emptyMarketStreamStats('data'),
      error: null,
      labor: emptyMarketStreamStats('labor'),
      sourceRefs: [
        'route:/api/public/pylon-stats',
        'route:/api/public/nip90-market/receipts/{receiptRef}',
      ],
    })

const emptyAvailableSettlementTotals = (): PublicPylonSettlementTotals => ({
  available: true,
  error: null,
  receiptRefs: [],
  satsPaid24h: 0,
  satsPaidTotal: 0,
  sourceRefs: [
    'route:/api/public/pylon-stats',
    'route:/api/public/nexus-pylon/receipts/{receiptRef}',
    'nexus.public.accepted_work_settlement_receipts',
  ],
})

const emptyUnavailableTreasuryPayoutTotals = (
  error: string,
): PublicTreasuryPayoutTotals => ({
  available: false,
  error,
  payoutCount24h: null,
  payoutCountTotal: null,
  satsPaid24h: null,
  satsPaidTotal: null,
  sourceRefs: ['route:/api/public/pylon-stats'],
})

const emptyAvailableTreasuryPayoutTotals =
  (): PublicTreasuryPayoutTotals => ({
    available: true,
    error: null,
    payoutCount24h: 0,
    payoutCountTotal: 0,
    satsPaid24h: 0,
    satsPaidTotal: 0,
    sourceRefs: [
      'route:/api/public/pylon-stats',
      'openagents.public.treasury_transactions.outbound_settled',
    ],
  })

const emptyUnavailableTrainingContributorTotals = (
  error: string,
): PublicTrainingContributorTotals => ({
  acceptedContributors: 0,
  assignedContributors: 0,
  available: false,
  error,
  modelProgressContributors: 0,
  sourceRefs: ['route:/api/public/pylon-stats'],
})

const emptyAvailableTrainingContributorTotals =
  (): PublicTrainingContributorTotals => ({
    acceptedContributors: 0,
    assignedContributors: 0,
    available: true,
    error: null,
    modelProgressContributors: 0,
    sourceRefs: [
      'route:/api/public/pylon-stats',
      `route:/api/public/training/runs/${DEFAULT_STATS_TRAINING_RUN_REF}`,
    ],
  })

const knownSum = (
  values: ReadonlyArray<number | null>,
): number | null => {
  const known = values.filter((value): value is number => value !== null)

  return known.length === 0
    ? null
    : known.reduce((total, value) => total + value, 0)
}

const marketSats24h = (
  totals: PublicNip90MarketSettlementTotals,
): number | null =>
  totals.available
    ? Math.floor(
        totals.compute.satsSettled24h +
          totals.data.satsSettled24h +
          totals.labor.satsSettled24h,
      )
    : null

const marketSatsTotal = (
  totals: PublicNip90MarketSettlementTotals,
): number | null =>
  totals.available
    ? Math.floor(
        totals.compute.satsSettledTotal +
          totals.data.satsSettledTotal +
          totals.labor.satsSettledTotal,
      )
    : null

const publicRealSatsSettled = (input: {
  acceptedWorkSats: number | null
  marketSats: number | null
  treasuryOutflowSats: number | null
}): number | null => {
  const localTreasuryOrAccepted =
    input.treasuryOutflowSats === null
      ? input.acceptedWorkSats
      : input.acceptedWorkSats === null
        ? input.treasuryOutflowSats
        : Math.max(input.treasuryOutflowSats, input.acceptedWorkSats)

  return knownSum([localTreasuryOrAccepted, input.marketSats])
}

const publicPylonAcceptedWorkSettlementGate = (
  totals: PublicPylonSettlementTotals,
): PublicPylonAcceptedWorkSettlementGate => {
  const hasSettledReceipts = totals.receiptRefs.length > 0
  const blockerRefs = totals.available
    ? hasSettledReceipts
      ? []
      : ['blocker.public.pylon_settlement.settled_receipts_zero']
    : ['blocker.public.pylon_settlement.receipts_unavailable']
  const state = !totals.available
    ? 'unavailable'
    : hasSettledReceipts
      ? 'ready'
      : 'blocked'

  return new PublicPylonAcceptedWorkSettlementGate({
    blockerRefs,
    caveatRefs: [
      'caveat.public.pylon_settlement.simulation_receipts_do_not_count',
      'caveat.public.pylon_settlement.payment_receipt_without_settlement_does_not_count',
      'caveat.public.pylon_settlement.duplicate_retries_count_once',
      'caveat.public.no_private_payment_material',
    ],
    gateRef: PUBLIC_PYLON_SETTLEMENT_TOTALS_GATE_REF,
    publicPaidWorkTotalsAllowed: state === 'ready',
    receiptBackedTotalsAvailable: totals.available,
    settledReceiptRefs: [...totals.receiptRefs],
    sourceRefs: uniqueRefs([
      PUBLIC_PYLON_SETTLEMENT_TOTALS_GATE_REF,
      ...totals.sourceRefs,
      ...totals.receiptRefs.map(
        receiptRef => `route:/api/public/nexus-pylon/receipts/${receiptRef}`,
      ),
    ]),
    state,
    stateLabel:
      state === 'ready'
        ? 'Receipt-backed accepted-work settlement totals ready'
        : state === 'blocked'
          ? 'No settled accepted-work bitcoin receipts counted yet'
          : `Accepted-work settlement totals unavailable${
              totals.error === null ? '' : `: ${totals.error}`
            }`,
  })
}

const satsFromBitcoinMillisats = (
  amount: NexusTreasuryPayoutIntentRecord['amount'],
): number | null =>
  amount.asset === 'bitcoin' &&
  amount.denomination === 'bitcoin_millisatoshi' &&
  Number.isInteger(amount.amountMinorUnits) &&
  amount.amountMinorUnits >= 0 &&
  amount.amountMinorUnits % 1000 === 0
    ? amount.amountMinorUnits / 1000
    : null

const settledWithin24h = (createdAt: string, nowUnixMs: number): boolean => {
  const createdMs = Date.parse(createdAt)

  return (
    Number.isFinite(createdMs) &&
    nowUnixMs - createdMs >= 0 &&
    nowUnixMs - createdMs <= SEEN_24H_WINDOW_MS
  )
}

const publicTreasuryPayoutTotalsFromTransactions = async (
  input: Readonly<{
    nowUnixMs: number
    treasuryPayoutStore: PublicTreasuryPayoutStatsStore
  }>,
): Promise<PublicTreasuryPayoutTotals> => {
  const transactions = await input.treasuryPayoutStore.listRecent(1000)
  const settledOut = transactions.filter(
    transaction =>
      transaction.direction === 'out' &&
      transaction.state === 'settled' &&
      Number.isInteger(transaction.amountSat) &&
      transaction.amountSat > 0,
  )
  const settledOut24h = settledOut.filter(transaction =>
    settledWithin24h(
      transaction.settledAt ?? transaction.createdAt,
      input.nowUnixMs,
    ),
  )

  return {
    ...emptyAvailableTreasuryPayoutTotals(),
    payoutCount24h: settledOut24h.length,
    payoutCountTotal: settledOut.length,
    satsPaid24h: settledOut24h.reduce(
      (total, transaction) => total + transaction.amountSat,
      0,
    ),
    satsPaidTotal: settledOut.reduce(
      (total, transaction) => total + transaction.amountSat,
      0,
    ),
  }
}

const publicTrainingContributorTotalsFromStore = async (
  input: Readonly<{
    nowUnixMs: number
    store: PublicTrainingContributorStatsStore
    trainingRunRef?: string
  }>,
): Promise<PublicTrainingContributorTotals> => {
  const trainingRunRef = input.trainingRunRef ?? DEFAULT_STATS_TRAINING_RUN_REF
  const nowIso = epochMillisToIsoTimestamp(input.nowUnixMs)
  const run = await input.store.readRun(trainingRunRef)

  if (run === undefined) {
    return emptyAvailableTrainingContributorTotals()
  }

  const [windows, leases, challenges] = await Promise.all([
    input.store.listWindowsForRun(trainingRunRef, 100),
    input.store.listWindowLeasesForRun(trainingRunRef, 1000),
    input.store.listVerificationChallengesForRun(trainingRunRef, 1000),
  ])
  const summary = publicTrainingRunSummary({
    challenges,
    leases,
    nowIso,
    run,
    windows,
  })
  const assignedContributors =
    summary.metrics.assignedContributorCount.value
  const acceptedContributors =
    summary.metrics.qualifiedContributorCount.value
  const modelProgressContributors = Math.max(
    assignedContributors,
    acceptedContributors,
    summary.realGradient.deviceRequirement.observedDistinctContributorDevices,
    summary.realGradient.leaderboardRows.length,
  )

  return {
    acceptedContributors,
    assignedContributors,
    available: true,
    error: null,
    modelProgressContributors,
    sourceRefs: uniqueRefs([
      'route:/api/public/pylon-stats',
      `route:/api/public/training/runs/${trainingRunRef}`,
      ...summary.sourceRefs,
      ...summary.metrics.assignedContributorCount.sourceRefs,
      ...summary.metrics.qualifiedContributorCount.sourceRefs,
      ...summary.realGradient.deviceRequirement.sourceRefs,
    ]),
  }
}

const marketStreamStatsFromReceipts = (
  input: Readonly<{
    nowUnixMs: number
    receipts: ReadonlyArray<PublicNip90MarketSettlementReceipt>
    streamKind: Nip90MarketStreamKind
  }>,
): PublicNip90MarketStreamStats => {
  const streamReceipts = input.receipts.filter(
    receipt => receipt.streamKind === input.streamKind,
  )
  const streamReceipts24h = streamReceipts.filter(receipt =>
    settledWithin24h(receipt.settledAt, input.nowUnixMs),
  )

  return new PublicNip90MarketStreamStats({
    jobsSettled24h: streamReceipts24h.length,
    jobsSettledTotal: streamReceipts.length,
    receiptRefs: uniqueRefs(streamReceipts.map(receipt => receipt.receiptRef)),
    satsSettled24h: streamReceipts24h.reduce(
      (total, receipt) => total + receipt.amountSats,
      0,
    ),
    satsSettledTotal: streamReceipts.reduce(
      (total, receipt) => total + receipt.amountSats,
      0,
    ),
    streamKind: input.streamKind,
  })
}

const publicNip90MarketSettlementTotalsFromReceipts = async (
  input: Readonly<{
    marketReceiptStore: Nip90MarketReceiptStore
    nowUnixMs: number
  }>,
): Promise<PublicNip90MarketSettlementTotals> => {
  const receipts = (
    await input.marketReceiptStore.listSettledMarketReceipts(1000)
  )
    .map(record =>
      publicNip90MarketReceiptFromRecord(
        record,
        epochMillisToIsoTimestamp(input.nowUnixMs),
      ),
    )
    .filter(
      (receipt): receipt is PublicNip90MarketSettlementReceipt =>
        receipt !== null,
    )
  const base = emptyAvailableMarketSettlementTotals()

  return new PublicNip90MarketSettlementStats({
    available: true,
    caveatRefs: base.caveatRefs,
    compute: marketStreamStatsFromReceipts({
      nowUnixMs: input.nowUnixMs,
      receipts,
      streamKind: 'compute',
    }),
    data: marketStreamStatsFromReceipts({
      nowUnixMs: input.nowUnixMs,
      receipts,
      streamKind: 'data',
    }),
    error: null,
    labor: marketStreamStatsFromReceipts({
      nowUnixMs: input.nowUnixMs,
      receipts,
      streamKind: 'labor',
    }),
    sourceRefs: uniqueRefs([
      ...base.sourceRefs,
      ...receipts.map(receipt => receipt.receiptRef),
      ...receipts.map(
        receipt =>
          `route:/api/public/nip90-market/receipts/${receipt.receiptRef}`,
      ),
    ]),
  })
}

const receiptPublicProjectionIsRealBitcoin = (
  receipt: NexusPaymentAuthorityReceiptRecord,
): boolean => {
  try {
    const projection = parseJsonUnknown(receipt.publicProjectionJson)

    return isRecord(projection) && projection.moneyMovement === 'real_bitcoin'
  } catch {
    return false
  }
}

const publicPylonSettlementTotalsFromReceipts = async (
  input: Readonly<{
    nowUnixMs: number
    receiptStore: PublicPylonSettlementReceiptStore
  }>,
): Promise<PublicPylonSettlementTotals> => {
  const nowIso = epochMillisToIsoTimestamp(input.nowUnixMs)
  const receipts = await input.receiptStore.listPaymentAuthorityReceipts(1000)
  const settlementReceipts = receipts
    .filter(receipt => receipt.receiptKind === 'settlement_recorded')
    .sort((left, right) => left.createdAt.localeCompare(right.createdAt))

  // #5050 perf: the old reduce did up to 3 sequential D1 reads PER receipt (a
  // serial N+1 that made /api/public/pylon-stats take ~5s). Resolve every
  // receipt's intent/event/attempt in PARALLEL instead, memoizing intent reads
  // by ref (multiple receipts share an intent). Then accumulate in sorted order
  // with the exact same first-pass-per-intent semantics.
  const intentReads = new Map<
    string,
    ReturnType<PublicPylonSettlementReceiptStore['readPayoutIntentByRef']>
  >()
  const readIntent = (ref: string) => {
    const existing = intentReads.get(ref)
    if (existing !== undefined) return existing
    const pending = input.receiptStore.readPayoutIntentByRef(ref)
    intentReads.set(ref, pending)
    return pending
  }
  const resolved = await Promise.all(
    settlementReceipts.map(async receipt => {
      const [intent, event, attempt] = await Promise.all([
        readIntent(receipt.payoutIntentRef),
        receipt.eventRef === null
          ? Promise.resolve(undefined)
          : input.receiptStore.readReconciliationEventByRef(receipt.eventRef),
        receipt.payoutAttemptRef === null
          ? Promise.resolve(undefined)
          : input.receiptStore.readPayoutAttemptByRef(receipt.payoutAttemptRef),
      ])
      return { attempt, event, intent, receipt }
    }),
  )

  const countedByIntent = new Map<
    string,
    Readonly<{ receiptRef: string; sats: number; settledAt: string }>
  >()
  for (const { attempt, event, intent, receipt } of resolved) {
    if (countedByIntent.has(receipt.payoutIntentRef)) continue
    if (intent === undefined || intent.acceptedWorkRefs.length === 0) continue

    const detail = nexusPylonPublicReceiptDetailFromLedger({
      appUrl: OPENAGENTS_PUBLIC_APP_URL,
      attempt,
      event,
      intent,
      nowIso,
      receipt,
    })
    const sats = satsFromBitcoinMillisats(intent.amount)

    if (
      sats === null ||
      !receiptPublicProjectionIsRealBitcoin(receipt) ||
      !detail.realBitcoinMoved ||
      detail.receiptKind !== 'settlement_recorded' ||
      detail.settlement.state !== 'settled' ||
      !detail.payoutMovement.terminalSettlementClaimAllowed
    ) {
      continue
    }

    countedByIntent.set(receipt.payoutIntentRef, {
      receiptRef: receipt.receiptRef,
      sats,
      settledAt: receipt.createdAt,
    })
  }

  const counted = [...countedByIntent.values()]
  const receiptRefs = uniqueRefs(counted.map(item => item.receiptRef))

  return {
    available: true,
    error: null,
    receiptRefs,
    satsPaid24h: counted
      .filter(item => settledWithin24h(item.settledAt, input.nowUnixMs))
      .reduce((total, item) => total + item.sats, 0),
    satsPaidTotal: counted.reduce((total, item) => total + item.sats, 0),
    sourceRefs: uniqueRefs([
      ...emptyAvailableSettlementTotals().sourceRefs,
      ...receiptRefs,
    ]),
  }
}

const recentPylonFromRegistration = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
): PublicRecentPylon => {
  const lastSeenAtUnixMs = pylonHeartbeatUnixMs(registration)
  const onlineNow = isOnlineNow(registration, nowUnixMs)

  return new PublicRecentPylon({
    pylonRef: registration.pylonRef,
    ownerAgentRef: `agent:${registration.ownerAgentUserId}`,
    nodeLabel: registration.displayName,
    nostrPubkeyShort: registration.pylonRef,
    clientVersion: registration.clientVersion,
    readyModel: null,
    runtimeState: registration.latestHeartbeatStatus,
    lastSeenAtUnixMs,
    lastSeenAtLabel: friendlyTimestampLabel(lastSeenAtUnixMs, nowUnixMs),
    lastHeartbeatAgeSeconds:
      lastSeenAtUnixMs === null
        ? null
        : Math.max(0, Math.floor((nowUnixMs - lastSeenAtUnixMs) / 1000)),
    onlineNow,
    walletReadyNow: onlineNow && registration.walletReady,
    assignmentReadyNow: isAssignmentReady(registration, nowUnixMs),
    cumulativeSettledSats: 0,
    tippingAvailable: registration.ownerAgentUserId.trim() !== '',
    tipEndpoint: `/api/pylons/${encodeURIComponent(registration.pylonRef)}/tips/ladder`,
    eligibleProductCount: 0,
    relayUrls: [],
    products: publicScannerSafeRefs(
      'capability.public.pylon_stats',
      registration.capabilityRefs,
    ),
  })
}

export const publicPylonStatsFromRegistrations = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
  nowUnixMs: number,
  settlementTotals: PublicPylonSettlementTotals = emptyUnavailableSettlementTotals(
    'Nexus/Pylon settlement receipt store unavailable.',
  ),
  marketSettlementTotals: PublicNip90MarketSettlementTotals = emptyUnavailableMarketSettlementTotals(
    'NIP-90 market receipt store unavailable.',
  ),
  treasuryPayoutTotals: PublicTreasuryPayoutTotals = emptyUnavailableTreasuryPayoutTotals(
    'Treasury payout transaction store unavailable.',
  ),
  trainingContributorTotals: PublicTrainingContributorTotals = emptyUnavailableTrainingContributorTotals(
    'Training run authority store unavailable.',
  ),
): PublicPylonStats => {
  const eligibleRegistrations = registrations.filter(
    isActiveEligibleRegistration,
  )
  const seen24hRegistrations = eligibleRegistrations.filter(registration =>
    isSeen24h(registration, nowUnixMs),
  )
  const onlineRegistrations = eligibleRegistrations.filter(registration =>
    isOnlineNow(registration, nowUnixMs),
  )
  const walletReadyRegistrations = onlineRegistrations.filter(
    registration => registration.walletReady,
  )
  const assignmentReadyRegistrations = onlineRegistrations.filter(
    registration => isAssignmentReady(registration, nowUnixMs),
  )
  const sourceRefs = [
    'route:/api/public/pylon-stats',
    'openagents.public.pylon_api.registrations',
  ]
  const pylonsByResourceMode: Record<string, number> = {}
  const pylonsByClientVersion: Record<string, number> = {}

  eligibleRegistrations.forEach(registration => {
    incrementCount(
      pylonsByResourceMode,
      registration.latestResourceMode ?? registration.resourceMode,
    )
    incrementCount(pylonsByClientVersion, registration.clientVersion)
  })

  const recentPylons = [...seen24hRegistrations]
    .sort(
      (left, right) =>
        (pylonHeartbeatUnixMs(right) ?? 0) - (pylonHeartbeatUnixMs(left) ?? 0),
    )
    .slice(0, MAX_RECENT_PYLONS)
    .map(registration => recentPylonFromRegistration(registration, nowUnixMs))
  const pylonsOnlineNow = onlineRegistrations.length
  const pylonsWalletReadyNow = walletReadyRegistrations.length
  const pylonsAssignmentReadyNow = assignmentReadyRegistrations.length
  const settlementGate = publicPylonAcceptedWorkSettlementGate(settlementTotals)
  const publicRealSatsSettled24h = publicRealSatsSettled({
    acceptedWorkSats: settlementTotals.satsPaid24h,
    marketSats: marketSats24h(marketSettlementTotals),
    treasuryOutflowSats: treasuryPayoutTotals.satsPaid24h,
  })
  const publicRealSatsSettledTotal = publicRealSatsSettled({
    acceptedWorkSats: settlementTotals.satsPaidTotal,
    marketSats: marketSatsTotal(marketSettlementTotals),
    treasuryOutflowSats: treasuryPayoutTotals.satsPaidTotal,
  })

  return new PublicPylonStats({
    available: true,
    status: 'live',
    error: null,
    sourceUrl: PUBLIC_PYLON_STATS_URL,
    hostedNexusRelayUrl: null,
    generatedAtUnixMs: nowUnixMs,
    asOfUnixMs: nowUnixMs,
    asOfLabel: friendlyTimestampLabel(nowUnixMs, nowUnixMs),
    staleness: PUBLIC_PYLON_STATS_STALENESS,
    minimumClientVersion: PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
    pylonsOnlineNow,
    pylonsSeen24h: seen24hRegistrations.length,
    pylonsRegisteredTotal: eligibleRegistrations.length,
    pylonsWalletReadyNow,
    pylonsAssignmentReadyNow,
    pylonsByResourceMode,
    pylonsByClientVersion,
    pylonSessionsOnlineNow: pylonsOnlineNow,
    sellablePylonsOnlineNow: pylonsAssignmentReadyNow,
    nexusPayoutSatsPaidTotal: settlementTotals.satsPaidTotal,
    nexusAcceptedWorkPayoutSatsPaidTotal: settlementTotals.satsPaidTotal,
    nexusAcceptedWorkPayoutSatsPaid24h: settlementTotals.satsPaid24h,
    nexusAcceptedWorkPayoutReceiptRefs: [...settlementTotals.receiptRefs],
    nexusAcceptedWorkSettlementGate: settlementGate,
    nip90MarketSettlementStats: marketSettlementTotals,
    treasuryPayoutSatsPaidTotal: treasuryPayoutTotals.satsPaidTotal,
    treasuryPayoutSatsPaid24h: treasuryPayoutTotals.satsPaid24h,
    treasuryPayoutCountTotal: treasuryPayoutTotals.payoutCountTotal,
    treasuryPayoutCount24h: treasuryPayoutTotals.payoutCount24h,
    publicRealSatsSettledTotal,
    publicRealSatsSettled24h,
    trainingAssignedContributors:
      trainingContributorTotals.assignedContributors,
    trainingAcceptedContributors:
      trainingContributorTotals.acceptedContributors,
    trainingModelProgressContributors:
      trainingContributorTotals.modelProgressContributors,
    counterWindows: publicPylonStatsCounterWindows(),
    recentPylons,
    earningLaunchGate: publicPylonEarningLaunchGate({
      available: true,
      pylonsAssignmentReadyNow,
      pylonsOnlineNow,
      pylonsWalletReadyNow,
      sourceRefs,
    }),
    caveatRefs: [
      'caveat.public.pylon_stats_are_registration_heartbeat_only',
      'caveat.public.recent_pylon_runtime_state_is_last_reported_not_live',
      'caveat.public.assignment_ready_is_not_payout_evidence',
      'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
      'caveat.public.accepted_work_totals_require_settled_real_bitcoin_receipts',
      'caveat.public.sats_settled_24h_includes_real_settled_treasury_outflows_nip90_and_accepted_work_receipts',
      'caveat.public.treasury_outflows_are_not_accepted_work_claims',
      'caveat.public.accepted_work_not_added_on_top_of_treasury_outflows_to_avoid_double_count',
      'caveat.public.training_contributors_are_live_run_contributor_refs_not_stale_registrations',
      'caveat.public.no_sensitive_material',
    ],
    sourceRefs: uniqueRefs([
      ...sourceRefs,
      ...settlementTotals.sourceRefs,
      ...marketSettlementTotals.sourceRefs,
      ...treasuryPayoutTotals.sourceRefs,
      ...trainingContributorTotals.sourceRefs,
    ]),
  })
}

export const publicPylonStatsFromNexusPayload = (
  payload: Record<string, unknown>,
): PublicPylonStats => {
  const legacyReceiptRefs = stringArrayFromUnknown(
    payload.nexus_accepted_work_payout_receipt_refs,
  )
  const rawNexusPayoutSatsPaidTotal = nullableInt(
    payload.nexus_payout_sats_paid_total,
  )
  const rawNexusPayoutSatsPaid24h = nullableInt(
    payload.nexus_payout_sats_paid_24h,
  )
  const legacyReceiptBackedTotalsAvailable = legacyReceiptRefs.length > 0
  const nexusPayoutSatsPaidTotal = legacyReceiptBackedTotalsAvailable
    ? rawNexusPayoutSatsPaidTotal
    : null
  const nexusPayoutSatsPaid24h = legacyReceiptBackedTotalsAvailable
    ? rawNexusPayoutSatsPaid24h
    : null
  const asOfUnixMs = nullableInt(payload.as_of_unix_ms)
  const pylonsOnlineNow = intValue(payload.pylons_online_now)
  const sellablePylonsOnlineNow = intValue(payload.sellable_pylons_online_now)
  const sourceRefs = ['nexus.public.stats', PUBLIC_NEXUS_STATS_URL]
  const legacyTotals = {
    available: legacyReceiptBackedTotalsAvailable,
    error: legacyReceiptBackedTotalsAvailable
      ? null
      : 'Legacy Nexus public payload did not include settlement receipt refs.',
    receiptRefs: legacyReceiptRefs,
    satsPaid24h: legacyReceiptBackedTotalsAvailable
      ? fallbackNullableInt(
          payload.nexus_accepted_work_payout_sats_paid_24h,
          nexusPayoutSatsPaid24h,
        )
      : null,
    satsPaidTotal: legacyReceiptBackedTotalsAvailable
      ? fallbackNullableInt(
          payload.nexus_accepted_work_payout_sats_paid_total,
          nexusPayoutSatsPaidTotal,
        )
      : null,
    sourceRefs,
  } satisfies PublicPylonSettlementTotals

  return new PublicPylonStats({
    available: true,
    status: 'live',
    error: null,
    sourceUrl: PUBLIC_NEXUS_STATS_URL,
    hostedNexusRelayUrl: optionalString(payload.hosted_nexus_relay_url) ?? null,
    generatedAtUnixMs: asOfUnixMs ?? currentEpochMillis(),
    asOfUnixMs,
    asOfLabel: timestampLabel(asOfUnixMs),
    staleness: PUBLIC_PYLON_STATS_STALENESS,
    minimumClientVersion: 'legacy-nexus',
    pylonsOnlineNow,
    pylonsSeen24h: intValue(payload.pylons_seen_24h),
    pylonsRegisteredTotal: pylonsOnlineNow,
    pylonsWalletReadyNow: sellablePylonsOnlineNow,
    pylonsAssignmentReadyNow: sellablePylonsOnlineNow,
    pylonsByResourceMode: {},
    pylonsByClientVersion: {},
    pylonSessionsOnlineNow: intValue(payload.pylon_sessions_online_now),
    sellablePylonsOnlineNow,
    nexusPayoutSatsPaidTotal,
    nexusAcceptedWorkPayoutSatsPaidTotal: legacyTotals.satsPaidTotal,
    nexusAcceptedWorkPayoutSatsPaid24h: legacyTotals.satsPaid24h,
    nexusAcceptedWorkPayoutReceiptRefs: [...legacyTotals.receiptRefs],
    nexusAcceptedWorkSettlementGate:
      publicPylonAcceptedWorkSettlementGate(legacyTotals),
    nip90MarketSettlementStats: emptyUnavailableMarketSettlementTotals(
      'Legacy Nexus public payload did not include NIP-90 market receipts.',
    ),
    treasuryPayoutSatsPaidTotal: null,
    treasuryPayoutSatsPaid24h: null,
    treasuryPayoutCountTotal: null,
    treasuryPayoutCount24h: null,
    publicRealSatsSettledTotal: legacyTotals.satsPaidTotal,
    publicRealSatsSettled24h: legacyTotals.satsPaid24h,
    trainingAssignedContributors: intValue(
      payload.training_assigned_contributors,
    ),
    trainingAcceptedContributors: intValue(
      payload.training_accepted_contributors,
    ),
    trainingModelProgressContributors: intValue(
      payload.training_model_progress_contributors,
    ),
    counterWindows: publicPylonStatsCounterWindows(),
    recentPylons: recentPylonsFromUnknown(payload.recent_pylons),
    earningLaunchGate: publicPylonEarningLaunchGate({
      available: true,
      pylonsAssignmentReadyNow: sellablePylonsOnlineNow,
      pylonsOnlineNow,
      pylonsWalletReadyNow: sellablePylonsOnlineNow,
      sourceRefs,
    }),
    caveatRefs: ['caveat.public.legacy_nexus_fixture'],
    sourceRefs,
  })
}

const unavailablePublicPylonStats = (error: string): PublicPylonStats =>
  new PublicPylonStats({
    available: false,
    status: 'unavailable',
    error,
    sourceUrl: PUBLIC_PYLON_STATS_URL,
    hostedNexusRelayUrl: null,
    generatedAtUnixMs: currentEpochMillis(),
    asOfUnixMs: null,
    asOfLabel: null,
    staleness: PUBLIC_PYLON_STATS_STALENESS,
    minimumClientVersion: PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
    pylonsOnlineNow: 0,
    pylonsSeen24h: 0,
    pylonsRegisteredTotal: 0,
    pylonsWalletReadyNow: 0,
    pylonsAssignmentReadyNow: 0,
    pylonsByResourceMode: {},
    pylonsByClientVersion: {},
    pylonSessionsOnlineNow: 0,
    sellablePylonsOnlineNow: 0,
    nexusPayoutSatsPaidTotal: null,
    nexusAcceptedWorkPayoutSatsPaidTotal: null,
    nexusAcceptedWorkPayoutSatsPaid24h: null,
    nexusAcceptedWorkPayoutReceiptRefs: [],
    nexusAcceptedWorkSettlementGate: publicPylonAcceptedWorkSettlementGate(
      emptyUnavailableSettlementTotals(error),
    ),
    nip90MarketSettlementStats: emptyUnavailableMarketSettlementTotals(error),
    treasuryPayoutSatsPaidTotal: null,
    treasuryPayoutSatsPaid24h: null,
    treasuryPayoutCountTotal: null,
    treasuryPayoutCount24h: null,
    publicRealSatsSettledTotal: null,
    publicRealSatsSettled24h: null,
    trainingAssignedContributors: 0,
    trainingAcceptedContributors: 0,
    trainingModelProgressContributors: 0,
    counterWindows: publicPylonStatsCounterWindows(),
    recentPylons: [],
    earningLaunchGate: publicPylonEarningLaunchGate({
      available: false,
      pylonsAssignmentReadyNow: 0,
      pylonsOnlineNow: 0,
      pylonsWalletReadyNow: 0,
      sourceRefs: ['route:/api/public/pylon-stats'],
    }),
    caveatRefs: ['caveat.public.pylon_stats_unavailable'],
    sourceRefs: ['route:/api/public/pylon-stats'],
  })

export const publicPylonStatsSnapshot = (
  input: Readonly<{
    nowUnixMs?: (() => number) | undefined
    marketReceiptStore?: Nip90MarketReceiptStore | undefined
    receiptStore?: PublicPylonSettlementReceiptStore | undefined
    store: PublicPylonStatsStore
    trainingStore?: PublicTrainingContributorStatsStore | undefined
    treasuryPayoutStore?: PublicTreasuryPayoutStatsStore | undefined
  }>,
): Effect.Effect<PublicPylonStats> =>
  Effect.gen(function* () {
    const nowUnixMs = input.nowUnixMs?.() ?? currentEpochMillis()
    const registrations = yield* Effect.tryPromise({
      catch: error => {
        const storeError = pylonApiStoreErrorFromUnknown(error)

        return new PublicPylonStatsSnapshotError({
          reason: storeError.reason,
        })
      },
      try: () => input.store.listRegistrations(1000),
    })
    const receiptStore = input.receiptStore
    const marketReceiptStore = input.marketReceiptStore
    const treasuryPayoutStore = input.treasuryPayoutStore
    const trainingStore = input.trainingStore
    const settlementTotals =
      receiptStore === undefined
        ? emptyUnavailableSettlementTotals(
            'Nexus/Pylon settlement receipt store unavailable.',
          )
        : yield* Effect.tryPromise({
            catch: error =>
              new PublicPylonStatsSnapshotError({
                reason: error instanceof Error ? error.message : String(error),
              }),
            try: () =>
              publicPylonSettlementTotalsFromReceipts({
                nowUnixMs,
                receiptStore,
              }),
          }).pipe(
            Effect.catch(error =>
              Effect.succeed(emptyUnavailableSettlementTotals(error.reason)),
            ),
          )
    const marketSettlementTotals =
      marketReceiptStore === undefined
        ? emptyUnavailableMarketSettlementTotals(
            'NIP-90 market receipt store unavailable.',
          )
        : yield* Effect.tryPromise({
            catch: error =>
              new PublicPylonStatsSnapshotError({
                reason: error instanceof Error ? error.message : String(error),
              }),
            try: () =>
              publicNip90MarketSettlementTotalsFromReceipts({
                marketReceiptStore,
                nowUnixMs,
              }),
          }).pipe(
            Effect.catch(error =>
              Effect.succeed(
                emptyUnavailableMarketSettlementTotals(error.reason),
              ),
            ),
          )
    const treasuryPayoutTotals =
      treasuryPayoutStore === undefined
        ? emptyUnavailableTreasuryPayoutTotals(
            'Treasury payout transaction store unavailable.',
          )
        : yield* Effect.tryPromise({
            catch: error =>
              new PublicPylonStatsSnapshotError({
                reason: error instanceof Error ? error.message : String(error),
              }),
            try: () =>
              publicTreasuryPayoutTotalsFromTransactions({
                nowUnixMs,
                treasuryPayoutStore,
              }),
          }).pipe(
            Effect.catch(error =>
              Effect.succeed(
                emptyUnavailableTreasuryPayoutTotals(error.reason),
              ),
            ),
          )
    const trainingContributorTotals =
      trainingStore === undefined
        ? emptyUnavailableTrainingContributorTotals(
            'Training run authority store unavailable.',
          )
        : yield* Effect.tryPromise({
            catch: error =>
              new PublicPylonStatsSnapshotError({
                reason: error instanceof Error ? error.message : String(error),
              }),
            try: () =>
              publicTrainingContributorTotalsFromStore({
                nowUnixMs,
                store: trainingStore,
              }),
          }).pipe(
            Effect.catch(error =>
              Effect.succeed(
                emptyUnavailableTrainingContributorTotals(error.reason),
              ),
            ),
          )

    return publicPylonStatsFromRegistrations(
      registrations,
      nowUnixMs,
      settlementTotals,
      marketSettlementTotals,
      treasuryPayoutTotals,
      trainingContributorTotals,
    )
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(unavailablePublicPylonStats(error.reason)),
    ),
  )
