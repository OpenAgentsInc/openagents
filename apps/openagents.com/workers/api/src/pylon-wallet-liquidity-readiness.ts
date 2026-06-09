import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonWalletLiquidityBucketKind = S.Literals([
  'anchor_reserve',
  'inbound_liquidity',
  'outbound_liquidity',
  'spendable_onchain',
  'total_channel_balance',
])
export type PylonWalletLiquidityBucketKind =
  typeof PylonWalletLiquidityBucketKind.Type

export const PylonWalletLiquidityEvidenceState = S.Literals([
  'blocked',
  'modeled',
  'reported',
  'stale',
  'unknown',
  'verified',
])
export type PylonWalletLiquidityEvidenceState =
  typeof PylonWalletLiquidityEvidenceState.Type

export const PylonWalletLiquidityDirectionState = S.Literals([
  'blocked',
  'degraded',
  'not_ready',
  'ready',
  'unknown',
])
export type PylonWalletLiquidityDirectionState =
  typeof PylonWalletLiquidityDirectionState.Type

export const PylonWalletLiquidityVisibility = S.Literals([
  'private',
  'public',
])
export type PylonWalletLiquidityVisibility =
  typeof PylonWalletLiquidityVisibility.Type

export const PylonWalletLiquidityAuthorityBoundary = S.Literals([
  'read_only_projection',
])
export type PylonWalletLiquidityAuthorityBoundary =
  typeof PylonWalletLiquidityAuthorityBoundary.Type

export class PylonWalletLiquidityAuthority extends S.Class<PylonWalletLiquidityAuthority>(
  'PylonWalletLiquidityAuthority',
)({
  authorityBoundary: PylonWalletLiquidityAuthorityBoundary,
  noChannelMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noLiquidityProvisionMutation: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
  noWalletMutation: S.Boolean,
}) {}

export class PylonWalletLiquidityBucket extends S.Class<PylonWalletLiquidityBucket>(
  'PylonWalletLiquidityBucket',
)({
  amountRef: S.NullOr(S.String),
  blockerRefs: S.Array(S.String),
  bucket: PylonWalletLiquidityBucketKind,
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  evidenceState: PylonWalletLiquidityEvidenceState,
}) {}

export class PylonWalletLiquidityRecord extends S.Class<PylonWalletLiquidityRecord>(
  'PylonWalletLiquidityRecord',
)({
  authority: PylonWalletLiquidityAuthority,
  blockerRefs: S.Array(S.String),
  buckets: S.Array(PylonWalletLiquidityBucket),
  caveatRefs: S.Array(S.String),
  channelPostureRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  payoutTargetAdmissionRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: PylonWalletLiquidityVisibility,
  receiveReadiness: PylonWalletLiquidityDirectionState,
  sendReadiness: PylonWalletLiquidityDirectionState,
  sourceRefs: S.Array(S.String),
  syncRefs: S.Array(S.String),
  updatedAtIso: S.String,
  walletRef: S.String,
  walletVisibility: PylonWalletLiquidityVisibility,
  warningRefs: S.Array(S.String),
}) {}

export class PylonWalletLiquidityProjection extends S.Class<PylonWalletLiquidityProjection>(
  'PylonWalletLiquidityProjection',
)({
  audience: OmniProjectionAudience,
  authority: PylonWalletLiquidityAuthority,
  blockerRefs: S.Array(S.String),
  buckets: S.Array(PylonWalletLiquidityBucket),
  caveatRefs: S.Array(S.String),
  channelMutationAllowed: S.Boolean,
  channelPostureRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  liquidityProvisionMutationAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutTargetAdmissionRefs: S.Array(S.String),
  payoutTargetMutationAllowed: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonWalletLiquidityVisibility,
  receiveReadiness: PylonWalletLiquidityDirectionState,
  receiveReadinessLabel: S.String,
  sendReadiness: PylonWalletLiquidityDirectionState,
  sendReadinessLabel: S.String,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  syncRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  walletMutationAllowed: S.Boolean,
  walletRef: S.String,
  walletVisibility: PylonWalletLiquidityVisibility,
  warningRefs: S.Array(S.String),
}) {}

export class PylonWalletLiquidityUnsafe extends S.TaggedErrorClass<PylonWalletLiquidityUnsafe>()(
  'PylonWalletLiquidityUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_WALLET_LIQUIDITY_READ_ONLY_AUTHORITY:
  PylonWalletLiquidityAuthority = {
    authorityBoundary: 'read_only_projection',
    noChannelMutation: true,
    noLiveWalletSpend: true,
    noLiquidityProvisionMutation: true,
    noPayoutDispatch: true,
    noPayoutTargetMutation: true,
    noSettlementMutation: true,
    noWalletMutation: true,
  }

const directionLabelByState:
  Readonly<Record<PylonWalletLiquidityDirectionState, string>> = {
    blocked: 'Blocked',
    degraded: 'Degraded',
    not_ready: 'Not ready',
    ready: 'Ready',
    unknown: 'Unknown',
  }

const requiredBucketKinds:
  ReadonlyArray<PylonWalletLiquidityBucketKind> = [
    'anchor_reserve',
    'inbound_liquidity',
    'outbound_liquidity',
    'spendable_onchain',
    'total_channel_balance',
  ]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeWalletLiquidityRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|invoice|liquidity|payload|payment|payout|prompt|provider|runner|run[_-]?log|state|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(amount\.private|blocker\.private|caveat\.private|channel\.private|evidence\.private|liquidity\.private|provider\.private|source\.private|sync\.private|target\.private|wallet\.private|warning\.private)/i
const customerUnsafeRefPattern =
  /(amount\.private|blocker\.private|caveat\.private|channel\.private|evidence\.private|liquidity\.private|provider\.private|source\.private|sync\.private|target\.private|wallet\.private|warning\.private)/i
const teamUnsafeRefPattern =
  /(amount\.private|channel\.private|liquidity\.private|provider\.private|target\.private|wallet\.private)/i

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
    unsafeWalletLiquidityRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonWalletLiquidityUnsafe({
      reason: `${label} contains wallet secrets, raw channel state, raw liquidity telemetry, private host data, payment material, payout targets, provider secrets, customer data, or raw timestamps.`,
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
  visibility: PylonWalletLiquidityVisibility,
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

export const pylonWalletLiquidityHasNoSpendAuthority = (
  authority: PylonWalletLiquidityAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_projection' &&
  authority.noChannelMutation &&
  authority.noLiveWalletSpend &&
  authority.noLiquidityProvisionMutation &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetMutation &&
  authority.noSettlementMutation &&
  authority.noWalletMutation

export const pylonWalletLiquidityCanMutateWallet = (
  record: PylonWalletLiquidityRecord,
): boolean => !pylonWalletLiquidityHasNoSpendAuthority(record.authority)

const assertBucketSafe = (bucket: PylonWalletLiquidityBucket): void => {
  assertSafeRefs(
    `wallet liquidity ${bucket.bucket} amount refs`,
    bucket.amountRef === null ? [] : [bucket.amountRef],
  )
  assertSafeRefs(
    `wallet liquidity ${bucket.bucket} evidence refs`,
    bucket.evidenceRefs,
  )
  assertSafeRefs(
    `wallet liquidity ${bucket.bucket} blocker refs`,
    bucket.blockerRefs,
  )
  assertSafeRefs(
    `wallet liquidity ${bucket.bucket} caveat refs`,
    bucket.caveatRefs,
  )

  if (
    (bucket.evidenceState === 'verified' ||
      bucket.evidenceState === 'reported') &&
    (bucket.amountRef === null || bucket.evidenceRefs.length === 0)
  ) {
    throw new PylonWalletLiquidityUnsafe({
      reason: `${bucket.bucket} liquidity requires amount and evidence refs when reported or verified.`,
    })
  }

  if (bucket.evidenceState === 'blocked' && bucket.blockerRefs.length === 0) {
    throw new PylonWalletLiquidityUnsafe({
      reason: `${bucket.bucket} liquidity is blocked but has no blocker refs.`,
    })
  }

  if (bucket.evidenceState === 'stale' && bucket.caveatRefs.length === 0) {
    throw new PylonWalletLiquidityUnsafe({
      reason: `${bucket.bucket} liquidity is stale but has no caveat refs.`,
    })
  }
}

const assertRequiredBuckets = (
  buckets: ReadonlyArray<PylonWalletLiquidityBucket>,
): void => {
  const bucketSet = new Set(buckets.map(bucket => bucket.bucket))
  const missing = requiredBucketKinds.filter(bucket => !bucketSet.has(bucket))

  if (missing.length > 0) {
    throw new PylonWalletLiquidityUnsafe({
      reason: `Pylon wallet liquidity record is missing required buckets: ${missing.join(', ')}.`,
    })
  }
}

const assertRecordSafe = (record: PylonWalletLiquidityRecord): void => {
  assertSafeRefs('wallet liquidity identity refs', [
    record.id,
    record.providerRef,
    record.walletRef,
  ])
  for (const bucket of record.buckets) {
    assertBucketSafe(bucket)
  }
  assertRequiredBuckets(record.buckets)
  assertSafeRefs(
    'wallet liquidity payout target admission refs',
    record.payoutTargetAdmissionRefs,
  )
  assertSafeRefs('wallet liquidity channel refs', record.channelPostureRefs)
  assertSafeRefs('wallet liquidity sync refs', record.syncRefs)
  assertSafeRefs('wallet liquidity blocker refs', record.blockerRefs)
  assertSafeRefs('wallet liquidity caveat refs', record.caveatRefs)
  assertSafeRefs('wallet liquidity warning refs', record.warningRefs)
  assertSafeRefs('wallet liquidity evidence refs', record.evidenceRefs)
  assertSafeRefs('wallet liquidity source refs', record.sourceRefs)

  if (!pylonWalletLiquidityHasNoSpendAuthority(record.authority)) {
    throw new PylonWalletLiquidityUnsafe({
      reason: 'Pylon wallet liquidity readiness is read-only and cannot carry wallet, channel, liquidity, payout, payout-target, or settlement mutation authority.',
    })
  }

  if (
    (record.sendReadiness === 'blocked' ||
      record.receiveReadiness === 'blocked') &&
    record.blockerRefs.length === 0
  ) {
    throw new PylonWalletLiquidityUnsafe({
      reason: 'Blocked send or receive readiness requires blocker refs.',
    })
  }

  if (
    (record.sendReadiness === 'degraded' ||
      record.receiveReadiness === 'degraded' ||
      record.sendReadiness === 'not_ready' ||
      record.receiveReadiness === 'not_ready') &&
    record.warningRefs.length === 0 &&
    record.caveatRefs.length === 0
  ) {
    throw new PylonWalletLiquidityUnsafe({
      reason: 'Degraded or not-ready liquidity requires warning or caveat refs.',
    })
  }
}

const bucketForAudience = (
  bucket: PylonWalletLiquidityBucket,
  audience: typeof OmniProjectionAudience.Type,
): PylonWalletLiquidityBucket => ({
  amountRef: safeRefsForAudience(
    `wallet liquidity ${bucket.bucket} amount refs`,
    bucket.amountRef === null ? [] : [bucket.amountRef],
    audience,
  )[0] ?? null,
  blockerRefs: safeRefsForAudience(
    `wallet liquidity ${bucket.bucket} blocker refs`,
    bucket.blockerRefs,
    audience,
  ),
  bucket: bucket.bucket,
  caveatRefs: safeRefsForAudience(
    `wallet liquidity ${bucket.bucket} caveat refs`,
    bucket.caveatRefs,
    audience,
  ),
  evidenceRefs: safeRefsForAudience(
    `wallet liquidity ${bucket.bucket} evidence refs`,
    bucket.evidenceRefs,
    audience,
  ),
  evidenceState: bucket.evidenceState,
})

const projectionText = (
  projection: PylonWalletLiquidityProjection,
): string =>
  [
    projection.id,
    projection.providerRef,
    projection.walletRef,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.channelPostureRefs,
    ...projection.evidenceRefs,
    ...projection.payoutTargetAdmissionRefs,
    ...projection.sourceRefs,
    ...projection.syncRefs,
    ...projection.warningRefs,
    ...projection.buckets.flatMap(bucket => [
      bucket.amountRef ?? '',
      ...bucket.blockerRefs,
      ...bucket.caveatRefs,
      ...bucket.evidenceRefs,
    ]),
  ].join(' ')

export const pylonWalletLiquidityProjectionHasPrivateMaterial = (
  projection: PylonWalletLiquidityProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeWalletLiquidityRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonWalletLiquidity = (
  record: PylonWalletLiquidityRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonWalletLiquidityProjection => {
  assertRecordSafe(record)

  const projection: PylonWalletLiquidityProjection = {
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'wallet liquidity blocker refs',
      record.blockerRefs,
      audience,
    ),
    buckets: record.buckets.map(bucket => bucketForAudience(bucket, audience)),
    caveatRefs: safeRefsForAudience(
      'wallet liquidity caveat refs',
      record.caveatRefs,
      audience,
    ),
    channelMutationAllowed: false,
    channelPostureRefs: safeRefsForAudience(
      'wallet liquidity channel refs',
      record.channelPostureRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'wallet liquidity evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefsForAudience('wallet liquidity id', [record.id], audience)[0] ??
      'wallet_liquidity.redacted',
    liveWalletSpendAllowed: false,
    liquidityProvisionMutationAllowed: false,
    payoutDispatchMutationAllowed: false,
    payoutTargetAdmissionRefs: safeRefsForAudience(
      'wallet liquidity payout target refs',
      record.payoutTargetAdmissionRefs,
      audience,
    ),
    payoutTargetMutationAllowed: false,
    providerRef: visibleRef(
      'wallet liquidity provider ref',
      record.providerRef,
      record.providerVisibility,
      'provider.redacted',
      audience,
    ),
    providerVisibility: record.providerVisibility,
    receiveReadiness: record.receiveReadiness,
    receiveReadinessLabel: directionLabelByState[record.receiveReadiness],
    sendReadiness: record.sendReadiness,
    sendReadinessLabel: directionLabelByState[record.sendReadiness],
    settlementMutationAllowed: false,
    sourceRefs: safeRefsForAudience(
      'wallet liquidity source refs',
      record.sourceRefs,
      audience,
    ),
    syncRefs: safeRefsForAudience(
      'wallet liquidity sync refs',
      record.syncRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletMutationAllowed: false,
    walletRef: visibleRef(
      'wallet liquidity wallet ref',
      record.walletRef,
      record.walletVisibility,
      'wallet.redacted',
      audience,
    ),
    walletVisibility: record.walletVisibility,
    warningRefs: safeRefsForAudience(
      'wallet liquidity warning refs',
      record.warningRefs,
      audience,
    ),
  }

  if (pylonWalletLiquidityProjectionHasPrivateMaterial(projection)) {
    throw new PylonWalletLiquidityUnsafe({
      reason: 'Pylon wallet liquidity projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_WALLET_LIQUIDITY_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonWalletLiquidityRecord> = [
    {
      authority: PYLON_WALLET_LIQUIDITY_READ_ONLY_AUTHORITY,
      blockerRefs: ['blocker.public.no_approved_payout_target'],
      buckets: [
        {
          amountRef: 'amount.private.anchor_reserve_snapshot',
          blockerRefs: [],
          bucket: 'anchor_reserve',
          caveatRefs: ['caveat.public.anchor_reserve_low'],
          evidenceRefs: ['evidence.private.anchor_reserve_probe'],
          evidenceState: 'reported',
        },
        {
          amountRef: 'amount.public.inbound_liquidity_bucket',
          blockerRefs: [],
          bucket: 'inbound_liquidity',
          caveatRefs: [],
          evidenceRefs: ['evidence.public.inbound_liquidity_probe'],
          evidenceState: 'verified',
        },
        {
          amountRef: 'amount.private.outbound_liquidity_bucket',
          blockerRefs: ['blocker.public.no_outbound_liquidity'],
          bucket: 'outbound_liquidity',
          caveatRefs: [],
          evidenceRefs: ['evidence.private.outbound_liquidity_probe'],
          evidenceState: 'blocked',
        },
        {
          amountRef: 'amount.public.spendable_onchain_bucket',
          blockerRefs: [],
          bucket: 'spendable_onchain',
          caveatRefs: [],
          evidenceRefs: ['evidence.public.spendable_onchain_probe'],
          evidenceState: 'reported',
        },
        {
          amountRef: 'amount.private.total_channel_balance_bucket',
          blockerRefs: [],
          bucket: 'total_channel_balance',
          caveatRefs: ['caveat.public.total_channel_balance_stale'],
          evidenceRefs: ['evidence.private.total_channel_balance_probe'],
          evidenceState: 'stale',
        },
      ],
      caveatRefs: ['caveat.public.liquidity_refresh_needed'],
      channelPostureRefs: [
        'channel.public.posture.summary',
        'channel.private.operator_snapshot',
      ],
      createdAtIso: '2026-06-07T06:00:00.000Z',
      evidenceRefs: ['evidence.public.wallet_liquidity_summary'],
      id: 'pylon_wallet_liquidity.provider_1',
      payoutTargetAdmissionRefs: [
        'target.public.admission_pending',
        'target.private.operator_trace',
      ],
      providerRef: 'provider.private.pylon_1',
      providerVisibility: 'private',
      receiveReadiness: 'ready',
      sendReadiness: 'blocked',
      sourceRefs: ['source.public.ldk_wallet_status_snapshot'],
      syncRefs: ['sync.public.wallet_fresh', 'sync.private.operator_snapshot'],
      updatedAtIso: '2026-06-07T06:45:00.000Z',
      walletRef: 'wallet.private.pylon_1',
      walletVisibility: 'private',
      warningRefs: [
        'warning.public.insufficient_anchor_reserve',
        'warning.public.no_outbound_liquidity',
      ],
    },
  ]
