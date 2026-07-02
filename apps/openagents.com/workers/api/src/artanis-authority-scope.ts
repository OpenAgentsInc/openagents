import { Schema as S } from 'effect'

export const ArtanisAuthorityScope = S.Literals([
  'owner_self',
  'shared_fleet',
  'owner_operator',
])
export type ArtanisAuthorityScope = typeof ArtanisAuthorityScope.Type

export const ARTANIS_OWNER_SELF_AUTHORITY_SCOPE: ArtanisAuthorityScope =
  'owner_self'
export const ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE: ArtanisAuthorityScope =
  'shared_fleet'
export const ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE: ArtanisAuthorityScope =
  'owner_operator'

export const ARTANIS_AUTHORITY_SCOPES: ReadonlyArray<ArtanisAuthorityScope> = [
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
  ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
]

export const isArtanisAuthorityScope = (
  value: unknown,
): value is ArtanisAuthorityScope =>
  typeof value === 'string' &&
  ARTANIS_AUTHORITY_SCOPES.includes(value as ArtanisAuthorityScope)

export const artanisAuthorityScopeAllowsOwnerLinkedCapacity = (
  authorityScope: ArtanisAuthorityScope,
): boolean => authorityScope === ARTANIS_OWNER_SELF_AUTHORITY_SCOPE

export const artanisAuthorityScopePublicRef = (
  authorityScope: ArtanisAuthorityScope,
): string => `authority.public.artanis.scope.${authorityScope}`

export const artanisAuthorityScopeEvidenceRef = (
  authorityScope: ArtanisAuthorityScope,
): string => `evidence.khala_coding.authority_scope.${authorityScope}`

export const artanisDefaultAuthorityScopeForRiskyAction = (
  riskyActionKind: string,
): ArtanisAuthorityScope =>
  riskyActionKind === 'pylon_job_dispatch'
    ? ARTANIS_OWNER_SELF_AUTHORITY_SCOPE
    : ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE
