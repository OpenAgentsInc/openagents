// Wiring-level tests for Omega hosted-identity self-provisioning.
//
// WHY THIS FILE EXISTS
// --------------------
// `omega-nostr-self-provision.ts` (the bounds) and `omega-nostr-session.ts`
// (the handler) both landed on 2026-07-28 with thorough unit tests. Every one
// of those tests INJECTS the `selfProvision` hooks. Production did not: the
// `makeOmegaNostrSessionService` call in `./index` supplied
// `resolveLinked`/`resolveOwner` and nothing else, so the hooks were
// `undefined`, the `enabled(env)` kill switch was never consulted, and
// `OMEGA_NOSTR_SELF_PROVISION_ENABLED` was inert — setting it to `true` on
// Cloud Run changed nothing. Every brand-new install signed a perfectly valid
// NIP-98 proof and got a permanent 401 (`this identity is not admitted for
// hosted Omega`), which is exactly the rc29 release blocker: a fresh install
// could not complete a single turn, so the alpha was usable only by the owner,
// whose pubkey already had an `auth_identities` row from the device-link flow.
//
// The whole unit suite stayed green through all of it, for the same reason the
// ST-3 khala-sync wiring bug stayed invisible: a fake that is always supplied
// cannot detect a production call site that never supplies it.
//
// These tests therefore drive the REAL route-table entry exported from
// `./index` against a fake env (in-memory identity DB + in-memory auth KV, no
// network). The load-bearing assertions:
//  - flag ABSENT: a fresh, valid, unlinked proof is refused 401 (the
//    documented fail-closed default).
//  - flag ARMED: the SAME proof shape mints a session, and the minted subject
//    is derived entirely from the signing pubkey — never the owner/admin.
//  - the creation budget is real and reachable through the production wiring
//    (global limit 0 => 429 naming `creation_global`).
//  - a RETURNING install does not spend the creation budget.
//
// Audit ref: docs/audits/2026-07-28-omega-nostr-self-provisioning-abuse-bounds.md

import { Effect } from 'effect'
import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import { describe, expect, test } from 'vitest'

import { makeMemoryAuthKvStore } from './auth/auth-kv'
import { OMEGA_NOSTR_SESSION_PATH } from './auth/omega-nostr-session'
import { type Env, exactRouteHandlerForPath } from './index'

const url = `https://openagents.com${OMEGA_NOSTR_SESSION_PATH}`

const authorization = (secret: Uint8Array): string =>
  `Nostr ${btoa(
    JSON.stringify(
      finalizeEvent(
        {
          content: '',
          created_at: Math.floor(Date.now() / 1_000),
          kind: 27_235,
          tags: [
            ['u', url],
            ['method', 'POST'],
            ['payload', emptyPayloadHash()],
          ],
        },
        secret,
      ),
    ),
  )}`

const emptyPayloadHash = (): string => hashPayloadBytes(new Uint8Array())

// ---------------------------------------------------------------------------
// A minimal identity DB that records writes and answers the two reads the
// self-provision path performs: the `auth_identities` link lookup
// (`resolveNostrLinkedUser`) and the `users` row existence probe
// (`omegaNostrUserRowExists`).
// ---------------------------------------------------------------------------

type SeededUser = Readonly<{ pubkey: string; userId: string }>

const fakeIdentityDb = (seeded: ReadonlyArray<SeededUser> = []) => {
  const userRows = new Set(seeded.map(entry => entry.userId))
  const links = new Map(seeded.map(entry => [entry.pubkey, entry.userId]))
  const batches: Array<ReadonlyArray<{ sql: string }>> = []

  return {
    batches,
    db: {
      batch: async (statements: ReadonlyArray<{ sql: string }>) => {
        batches.push(statements)
        for (const statement of statements) {
          if (statement.sql.includes('INSERT INTO users')) {
            const params = (statement as { params?: ReadonlyArray<unknown> })
              .params
            userRows.add(String(params?.[0] ?? ''))
          }
          if (statement.sql.includes('INSERT INTO auth_identities')) {
            const params = (statement as { params?: ReadonlyArray<unknown> })
              .params
            links.set(String(params?.[2] ?? ''), String(params?.[1] ?? ''))
          }
        }
      },
      query: async (sql: string, params: ReadonlyArray<unknown> = []) => {
        // omegaNostrUserRowExists
        if (sql.includes('SELECT 1 AS present FROM users')) {
          return userRows.has(String(params[0] ?? '')) ? [{ present: 1 }] : []
        }
        // resolveNostrLinkedUser
        if (sql.includes("FROM auth_identities AS nostr")) {
          const userId = links.get(String(params[0] ?? ''))
          return userId === undefined
            ? []
            : [
                {
                  avatar_url: '',
                  display_name: 'seeded',
                  github_id: null,
                  github_login: null,
                  id: userId,
                  primary_email: `${params[0]}@nostr.invalid`,
                },
              ]
        }
        return []
      },
    },
    links,
    userRows,
  }
}

const fakeCtx = (): ExecutionContext =>
  ({
    passThroughOnException: () => {},
    props: {},
    waitUntil: () => {},
  }) as unknown as ExecutionContext

const dispatch = async (request: Request, env: Env): Promise<Response> => {
  const handler = exactRouteHandlerForPath(OMEGA_NOSTR_SESSION_PATH)
  if (handler === undefined) {
    throw new Error(`route table has no entry for ${OMEGA_NOSTR_SESSION_PATH}`)
  }
  return Effect.runPromise(handler(request, env, fakeCtx()))
}

const signInRequest = (secret: Uint8Array): Request =>
  new Request(url, {
    headers: { authorization: authorization(secret) },
    method: 'POST',
  })

const envFor = (
  identityDb: ReturnType<typeof fakeIdentityDb>,
  overrides: Readonly<Record<string, string>> = {},
): Env =>
  ({
    AUTH_KV: makeMemoryAuthKvStore(),
    IDENTITY_DB: identityDb.db,
    ...overrides,
  }) as unknown as Env

describe('Omega hosted-identity self-provisioning route wiring', () => {
  test('refuses a fresh unlinked identity when the flag is absent', async () => {
    const secret = generateSecretKey()
    const identityDb = fakeIdentityDb()

    const response = await dispatch(signInRequest(secret), envFor(identityDb))

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
    // Fail-closed means nothing was created either.
    expect(identityDb.batches).toEqual([])
  })

  test('mints a session for a fresh identity when the flag is armed', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const identityDb = fakeIdentityDb()

    const response = await dispatch(
      signInRequest(secret),
      envFor(identityDb, { OMEGA_NOSTR_SELF_PROVISION_ENABLED: 'true' }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      accessToken: string
      expiresIn: number
      user: { email: string; provider: string; userId: string }
    }
    expect(body.accessToken).toMatch(/^oa_omega_/u)
    expect(body.expiresIn).toBe(900)
    // The subject is derived ENTIRELY from the signing pubkey. If this ever
    // returns the owner/admin subject, the endpoint has become a backdoor.
    expect(body.user.userId).toBe(`nostr:${pubkey}`)
    expect(body.user.email).toBe(`${pubkey}@nostr.invalid`)
    expect(body.user.provider).toBe('nostr')
    // The account really was persisted through the production upsert.
    expect(identityDb.userRows.has(`nostr:${pubkey}`)).toBe(true)
    expect(identityDb.links.get(pubkey)).toBe(`nostr:${pubkey}`)
  })

  test('the armed flag is what changes the outcome, not the proof', async () => {
    // Same key, same env shape, only the flag differs — this is the assertion
    // that would have caught the inert kill switch.
    const secret = generateSecretKey()

    const refused = await dispatch(
      signInRequest(secret),
      envFor(fakeIdentityDb()),
    )
    const admitted = await dispatch(
      signInRequest(secret),
      envFor(fakeIdentityDb(), { OMEGA_NOSTR_SELF_PROVISION_ENABLED: '1' }),
    )

    expect(refused.status).toBe(401)
    expect(admitted.status).toBe(200)
  })

  test('the global creation budget is enforced through the real wiring', async () => {
    const secret = generateSecretKey()
    const identityDb = fakeIdentityDb()

    const response = await dispatch(
      signInRequest(secret),
      envFor(identityDb, {
        OMEGA_NOSTR_SELF_PROVISION_ENABLED: '1',
        OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT: '0',
      }),
    )

    expect(response.status).toBe(429)
    const body = (await response.json()) as { error: string; scope: string }
    expect(body.error).toBe('omega_nostr_self_provision_rate_limited')
    expect(body.scope).toBe('creation_global')
    expect(response.headers.get('retry-after')).not.toBeNull()
    // A refused creation must not leave a half-made account behind.
    expect(identityDb.batches).toEqual([])
  })

  test('a returning install does not spend the creation budget', async () => {
    // The creation budget is exhausted (limit 0), but this pubkey is already
    // linked, so the mint path must not consult the creation buckets at all.
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const identityDb = fakeIdentityDb([
      { pubkey, userId: `nostr:${pubkey}` },
    ])

    const response = await dispatch(
      signInRequest(secret),
      envFor(identityDb, {
        OMEGA_NOSTR_SELF_PROVISION_ENABLED: '1',
        OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT: '0',
      }),
    )

    expect(response.status).toBe(200)
  })
})
