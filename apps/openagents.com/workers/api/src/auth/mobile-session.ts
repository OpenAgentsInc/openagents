import type { Tokens } from '@openauthjs/openauth/client'
import type { StorageAdapter } from '@openauthjs/openauth/storage/storage'

import { parseBase64UrlJsonRecord } from '../json-boundary'
import { currentEpochSeconds } from '../runtime-primitives'
import { readBearerToken } from './bearer-token'
import type { VerifiedSession } from './session'

export const DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID =
  'openagents-khala-mobile'
export const KHALA_MOBILE_OPENAUTH_REDIRECT_URI = 'khala://auth'

const PKCE_S256_CHALLENGE = /^[A-Za-z0-9_-]{43,128}$/

export type AuthIssuerRedirectPolicy = Readonly<{
  mobileClientId?: string | undefined
  webClientId: string
}>

export type AuthIssuerRedirectPolicyInput = Readonly<{
  audience?: string | undefined
  clientID: string
  redirectURI: string
}>

export const authIssuerAllowsWebRedirectHostname = (
  hostname: string,
): boolean =>
  hostname === 'openagents.com' ||
  hostname === 'auth.openagents.com' ||
  // Isolated staging Worker. WIDEN-ONLY: this lets the prod issuer accept the
  // staging-origin auth callback so a human can sign in on staging and exercise
  // the billing/credit flow. The staging Worker delegates auth to this same
  // prod issuer (OPENAUTH_ISSUER_URL=auth.openagents.com), so the allowlist must
  // live here. Prod hosts above are unchanged.
  hostname === 'openagents-staging.openagents.workers.dev' ||
  hostname === 'localhost' ||
  hostname === '127.0.0.1'

export const mobileOpenAuthClientId = (
  configured?: string | undefined,
): string => {
  const trimmed = configured?.trim()

  return trimmed === undefined || trimmed === ''
    ? DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID
    : trimmed
}

export const authIssuerAllowsRedirect = (
  input: AuthIssuerRedirectPolicyInput,
  request: Request,
  policy: AuthIssuerRedirectPolicy,
): boolean => {
  const mobileClient = mobileOpenAuthClientId(policy.mobileClientId)

  let redirect: URL
  try {
    redirect = new URL(input.redirectURI)
  } catch {
    return false
  }

  if (input.clientID === policy.webClientId) {
    return authIssuerAllowsWebRedirectHostname(redirect.hostname)
  }

  if (input.clientID !== mobileClient) {
    return false
  }

  const query = new URL(request.url).searchParams
  const isMobileRedirect =
    redirect.protocol === 'khala:' &&
    redirect.hostname === 'auth' &&
    (redirect.pathname === '' || redirect.pathname === '/')
  const isGitHubCodePkce =
    query.get('provider') === 'github' &&
    query.get('response_type') === 'code' &&
    query.get('code_challenge_method') === 'S256' &&
    PKCE_S256_CHALLENGE.test(query.get('code_challenge') ?? '')

  return isMobileRedirect && isGitHubCodePkce
}

/**
 * Exported so other mobile-bearer-authorized surfaces can compute the SAME KV
 * revocation-lookup key from a raw access token without duplicating the hash
 * scheme (MM-G1, #8485: `push/push-device-tokens.ts` stores this key
 * alongside a device's push-token registration at registration time, then
 * later checks `AUTH_STORAGE.get(key)` directly to prune a registration once
 * that exact access token is revoked — never storing the raw token itself).
 */
export const mobileRevokedAccessKey = async (accessToken: string): Promise<string> => {
  const bytes = new TextEncoder().encode(accessToken)
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  const hash = Array.from(new Uint8Array(digest))
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

  return `khala-mobile:openauth:revoked-access:${hash}`
}

const accessTokenTtlSeconds = (accessToken: string): number => {
  const [, claims] = accessToken.split('.')
  const exp =
    claims === undefined ? undefined : parseBase64UrlJsonRecord(claims)?.exp

  if (typeof exp !== 'number') {
    return 60
  }

  return Math.max(60, Math.ceil(exp - currentEpochSeconds()))
}

export type MobileAccessRevocationStore = Pick<KVNamespace, 'get' | 'put'>

export const isMobileAccessTokenRevoked = async (
  store: MobileAccessRevocationStore,
  accessToken: string,
): Promise<boolean> =>
  (await store.get(await mobileRevokedAccessKey(accessToken))) !== null

export const revokeMobileAccessToken = async (
  store: MobileAccessRevocationStore,
  accessToken: string,
): Promise<void> => {
  await store.put(await mobileRevokedAccessKey(accessToken), '1', {
    expirationTtl: accessTokenTtlSeconds(accessToken),
  })
}

export const openAuthRefreshStorageKeyFromToken = (
  refreshToken: string,
): ['oauth:refresh', string, string] | undefined => {
  const parts = refreshToken.split(':')
  const token = parts.pop()
  const subject = parts.join(':')

  if (token === undefined || token === '' || subject === '') {
    return undefined
  }

  return ['oauth:refresh', subject, token]
}

export const revokeOpenAuthRefreshToken = async (
  storage: StorageAdapter,
  refreshToken: string | undefined,
): Promise<boolean> => {
  if (refreshToken === undefined || refreshToken.trim() === '') {
    return false
  }

  const key = openAuthRefreshStorageKeyFromToken(refreshToken)

  if (key === undefined) {
    return false
  }

  await storage.remove(key)

  return true
}

export type UserBearerSessionBoundary<User, Bindings> = Readonly<{
  requireUserBearerSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<VerifiedSession<User> | undefined>
}>

export const makeUserBearerSessionBoundary = <User, Bindings>(
  dependencies: Readonly<{
    isAccessTokenRevoked: (
      env: Bindings,
      accessToken: string,
    ) => Promise<boolean>
    persistUser: (env: Bindings, user: User) => Promise<void>
    verifyTokens: (
      accessToken: string,
      refreshToken: string | undefined,
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ) => Promise<VerifiedSession<User> | undefined>
  }>,
): UserBearerSessionBoundary<User, Bindings> => ({
  requireUserBearerSession: async (request, env, ctx) => {
    const accessToken = readBearerToken(request)

    if (accessToken === undefined) {
      return undefined
    }

    if (await dependencies.isAccessTokenRevoked(env, accessToken)) {
      return undefined
    }

    const session = await dependencies.verifyTokens(
      accessToken,
      undefined,
      request,
      env,
      ctx,
    )

    if (session === undefined) {
      return undefined
    }

    await dependencies.persistUser(env, session.user)

    return session
  },
})

export type MobileAuthSessionJson<User> = Readonly<{
  authenticated: true
  tokens?: Tokens | undefined
  user: User
}>
