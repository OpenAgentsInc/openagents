import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonLdkSettlementReadinessState = S.Literals([
  'attention_required',
  'blocked',
  'degraded',
  'ready',
  'stale',
  'unknown',
])
export type PylonLdkSettlementReadinessState =
  typeof PylonLdkSettlementReadinessState.Type

export const PylonLdkSettlementRailKind = S.Literals([
  'ldk',
  'nexus',
  'pylon',
  'treasury',
])
export type PylonLdkSettlementRailKind =
  typeof PylonLdkSettlementRailKind.Type

export const PylonLdkReadinessVisibility = S.Literals(['private', 'public'])
export type PylonLdkReadinessVisibility =
  typeof PylonLdkReadinessVisibility.Type

export const PylonLdkReadinessAuthorityBoundary = S.Literals([
  'read_only_projection',
])
export type PylonLdkReadinessAuthorityBoundary =
  typeof PylonLdkReadinessAuthorityBoundary.Type

export class PylonLdkReadinessAuthority extends S.Class<PylonLdkReadinessAuthority>(
  'PylonLdkReadinessAuthority',
)({
  authorityBoundary: PylonLdkReadinessAuthorityBoundary,
  noBuyerChargeMutation: S.Boolean,
  noChannelOpenMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noNexusMutation: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetDisclosure: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
  noTreasuryMutation: S.Boolean,
}) {}

export class PylonLdkReadinessRecord extends S.Class<PylonLdkReadinessRecord>(
  'PylonLdkReadinessRecord',
)({
  authority: PylonLdkReadinessAuthority,
  balanceEvidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  channelPostureRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  failedRouteCount: S.Number,
  failedRouteRefs: S.Array(S.String),
  id: S.String,
  noRouteCount: S.Number,
  operatorActionRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: PylonLdkReadinessVisibility,
  readinessState: PylonLdkSettlementReadinessState,
  railKind: PylonLdkSettlementRailKind,
  railRef: S.String,
  railVisibility: PylonLdkReadinessVisibility,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonLdkReadinessProjection extends S.Class<PylonLdkReadinessProjection>(
  'PylonLdkReadinessProjection',
)({
  audience: OmniProjectionAudience,
  authority: PylonLdkReadinessAuthority,
  balanceEvidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  channelOpenMutationAllowed: S.Boolean,
  channelPostureRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  failedRouteCount: S.Number,
  failedRouteRefs: S.Array(S.String),
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  nexusMutationAllowed: S.Boolean,
  noRouteCount: S.Number,
  operatorActionRefs: S.Array(S.String),
  payoutDispatchMutationAllowed: S.Boolean,
  payoutTargetMutationAllowed: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonLdkReadinessVisibility,
  readinessState: PylonLdkSettlementReadinessState,
  readinessStateLabel: S.String,
  railKind: PylonLdkSettlementRailKind,
  railRef: S.String,
  railVisibility: PylonLdkReadinessVisibility,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  treasuryMutationAllowed: S.Boolean,
  updatedAtDisplay: S.String,
}) {}

export class PylonLdkReadinessUnsafe extends S.TaggedErrorClass<PylonLdkReadinessUnsafe>()(
  'PylonLdkReadinessUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_LDK_READINESS_READ_ONLY_AUTHORITY:
  PylonLdkReadinessAuthority = {
    authorityBoundary: 'read_only_projection',
    noBuyerChargeMutation: true,
    noChannelOpenMutation: true,
    noLiveWalletSpend: true,
    noNexusMutation: true,
    noPayoutDispatch: true,
    noPayoutTargetDisclosure: true,
    noPayoutTargetMutation: true,
    noSettlementMutation: true,
    noTreasuryMutation: true,
  }

const readinessLabelByState:
  Readonly<Record<PylonLdkSettlementReadinessState, string>> = {
    attention_required: 'Needs attention',
    blocked: 'Blocked',
    degraded: 'Degraded',
    ready: 'Ready',
    stale: 'Stale',
    unknown: 'Unknown',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePylonLdkReadinessRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|invoice|payment|payload|payout|prompt|provider|runner|run[_-]?log|state|webhook)|recovery[_-]?phrase|runner[_-]?log|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(balance\.private|channel\.private|failed_route\.private|operator\.action|provider\.private|rail\.private|source\.private)/i
const customerUnsafeRefPattern =
  /(balance\.private|channel\.private|failed_route\.private|operator\.action|provider\.private|rail\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(balance\.private|channel\.private|failed_route\.private|operator\.action|provider\.private|rail\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafePylonLdkReadinessRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonLdkReadinessUnsafe({
      reason: `${label} contains wallet material, raw payment material, invoices, preimages, payout targets, private channel state, provider secrets, raw logs, customer data, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const visibleRef = (
  label: string,
  ref: string,
  visibility: PylonLdkReadinessVisibility,
  redactedRef: string,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    visibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience(label, [ref], audience)[0] ?? redactedRef
  }

  return redactedRef
}

export const pylonLdkReadinessHasNoSpendAuthority = (
  authority: PylonLdkReadinessAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_projection' &&
  authority.noBuyerChargeMutation &&
  authority.noChannelOpenMutation &&
  authority.noLiveWalletSpend &&
  authority.noNexusMutation &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetDisclosure &&
  authority.noPayoutTargetMutation &&
  authority.noSettlementMutation &&
  authority.noTreasuryMutation

export const pylonLdkReadinessCanMutateSettlement = (
  record: PylonLdkReadinessRecord,
): boolean => !pylonLdkReadinessHasNoSpendAuthority(record.authority)

const assertCounts = (record: PylonLdkReadinessRecord): void => {
  if (
    !Number.isInteger(record.failedRouteCount) ||
    record.failedRouteCount < 0 ||
    !Number.isInteger(record.noRouteCount) ||
    record.noRouteCount < 0
  ) {
    throw new PylonLdkReadinessUnsafe({
      reason: 'LDK readiness route counts must be non-negative integers.',
    })
  }
}

const assertRecordSafe = (record: PylonLdkReadinessRecord): void => {
  assertSafeRefs('LDK readiness identity refs', [
    record.id,
    record.providerRef,
    record.railRef,
  ])
  assertSafeRefs('LDK readiness balance refs', record.balanceEvidenceRefs)
  assertSafeRefs('LDK readiness channel refs', record.channelPostureRefs)
  assertSafeRefs('LDK readiness failed route refs', record.failedRouteRefs)
  assertSafeRefs('LDK readiness operator action refs', record.operatorActionRefs)
  assertSafeRefs('LDK readiness blocker refs', record.blockerRefs)
  assertSafeRefs('LDK readiness caveat refs', record.caveatRefs)
  assertSafeRefs('LDK readiness evidence refs', record.evidenceRefs)
  assertSafeRefs('LDK readiness source refs', record.sourceRefs)
  assertCounts(record)

  if (!pylonLdkReadinessHasNoSpendAuthority(record.authority)) {
    throw new PylonLdkReadinessUnsafe({
      reason: 'LDK readiness projections must be read-only and cannot carry live bitcoin spend, channel-open, payout-dispatch, payout-target, buyer-charge, Nexus, Treasury, or settlement mutation authority.',
    })
  }

  if (record.readinessState === 'ready') {
    if (
      record.balanceEvidenceRefs.length === 0 ||
      record.channelPostureRefs.length === 0 ||
      record.evidenceRefs.length === 0 ||
      record.sourceRefs.length === 0
    ) {
      throw new PylonLdkReadinessUnsafe({
        reason: 'Ready LDK settlement rails require balance, channel, evidence, and source refs.',
      })
    }
  }

  if (record.readinessState === 'blocked' && record.blockerRefs.length === 0) {
    throw new PylonLdkReadinessUnsafe({
      reason: 'Blocked LDK settlement readiness requires blocker refs.',
    })
  }

  if (
    (record.readinessState === 'attention_required' ||
      record.readinessState === 'degraded' ||
      record.readinessState === 'stale') &&
    record.caveatRefs.length === 0 &&
    record.operatorActionRefs.length === 0
  ) {
    throw new PylonLdkReadinessUnsafe({
      reason: 'Non-ready LDK settlement readiness requires caveat or operator action refs.',
    })
  }

  if (record.noRouteCount > 0 && record.failedRouteRefs.length === 0) {
    throw new PylonLdkReadinessUnsafe({
      reason: 'No-route readiness counts require failed route refs.',
    })
  }
}

const projectionText = (
  projection: PylonLdkReadinessProjection,
): string =>
  [
    projection.id,
    projection.providerRef,
    projection.railRef,
    ...projection.balanceEvidenceRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.channelPostureRefs,
    ...projection.evidenceRefs,
    ...projection.failedRouteRefs,
    ...projection.operatorActionRefs,
    ...projection.sourceRefs,
  ].join(' ')

export const pylonLdkReadinessProjectionHasPrivateMaterial = (
  projection: PylonLdkReadinessProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafePylonLdkReadinessRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonLdkReadiness = (
  record: PylonLdkReadinessRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonLdkReadinessProjection => {
  assertRecordSafe(record)

  const projection: PylonLdkReadinessProjection = {
    audience,
    authority: record.authority,
    balanceEvidenceRefs: safeRefsForAudience(
      'LDK readiness balance refs',
      record.balanceEvidenceRefs,
      audience,
    ),
    blockerRefs: safeRefsForAudience(
      'LDK readiness blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'LDK readiness caveat refs',
      record.caveatRefs,
      audience,
    ),
    channelOpenMutationAllowed: false,
    channelPostureRefs: safeRefsForAudience(
      'LDK readiness channel refs',
      record.channelPostureRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'LDK readiness evidence refs',
      record.evidenceRefs,
      audience,
    ),
    failedRouteCount: record.failedRouteCount,
    failedRouteRefs: safeRefsForAudience(
      'LDK readiness failed route refs',
      record.failedRouteRefs,
      audience,
    ),
    id: safeRefsForAudience('LDK readiness id', [record.id], audience)[0] ??
      'ldk_readiness.redacted',
    liveWalletSpendAllowed: false,
    nexusMutationAllowed: false,
    noRouteCount: record.noRouteCount,
    operatorActionRefs: safeRefsForAudience(
      'LDK readiness operator action refs',
      record.operatorActionRefs,
      audience,
    ),
    payoutDispatchMutationAllowed: false,
    payoutTargetMutationAllowed: false,
    providerRef: visibleRef(
      'LDK readiness provider ref',
      record.providerRef,
      record.providerVisibility,
      'provider.redacted',
      audience,
    ),
    providerVisibility: record.providerVisibility,
    readinessState: record.readinessState,
    readinessStateLabel: readinessLabelByState[record.readinessState],
    railKind: record.railKind,
    railRef: visibleRef(
      'LDK readiness rail ref',
      record.railRef,
      record.railVisibility,
      'rail.redacted',
      audience,
    ),
    railVisibility: record.railVisibility,
    settlementMutationAllowed: false,
    sourceRefs: safeRefsForAudience(
      'LDK readiness source refs',
      record.sourceRefs,
      audience,
    ),
    treasuryMutationAllowed: false,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (pylonLdkReadinessProjectionHasPrivateMaterial(projection)) {
    throw new PylonLdkReadinessUnsafe({
      reason: 'LDK readiness projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_LDK_READINESS_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonLdkReadinessRecord> = [
    {
      authority: PYLON_LDK_READINESS_READ_ONLY_AUTHORITY,
      balanceEvidenceRefs: [
        'balance.public.summary.pylon_provider_1',
        'balance.private.operator_snapshot_1',
      ],
      blockerRefs: [],
      caveatRefs: ['caveat.liquidity_refresh_window'],
      channelPostureRefs: [
        'channel.public.posture.pylon_provider_1',
        'channel.private.operator_snapshot_1',
      ],
      createdAtIso: '2026-06-07T04:00:00.000Z',
      evidenceRefs: ['evidence.ldk_readiness.probe_1'],
      failedRouteCount: 2,
      failedRouteRefs: [
        'failed_route.public.no_route_summary',
        'failed_route.private.operator_trace',
      ],
      id: 'pylon_ldk_readiness.provider_1',
      noRouteCount: 1,
      operatorActionRefs: ['operator.action.refresh_liquidity_probe'],
      providerRef: 'provider.private.local_pylon_1',
      providerVisibility: 'private',
      readinessState: 'degraded',
      railKind: 'ldk',
      railRef: 'rail.private.ldk_node_1',
      railVisibility: 'private',
      sourceRefs: ['source.public.nexus_treasury_snapshot_1'],
      updatedAtIso: '2026-06-07T04:45:00.000Z',
    },
    {
      authority: PYLON_LDK_READINESS_READ_ONLY_AUTHORITY,
      balanceEvidenceRefs: ['balance.public.summary.pylon_provider_2'],
      blockerRefs: [],
      caveatRefs: [],
      channelPostureRefs: ['channel.public.posture.pylon_provider_2'],
      createdAtIso: '2026-06-07T04:10:00.000Z',
      evidenceRefs: ['evidence.ldk_readiness.probe_2'],
      failedRouteCount: 0,
      failedRouteRefs: [],
      id: 'pylon_ldk_readiness.provider_2',
      noRouteCount: 0,
      operatorActionRefs: [],
      providerRef: 'provider.public.pylon_2',
      providerVisibility: 'public',
      readinessState: 'ready',
      railKind: 'treasury',
      railRef: 'rail.public.treasury_ldk_1',
      railVisibility: 'public',
      sourceRefs: ['source.public.treasury_snapshot_2'],
      updatedAtIso: '2026-06-07T04:50:00.000Z',
    },
  ]
