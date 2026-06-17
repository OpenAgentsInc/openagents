import { Effect, Schema as S } from 'effect'

import {
  ArtanisApprovalGateRecord,
  artanisApprovalGateEffective,
} from './artanis-approval-gates'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import type {
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
} from './nexus-treasury-payout-ledger'
import {
  PublicPylonAcceptedWorkSettlementGate,
  PublicPylonEarningLaunchGate,
  publicPylonStatsCounterWindows,
  PublicPylonStats,
  emptyUnavailableMarketSettlementTotals,
} from './public-pylon-stats'
import { PylonMarketplaceJobKind } from './pylon-marketplace-jobs'
import { PylonResourceMode } from './pylon-resource-mode-setup'
import { epochMillisToIsoTimestamp } from './runtime-primitives'
import {
  type TreasuryPaymentAuthorityDispatchResult,
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityIntentCreationResult,
  type TreasuryPaymentAuthorityPayoutPreview,
  type TreasuryPaymentAuthorityRejectionReason,
  type TreasuryPaymentAuthorityShape,
  type TreasuryPaymentAuthorityWalletReadiness,
} from './treasury-payment-authority'

export const ArtanisNexusPylonAdapterAudience = S.Literals([
  'operator',
  'public_artanis',
  'public_forum',
])
export type ArtanisNexusPylonAdapterAudience =
  typeof ArtanisNexusPylonAdapterAudience.Type

export const ArtanisNexusPylonAdapterSurface = S.Literals([
  'acceptance',
  'artifacts',
  'job_assignments',
  'job_offers',
  'provider_inventory',
  'pylon_readiness',
  'payout_settlement_caveats',
  'run_status',
  'stats',
])
export type ArtanisNexusPylonAdapterSurface =
  typeof ArtanisNexusPylonAdapterSurface.Type

export const ArtanisNexusPylonFleetState = S.Literals([
  'degraded',
  'live',
  'stale',
  'unavailable',
])
export type ArtanisNexusPylonFleetState =
  typeof ArtanisNexusPylonFleetState.Type

export const ArtanisNexusPylonAdapterMode = S.Literals(['fake', 'live'])
export type ArtanisNexusPylonAdapterMode =
  typeof ArtanisNexusPylonAdapterMode.Type

export const ArtanisNexusPylonDispatchState = S.Literals([
  'approved',
  'blocked',
  'dispatch_recorded',
  'failed',
  'held_for_approval',
  'proposed',
])
export type ArtanisNexusPylonDispatchState =
  typeof ArtanisNexusPylonDispatchState.Type

export const ArtanisNexusPylonPaymentAuthorityState = S.Literals([
  'assignment_created',
  'awaiting_approval',
  'dispatch_authorized',
  'dispatch_blocked',
  'payout_intent_created',
  'previewed',
  'proposed',
  'settlement_complete',
  'settlement_failed',
  'settlement_pending',
  'wallet_ready',
])
export type ArtanisNexusPylonPaymentAuthorityState =
  typeof ArtanisNexusPylonPaymentAuthorityState.Type

export class ArtanisNexusPylonAdminAuthority extends S.Class<ArtanisNexusPylonAdminAuthority>(
  'ArtanisNexusPylonAdminAuthority',
)({
  approvedFakeDispatchAllowed: S.Boolean,
  deploymentAllowed: S.Boolean,
  livePylonJobDispatchAllowed: S.Boolean,
  paymentSpendAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  readOnlyFleetMonitoringAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  trainingLaunchAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
  workDispatchProposalAllowed: S.Boolean,
}) {}

export class ArtanisNexusPylonFleetSnapshotRecord extends S.Class<ArtanisNexusPylonFleetSnapshotRecord>(
  'ArtanisNexusPylonFleetSnapshotRecord',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  fleetState: ArtanisNexusPylonFleetState,
  hostedNexusRelayRef: S.NullOr(S.String),
  nexusAcceptedWorkBitcoinPaidRef: S.NullOr(S.String),
  pylonRefs: S.Array(S.String),
  pylonsOnlineNow: S.Number,
  pylonsSeen24h: S.Number,
  pylonSessionsOnlineNow: S.Number,
  sellablePylonsOnlineNow: S.Number,
  snapshotRef: S.String,
  sourceRefs: S.Array(S.String),
  staleAfterIso: S.String,
  surfaces: S.Array(ArtanisNexusPylonAdapterSurface),
  trainingAcceptedContributors: S.Number,
  trainingAssignedContributors: S.Number,
  trainingModelProgressContributors: S.Number,
  updatedAtIso: S.String,
}) {}

export class ArtanisNexusPylonDispatchRecord extends S.Class<ArtanisNexusPylonDispatchRecord>(
  'ArtanisNexusPylonDispatchRecord',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  acceptedWorkRefs: S.Array(S.String),
  adapterMode: ArtanisNexusPylonAdapterMode,
  approvalGateRef: S.String,
  approvalRequirementRefs: S.Array(S.String),
  authorityReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  dispatchRef: S.String,
  estimatedCostRefs: S.Array(S.String),
  idempotencyKey: S.String,
  jobKind: PylonMarketplaceJobKind,
  marketplaceJobRef: S.String,
  nexusRouteRef: S.String,
  operatorDetailRefs: S.Array(S.String),
  paymentAuthorityRefs: S.Array(S.String),
  paymentAuthorityState: ArtanisNexusPylonPaymentAuthorityState,
  privateEvidenceRefs: S.Array(S.String),
  proposalRef: S.String,
  providerEligibilityRefs: S.Array(S.String),
  payoutAttemptRefs: S.Array(S.String),
  payoutIntentRefs: S.Array(S.String),
  payoutTargetApprovalRefs: S.Array(S.String),
  pylonRouteRef: S.String,
  receiptRefs: S.Array(S.String),
  resourceMode: PylonResourceMode,
  runStatusRefs: S.Array(S.String),
  settlementBridgeRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendLimitRefs: S.Array(S.String),
  state: ArtanisNexusPylonDispatchState,
  updatedAtIso: S.String,
  walletReadinessRefs: S.Array(S.String),
}) {}

export class ArtanisNexusPylonAdminAdapterRecord extends S.Class<ArtanisNexusPylonAdminAdapterRecord>(
  'ArtanisNexusPylonAdminAdapterRecord',
)({
  agentId: S.String,
  authority: ArtanisNexusPylonAdminAuthority,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  dispatchRecords: S.Array(ArtanisNexusPylonDispatchRecord),
  fleetSnapshots: S.Array(ArtanisNexusPylonFleetSnapshotRecord),
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisNexusPylonFleetSnapshotProjection extends S.Class<ArtanisNexusPylonFleetSnapshotProjection>(
  'ArtanisNexusPylonFleetSnapshotProjection',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  fleetState: ArtanisNexusPylonFleetState,
  hostedNexusRelayRef: S.NullOr(S.String),
  nexusAcceptedWorkBitcoinPaidRef: S.NullOr(S.String),
  pylonRefs: S.Array(S.String),
  pylonsOnlineNow: S.Number,
  pylonsSeen24h: S.Number,
  pylonSessionsOnlineNow: S.Number,
  sellablePylonsOnlineNow: S.Number,
  snapshotRef: S.String,
  sourceRefs: S.Array(S.String),
  staleAfterDisplay: S.String,
  surfaces: S.Array(ArtanisNexusPylonAdapterSurface),
  trainingAcceptedContributors: S.Number,
  trainingAssignedContributors: S.Number,
  trainingModelProgressContributors: S.Number,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisNexusPylonDispatchProjection extends S.Class<ArtanisNexusPylonDispatchProjection>(
  'ArtanisNexusPylonDispatchProjection',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  acceptedWorkRefs: S.Array(S.String),
  adapterMode: ArtanisNexusPylonAdapterMode,
  approvalGateRef: S.String,
  approvalRequirementRefs: S.Array(S.String),
  authorityReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dispatchRef: S.String,
  estimatedCostRefs: S.Array(S.String),
  fakeDispatchReceiptRecorded: S.Boolean,
  idempotencyKey: S.String,
  jobKind: PylonMarketplaceJobKind,
  liveDispatchClaimAllowed: S.Boolean,
  marketplaceJobRef: S.String,
  nexusRouteRef: S.String,
  operatorDetailRefs: S.Array(S.String),
  paymentAuthorityBlocked: S.Boolean,
  paymentAuthorityGatePassed: S.Boolean,
  paymentAuthorityRefs: S.Array(S.String),
  paymentAuthorityState: ArtanisNexusPylonPaymentAuthorityState,
  paymentAuthorityStateLabel: S.String,
  privateEvidenceRefs: S.Array(S.String),
  proposalRef: S.String,
  providerEligibilityRefs: S.Array(S.String),
  payoutAttemptRefs: S.Array(S.String),
  payoutIntentRefs: S.Array(S.String),
  payoutTargetApprovalRefs: S.Array(S.String),
  pylonRouteRef: S.String,
  receiptRefs: S.Array(S.String),
  resourceMode: PylonResourceMode,
  runStatusRefs: S.Array(S.String),
  settlementBridgeRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendLimitRefs: S.Array(S.String),
  state: ArtanisNexusPylonDispatchState,
  updatedAtDisplay: S.String,
  walletReadinessRefs: S.Array(S.String),
}) {}

export class ArtanisNexusPylonAdminAdapterProjection extends S.Class<ArtanisNexusPylonAdminAdapterProjection>(
  'ArtanisNexusPylonAdminAdapterProjection',
)({
  agentId: S.String,
  audience: ArtanisNexusPylonAdapterAudience,
  authority: ArtanisNexusPylonAdminAuthority,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dispatchRecords: S.Array(ArtanisNexusPylonDispatchProjection),
  fleetSnapshots: S.Array(ArtanisNexusPylonFleetSnapshotProjection),
  latestFleetState: ArtanisNexusPylonFleetState,
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisNexusPylonDispatchAdapterCall extends S.Class<ArtanisNexusPylonDispatchAdapterCall>(
  'ArtanisNexusPylonDispatchAdapterCall',
)({
  adapterMode: ArtanisNexusPylonAdapterMode,
  dispatchRef: S.String,
  idempotencyKey: S.String,
  nexusRouteRef: S.String,
  proposalRef: S.String,
  pylonRouteRef: S.String,
}) {}

export class ArtanisNexusPylonDispatchAdapterReceipt extends S.Class<ArtanisNexusPylonDispatchAdapterReceipt>(
  'ArtanisNexusPylonDispatchAdapterReceipt',
)({
  adapterMode: ArtanisNexusPylonAdapterMode,
  dispatchRef: S.String,
  nexusRouteRef: S.String,
  pylonRouteRef: S.String,
  receiptRef: S.String,
  runStatusRef: S.String,
}) {}

export class ArtanisNexusPylonAdapterUnsafe extends S.TaggedErrorClass<ArtanisNexusPylonAdapterUnsafe>()(
  'ArtanisNexusPylonAdapterUnsafe',
  {
    reason: S.String,
  },
) {}

export type ArtanisNexusPylonDispatchAdapter = Readonly<{
  dispatch: (
    record: ArtanisNexusPylonDispatchRecord,
  ) => ArtanisNexusPylonDispatchAdapterReceipt
}>

export const ARTANIS_NEXUS_PYLON_ADMIN_ADAPTER_SURFACES: ReadonlyArray<ArtanisNexusPylonAdapterSurface> =
  [
    'acceptance',
    'artifacts',
    'job_assignments',
    'job_offers',
    'payout_settlement_caveats',
    'provider_inventory',
    'pylon_readiness',
    'run_status',
    'stats',
  ]

export const ARTANIS_NEXUS_PYLON_ADMIN_NO_LIVE_AUTHORITY: ArtanisNexusPylonAdminAuthority =
  {
    approvedFakeDispatchAllowed: true,
    deploymentAllowed: false,
    livePylonJobDispatchAllowed: false,
    paymentSpendAllowed: false,
    providerMutationAllowed: false,
    readOnlyFleetMonitoringAllowed: true,
    runtimePromotionAllowed: false,
    settlementMutationAllowed: false,
    trainingLaunchAllowed: false,
    walletSpendAllowed: false,
    workDispatchProposalAllowed: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeAdapterRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset[._-]?(raw|private|secret|payload)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[._-]?(artifact|raw|secret|weights)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(archive|customer|dataset|key|model|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(authority\.operator|evidence\.private|nexus\.private|operator\.|provider\.private|pylon\.private|receipt\.operator|workroom\.private)/i
const staleStatsAfterMs = 15 * 60 * 1000

const paymentAuthorityStateRank: Readonly<
  Record<ArtanisNexusPylonPaymentAuthorityState, number>
> = {
  assignment_created: 3,
  awaiting_approval: 2,
  dispatch_authorized: 7,
  dispatch_blocked: -1,
  payout_intent_created: 6,
  previewed: 1,
  proposed: 0,
  settlement_complete: 9,
  settlement_failed: -1,
  settlement_pending: 8,
  wallet_ready: 4,
}

const paymentAuthorityStateLabel: Readonly<
  Record<ArtanisNexusPylonPaymentAuthorityState, string>
> = {
  assignment_created: 'Assignment created',
  awaiting_approval: 'Awaiting approval',
  dispatch_authorized: 'Dispatch authorized',
  dispatch_blocked: 'Dispatch blocked',
  payout_intent_created: 'Payout intent created',
  previewed: 'Previewed',
  proposed: 'Proposed',
  settlement_complete: 'Settlement complete',
  settlement_failed: 'Settlement failed',
  settlement_pending: 'Settlement pending',
  wallet_ready: 'Wallet ready',
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const paymentAuthorityStateAtLeast = (
  state: ArtanisNexusPylonPaymentAuthorityState,
  threshold: ArtanisNexusPylonPaymentAuthorityState,
): boolean =>
  paymentAuthorityStateRank[state] >= paymentAuthorityStateRank[threshold]

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeAdapterRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, raw artifact, raw dataset, or raw timestamp material.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisNexusPylonAdapterAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const safeSuffix = (value: string): string => {
  const suffix = value
    .trim()
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96)

  return suffix === '' ? 'unknown' : suffix
}

const staleAfterIso = (asOfUnixMs: number | null, nowIso: string): string =>
  epochMillisToIsoTimestamp(
    (asOfUnixMs ?? Date.parse(nowIso)) + staleStatsAfterMs,
  )

const fleetStateFromStats = (
  stats: PublicPylonStats,
  nowIso: string,
): ArtanisNexusPylonFleetState => {
  if (!stats.available) {
    return 'unavailable'
  }

  if (
    stats.asOfUnixMs !== null &&
    Date.parse(nowIso) - stats.asOfUnixMs > staleStatsAfterMs
  ) {
    return 'stale'
  }

  return stats.pylonsAssignmentReadyNow > 0 ? 'live' : 'degraded'
}

const pylonRefsFromStats = (stats: PublicPylonStats): ReadonlyArray<string> =>
  uniqueRefs(
    stats.recentPylons.map(
      (pylon, index) =>
        `pylon.public.recent.${safeSuffix(
          pylon.nostrPubkeyShort === 'unknown'
            ? `index_${index}`
            : pylon.nostrPubkeyShort,
        )}`,
    ),
  )

const optionalRef = (value: string | null): string | null =>
  value === null ? null : `nexus.public.relay.${safeSuffix(value)}`

const bitcoinPaidRef = (value: number | null): string | null =>
  value === null
    ? null
    : `bitcoin.public.accepted_work_paid.amount_${Math.max(0, value)}_sats`

export const artanisNexusPylonFleetSnapshotFromStats = (
  stats: PublicPylonStats,
  nowIso: string,
): ArtanisNexusPylonFleetSnapshotRecord => {
  const fleetState = fleetStateFromStats(stats, nowIso)

  return new ArtanisNexusPylonFleetSnapshotRecord({
    blockerRefs:
      fleetState === 'unavailable'
        ? ['blocker.public.omega_pylon_stats_unavailable']
        : fleetState === 'stale'
          ? ['blocker.public.omega_pylon_stats_stale']
          : [],
    caveatRefs: [
      'caveat.public.pylon_stats_are_read_only',
      'caveat.public.online_not_assignment_paid_or_settled',
      'caveat.public.no_sensitive_material',
      ...stats.caveatRefs,
    ],
    createdAtIso: nowIso,
    fleetState,
    hostedNexusRelayRef: optionalRef(stats.hostedNexusRelayUrl),
    nexusAcceptedWorkBitcoinPaidRef: bitcoinPaidRef(
      stats.nexusAcceptedWorkPayoutSatsPaidTotal,
    ),
    pylonRefs: pylonRefsFromStats(stats),
    pylonsOnlineNow: stats.pylonsOnlineNow,
    pylonsSeen24h: stats.pylonsSeen24h,
    pylonSessionsOnlineNow: stats.pylonSessionsOnlineNow,
    sellablePylonsOnlineNow: stats.sellablePylonsOnlineNow,
    snapshotRef: `snapshot.public.artanis.nexus_pylon.${safeSuffix(nowIso)}`,
    sourceRefs: uniqueRefs([
      'omega.public.pylon_api.registrations',
      ...stats.sourceRefs,
      stats.sourceUrl,
    ]),
    staleAfterIso: staleAfterIso(stats.asOfUnixMs, nowIso),
    surfaces: [...ARTANIS_NEXUS_PYLON_ADMIN_ADAPTER_SURFACES],
    trainingAcceptedContributors: stats.trainingAcceptedContributors,
    trainingAssignedContributors: stats.trainingAssignedContributors,
    trainingModelProgressContributors: stats.trainingModelProgressContributors,
    updatedAtIso: nowIso,
  })
}

const assertNoLiveAuthority = (
  authority: ArtanisNexusPylonAdminAuthority,
): void => {
  if (
    authority.livePylonJobDispatchAllowed ||
    authority.providerMutationAllowed ||
    authority.walletSpendAllowed ||
    authority.paymentSpendAllowed ||
    authority.settlementMutationAllowed ||
    authority.trainingLaunchAllowed ||
    authority.deploymentAllowed ||
    authority.runtimePromotionAllowed
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Artanis Nexus/Pylon adapter records do not grant live dispatch, provider mutation, wallet spend, payment spend, settlement, training, deployment, or runtime-promotion authority.',
    })
  }
}

const assertFleetSnapshot = (
  snapshot: ArtanisNexusPylonFleetSnapshotRecord,
): void => {
  assertValidIso('fleetSnapshot.createdAtIso', snapshot.createdAtIso)
  assertValidIso('fleetSnapshot.updatedAtIso', snapshot.updatedAtIso)
  assertValidIso('fleetSnapshot.staleAfterIso', snapshot.staleAfterIso)
  assertSafeRefs('Artanis Nexus/Pylon fleet snapshot ref', [
    snapshot.snapshotRef,
  ])
  assertSafeRefs('Artanis Nexus/Pylon fleet source refs', snapshot.sourceRefs)
  assertSafeRefs('Artanis Nexus/Pylon fleet caveat refs', snapshot.caveatRefs)
  assertSafeRefs('Artanis Nexus/Pylon fleet blocker refs', snapshot.blockerRefs)
  assertSafeRefs('Artanis Nexus/Pylon fleet pylon refs', snapshot.pylonRefs)
  assertSafeRefs('Artanis Nexus/Pylon hosted relay refs', [
    snapshot.hostedNexusRelayRef ?? 'nexus.public.relay.none',
  ])
  assertSafeRefs('Artanis Nexus/Pylon bitcoin paid refs', [
    snapshot.nexusAcceptedWorkBitcoinPaidRef ?? 'bitcoin.public.none',
  ])

  if (!hasAny(snapshot.surfaces)) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Artanis Nexus/Pylon fleet snapshots require adapter surfaces.',
    })
  }

  if (snapshot.fleetState === 'unavailable' && !hasAny(snapshot.blockerRefs)) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Unavailable Nexus/Pylon fleet snapshots require blocker refs.',
    })
  }
}

const approvedOrDispatched = (state: ArtanisNexusPylonDispatchState): boolean =>
  state === 'approved' || state === 'dispatch_recorded'

const assertDispatchRecord = (
  record: ArtanisNexusPylonDispatchRecord,
): void => {
  assertValidIso('dispatch.createdAtIso', record.createdAtIso)
  assertValidIso('dispatch.updatedAtIso', record.updatedAtIso)
  assertSafeRefs('Artanis Nexus/Pylon dispatch identity refs', [
    record.approvalGateRef,
    record.dispatchRef,
    record.idempotencyKey,
    record.marketplaceJobRef,
    record.nexusRouteRef,
    record.proposalRef,
    record.pylonRouteRef,
  ])
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch acceptance criteria refs',
    record.acceptanceCriteriaRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch accepted-work refs',
    record.acceptedWorkRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch approval refs',
    record.approvalRequirementRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch authority receipts',
    record.authorityReceiptRefs,
  )
  assertSafeRefs('Artanis Nexus/Pylon dispatch blockers', record.blockerRefs)
  assertSafeRefs('Artanis Nexus/Pylon dispatch caveats', record.caveatRefs)
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch estimated cost refs',
    record.estimatedCostRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch operator detail refs',
    record.operatorDetailRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch payment authority refs',
    record.paymentAuthorityRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch private evidence refs',
    record.privateEvidenceRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch provider eligibility refs',
    record.providerEligibilityRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch payout attempt refs',
    record.payoutAttemptRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch payout intent refs',
    record.payoutIntentRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch payout target approval refs',
    record.payoutTargetApprovalRefs,
  )
  assertSafeRefs('Artanis Nexus/Pylon dispatch receipts', record.receiptRefs)
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch run status refs',
    record.runStatusRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch settlement bridge refs',
    record.settlementBridgeRefs,
  )
  assertSafeRefs('Artanis Nexus/Pylon dispatch source refs', record.sourceRefs)
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch spend limit refs',
    record.spendLimitRefs,
  )
  assertSafeRefs(
    'Artanis Nexus/Pylon dispatch wallet readiness refs',
    record.walletReadinessRefs,
  )

  if (
    !hasAny(record.sourceRefs) ||
    !hasAny(record.acceptanceCriteriaRefs) ||
    !hasAny(record.estimatedCostRefs) ||
    !hasAny(record.spendLimitRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Artanis Nexus/Pylon dispatch records require source, acceptance, estimated-cost, and spend-limit refs.',
    })
  }

  if (
    approvedOrDispatched(record.state) &&
    (!hasAny(record.authorityReceiptRefs) ||
      !hasAny(record.providerEligibilityRefs) ||
      record.approvalGateRef === 'approval.public.none')
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Approved Nexus/Pylon dispatch records require approval, authority receipt, and provider eligibility refs.',
    })
  }

  if (
    record.state === 'dispatch_recorded' &&
    (!hasAny(record.receiptRefs) || !hasAny(record.runStatusRefs))
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Recorded Nexus/Pylon dispatches require adapter receipt and run-status refs.',
    })
  }

  if (record.state === 'blocked' && !hasAny(record.blockerRefs)) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Blocked Nexus/Pylon dispatches require blocker refs.',
    })
  }

  if (
    record.paymentAuthorityState === 'dispatch_blocked' &&
    !hasAny(record.blockerRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Payment-blocked Nexus/Pylon dispatches require specific blocker refs.',
    })
  }

  if (
    record.paymentAuthorityState === 'settlement_failed' &&
    !hasAny(record.blockerRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Failed Nexus/Pylon settlement states require specific blocker refs.',
    })
  }

  if (
    paymentAuthorityStateAtLeast(record.paymentAuthorityState, 'previewed') &&
    !hasAny(record.paymentAuthorityRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Previewed Nexus/Pylon payment-backed dispatches require payment authority refs.',
    })
  }

  if (
    paymentAuthorityStateAtLeast(
      record.paymentAuthorityState,
      'assignment_created',
    ) &&
    !hasAny(record.providerEligibilityRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Assignment-created Nexus/Pylon dispatches require provider eligibility refs.',
    })
  }

  if (
    paymentAuthorityStateAtLeast(
      record.paymentAuthorityState,
      'wallet_ready',
    ) &&
    !hasAny(record.walletReadinessRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Wallet-ready Nexus/Pylon dispatches require wallet readiness refs.',
    })
  }

  if (
    paymentAuthorityStateAtLeast(
      record.paymentAuthorityState,
      'payout_intent_created',
    ) &&
    (!hasAny(record.acceptedWorkRefs) ||
      !hasAny(record.payoutIntentRefs) ||
      !hasAny(record.payoutTargetApprovalRefs))
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Payout-intent Nexus/Pylon dispatches require accepted-work, payout-intent, and payout-target approval refs.',
    })
  }

  if (
    paymentAuthorityStateAtLeast(
      record.paymentAuthorityState,
      'dispatch_authorized',
    ) &&
    !hasAny(record.payoutAttemptRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Dispatch-authorized Nexus/Pylon payment states require payout attempt refs.',
    })
  }

  if (
    paymentAuthorityStateAtLeast(
      record.paymentAuthorityState,
      'settlement_pending',
    ) &&
    !hasAny(record.settlementBridgeRefs)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Settlement-pending Nexus/Pylon payment states require settlement bridge refs.',
    })
  }

  if (record.adapterMode === 'live' && record.state === 'dispatch_recorded') {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Live Nexus/Pylon dispatch recording is not enabled by this adapter contract.',
    })
  }
}

const assertLedger = (ledger: ArtanisNexusPylonAdminAdapterRecord): void => {
  assertValidIso('ledger.createdAtIso', ledger.createdAtIso)
  assertValidIso('ledger.updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('Artanis Nexus/Pylon adapter agent id', [ledger.agentId])
  assertSafeRefs('Artanis Nexus/Pylon adapter ledger ref', [ledger.ledgerRef])
  assertSafeRefs('Artanis Nexus/Pylon adapter caveat refs', ledger.caveatRefs)
  assertSafeRefs('Artanis Nexus/Pylon adapter source refs', ledger.sourceRefs)
  assertNoLiveAuthority(ledger.authority)

  if (ledger.agentId !== 'agent_artanis') {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Artanis Nexus/Pylon adapter ledgers must use agent_artanis.',
    })
  }

  if (!hasAny(ledger.fleetSnapshots)) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Artanis Nexus/Pylon adapter ledgers require fleet snapshots.',
    })
  }

  ledger.fleetSnapshots.forEach(assertFleetSnapshot)
  ledger.dispatchRecords.forEach(assertDispatchRecord)
}

const projectFleetSnapshot = (
  snapshot: ArtanisNexusPylonFleetSnapshotRecord,
  audience: ArtanisNexusPylonAdapterAudience,
  nowIso: string,
): ArtanisNexusPylonFleetSnapshotProjection =>
  new ArtanisNexusPylonFleetSnapshotProjection({
    blockerRefs: refsForAudience(
      'Artanis Nexus/Pylon fleet blocker refs',
      snapshot.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis Nexus/Pylon fleet caveat refs',
      snapshot.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      snapshot.createdAtIso,
      nowIso,
    ),
    fleetState: snapshot.fleetState,
    hostedNexusRelayRef:
      refsForAudience(
        'Artanis Nexus/Pylon hosted relay refs',
        [snapshot.hostedNexusRelayRef ?? 'nexus.public.relay.none'],
        audience,
      )[0] ?? null,
    nexusAcceptedWorkBitcoinPaidRef:
      refsForAudience(
        'Artanis Nexus/Pylon bitcoin paid refs',
        [snapshot.nexusAcceptedWorkBitcoinPaidRef ?? 'bitcoin.public.none'],
        audience,
      )[0] ?? null,
    pylonRefs: refsForAudience(
      'Artanis Nexus/Pylon pylon refs',
      snapshot.pylonRefs,
      audience,
    ),
    pylonsOnlineNow: snapshot.pylonsOnlineNow,
    pylonsSeen24h: snapshot.pylonsSeen24h,
    pylonSessionsOnlineNow: snapshot.pylonSessionsOnlineNow,
    sellablePylonsOnlineNow: snapshot.sellablePylonsOnlineNow,
    snapshotRef:
      refsForAudience(
        'Artanis Nexus/Pylon snapshot ref',
        [snapshot.snapshotRef],
        audience,
      )[0] ?? 'snapshot.redacted.artanis_nexus_pylon',
    sourceRefs: refsForAudience(
      'Artanis Nexus/Pylon fleet source refs',
      snapshot.sourceRefs,
      audience,
    ),
    staleAfterDisplay: friendlyBlueprintMissionBriefingTime(
      snapshot.staleAfterIso,
      nowIso,
    ),
    surfaces: [...snapshot.surfaces].sort(),
    trainingAcceptedContributors: snapshot.trainingAcceptedContributors,
    trainingAssignedContributors: snapshot.trainingAssignedContributors,
    trainingModelProgressContributors:
      snapshot.trainingModelProgressContributors,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      snapshot.updatedAtIso,
      nowIso,
    ),
  })

const projectDispatchRecord = (
  record: ArtanisNexusPylonDispatchRecord,
  audience: ArtanisNexusPylonAdapterAudience,
  nowIso: string,
): ArtanisNexusPylonDispatchProjection =>
  new ArtanisNexusPylonDispatchProjection({
    acceptanceCriteriaRefs: refsForAudience(
      'Artanis Nexus/Pylon acceptance criteria refs',
      record.acceptanceCriteriaRefs,
      audience,
    ),
    acceptedWorkRefs: refsForAudience(
      'Artanis Nexus/Pylon accepted-work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    adapterMode: record.adapterMode,
    approvalGateRef:
      refsForAudience(
        'Artanis Nexus/Pylon approval gate refs',
        [record.approvalGateRef],
        audience,
      )[0] ?? 'approval.redacted.artanis_nexus_pylon',
    approvalRequirementRefs: refsForAudience(
      'Artanis Nexus/Pylon approval requirement refs',
      record.approvalRequirementRefs,
      audience,
    ),
    authorityReceiptRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon authority receipt refs',
            record.authorityReceiptRefs,
            audience,
          )
        : [],
    blockerRefs: refsForAudience(
      'Artanis Nexus/Pylon blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis Nexus/Pylon caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    dispatchRef:
      refsForAudience(
        'Artanis Nexus/Pylon dispatch ref',
        [record.dispatchRef],
        audience,
      )[0] ?? 'dispatch.redacted.artanis_nexus_pylon',
    estimatedCostRefs: refsForAudience(
      'Artanis Nexus/Pylon estimated cost refs',
      record.estimatedCostRefs,
      audience,
    ),
    fakeDispatchReceiptRecorded:
      record.adapterMode === 'fake' && record.state === 'dispatch_recorded',
    idempotencyKey:
      audience === 'operator'
        ? (refsForAudience(
            'Artanis Nexus/Pylon idempotency key',
            [record.idempotencyKey],
            audience,
          )[0] ?? 'idempotency.redacted.artanis_nexus_pylon')
        : 'idempotency.redacted.artanis_nexus_pylon',
    jobKind: record.jobKind,
    liveDispatchClaimAllowed: false,
    marketplaceJobRef:
      refsForAudience(
        'Artanis Nexus/Pylon marketplace job ref',
        [record.marketplaceJobRef],
        audience,
      )[0] ?? 'marketplace_job.redacted.artanis_nexus_pylon',
    nexusRouteRef:
      refsForAudience(
        'Artanis Nexus/Pylon Nexus route ref',
        [record.nexusRouteRef],
        audience,
      )[0] ?? 'nexus_route.redacted.artanis_nexus_pylon',
    operatorDetailRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon operator detail refs',
            record.operatorDetailRefs,
            audience,
          )
        : [],
    paymentAuthorityBlocked:
      record.paymentAuthorityState === 'dispatch_blocked' ||
      record.paymentAuthorityState === 'settlement_failed',
    paymentAuthorityGatePassed: paymentAuthorityStateAtLeast(
      record.paymentAuthorityState,
      'dispatch_authorized',
    ),
    paymentAuthorityRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon payment authority refs',
            record.paymentAuthorityRefs,
            audience,
          )
        : [],
    paymentAuthorityState: record.paymentAuthorityState,
    paymentAuthorityStateLabel:
      paymentAuthorityStateLabel[record.paymentAuthorityState],
    privateEvidenceRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon private evidence refs',
            record.privateEvidenceRefs,
            audience,
          )
        : [],
    proposalRef:
      refsForAudience(
        'Artanis Nexus/Pylon proposal ref',
        [record.proposalRef],
        audience,
      )[0] ?? 'proposal.redacted.artanis_nexus_pylon',
    providerEligibilityRefs: refsForAudience(
      'Artanis Nexus/Pylon provider eligibility refs',
      record.providerEligibilityRefs,
      audience,
    ),
    payoutAttemptRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon payout attempt refs',
            record.payoutAttemptRefs,
            audience,
          )
        : [],
    payoutIntentRefs: refsForAudience(
      'Artanis Nexus/Pylon payout intent refs',
      record.payoutIntentRefs,
      audience,
    ),
    payoutTargetApprovalRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon payout target approval refs',
            record.payoutTargetApprovalRefs,
            audience,
          )
        : [],
    pylonRouteRef:
      refsForAudience(
        'Artanis Nexus/Pylon Pylon route ref',
        [record.pylonRouteRef],
        audience,
      )[0] ?? 'pylon_route.redacted.artanis_nexus_pylon',
    receiptRefs: refsForAudience(
      'Artanis Nexus/Pylon receipt refs',
      record.receiptRefs,
      audience,
    ),
    resourceMode: record.resourceMode,
    runStatusRefs: refsForAudience(
      'Artanis Nexus/Pylon run status refs',
      record.runStatusRefs,
      audience,
    ),
    settlementBridgeRefs: refsForAudience(
      'Artanis Nexus/Pylon settlement bridge refs',
      record.settlementBridgeRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Artanis Nexus/Pylon dispatch source refs',
      record.sourceRefs,
      audience,
    ),
    spendLimitRefs: refsForAudience(
      'Artanis Nexus/Pylon spend limit refs',
      record.spendLimitRefs,
      audience,
    ),
    state: record.state,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletReadinessRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis Nexus/Pylon wallet readiness refs',
            record.walletReadinessRefs,
            audience,
          )
        : [],
  })

const projectionValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionValues)
  }

  return []
}

const allowedPublicLiteralValues = new Set<string>([
  ...ARTANIS_NEXUS_PYLON_ADMIN_ADAPTER_SURFACES,
  'approved',
  'assignment_created',
  'awaiting_approval',
  'background_20',
  'balanced',
  'benchmark_evaluation',
  'blocked',
  'dedicated_full_blast',
  'degraded',
  'dispatch_authorized',
  'dispatch_blocked',
  'dispatch_recorded',
  'embedding_data_prep',
  'failed',
  'fake',
  'gepa_dspy_optimization',
  'held_for_approval',
  'inference',
  'live',
  'lora_finetuning',
  'operator',
  'overnight_full',
  'payout_intent_created',
  'previewed',
  'proposed',
  'public_artanis',
  'public_forum',
  'settlement_complete',
  'settlement_failed',
  'settlement_pending',
  'stale',
  'training',
  'unavailable',
  'validation',
  'wallet_ready',
])

export const artanisNexusPylonProjectionHasPrivateMaterial = (
  projection: ArtanisNexusPylonAdminAdapterProjection,
): boolean =>
  projectionValues(projection).some(
    value =>
      !allowedPublicLiteralValues.has(value) &&
      (unsafeAdapterRefPattern.test(value) ||
        rawTimestampPattern.test(value) ||
        publicUnsafeRefPattern.test(value)),
  )

export const projectArtanisNexusPylonAdminAdapter = (
  ledger: ArtanisNexusPylonAdminAdapterRecord,
  audience: ArtanisNexusPylonAdapterAudience,
  nowIso: string,
): ArtanisNexusPylonAdminAdapterProjection => {
  assertLedger(ledger)

  const fleetSnapshots = ledger.fleetSnapshots.map(snapshot =>
    projectFleetSnapshot(snapshot, audience, nowIso),
  )
  const dispatchRecords = ledger.dispatchRecords.map(record =>
    projectDispatchRecord(record, audience, nowIso),
  )
  const latestFleetState = fleetSnapshots[0]?.fleetState ?? 'unavailable'
  const projection = new ArtanisNexusPylonAdminAdapterProjection({
    agentId: ledger.agentId,
    audience,
    authority: ledger.authority,
    caveatRefs: refsForAudience(
      'Artanis Nexus/Pylon adapter caveat refs',
      ledger.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.createdAtIso,
      nowIso,
    ),
    dispatchRecords,
    fleetSnapshots,
    latestFleetState,
    ledgerRef:
      refsForAudience(
        'Artanis Nexus/Pylon adapter ledger ref',
        [ledger.ledgerRef],
        audience,
      )[0] ?? 'ledger.redacted.artanis_nexus_pylon',
    sourceRefs: refsForAudience(
      'Artanis Nexus/Pylon adapter source refs',
      ledger.sourceRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
  })

  if (
    audience !== 'operator' &&
    artanisNexusPylonProjectionHasPrivateMaterial(projection)
  ) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Public Artanis Nexus/Pylon adapter projection contains private material.',
    })
  }

  return projection
}

const dispatchReceiptRef = (record: ArtanisNexusPylonDispatchRecord): string =>
  `receipt.public.artanis.nexus_pylon_dispatch.${safeSuffix(record.dispatchRef)}`

const runStatusRef = (record: ArtanisNexusPylonDispatchRecord): string =>
  `run.public.artanis.nexus_pylon_dispatch.${safeSuffix(record.dispatchRef)}.queued`

export const createFakeArtanisNexusPylonDispatchAdapter = (): Readonly<{
  calls: Array<ArtanisNexusPylonDispatchAdapterCall>
  adapter: ArtanisNexusPylonDispatchAdapter
}> => {
  const calls: Array<ArtanisNexusPylonDispatchAdapterCall> = []
  const adapter: ArtanisNexusPylonDispatchAdapter = {
    dispatch: record => {
      const call = new ArtanisNexusPylonDispatchAdapterCall({
        adapterMode: record.adapterMode,
        dispatchRef: record.dispatchRef,
        idempotencyKey: record.idempotencyKey,
        nexusRouteRef: record.nexusRouteRef,
        proposalRef: record.proposalRef,
        pylonRouteRef: record.pylonRouteRef,
      })
      calls.push(call)

      return new ArtanisNexusPylonDispatchAdapterReceipt({
        adapterMode: record.adapterMode,
        dispatchRef: record.dispatchRef,
        nexusRouteRef: record.nexusRouteRef,
        pylonRouteRef: record.pylonRouteRef,
        receiptRef: dispatchReceiptRef(record),
        runStatusRef: runStatusRef(record),
      })
    },
  }

  return { adapter, calls }
}

export const dispatchApprovedArtanisNexusPylonRecord = (
  input: Readonly<{
    adapter: ArtanisNexusPylonDispatchAdapter
    approvalGate: ArtanisApprovalGateRecord
    nowIso: string
    record: ArtanisNexusPylonDispatchRecord
  }>,
): Readonly<{
  receipt: ArtanisNexusPylonDispatchAdapterReceipt
  record: ArtanisNexusPylonDispatchRecord
}> => {
  assertDispatchRecord(input.record)

  if (input.record.adapterMode !== 'fake') {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Only the fake Nexus/Pylon adapter can be dispatched by this contract.',
    })
  }

  if (input.record.state !== 'approved') {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Only approved Nexus/Pylon dispatch records can be dispatched.',
    })
  }

  if (input.approvalGate.kind !== 'pylon_job_dispatch') {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Nexus/Pylon dispatch requires a pylon_job_dispatch approval gate.',
    })
  }

  if (input.approvalGate.gateRef !== input.record.approvalGateRef) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason:
        'Nexus/Pylon dispatch approval gate must match the dispatch record.',
    })
  }

  if (!artanisApprovalGateEffective(input.approvalGate, input.nowIso)) {
    throw new ArtanisNexusPylonAdapterUnsafe({
      reason: 'Nexus/Pylon dispatch approval gate is not effective.',
    })
  }

  const receipt = input.adapter.dispatch(input.record)
  const record = new ArtanisNexusPylonDispatchRecord({
    ...input.record,
    receiptRefs: uniqueRefs([...input.record.receiptRefs, receipt.receiptRef]),
    runStatusRefs: uniqueRefs([
      ...input.record.runStatusRefs,
      receipt.runStatusRef,
    ]),
    state: 'dispatch_recorded',
    updatedAtIso: input.nowIso,
  })
  assertDispatchRecord(record)

  return { receipt, record }
}

export type ArtanisNexusPylonPaymentBackedDispatchResult = Readonly<{
  blockedReason: TreasuryPaymentAuthorityRejectionReason | null
  creation: TreasuryPaymentAuthorityIntentCreationResult | null
  dispatch: TreasuryPaymentAuthorityDispatchResult | null
  preview: TreasuryPaymentAuthorityPayoutPreview | null
  record: ArtanisNexusPylonDispatchRecord
}>

const paymentAuthorityRef = (suffix: string): string =>
  `payment_authority.public.artanis_nexus_pylon.${safeSuffix(suffix)}`

const walletReadinessRef = (
  walletReadiness: TreasuryPaymentAuthorityWalletReadiness,
  suffix: string,
): string => `wallet_readiness.public.${walletReadiness}.${safeSuffix(suffix)}`

const runStatusForPaymentDispatch = (
  record: ArtanisNexusPylonDispatchRecord,
): string =>
  `run.public.artanis.nexus_pylon_payment_dispatch.${safeSuffix(record.dispatchRef)}.authorized`

const blockedPaymentBackedDispatchResult = (
  input: Readonly<{
    attempt: NexusTreasuryPayoutAttemptRecord
    error: TreasuryPaymentAuthorityError
    intent: NexusTreasuryPayoutIntentRecord
    nowIso: string
    record: ArtanisNexusPylonDispatchRecord
    walletReadiness: TreasuryPaymentAuthorityWalletReadiness
  }>,
): ArtanisNexusPylonPaymentBackedDispatchResult => {
  const reasonRef = `blocker.public.payment_authority.${input.error.reason}`
  const record = new ArtanisNexusPylonDispatchRecord({
    ...input.record,
    acceptedWorkRefs: uniqueRefs([
      ...input.record.acceptedWorkRefs,
      ...input.intent.acceptedWorkRefs,
    ]),
    blockerRefs: uniqueRefs([...input.record.blockerRefs, reasonRef]),
    paymentAuthorityRefs: uniqueRefs([
      ...input.record.paymentAuthorityRefs,
      paymentAuthorityRef(`blocked.${input.error.reason}`),
      input.intent.policySnapshotRef,
    ]),
    paymentAuthorityState: 'dispatch_blocked',
    payoutAttemptRefs: uniqueRefs(input.record.payoutAttemptRefs),
    payoutIntentRefs: uniqueRefs([
      ...input.record.payoutIntentRefs,
      input.intent.payoutIntentRef,
    ]),
    payoutTargetApprovalRefs: uniqueRefs([
      ...input.record.payoutTargetApprovalRefs,
      ...(input.intent.payoutTargetApprovalRef === null
        ? []
        : [input.intent.payoutTargetApprovalRef]),
    ]),
    state: 'blocked',
    updatedAtIso: input.nowIso,
    walletReadinessRefs: uniqueRefs([
      ...input.record.walletReadinessRefs,
      walletReadinessRef(input.walletReadiness, input.intent.payoutIntentRef),
    ]),
  })
  assertDispatchRecord(record)

  return {
    blockedReason: input.error.reason,
    creation: null,
    dispatch: null,
    preview: null,
    record,
  }
}

export const runArtanisNexusPylonPaymentBackedDispatch = (
  input: Readonly<{
    attempt: NexusTreasuryPayoutAttemptRecord
    intent: NexusTreasuryPayoutIntentRecord
    nowIso: string
    paymentAuthority: TreasuryPaymentAuthorityShape
    record: ArtanisNexusPylonDispatchRecord
    settlementBridgeRefs?: ReadonlyArray<string> | undefined
    walletReadiness: TreasuryPaymentAuthorityWalletReadiness
  }>,
): Effect.Effect<ArtanisNexusPylonPaymentBackedDispatchResult, never> =>
  Effect.gen(function* () {
    assertDispatchRecord(input.record)

    const preview = yield* input.paymentAuthority.previewPayout({
      intent: input.intent,
      walletReadiness: input.walletReadiness,
    })
    const creation = yield* input.paymentAuthority.createPayoutIntent({
      intent: input.intent,
      walletReadiness: input.walletReadiness,
    })
    const dispatch = yield* input.paymentAuthority.dispatchPayout({
      attempt: input.attempt,
      payoutIntentRef: input.intent.payoutIntentRef,
    })
    const record = new ArtanisNexusPylonDispatchRecord({
      ...input.record,
      acceptedWorkRefs: uniqueRefs([
        ...input.record.acceptedWorkRefs,
        ...input.intent.acceptedWorkRefs,
      ]),
      authorityReceiptRefs: uniqueRefs([
        ...input.record.authorityReceiptRefs,
        paymentAuthorityRef(`preview.${input.intent.payoutIntentRef}`),
        paymentAuthorityRef(`intent.${input.intent.payoutIntentRef}`),
      ]),
      paymentAuthorityRefs: uniqueRefs([
        ...input.record.paymentAuthorityRefs,
        input.intent.policySnapshotRef,
        preview.payoutIntentRef,
        paymentAuthorityRef(`dispatch.${dispatch.attempt.payoutAttemptRef}`),
      ]),
      paymentAuthorityState: 'dispatch_authorized',
      payoutAttemptRefs: uniqueRefs([
        ...input.record.payoutAttemptRefs,
        dispatch.attempt.payoutAttemptRef,
        dispatch.attempt.adapterAttemptRef,
      ]),
      payoutIntentRefs: uniqueRefs([
        ...input.record.payoutIntentRefs,
        creation.intent.payoutIntentRef,
      ]),
      payoutTargetApprovalRefs: uniqueRefs([
        ...input.record.payoutTargetApprovalRefs,
        preview.payoutTargetApprovalRef,
      ]),
      receiptRefs: uniqueRefs([
        ...input.record.receiptRefs,
        dispatch.attempt.payoutAttemptRef,
      ]),
      runStatusRefs: uniqueRefs([
        ...input.record.runStatusRefs,
        runStatusForPaymentDispatch(input.record),
      ]),
      settlementBridgeRefs: uniqueRefs([
        ...input.record.settlementBridgeRefs,
        ...(input.settlementBridgeRefs ?? []),
      ]),
      state: 'dispatch_recorded',
      updatedAtIso: input.nowIso,
      walletReadinessRefs: uniqueRefs([
        ...input.record.walletReadinessRefs,
        walletReadinessRef(input.walletReadiness, input.intent.payoutIntentRef),
      ]),
    })
    assertDispatchRecord(record)

    return {
      blockedReason: null,
      creation,
      dispatch,
      preview,
      record,
    }
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        blockedPaymentBackedDispatchResult({
          attempt: input.attempt,
          error,
          intent: input.intent,
          nowIso: input.nowIso,
          record: input.record,
          walletReadiness: input.walletReadiness,
        }),
      ),
    ),
  )

const dispatchRecord = (
  input: Omit<
    ArtanisNexusPylonDispatchRecord,
    'dispatchRef' | 'idempotencyKey'
  > & {
    dispatchRefSuffix: string
  },
): ArtanisNexusPylonDispatchRecord => {
  const { dispatchRefSuffix, ...record } = input

  return new ArtanisNexusPylonDispatchRecord({
    ...record,
    dispatchRef: `dispatch.public.artanis.nexus_pylon.${dispatchRefSuffix}`,
    idempotencyKey: `artanis-nexus-pylon-dispatch:${dispatchRefSuffix}:v1`,
  })
}

export const exampleArtanisNexusPylonAdminAdapterLedger =
  (): ArtanisNexusPylonAdminAdapterRecord => {
    const nowIso = '2026-06-07T06:00:00.000Z'
    const stats = new PublicPylonStats({
      asOfLabel: '2026-06-07T05:55:00.000Z',
      asOfUnixMs: Date.parse('2026-06-07T05:55:00.000Z'),
      available: true,
      error: null,
      hostedNexusRelayUrl: 'https://nexus.openagents.com',
      minimumClientVersion: 'legacy-nexus',
      nexusAcceptedWorkPayoutReceiptRefs: [
        'receipt.nexus.public.example.accepted_work_settlement',
      ],
      nexusAcceptedWorkPayoutSatsPaid24h: 21000,
      nexusAcceptedWorkPayoutSatsPaidTotal: 101000,
      nexusAcceptedWorkSettlementGate:
        new PublicPylonAcceptedWorkSettlementGate({
          blockerRefs: [],
          caveatRefs: [
            'caveat.public.pylon_settlement.simulation_receipts_do_not_count',
            'caveat.public.pylon_settlement.payment_receipt_without_settlement_does_not_count',
            'caveat.public.pylon_settlement.duplicate_retries_count_once',
            'caveat.public.no_private_payment_material',
          ],
          gateRef: 'gate.public.pylon.accepted_work_settlement_receipts.v1',
          publicPaidWorkTotalsAllowed: true,
          receiptBackedTotalsAvailable: true,
          settledReceiptRefs: [
            'receipt.nexus.public.example.accepted_work_settlement',
          ],
          sourceRefs: [
            'gate.public.pylon.accepted_work_settlement_receipts.v1',
            'nexus.public.stats',
            'route:/api/public/nexus-pylon/receipts/receipt.nexus.public.example.accepted_work_settlement',
          ],
          state: 'ready',
          stateLabel: 'Receipt-backed accepted-work settlement totals ready',
        }),
      nip90MarketSettlementStats: emptyUnavailableMarketSettlementTotals(
        'NIP-90 market receipt store unavailable.',
      ),
      nexusPayoutSatsPaidTotal: 101000,
      publicRealSatsSettled24h: 21000,
      publicRealSatsSettledTotal: 101000,
      pylonSessionsOnlineNow: 3,
      pylonsAssignmentReadyNow: 2,
      pylonsByClientVersion: {},
      pylonsByResourceMode: {},
      pylonsOnlineNow: 3,
      pylonsRegisteredTotal: 3,
      pylonsSeen24h: 5,
      pylonsWalletReadyNow: 2,
      recentPylons: [],
      sellablePylonsOnlineNow: 2,
      sourceUrl: 'https://nexus.openagents.com/api/stats',
      sourceRefs: ['nexus.public.stats'],
      status: 'live',
      trainingAcceptedContributors: 1,
      trainingAssignedContributors: 2,
      trainingModelProgressContributors: 1,
      treasuryPayoutCount24h: null,
      treasuryPayoutCountTotal: null,
      treasuryPayoutSatsPaid24h: null,
      treasuryPayoutSatsPaidTotal: null,
      counterWindows: publicPylonStatsCounterWindows(),
      earningLaunchGate: new PublicPylonEarningLaunchGate({
        blockedClaimRefs: [],
        blockerRefs: [],
        caveatRefs: [
          'caveat.public.pylon_online_is_not_paid_work',
          'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
          'caveat.public.assignment_ready_is_not_acceptance_or_settlement',
          'caveat.public.no_unconditional_earning_promise',
        ],
        gateRef: 'gate.public.pylon.earning_network_counters.v1',
        publicEarningCopyAllowed: true,
        requiredAssignmentReadyPylonsPresent: true,
        requiredOnlinePylonsPresent: true,
        requiredWalletReadyPylonsPresent: true,
        sourceRefs: ['nexus.public.stats'],
        state: 'ready',
        stateLabel: 'Ready for bounded public earning copy',
      }),
      caveatRefs: ['caveat.public.legacy_nexus_fixture'],
    })

    return new ArtanisNexusPylonAdminAdapterRecord({
      agentId: 'agent_artanis',
      authority: ARTANIS_NEXUS_PYLON_ADMIN_NO_LIVE_AUTHORITY,
      caveatRefs: [
        'caveat.public.nexus_pylon_adapter_fake_dispatch_only',
        'caveat.public.no_live_sensitive_authority',
      ],
      createdAtIso: nowIso,
      dispatchRecords: [
        dispatchRecord({
          acceptanceCriteriaRefs: ['criteria.public.pylon_inference_trace'],
          acceptedWorkRefs: [],
          adapterMode: 'fake',
          approvalGateRef: 'gate.public.artanis.pylon_job_dispatch_approved',
          approvalRequirementRefs: [
            'approval.public.artanis.pylon_dispatch_approved',
          ],
          authorityReceiptRefs: [
            'authority.public.artanis.pylon_dispatch.approved',
          ],
          blockerRefs: [],
          caveatRefs: [
            'caveat.public.fake_adapter_only',
            'caveat.public.accepted_work_settlement_separate',
          ],
          createdAtIso: '2026-06-07T06:01:00.000Z',
          dispatchRefSuffix: 'pylon_inference_fake_dispatch',
          estimatedCostRefs: ['cost.public.pylon_inference_low'],
          jobKind: 'inference',
          marketplaceJobRef: 'job.public.pylon.inference.seeded_001',
          nexusRouteRef: 'nexus.route.public.pylon.assign_job.v1',
          operatorDetailRefs: ['operator.artanis.nexus_pylon.dispatch_review'],
          paymentAuthorityRefs: [],
          paymentAuthorityState: 'proposed',
          privateEvidenceRefs: ['evidence.private.artanis.nexus_pylon.review'],
          proposalRef: 'work.public.artanis.pylon_inference_accepted',
          providerEligibilityRefs: ['eligibility.public.pylon.sellable_online'],
          payoutAttemptRefs: [],
          payoutIntentRefs: [],
          payoutTargetApprovalRefs: [],
          pylonRouteRef: 'pylon.route.public.assignment.receive.v1',
          receiptRefs: [],
          resourceMode: 'background_20',
          runStatusRefs: [],
          settlementBridgeRefs: [],
          sourceRefs: [
            'nexus.public.stats',
            'pylon.public.stats',
            'work.public.artanis.pylon_inference_accepted',
          ],
          spendLimitRefs: ['spend_limit.public.pylon_inference_zero_spend'],
          state: 'approved',
          updatedAtIso: '2026-06-07T06:01:00.000Z',
          walletReadinessRefs: [],
        }),
        dispatchRecord({
          acceptanceCriteriaRefs: ['criteria.public.lora_adapter_eval'],
          acceptedWorkRefs: [],
          adapterMode: 'live',
          approvalGateRef: 'approval.public.none',
          approvalRequirementRefs: [
            'approval.public.artanis.training_launch_pending',
          ],
          authorityReceiptRefs: [],
          blockerRefs: ['blocker.public.live_dispatch_not_enabled'],
          caveatRefs: ['caveat.public.live_dispatch_requires_launch_gate'],
          createdAtIso: '2026-06-07T06:02:00.000Z',
          dispatchRefSuffix: 'training_live_dispatch_blocked',
          estimatedCostRefs: ['cost.public.training_requires_budget'],
          jobKind: 'lora_finetuning',
          marketplaceJobRef: 'job.public.pylon.lora.seeded_002',
          nexusRouteRef: 'nexus.route.public.pylon.assign_job.v1',
          operatorDetailRefs: ['operator.artanis.training_dispatch_review'],
          paymentAuthorityRefs: [],
          paymentAuthorityState: 'dispatch_blocked',
          privateEvidenceRefs: ['evidence.private.artanis.training_budget'],
          proposalRef: 'work.public.artanis.lora_training_blocked',
          providerEligibilityRefs: [],
          payoutAttemptRefs: [],
          payoutIntentRefs: [],
          payoutTargetApprovalRefs: [],
          pylonRouteRef: 'pylon.route.public.assignment.receive.v1',
          receiptRefs: [],
          resourceMode: 'overnight_full',
          runStatusRefs: [],
          settlementBridgeRefs: [],
          sourceRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
          spendLimitRefs: ['spend_limit.public.training_not_approved'],
          state: 'blocked',
          updatedAtIso: '2026-06-07T06:02:00.000Z',
          walletReadinessRefs: [],
        }),
      ],
      fleetSnapshots: [artanisNexusPylonFleetSnapshotFromStats(stats, nowIso)],
      ledgerRef: 'ledger.public.artanis.nexus_pylon_admin_adapters',
      sourceRefs: [
        'docs.public.artanis.nexus_pylon_admin_adapters',
        'nexus.public.stats',
        'pylon.public.stats',
      ],
      updatedAtIso: '2026-06-07T06:03:00.000Z',
    })
  }
