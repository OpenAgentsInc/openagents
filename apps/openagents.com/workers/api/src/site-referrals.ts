import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const PENDING_REFERRAL_COOKIE = 'oa_pending_referral_attribution'
export const PENDING_REFERRAL_MAX_AGE_SECONDS = 60 * 60 * 24 * 30

export const SiteReferralSourcePolicyState = S.Literals([
  'active',
  'disabled',
  'disputed',
  'expired',
  'archived',
])
export type SiteReferralSourcePolicyState =
  typeof SiteReferralSourcePolicyState.Type

export const ReferralInviteScope = S.Literals([
  'site_join',
  'order_start',
  'agent_claim',
])
export type ReferralInviteScope = typeof ReferralInviteScope.Type

export const ReferralInviteAudiencePath = S.Literals(['human', 'agent'])
export type ReferralInviteAudiencePath = typeof ReferralInviteAudiencePath.Type

export const ReferralInvitePolicyState = S.Literals([
  'active',
  'redeemed',
  'expired',
  'disabled',
  'disputed',
])
export type ReferralInvitePolicyState = typeof ReferralInvitePolicyState.Type

export const ReferralAttributionPolicyState = S.Literals([
  'pending',
  'claimed',
  'expired',
  'disabled',
  'disputed',
  'archived',
])
export type ReferralAttributionPolicyState =
  typeof ReferralAttributionPolicyState.Type

export const ReferralAttributionTarget = S.Literals([
  'home',
  'order',
  'agent_claim',
])
export type ReferralAttributionTarget = typeof ReferralAttributionTarget.Type

export const SiteReferralSourceRecord = S.Struct({
  id: S.String,
  siteId: S.String,
  siteVersionId: S.NullOr(S.String),
  referrerUserId: S.String,
  publicSourceRef: S.String,
  publicSlug: S.String,
  campaignRef: S.NullOr(S.String),
  sourceLabel: S.NullOr(S.String),
  policyState: SiteReferralSourcePolicyState,
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type SiteReferralSourceRecord = typeof SiteReferralSourceRecord.Type

export const ReferralInviteRecord = S.Struct({
  id: S.String,
  referralSourceId: S.String,
  publicInviteRef: S.String,
  tokenHash: S.String,
  scope: ReferralInviteScope,
  audiencePath: ReferralInviteAudiencePath,
  policyState: ReferralInvitePolicyState,
  expiresAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type ReferralInviteRecord = typeof ReferralInviteRecord.Type

export const ReferralAttributionRecord = S.Struct({
  id: S.String,
  referralSourceId: S.String,
  referralInviteId: S.NullOr(S.String),
  publicSourceRef: S.String,
  publicInviteRef: S.NullOr(S.String),
  capturePath: ReferralInviteAudiencePath,
  target: ReferralAttributionTarget,
  policyState: ReferralAttributionPolicyState,
  firstVerifiedAt: S.NullOr(S.String),
  claimedUserId: S.NullOr(S.String),
  expiresAt: S.String,
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type ReferralAttributionRecord = typeof ReferralAttributionRecord.Type

export const PublicSiteReferralSource = S.Struct({
  id: S.String,
  siteId: S.String,
  siteVersionId: S.NullOr(S.String),
  publicSourceRef: S.String,
  publicSlug: S.String,
  campaignRef: S.NullOr(S.String),
  sourceLabel: S.NullOr(S.String),
  policyState: SiteReferralSourcePolicyState,
})
export type PublicSiteReferralSource = typeof PublicSiteReferralSource.Type

export const PublicReferralInvite = S.Struct({
  id: S.String,
  referralSourceId: S.String,
  publicInviteRef: S.String,
  scope: ReferralInviteScope,
  audiencePath: ReferralInviteAudiencePath,
  policyState: ReferralInvitePolicyState,
  expiresAt: S.NullOr(S.String),
})
export type PublicReferralInvite = typeof PublicReferralInvite.Type

export const PublicReferralAttribution = S.Struct({
  id: S.String,
  referralSourceId: S.String,
  referralInviteId: S.NullOr(S.String),
  capturePath: ReferralInviteAudiencePath,
  target: ReferralAttributionTarget,
  policyState: ReferralAttributionPolicyState,
  expiresAt: S.String,
})
export type PublicReferralAttribution = typeof PublicReferralAttribution.Type

export class SiteReferralUnsafePayload extends S.TaggedErrorClass<SiteReferralUnsafePayload>()(
  'SiteReferralUnsafePayload',
  {
    reason: S.String,
  },
) {}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const SAFE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,120}$/
const PROHIBITED_REF_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|mdk_access_token|wallet_secret|private_key|webhook_secret|token_hash)/i

const safeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_REF_PATTERN.test(value)

const safeSlug = (value: string): boolean =>
  SAFE_SLUG_PATTERN.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_REF_PATTERN.test(value)

const assertPublicSafeJson = (label: string, value: unknown): void => {
  const json = JSON.stringify(value)

  if (containsProviderSecretMaterial(json) || PROHIBITED_REF_PATTERN.test(json)) {
    throw new SiteReferralUnsafePayload({
      reason: `${label} contains secret-shaped material.`,
    })
  }
}

export const referralCaptureRedirectLocation = (
  target: ReferralAttributionTarget,
): string => {
  if (target === 'order') {
    return '/order'
  }

  if (target === 'agent_claim') {
    return '/AGENTS.md'
  }

  return '/'
}

export const referralInviteTarget = (
  scope: ReferralInviteScope,
): ReferralAttributionTarget => {
  if (scope === 'order_start') {
    return 'order'
  }

  if (scope === 'agent_claim') {
    return 'agent_claim'
  }

  return 'home'
}

export const referralSourceUnavailableReason = (
  record: SiteReferralSourceRecord | null,
): 'unknown' | 'disabled' | 'disputed' | 'expired' | 'archived' | undefined => {
  if (record === null) {
    return 'unknown'
  }

  if (record.policyState === 'active') {
    return undefined
  }

  return record.policyState
}

export const referralInviteUnavailableReason = (
  record: ReferralInviteRecord | null,
  nowIso: string,
): 'unknown' | 'disabled' | 'disputed' | 'expired' | 'redeemed' | undefined => {
  if (record === null) {
    return 'unknown'
  }

  if (record.policyState !== 'active') {
    return record.policyState
  }

  if (record.expiresAt !== null && record.expiresAt <= nowIso) {
    return 'expired'
  }

  return undefined
}

export const publicSiteReferralSource = (
  record: SiteReferralSourceRecord,
): PublicSiteReferralSource => {
  if (!safeRef(record.publicSourceRef)) {
    throw new SiteReferralUnsafePayload({
      reason: 'publicSourceRef must be a public-safe source ref.',
    })
  }

  if (!safeSlug(record.publicSlug)) {
    throw new SiteReferralUnsafePayload({
      reason: 'publicSlug must be a public-safe source slug.',
    })
  }

  const projection = {
    campaignRef: record.campaignRef,
    id: record.id,
    policyState: record.policyState,
    publicSlug: record.publicSlug,
    publicSourceRef: record.publicSourceRef,
    siteId: record.siteId,
    siteVersionId: record.siteVersionId,
    sourceLabel: record.sourceLabel,
  } satisfies PublicSiteReferralSource

  assertPublicSafeJson('Public Site referral source', projection)

  return projection
}

export const publicReferralInvite = (
  record: ReferralInviteRecord,
): PublicReferralInvite => {
  if (!safeRef(record.publicInviteRef)) {
    throw new SiteReferralUnsafePayload({
      reason: 'publicInviteRef must be a public-safe invite ref.',
    })
  }

  const projection = {
    audiencePath: record.audiencePath,
    expiresAt: record.expiresAt,
    id: record.id,
    policyState: record.policyState,
    publicInviteRef: record.publicInviteRef,
    referralSourceId: record.referralSourceId,
    scope: record.scope,
  } satisfies PublicReferralInvite

  assertPublicSafeJson('Public referral invite', projection)

  return projection
}

export const publicReferralAttribution = (
  record: ReferralAttributionRecord,
): PublicReferralAttribution => {
  if (!safeRef(record.publicSourceRef)) {
    throw new SiteReferralUnsafePayload({
      reason: 'publicSourceRef must be a public-safe source ref.',
    })
  }

  if (record.publicInviteRef !== null && !safeRef(record.publicInviteRef)) {
    throw new SiteReferralUnsafePayload({
      reason: 'publicInviteRef must be a public-safe invite ref.',
    })
  }

  const projection = {
    capturePath: record.capturePath,
    expiresAt: record.expiresAt,
    id: record.id,
    policyState: record.policyState,
    referralInviteId: record.referralInviteId,
    referralSourceId: record.referralSourceId,
    target: record.target,
  } satisfies PublicReferralAttribution

  assertPublicSafeJson('Public referral attribution', projection)

  return projection
}
