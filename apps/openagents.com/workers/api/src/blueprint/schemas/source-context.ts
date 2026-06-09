import { Schema as S } from 'effect'

import {
  canProjectOmniClassifiedRecord,
  OmniDataClassification,
  type OmniProjectionAudience,
  OmniTrustTier,
} from '../../omni-data-classification'

export const BlueprintSourceKind = S.Literals([
  'artifact',
  'customer_asset',
  'email',
  'exa_brief',
  'generated_summary',
  'order',
  'repo',
])
export type BlueprintSourceKind = typeof BlueprintSourceKind.Type

export const BlueprintSourceFreshness = S.Literals([
  'current',
  'recent',
  'stale',
  'unknown',
])
export type BlueprintSourceFreshness = typeof BlueprintSourceFreshness.Type

export const BlueprintSourceConsentState = S.Literals([
  'customer_provided',
  'internal_only',
  'public',
  'revoked',
  'unavailable',
])
export type BlueprintSourceConsentState =
  typeof BlueprintSourceConsentState.Type

export const BlueprintSourceConfidence = S.Literals([
  'high',
  'low',
  'medium',
  'unknown',
])
export type BlueprintSourceConfidence =
  typeof BlueprintSourceConfidence.Type

export const BlueprintSourceAuthority = S.Struct({
  classificationCaveatRef: S.String,
  confidence: BlueprintSourceConfidence,
  consentState: BlueprintSourceConsentState,
  customerSafe: S.Boolean,
  dataClassification: OmniDataClassification,
  excludedReasonRef: S.NullOr(S.String),
  freshness: BlueprintSourceFreshness,
  includedInContext: S.Boolean,
  publicSafe: S.Boolean,
  publicSummaryRef: S.NullOr(S.String),
  sourceKind: BlueprintSourceKind,
  sourceRef: S.String,
  trustTier: OmniTrustTier,
})
export type BlueprintSourceAuthority =
  typeof BlueprintSourceAuthority.Type

export const BlueprintContextPack = S.Struct({
  createdAt: S.String,
  customerSafeProjection: S.Boolean,
  dataClassification: OmniDataClassification,
  excludedContextRefs: S.Array(S.String),
  id: S.String,
  includedContextRefs: S.Array(S.String),
  publicSafeProjection: S.Boolean,
  sourceAuthorities: S.Array(BlueprintSourceAuthority),
  trustTier: OmniTrustTier,
  updatedAt: S.String,
})
export type BlueprintContextPack = typeof BlueprintContextPack.Type

export const blueprintSourceCanProject = (
  source: BlueprintSourceAuthority,
  audience: OmniProjectionAudience,
): boolean => {
  if (audience === 'public' && !source.publicSafe) {
    return false
  }

  if (audience === 'customer' && !source.customerSafe) {
    return false
  }

  return canProjectOmniClassifiedRecord(source, audience)
}

export const blueprintContextPackProjection = (
  pack: BlueprintContextPack,
  audience: OmniProjectionAudience,
) => {
  const sourceRefs = pack.sourceAuthorities
    .filter(source => source.includedInContext)
    .filter(source => blueprintSourceCanProject(source, audience))
    .map(source => source.sourceRef)

  return {
    dataClassification: pack.dataClassification,
    excludedContextCount: pack.excludedContextRefs.length,
    id: pack.id,
    publicSafeProjection:
      audience === 'public'
        ? pack.publicSafeProjection && sourceRefs.length > 0
        : pack.publicSafeProjection,
    sourceRefs,
    trustTier: pack.trustTier,
  }
}
