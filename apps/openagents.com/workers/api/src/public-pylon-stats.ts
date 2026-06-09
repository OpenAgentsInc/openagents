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
  type PylonApiRegistrationRecord,
  pylonApiStoreErrorFromUnknown,
  pylonClientVersionMeetsMinimum,
} from './pylon-api'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

export const PUBLIC_NEXUS_STATS_URL = 'https://nexus.openagents.com/api/stats'
export const PUBLIC_PYLON_STATS_URL =
  'https://openagents.com/api/public/pylon-stats'
export const PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION = '0.2.5'

const ONLINE_WINDOW_MS = 5 * 60 * 1000
const SEEN_24H_WINDOW_MS = 24 * 60 * 60 * 1000
const MAX_RECENT_PYLONS = 12
const PUBLIC_PYLON_EARNING_LAUNCH_GATE_REF =
  'gate.public.pylon.earning_network_counters.v1'
const PUBLIC_PYLON_SETTLEMENT_TOTALS_GATE_REF =
  'gate.public.pylon.accepted_work_settlement_receipts.v1'
const OPENAGENTS_PUBLIC_APP_URL = 'https://openagents.com'
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

export class PublicRecentPylon extends S.Class<PublicRecentPylon>(
  'PublicRecentPylon',
)({
  nodeLabel: S.NullOr(S.String),
  nostrPubkeyShort: S.String,
  clientVersion: S.NullOr(S.String),
  readyModel: S.NullOr(S.String),
  runtimeState: S.NullOr(S.String),
  lastSeenAtUnixMs: S.NullOr(S.Int),
  lastSeenAtLabel: S.NullOr(S.String),
  eligibleProductCount: S.Int,
  relayUrls: S.Array(S.String),
  products: S.Array(S.String),
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

export class PublicPylonStats extends S.Class<PublicPylonStats>(
  'PublicPylonStats',
)({
  available: S.Boolean,
  status: S.Literals(['live', 'unavailable']),
  error: S.NullOr(S.String),
  sourceUrl: S.String,
  hostedNexusRelayUrl: S.NullOr(S.String),
  asOfUnixMs: S.NullOr(S.Int),
  asOfLabel: S.NullOr(S.String),
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
  trainingAssignedContributors: S.Int,
  trainingAcceptedContributors: S.Int,
  trainingModelProgressContributors: S.Int,
  recentPylons: S.Array(PublicRecentPylon),
  earningLaunchGate: PublicPylonEarningLaunchGate,
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
}) {}

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
    nodeLabel: optionalString(value.node_label) ?? null,
    nostrPubkeyShort: optionalString(value.nostr_pubkey_short) ?? 'unknown',
    clientVersion: optionalString(value.client_version) ?? null,
    readyModel: optionalString(value.ready_model) ?? null,
    runtimeState: optionalString(value.runtime_state) ?? null,
    lastSeenAtUnixMs,
    lastSeenAtLabel: timestampLabel(lastSeenAtUnixMs),
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
  const countedByIntent = await settlementReceipts.reduce(
    async (previous, receipt) => {
      const counted = await previous

      if (counted.has(receipt.payoutIntentRef)) {
        return counted
      }

      const intent = await input.receiptStore.readPayoutIntentByRef(
        receipt.payoutIntentRef,
      )
      const event =
        receipt.eventRef === null
          ? undefined
          : await input.receiptStore.readReconciliationEventByRef(
              receipt.eventRef,
            )
      const attempt =
        receipt.payoutAttemptRef === null
          ? undefined
          : await input.receiptStore.readPayoutAttemptByRef(
              receipt.payoutAttemptRef,
            )

      if (intent === undefined || intent.acceptedWorkRefs.length === 0) {
        return counted
      }

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
        return counted
      }

      counted.set(receipt.payoutIntentRef, {
        receiptRef: receipt.receiptRef,
        sats,
        settledAt: receipt.createdAt,
      })

      return counted
    },
    Promise.resolve(
      new Map<
        string,
        Readonly<{ receiptRef: string; sats: number; settledAt: string }>
      >(),
    ),
  )

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

  return new PublicRecentPylon({
    nodeLabel: registration.displayName,
    nostrPubkeyShort: registration.pylonRef,
    clientVersion: registration.clientVersion,
    readyModel: null,
    runtimeState: registration.latestHeartbeatStatus,
    lastSeenAtUnixMs,
    lastSeenAtLabel: friendlyTimestampLabel(lastSeenAtUnixMs, nowUnixMs),
    eligibleProductCount: 0,
    relayUrls: [],
    products: [...registration.capabilityRefs],
  })
}

export const publicPylonStatsFromRegistrations = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
  nowUnixMs: number,
  settlementTotals: PublicPylonSettlementTotals = emptyUnavailableSettlementTotals(
    'Nexus/Pylon settlement receipt store unavailable.',
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

  return new PublicPylonStats({
    available: true,
    status: 'live',
    error: null,
    sourceUrl: PUBLIC_PYLON_STATS_URL,
    hostedNexusRelayUrl: null,
    asOfUnixMs: nowUnixMs,
    asOfLabel: friendlyTimestampLabel(nowUnixMs, nowUnixMs),
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
    trainingAssignedContributors: 0,
    trainingAcceptedContributors: 0,
    trainingModelProgressContributors: 0,
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
      'caveat.public.assignment_ready_is_not_payout_evidence',
      'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
      'caveat.public.accepted_work_totals_require_settled_real_bitcoin_receipts',
      'caveat.public.no_sensitive_material',
    ],
    sourceRefs: uniqueRefs([...sourceRefs, ...settlementTotals.sourceRefs]),
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
    asOfUnixMs,
    asOfLabel: timestampLabel(asOfUnixMs),
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
    trainingAssignedContributors: intValue(
      payload.training_assigned_contributors,
    ),
    trainingAcceptedContributors: intValue(
      payload.training_accepted_contributors,
    ),
    trainingModelProgressContributors: intValue(
      payload.training_model_progress_contributors,
    ),
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
    asOfUnixMs: null,
    asOfLabel: null,
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
    trainingAssignedContributors: 0,
    trainingAcceptedContributors: 0,
    trainingModelProgressContributors: 0,
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
    receiptStore?: PublicPylonSettlementReceiptStore | undefined
    store: PublicPylonStatsStore
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

    return publicPylonStatsFromRegistrations(
      registrations,
      nowUnixMs,
      settlementTotals,
    )
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(unavailablePublicPylonStats(error.reason)),
    ),
  )
