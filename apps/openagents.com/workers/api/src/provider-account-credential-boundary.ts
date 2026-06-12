import {
  assertNoProviderSecretMaterial,
  requirePublicSecretReference,
} from '@openagentsinc/provider-account-schema'

import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountHealth,
  ProviderAccountProvider,
  ProviderAccountRecord,
  ProviderAccountStatus,
} from './provider-account-domain'

export const PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_VERSION =
  'provider-account-credential-boundary:v1' as const

const PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_COLLECTION =
  'provider_account_credential_boundary_public'

export type ProviderAccountCredentialLeaseAuthority = 'eligible' | 'blocked'

export type ProviderAccountCredentialBlocker =
  | 'account_deleted'
  | `status:${ProviderAccountStatus}`
  | `health:${ProviderAccountHealth}`
  | 'missing_credential_ref'
  | 'grant_expired'
  | `grant_status:${ProviderAccountAuthGrantRecord['status']}`

export type ProviderAccountCredentialBoundaryInput = Readonly<{
  account: Pick<
    ProviderAccountRecord,
    | 'deletedAt'
    | 'health'
    | 'provider'
    | 'providerAccountRef'
    | 'secretRef'
    | 'status'
  >
  activeLeaseRefs?: ReadonlyArray<string> | undefined
  artifactRefs?: ReadonlyArray<string> | undefined
  grant?:
    | Pick<
        ProviderAccountAuthGrantRecord,
        'expiresAt' | 'grantRef' | 'providerAccountRef' | 'status'
      >
    | undefined
  now: string
  receiptRefs?: ReadonlyArray<string> | undefined
}>

export type ProviderAccountCredentialBoundaryProjection = Readonly<{
  generatedAt: string
  boundaryVersion: typeof PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_VERSION
  provider: ProviderAccountProvider
  providerAccountRef: string
  accountRef: string
  credentialRef: string | null
  hasCredentialRef: boolean
  status: ProviderAccountStatus
  health: ProviderAccountHealth
  leaseAuthority: ProviderAccountCredentialLeaseAuthority
  blockerRefs: ReadonlyArray<string>
  reconnectActionRef: string | null
  cacheInvalidationRefs: ReadonlyArray<string>
  activeLeaseRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
}>

const unique = <Value extends string>(
  values: ReadonlyArray<Value>,
): ReadonlyArray<Value> => [...new Set(values)]

const grantBlockers = (
  grant: ProviderAccountCredentialBoundaryInput['grant'],
  now: string,
): ReadonlyArray<ProviderAccountCredentialBlocker> => {
  if (grant === undefined) {
    return []
  }

  if (grant.status !== 'issued') {
    return [`grant_status:${grant.status}`]
  }

  return Date.parse(grant.expiresAt) <= Date.parse(now) ? ['grant_expired'] : []
}

const accountBlockers = (
  account: ProviderAccountCredentialBoundaryInput['account'],
): ReadonlyArray<ProviderAccountCredentialBlocker> => [
  ...(account.deletedAt === null ? [] : (['account_deleted'] as const)),
  ...(account.status === 'connected'
    ? []
    : ([`status:${account.status}`] as const)),
  ...(account.health === 'healthy'
    ? []
    : ([`health:${account.health}`] as const)),
  ...(account.secretRef === null ? (['missing_credential_ref'] as const) : []),
]

const requiresCredentialCacheInvalidation = (
  blockers: ReadonlyArray<ProviderAccountCredentialBlocker>,
): boolean =>
  blockers.some(
    blocker =>
      blocker === 'account_deleted' ||
      blocker === 'missing_credential_ref' ||
      blocker === 'grant_expired' ||
      blocker.startsWith('grant_status:') ||
      blocker === 'status:disconnected' ||
      blocker === 'status:denied' ||
      blocker === 'status:expired' ||
      blocker === 'health:requires_reauth',
  )

const blockerRef = (
  providerAccountRef: string,
  blocker: ProviderAccountCredentialBlocker,
): string =>
  `provider-account-blocker:${providerAccountRef}:${blocker.replace(/:/g, '.')}`

export const projectProviderAccountCredentialBoundary = (
  input: ProviderAccountCredentialBoundaryInput,
): ProviderAccountCredentialBoundaryProjection => {
  const blockers = unique([
    ...accountBlockers(input.account),
    ...grantBlockers(input.grant, input.now),
  ])
  const credentialRef =
    input.account.secretRef === null
      ? null
      : requirePublicSecretReference(input.account.secretRef)
  const cacheInvalidationRefs = requiresCredentialCacheInvalidation(blockers)
    ? [
        `provider-account-cache:${input.account.providerAccountRef}`,
        ...(input.grant === undefined
          ? []
          : [`provider-account-grant-cache:${input.grant.grantRef}`]),
      ]
    : []
  const projection: ProviderAccountCredentialBoundaryProjection = {
    generatedAt: input.now,
    boundaryVersion: PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_VERSION,
    provider: input.account.provider,
    providerAccountRef: input.account.providerAccountRef,
    accountRef: `providerAccount:${input.account.providerAccountRef}`,
    credentialRef,
    hasCredentialRef: credentialRef !== null,
    status: input.account.status,
    health: input.account.health,
    leaseAuthority: blockers.length === 0 ? 'eligible' : 'blocked',
    blockerRefs: blockers.map(blocker =>
      blockerRef(input.account.providerAccountRef, blocker),
    ),
    reconnectActionRef:
      blockers.length === 0
        ? null
        : `provider-account-reconnect:${input.account.providerAccountRef}`,
    cacheInvalidationRefs,
    activeLeaseRefs: input.activeLeaseRefs ?? [],
    artifactRefs: input.artifactRefs ?? [],
    receiptRefs: input.receiptRefs ?? [],
  }

  assertNoProviderSecretMaterial(
    projection,
    PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_COLLECTION,
  )

  return projection
}
