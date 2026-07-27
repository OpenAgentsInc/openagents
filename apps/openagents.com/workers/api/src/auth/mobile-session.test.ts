/* eslint-disable @typescript-eslint/consistent-type-assertions */
import {
  KHALA_SYNC_PROTOCOL_VERSION,
  personalScope,
} from '@openagentsinc/khala-sync'
import { generatePKCE } from '@openauthjs/openauth/pkce'
import { describe, expect, test } from 'vitest'

import worker from '../index'
import { paymentsLedgerDbFromD1 } from '../test/payments-ledger-sqlite'
import { IDENTITY_AUTH_DOMAIN_D1_SCHEMA, makeSqliteD1 } from '../test/sqlite-d1'
import { makeMemoryAuthKvStore } from './auth-kv'
import {
  DEFAULT_OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID,
  DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
  OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_PATH,
  OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
  authIssuerAllowsRedirect,
  isMobileAccessTokenRevoked,
  makeUserBearerSessionBoundary,
  openAuthRefreshStorageKeyFromToken,
  readMobileOpenAuthSignOutRefreshToken,
  revokeMobileAccessToken,
} from './mobile-session'
import { makeKvOpenAuthStorage } from './openauth-storage'

type StoredValue = Readonly<{
  expirationTtl?: number
  value: string
}>

const executionContext = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as never

describe('contract openagents_mobile.session.recovered_validation_rotation.v1 — server boundary', () => {
  test('accepts a server-issued native session without sending it to OpenAuth', async () => {
    let openAuthCalls = 0
    const boundary = makeUserBearerSessionBoundary<
      string,
      Record<string, never>
    >({
      isAccessTokenRevoked: async () => false,
      persistUser: async () => undefined,
      verifyNativeToken: async access =>
        access.startsWith('oa_omega_') ? { user: 'omega-owner' } : undefined,
      verifyTokens: async () => {
        openAuthCalls += 1
        return undefined
      },
    })

    await expect(
      boundary.requireUserBearerSession(
        new Request('https://openagents.com/api/mobile/auth/session', {
          headers: { authorization: 'Bearer oa_omega_fixture' },
        }),
        {},
        executionContext,
      ),
    ).resolves.toEqual({ user: 'omega-owner' })
    expect(openAuthCalls).toBe(0)
  })

  test('passes only a bounded native refresh header to the existing verifier', async () => {
    const calls: Array<
      Readonly<{ access: string; refresh: string | undefined }>
    > = []
    const persisted: Array<string> = []
    const boundary = makeUserBearerSessionBoundary<
      string,
      Record<string, never>
    >({
      isAccessTokenRevoked: async () => false,
      persistUser: async (_env, user) => {
        persisted.push(user)
      },
      verifyTokens: async (access, refresh) => {
        calls.push({ access, refresh })
        return { user: 'owner.fixture' }
      },
    })
    const run = (refresh: string | undefined) =>
      boundary.requireUserBearerSession(
        new Request('https://openagents.com/api/mobile/auth/session', {
          headers: {
            authorization: 'Bearer access-fixture',
            ...(refresh === undefined
              ? {}
              : { 'x-openagents-refresh-token': refresh }),
          },
        }),
        {},
        executionContext,
      )

    await expect(run('refresh-fixture')).resolves.toEqual({
      user: 'owner.fixture',
    })
    await expect(run('short')).resolves.toEqual({ user: 'owner.fixture' })
    await expect(run('x'.repeat(2001))).resolves.toEqual({
      user: 'owner.fixture',
    })
    await boundary.requireUserBearerSession(
      new Request('https://openagents.com/api/mobile/credits/balance', {
        headers: {
          authorization: 'Bearer access-fixture',
          'x-openagents-refresh-token': 'refresh-must-not-rotate-here',
        },
      }),
      {},
      executionContext,
    )
    expect(calls).toEqual([
      { access: 'access-fixture', refresh: 'refresh-fixture' },
      { access: 'access-fixture', refresh: undefined },
      { access: 'access-fixture', refresh: undefined },
      { access: 'access-fixture', refresh: undefined },
    ])
    expect(persisted).toEqual([
      'owner.fixture',
      'owner.fixture',
      'owner.fixture',
      'owner.fixture',
    ])
    expect(
      readMobileOpenAuthSignOutRefreshToken(
        new Request('https://openagents.com/api/mobile/auth/session', {
          method: 'DELETE',
          headers: { 'x-openagents-refresh-token': 'refresh-for-sign-out' },
        }),
      ),
    ).toBe('refresh-for-sign-out')
  })
})

const workerConfig = {
  GITHUB_CLIENT_ID: 'github-client',
  GITHUB_CLIENT_SECRET: 'github-secret',
  OPENAGENTS_APP_URL: 'https://openagents.com',
  OPENAUTH_CLIENT_ID: 'openagents-web',
  OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
  OPENAUTH_MOBILE_CLIENT_ID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
}

const makeMemoryKv = (): KVNamespace => {
  const values = new Map<string, StoredValue>()

  return {
    get: (key: string) => Promise.resolve(values.get(key)?.value ?? null),
    put: (key: string, value: string, options?: { expirationTtl?: number }) => {
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
      clientID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
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
      redirectURI: input.redirectURI ?? OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
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

const postToken = (env: unknown, body: URLSearchParams): Promise<Response> =>
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

  test('contract openagents_desktop.session.loopback_pkce_policy.v1 allows only the exact Desktop loopback public-client tuple', () => {
    const allowedRequest = new Request(
      'https://auth.openagents.com/authorize?provider=github&response_type=code&code_challenge_method=S256&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ',
    )
    const allowed = (
      redirectURI: string,
      request: Request = allowedRequest,
      clientID = DEFAULT_OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID,
    ) =>
      authIssuerAllowsRedirect({ clientID, redirectURI }, request, {
        webClientId: 'openagents-web',
      })

    expect(OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_PATH).toBe('/auth/callback')
    expect(allowed('http://127.0.0.1:49152/auth/callback')).toBe(true)
    expect(allowed('http://127.0.0.1:65535/auth/callback')).toBe(true)

    for (const rejected of [
      'http://127.0.0.1/auth/callback',
      'http://127.0.0.1:80/auth/callback',
      'http://localhost:49152/auth/callback',
      'http://[::1]:49152/auth/callback',
      'https://127.0.0.1:49152/auth/callback',
      'http://127.0.0.1:49152/auth/callback/',
      'http://127.0.0.1:49152/auth/callback?code=preloaded',
      'http://127.0.0.1:49152/auth/callback#fragment',
      'http://user@127.0.0.1:49152/auth/callback',
    ]) {
      expect(allowed(rejected)).toBe(false)
    }

    expect(
      allowed(
        'http://127.0.0.1:49152/auth/callback',
        new Request(
          'https://auth.openagents.com/authorize?provider=github&response_type=code&code_challenge_method=plain&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ',
        ),
      ),
    ).toBe(false)
    expect(
      allowed(
        'http://127.0.0.1:49152/auth/callback',
        new Request(
          'https://auth.openagents.com/authorize?provider=code&response_type=code&code_challenge_method=S256&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ',
        ),
      ),
    ).toBe(false)
    expect(
      allowed(
        'http://127.0.0.1:49152/auth/callback',
        allowedRequest,
        DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
      ),
    ).toBe(false)
  })

  test('allows only exact mobile public-client native redirects with GitHub code + S256 PKCE', () => {
    const allowedRequest = new Request(
      'https://auth.openagents.com/authorize?provider=github&response_type=code&code_challenge_method=S256&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ',
    )

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(true)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
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
          clientID: 'configured-other-mobile-client',
          redirectURI: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
        },
        allowedRequest,
        {
          mobileClientId: 'configured-other-mobile-client',
          webClientId: 'openagents-web',
        },
      ),
    ).toBe(false)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: 'unknown-client',
          redirectURI: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(false)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: 'openagents://auth?callback=1',
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(false)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: 'openagents://auth/',
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(false)

    expect(
      authIssuerAllowsRedirect(
        {
          clientID: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          redirectURI: 'https://openagents.com/auth/callback',
        },
        allowedRequest,
        { webClientId: 'openagents-web' },
      ),
    ).toBe(false)
  })

  test('derives the OpenAuth refresh storage key without exposing token material', () => {
    expect(
      openAuthRefreshStorageKeyFromToken('github:12345:refresh-id'),
    ).toEqual(['oauth:refresh', 'github:12345', 'refresh-id'])
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

  test('revokes an opaque Omega session for its full lifetime', async () => {
    let expirationTtl: number | undefined
    const store = {
      get: () => Promise.resolve(null),
      put: (
        _key: string,
        _value: string,
        options?: Readonly<{ expirationTtl?: number }>,
      ) => {
        expirationTtl = options?.expirationTtl
        return Promise.resolve()
      },
    }

    await revokeMobileAccessToken(store, `oa_omega_${'a'.repeat(43)}`)

    expect(expirationTtl).toBe(15 * 60)
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
          client_id: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          code,
          code_verifier: pkce.verifier,
          grant_type: 'authorization_code',
          redirect_uri: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
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
        githubLogin: string
        ownerUserId: string
        syncToken: string
      }

      expect(mobileSession.status).toBe(200)
      expect(mobileSession.headers.get('cache-control')).toBe('no-store')
      // Includes the GitHub login so the mobile onboarding greeting can
      // personalize ("Welcome, <login>").
      expect(mobileSessionBody).toEqual({
        githubLogin: 'octo-mobile',
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
          githubLogin: string
          ownerUserId: string
          syncToken: string
        }

      expect(refreshedMobileSession.status).toBe(200)
      expect(refreshedMobileSessionBody).toEqual({
        githubLogin: 'octo-mobile',
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
          client_id: DEFAULT_OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
          code,
          code_verifier: 'wrong-verifier',
          grant_type: 'authorization_code',
          redirect_uri: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
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
