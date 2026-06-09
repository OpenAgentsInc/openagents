import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonWalletTelemetrySurface = S.Literals([
  'backup',
  'channel',
  'liquidity',
  'lsp',
  'sync',
  'warning',
])
export type PylonWalletTelemetrySurface =
  typeof PylonWalletTelemetrySurface.Type

export const PylonWalletTelemetryState = S.Literals([
  'attention_required',
  'blocked',
  'degraded',
  'ok',
  'unknown',
])
export type PylonWalletTelemetryState =
  typeof PylonWalletTelemetryState.Type

export const PylonWalletTelemetryFreshness = S.Literals([
  'expired',
  'fresh',
  'stale',
  'unknown',
])
export type PylonWalletTelemetryFreshness =
  typeof PylonWalletTelemetryFreshness.Type

export const PylonWalletTelemetrySeverity = S.Literals([
  'blocked',
  'critical',
  'info',
  'warning',
])
export type PylonWalletTelemetrySeverity =
  typeof PylonWalletTelemetrySeverity.Type

export const PylonWalletTelemetryVisibility = S.Literals([
  'private',
  'public',
])
export type PylonWalletTelemetryVisibility =
  typeof PylonWalletTelemetryVisibility.Type

export const PylonWalletTelemetryAuthorityBoundary = S.Literals([
  'read_only_projection',
])
export type PylonWalletTelemetryAuthorityBoundary =
  typeof PylonWalletTelemetryAuthorityBoundary.Type

export class PylonWalletTelemetryAuthority extends S.Class<PylonWalletTelemetryAuthority>(
  'PylonWalletTelemetryAuthority',
)({
  authorityBoundary: PylonWalletTelemetryAuthorityBoundary,
  noBackupMutation: S.Boolean,
  noChannelMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noLspMutation: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noSettlementMutation: S.Boolean,
  noWalletMutation: S.Boolean,
}) {}

export class PylonWalletTelemetryItem extends S.Class<PylonWalletTelemetryItem>(
  'PylonWalletTelemetryItem',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  freshness: PylonWalletTelemetryFreshness,
  operatorActionRefs: S.Array(S.String),
  severity: PylonWalletTelemetrySeverity,
  sourceRefs: S.Array(S.String),
  state: PylonWalletTelemetryState,
  surface: PylonWalletTelemetrySurface,
  warningRefs: S.Array(S.String),
}) {}

export class PylonWalletTelemetryRecord extends S.Class<PylonWalletTelemetryRecord>(
  'PylonWalletTelemetryRecord',
)({
  authority: PylonWalletTelemetryAuthority,
  createdAtIso: S.String,
  id: S.String,
  items: S.Array(PylonWalletTelemetryItem),
  providerRef: S.String,
  providerVisibility: PylonWalletTelemetryVisibility,
  updatedAtIso: S.String,
  walletRef: S.String,
  walletVisibility: PylonWalletTelemetryVisibility,
}) {}

export class PylonWalletTelemetryProjection extends S.Class<PylonWalletTelemetryProjection>(
  'PylonWalletTelemetryProjection',
)({
  audience: OmniProjectionAudience,
  authority: PylonWalletTelemetryAuthority,
  backupMutationAllowed: S.Boolean,
  channelMutationAllowed: S.Boolean,
  createdAtDisplay: S.String,
  id: S.String,
  items: S.Array(PylonWalletTelemetryItem),
  liveWalletSpendAllowed: S.Boolean,
  lspMutationAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonWalletTelemetryVisibility,
  settlementMutationAllowed: S.Boolean,
  updatedAtDisplay: S.String,
  walletMutationAllowed: S.Boolean,
  walletRef: S.String,
  walletVisibility: PylonWalletTelemetryVisibility,
}) {}

export class PylonWalletTelemetryUnsafe extends S.TaggedErrorClass<PylonWalletTelemetryUnsafe>()(
  'PylonWalletTelemetryUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_WALLET_TELEMETRY_READ_ONLY_AUTHORITY:
  PylonWalletTelemetryAuthority = {
    authorityBoundary: 'read_only_projection',
    noBackupMutation: true,
    noChannelMutation: true,
    noLiveWalletSpend: true,
    noLspMutation: true,
    noPayoutDispatch: true,
    noSettlementMutation: true,
    noWalletMutation: true,
  }

const requiredSurfaces: ReadonlyArray<PylonWalletTelemetrySurface> = [
  'backup',
  'channel',
  'liquidity',
  'lsp',
  'sync',
  'warning',
]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeWalletTelemetryRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|backup|channel|invoice|liquidity|payload|payment|payout|prompt|provider|runner|run[_-]?log|state|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(backup\.private|blocker\.private|caveat\.private|channel\.private|evidence\.private|liquidity\.private|lsp\.private|operator\.action|provider\.private|source\.private|sync\.private|wallet\.private|warning\.private)/i
const customerUnsafeRefPattern =
  /(backup\.private|blocker\.private|caveat\.private|channel\.private|evidence\.private|liquidity\.private|lsp\.private|operator\.action|provider\.private|source\.private|sync\.private|wallet\.private|warning\.private)/i
const teamUnsafeRefPattern =
  /(backup\.private|channel\.private|evidence\.private|liquidity\.private|lsp\.private|provider\.private|sync\.private|wallet\.private)/i

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
    unsafeWalletTelemetryRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonWalletTelemetryUnsafe({
      reason: `${label} contains recovery phrases, raw entropy, private keys, preimages, raw channel monitor state, raw telemetry, wallet material, credentials, provider secrets, customer data, or raw timestamps.`,
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
  visibility: PylonWalletTelemetryVisibility,
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

export const pylonWalletTelemetryHasNoMutationAuthority = (
  authority: PylonWalletTelemetryAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_projection' &&
  authority.noBackupMutation &&
  authority.noChannelMutation &&
  authority.noLiveWalletSpend &&
  authority.noLspMutation &&
  authority.noPayoutDispatch &&
  authority.noSettlementMutation &&
  authority.noWalletMutation

export const pylonWalletTelemetryCanMutateWallet = (
  record: PylonWalletTelemetryRecord,
): boolean => !pylonWalletTelemetryHasNoMutationAuthority(record.authority)

const assertItemSafe = (item: PylonWalletTelemetryItem): void => {
  assertSafeRefs(`${item.surface} telemetry blocker refs`, item.blockerRefs)
  assertSafeRefs(`${item.surface} telemetry caveat refs`, item.caveatRefs)
  assertSafeRefs(`${item.surface} telemetry evidence refs`, item.evidenceRefs)
  assertSafeRefs(
    `${item.surface} telemetry operator action refs`,
    item.operatorActionRefs,
  )
  assertSafeRefs(`${item.surface} telemetry source refs`, item.sourceRefs)
  assertSafeRefs(`${item.surface} telemetry warning refs`, item.warningRefs)

  if (
    item.freshness === 'stale' ||
    item.freshness === 'expired' ||
    item.state === 'attention_required' ||
    item.state === 'degraded'
  ) {
    if (
      item.warningRefs.length === 0 &&
      item.caveatRefs.length === 0 &&
      item.operatorActionRefs.length === 0
    ) {
      throw new PylonWalletTelemetryUnsafe({
        reason: `${item.surface} telemetry needs warning, caveat, or operator action refs.`,
      })
    }
  }

  if (
    (item.state === 'blocked' || item.severity === 'blocked') &&
    item.blockerRefs.length === 0
  ) {
    throw new PylonWalletTelemetryUnsafe({
      reason: `${item.surface} telemetry is blocked but has no blocker refs.`,
    })
  }

  if (
    (item.severity === 'critical' || item.severity === 'blocked') &&
    item.operatorActionRefs.length === 0
  ) {
    throw new PylonWalletTelemetryUnsafe({
      reason: `${item.surface} telemetry is critical or blocked but has no operator action refs.`,
    })
  }
}

const assertRequiredSurfaces = (
  items: ReadonlyArray<PylonWalletTelemetryItem>,
): void => {
  const surfaceSet = new Set(items.map(item => item.surface))
  const missing = requiredSurfaces.filter(surface => !surfaceSet.has(surface))

  if (missing.length > 0) {
    throw new PylonWalletTelemetryUnsafe({
      reason: `Pylon wallet telemetry is missing required surfaces: ${missing.join(', ')}.`,
    })
  }
}

const assertRecordSafe = (record: PylonWalletTelemetryRecord): void => {
  assertSafeRefs('wallet telemetry identity refs', [
    record.id,
    record.providerRef,
    record.walletRef,
  ])
  for (const item of record.items) {
    assertItemSafe(item)
  }
  assertRequiredSurfaces(record.items)

  if (!pylonWalletTelemetryHasNoMutationAuthority(record.authority)) {
    throw new PylonWalletTelemetryUnsafe({
      reason: 'Pylon wallet telemetry projections are read-only and cannot carry wallet, channel, LSP, backup, live spend, payout, or settlement mutation authority.',
    })
  }
}

const itemForAudience = (
  item: PylonWalletTelemetryItem,
  audience: typeof OmniProjectionAudience.Type,
): PylonWalletTelemetryItem => ({
  blockerRefs: safeRefsForAudience(
    `${item.surface} telemetry blocker refs`,
    item.blockerRefs,
    audience,
  ),
  caveatRefs: safeRefsForAudience(
    `${item.surface} telemetry caveat refs`,
    item.caveatRefs,
    audience,
  ),
  evidenceRefs: safeRefsForAudience(
    `${item.surface} telemetry evidence refs`,
    item.evidenceRefs,
    audience,
  ),
  freshness: item.freshness,
  operatorActionRefs: safeRefsForAudience(
    `${item.surface} telemetry operator action refs`,
    item.operatorActionRefs,
    audience,
  ),
  severity: item.severity,
  sourceRefs: safeRefsForAudience(
    `${item.surface} telemetry source refs`,
    item.sourceRefs,
    audience,
  ),
  state: item.state,
  surface: item.surface,
  warningRefs: safeRefsForAudience(
    `${item.surface} telemetry warning refs`,
    item.warningRefs,
    audience,
  ),
})

const projectionText = (
  projection: PylonWalletTelemetryProjection,
): string =>
  [
    projection.id,
    projection.providerRef,
    projection.walletRef,
    ...projection.items.flatMap(item => [
      ...item.blockerRefs,
      ...item.caveatRefs,
      ...item.evidenceRefs,
      ...item.operatorActionRefs,
      ...item.sourceRefs,
      ...item.warningRefs,
    ]),
  ].join(' ')

export const pylonWalletTelemetryProjectionHasPrivateMaterial = (
  projection: PylonWalletTelemetryProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeWalletTelemetryRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonWalletTelemetry = (
  record: PylonWalletTelemetryRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonWalletTelemetryProjection => {
  assertRecordSafe(record)

  const projection: PylonWalletTelemetryProjection = {
    audience,
    authority: record.authority,
    backupMutationAllowed: false,
    channelMutationAllowed: false,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    id: safeRefsForAudience('wallet telemetry id', [record.id], audience)[0] ??
      'wallet_telemetry.redacted',
    items: record.items.map(item => itemForAudience(item, audience)),
    liveWalletSpendAllowed: false,
    lspMutationAllowed: false,
    payoutDispatchMutationAllowed: false,
    providerRef: visibleRef(
      'wallet telemetry provider ref',
      record.providerRef,
      record.providerVisibility,
      'provider.redacted',
      audience,
    ),
    providerVisibility: record.providerVisibility,
    settlementMutationAllowed: false,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletMutationAllowed: false,
    walletRef: visibleRef(
      'wallet telemetry wallet ref',
      record.walletRef,
      record.walletVisibility,
      'wallet.redacted',
      audience,
    ),
    walletVisibility: record.walletVisibility,
  }

  if (pylonWalletTelemetryProjectionHasPrivateMaterial(projection)) {
    throw new PylonWalletTelemetryUnsafe({
      reason: 'Pylon wallet telemetry projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_WALLET_TELEMETRY_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonWalletTelemetryRecord> = [
    {
      authority: PYLON_WALLET_TELEMETRY_READ_ONLY_AUTHORITY,
      createdAtIso: '2026-06-07T07:00:00.000Z',
      id: 'pylon_wallet_telemetry.provider_1',
      items: [
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['sync.public.wallet_status_fresh'],
          freshness: 'fresh',
          operatorActionRefs: [],
          severity: 'info',
          sourceRefs: ['source.public.pylon_status_probe'],
          state: 'ok',
          surface: 'sync',
          warningRefs: [],
        },
        {
          blockerRefs: ['blocker.public.channel_unavailable'],
          caveatRefs: [],
          evidenceRefs: [
            'channel.public.posture_summary',
            'channel.private.operator_trace',
          ],
          freshness: 'fresh',
          operatorActionRefs: ['operator.action.inspect_channel_posture'],
          severity: 'blocked',
          sourceRefs: ['source.private.channel_probe'],
          state: 'blocked',
          surface: 'channel',
          warningRefs: ['warning.public.channel_unavailable'],
        },
        {
          blockerRefs: [],
          caveatRefs: ['caveat.public.liquidity_probe_pending'],
          evidenceRefs: ['liquidity.private.operator_snapshot'],
          freshness: 'stale',
          operatorActionRefs: ['operator.action.refresh_liquidity_probe'],
          severity: 'warning',
          sourceRefs: ['source.private.liquidity_probe'],
          state: 'degraded',
          surface: 'liquidity',
          warningRefs: ['warning.public.insufficient_outbound_liquidity'],
        },
        {
          blockerRefs: [],
          caveatRefs: ['caveat.public.lsp_not_configured'],
          evidenceRefs: ['lsp.private.operator_snapshot'],
          freshness: 'unknown',
          operatorActionRefs: [],
          severity: 'warning',
          sourceRefs: ['source.private.lsp_probe'],
          state: 'attention_required',
          surface: 'lsp',
          warningRefs: ['warning.public.lsp_not_configured'],
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['backup.public.encrypted_backup_seen'],
          freshness: 'fresh',
          operatorActionRefs: [],
          severity: 'info',
          sourceRefs: ['source.public.backup_status_probe'],
          state: 'ok',
          surface: 'backup',
          warningRefs: [],
        },
        {
          blockerRefs: [],
          caveatRefs: ['caveat.public.operator_review_needed'],
          evidenceRefs: ['warning.public.wallet_attention_summary'],
          freshness: 'fresh',
          operatorActionRefs: ['operator.action.review_wallet_warnings'],
          severity: 'critical',
          sourceRefs: ['source.public.warning_summary'],
          state: 'attention_required',
          surface: 'warning',
          warningRefs: ['warning.public.wallet_attention_required'],
        },
      ],
      providerRef: 'provider.private.pylon_1',
      providerVisibility: 'private',
      updatedAtIso: '2026-06-07T07:45:00.000Z',
      walletRef: 'wallet.private.pylon_1',
      walletVisibility: 'private',
    },
  ]
