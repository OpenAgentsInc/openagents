/* eslint-disable @typescript-eslint/consistent-type-assertions */
import { paymentsLedgerDbFromD1 } from '../test/payments-ledger-sqlite'
import { generatePKCE } from '@openauthjs/openauth/pkce'
import { describe, expect, test } from 'vitest'

import {
  KHALA_SYNC_PROTOCOL_VERSION,
  personalScope,
} from '@openagentsinc/khala-sync'

import {
  DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
  KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
  authIssuerAllowsRedirect,
  isMobileAccessTokenRevoked,
  openAuthRefreshStorageKeyFromToken,
  revokeMobileAccessToken,
} from './mobile-session'
import { makeMemoryAuthKvStore } from './auth-kv'
import { makeKvOpenAuthStorage } from './openauth-storage'
import worker from '../index'
import {
  IDENTITY_AUTH_DOMAIN_D1_SCHEMA,
  makeSqliteD1,
} from '../test/sqlite-d1'

type StoredValue = Readonly<{
  expirationTtl?: number
  value: string
}>

const executionContext = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as never

const workerConfig = {
  GITHUB_CLIENT_ID: 'github-client',
  GITHUB_CLIENT_SECRET: 'github-secret',
  OPENAGENTS_APP_URL: 'https://openagents.com',
  OPENAUTH_CLIENT_ID: 'openagents-web',
  OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
  OPENAUTH_MOBILE_CLIENT_ID: DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
}

const makeMemoryKv = (): KVNamespace => {
  const values = new Map<string, StoredValue>()

  return {
    get: (key: string) => Promise.resolve(values.get(key)?.value ?? null),
    put: (
      key: string,
      value: string,
      options?: { expirationTtl?: number },
    ) => {
      const expirationTtl = options?.expirationTtl

      values.set(
        key,
        expirationTtl === undefined ? { value } : { value, expirationTtl },
      )

      return Promise.resolve()
    },
    delete: (key: string) => {
      values.delete(key)

      return Promise.resolve()
    },
  } as KVNamespace
}

const makeEnv = () => {
  const sqlite = makeSqliteD1()
  sqlite.exec(IDENTITY_AUTH_DOMAIN_D1_SCHEMA)
  sqlite.exec(
    `CREATE UNIQUE INDEX auth_identities_provider_subject_unique
       ON auth_identities(provider, provider_subject)`,
  )

  // CFG-3 (#8518): the issuer StorageAdapter and the revocation markers both
  // live on the SAME owned KvStore now; the test seeds and the worker reads
  // one shared memory store (via the AUTH_KV test override).
  const kv = makeMemoryAuthKvStore()

  return {
    close: sqlite.close,
    env: {
      ...workerConfig,
      AUTH_KV: kv,
      // CFG-4 Domain 2 (#8519): `users`/`auth_identities` are Postgres-
      // authoritative; the worker's identity handle is backed by the same
      // SQLite database in this test (same override pattern as AUTH_KV).
      IDENTITY_DB: paymentsLedgerDbFromD1(sqlite.db as never),
      OPENAGENTS_DB: sqlite.db,
    } as never,
    storage: makeKvOpenAuthStorage(kv),
  }
}

const seedAuthorizationCode = async (
  storage: ReturnType<typeof makeKvOpenAuthStorage>,
  input: Readonly<{
    challenge: string
    code: string
    method: 'S256'
    redirectURI?: string
  }>,
) => {
  await storage.set(
    ['oauth:code', input.code],
    {
      clientID: DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
      pkce: {
        challenge: input.challenge,
        method: input.method,
      },
      properties: {
        userId: 'github:12345',
        provider: 'github',
        githubId: '12345',
        login: 'octo-mobile',
        email: 'octo@example.com',
        name: 'Octo Mobile',
        avatarUrl: 'https://avatars.example/octo.png',
      },
      redirectURI: input.redirectURI ?? KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
      subject: 'github:12345',
      ttl: {
        access: 3600,
        refresh: 3600,
      },
      type: 'user',
    },
    new Date(Date.now() + 60_000),
  )
}

const postToken = (
  env: unknown,
  body: URLSearchParams,
): Promise<Response> =>
  worker.fetch(
    new Request('https://auth.openagents.com/token', {
      body,
      method: 'POST',
    }) as never,
    env as never,
    executionContext,
  )

const postMobileSession = (
  env: unknown,
  accessToken?: string | undefined,
): Promise<Response> => {
  const requestInit: RequestInit = { method: 'POST' }

  if (accessToken !== undefined) {
    requestInit.headers = { authorization: `Bearer ${accessToken}` }
  }

  return worker.fetch(
    new Request(
      'https://openagents.com/api/mobile/session',
      requestInit,
    ) as never,
    env as never,
    executionContext,
  )
}

const postSyncBootstrap = (
  env: unknown,
  accessToken: string,
  scope: string,
): Promise<Response> =>
  worker.fetch(
    new Request('https://openagents.com/api/sync/bootstrap', {
      body: JSON.stringify({
        clientGroupId: 'mobile-cg',
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        schemaVersion: 1,
        scope,
      }),
      headers: {
        authorization: `Bearer ${accessToken}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    }) as never,
    env as never,
    executionContext,
  )

describe('Khala mobile OpenAuth session policy', () => {
  test('allows web client redirects without changing the browser hostname policy', () => {
    expect(
      authIssuerAllowsRedirect(
        {
          clientID: 'openagents-web',
          redirectURI: 'https://openagents.com/auth/callback',
        },
        new Request('https://auth.openagents.com/authorize'),
        { webClientId: 'openagents-web' },
      ),
    ).toBe(true)
  })

  test('allows only the mobile public client with GitHub code + S256 PKCE on khala://auth', () => {
    const allowedRequest = new Request(
      'https://auth.openagents.com/authorize?provider=github&response_type=code&code_challenge_method=S256&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ',
    )

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(true)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
        },
        new Request(
          'https://auth.openagents.com/authorize?provider=github&response_type=code&code_challenge_method=plain&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ',
        ),
        { webClientId: 'openagents-web' },
      ),
    ).toBe(false)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: 'unknown-client',
          redirectURI: KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(false)
  })

  test('derives the OpenAuth refresh storage key without exposing token material', () => {
    expect(openAuthRefreshStorageKeyFromToken('github:12345:refresh-id')).toEqual(
      ['oauth:refresh', 'github:12345', 'refresh-id'],
    )
    expect(openAuthRefreshStorageKeyFromToken('malformed')).toBeUndefined()
  })

  test('records exact access-token revocation by hash', async () => {
    const kv = makeMemoryKv()
    const token =
      'eyJhbGciOiJSUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.signature-not-real'

    await expect(isMobileAccessTokenRevoked(kv, token)).resolves.toBe(false)
    await revokeMobileAccessToken(kv, token)
    await expect(isMobileAccessTokenRevoked(kv, token)).resolves.toBe(true)
  })

  test('exchanges a seeded OpenAuth code with S256 PKCE, refreshes, verifies bearer auth, and signs out', async () => {
    const { close, env, storage } = makeEnv()
    const pkce = await generatePKCE()
    const code = 'mobile-auth-code-ok'

    try {
      await seedAuthorizationCode(storage, {
        challenge: pkce.challenge,
        code,
        method: 'S256',
      })

      const exchanged = await postToken(
        env,
        new URLSearchParams({
          client_id: DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
          code,
          code_verifier: pkce.verifier,
          grant_type: 'authorization_code',
          redirect_uri: KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
        }),
      )
      const tokens = (await exchanged.json()) as {
        access_token: string
        expires_in: number
        refresh_token: string
      }

      expect(exchanged.status).toBe(200)
      expect(tokens.access_token).toMatch(/^ey/)
      expect(tokens.refresh_token).toContain('github:12345:')
      expect(tokens.expires_in).toBeGreaterThan(0)

      const session = await worker.fetch(
        new Request('https://openagents.com/api/mobile/auth/session', {
          headers: { authorization: `Bearer ${tokens.access_token}` },
        }) as never,
        env,
        executionContext,
      )
      const sessionBody = (await session.json()) as {
        authenticated: boolean
        user: { userId: string; login: string }
      }

      expect(session.status).toBe(200)
      expect(sessionBody).toMatchObject({
        authenticated: true,
        user: {
          login: 'octo-mobile',
          userId: 'github:12345',
        },
      })

      const unsignedMobileSession = await postMobileSession(env)
      expect(unsignedMobileSession.status).toBe(401)

      const mobileSession = await postMobileSession(env, tokens.access_token)
      const mobileSessionBody = (await mobileSession.json()) as {
        ownerUserId: string
        syncToken: string
      }

      expect(mobileSession.status).toBe(200)
      expect(mobileSession.headers.get('cache-control')).toBe('no-store')
      expect(mobileSessionBody).toEqual({
        ownerUserId: 'github:12345',
        syncToken: tokens.access_token,
      })

      const ownSyncBootstrap = await postSyncBootstrap(
        env,
        mobileSessionBody.syncToken,
        personalScope(mobileSessionBody.ownerUserId),
      )
      const ownSyncBody = (await ownSyncBootstrap.json()) as { code: string }

      expect(ownSyncBootstrap.status).toBe(503)
      expect(ownSyncBody.code).toBe('storage_unavailable')

      const foreignSyncBootstrap = await postSyncBootstrap(
        env,
        mobileSessionBody.syncToken,
        personalScope('someone-else'),
      )
      const foreignSyncBody = (await foreignSyncBootstrap.json()) as {
        code: string
      }

      expect(foreignSyncBootstrap.status).toBe(403)
      expect(foreignSyncBody.code).toBe('unauthorized_scope')

      const refreshed = await postToken(
        env,
        new URLSearchParams({
          grant_type: 'refresh_token',
          refresh_token: tokens.refresh_token,
        }),
      )
      const refreshedTokens = (await refreshed.json()) as {
        access_token: string
        refresh_token: string
      }

      expect(refreshed.status).toBe(200)
      expect(refreshedTokens.access_token).toMatch(/^ey/)
      expect(refreshedTokens.refresh_token).not.toBe(tokens.refresh_token)

      const refreshedMobileSession = await postMobileSession(
        env,
        refreshedTokens.access_token,
      )
      const refreshedMobileSessionBody =
        (await refreshedMobileSession.json()) as {
          ownerUserId: string
          syncToken: string
        }

      expect(refreshedMobileSession.status).toBe(200)
      expect(refreshedMobileSessionBody).toEqual({
        ownerUserId: 'github:12345',
        syncToken: refreshedTokens.access_token,
      })

      const signOut = await worker.fetch(
        new Request('https://openagents.com/api/mobile/auth/session', {
          body: JSON.stringify({ refreshToken: refreshedTokens.refresh_token }),
          headers: {
            authorization: `Bearer ${refreshedTokens.access_token}`,
            'content-type': 'application/json',
          },
          method: 'DELETE',
        }) as never,
        env,
        executionContext,
      )
      const signOutBody = (await signOut.json()) as {
        accessRevoked: boolean
        refreshRevoked: boolean
        signedOut: boolean
      }

      expect(signOut.status).toBe(200)
      expect(signOutBody).toEqual({
        accessRevoked: true,
        refreshRevoked: true,
        signedOut: true,
      })

      const afterSignOut = await worker.fetch(
        new Request('https://openagents.com/api/mobile/auth/session', {
          headers: { authorization: `Bearer ${refreshedTokens.access_token}` },
        }) as never,
        env,
        executionContext,
      )
      expect(afterSignOut.status).toBe(401)

      const mobileSessionAfterSignOut = await postMobileSession(
        env,
        refreshedTokens.access_token,
      )
      expect(mobileSessionAfterSignOut.status).toBe(401)

      const syncAfterSignOut = await postSyncBootstrap(
        env,
        refreshedTokens.access_token,
        personalScope('github:12345'),
      )
      const syncAfterSignOutBody = (await syncAfterSignOut.json()) as {
        code: string
      }

      expect(syncAfterSignOut.status).toBe(401)
      expect(syncAfterSignOutBody.code).toBe('unauthenticated')

      const refreshAfterSignOut = await postToken(
        env,
        new URLSearchParams({
          grant_type: 'refresh_token',
          refresh_token: refreshedTokens.refresh_token,
        }),
      )
      expect(refreshAfterSignOut.status).toBe(400)
    } finally {
      close()
    }
  })

  test('rejects an authorization-code exchange with the wrong PKCE verifier', async () => {
    const { close, env, storage } = makeEnv()
    const pkce = await generatePKCE()
    const code = 'mobile-auth-code-bad-verifier'

    try {
      await seedAuthorizationCode(storage, {
        challenge: pkce.challenge,
        code,
        method: 'S256',
      })

      const exchanged = await postToken(
        env,
        new URLSearchParams({
          client_id: DEFAULT_KHALA_MOBILE_OPENAUTH_CLIENT_ID,
          code,
          code_verifier: 'wrong-verifier',
          grant_type: 'authorization_code',
          redirect_uri: KHALA_MOBILE_OPENAUTH_REDIRECT_URI,
        }),
      )
      const body = (await exchanged.json()) as { error: string }

      expect(exchanged.status).toBe(400)
      expect(body.error).toBe('invalid_grant')
    } finally {
      close()
    }
  })
})
