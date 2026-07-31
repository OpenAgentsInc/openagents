import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import { describe, expect, test, vi } from 'vitest'

import { makeMemoryAuthKvStore } from './auth-kv'
import {
  OMEGA_NOSTR_SESSION_PATH,
  makeOmegaNostrSessionHandler,
  makeOmegaNostrSessionService,
  readOmegaNostrSession,
} from './omega-nostr-session'

const now = new Date('2026-07-27T17:00:00.000Z')
const url = `https://openagents.com${OMEGA_NOSTR_SESSION_PATH}`

const authorization = (
  secret: Uint8Array,
  requestUrl = url,
  createdAt = now,
  payload: Uint8Array = new Uint8Array(),
  extraTags: ReadonlyArray<ReadonlyArray<string>> = [],
): string => {
  const event = finalizeEvent(
    {
      content: '',
      created_at: Math.floor(createdAt.getTime() / 1_000),
      kind: 27_235,
      tags: [
        ['u', requestUrl],
        ['method', 'POST'],
        ['payload', hashPayloadBytes(payload)],
        ...extraTags.map(tag => [...tag]),
      ],
    },
    secret,
  )
  return `Nostr ${btoa(JSON.stringify(event))}`
}

describe('Omega background Nostr session', () => {
  test('mints a short-lived bearer session from the configured owner key', async () => {
    const secret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const owner = { userId: 'github:owner', email: 'owner@example.com' }
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveOwner: async () => owner,
    })
    const response = await handler(
      new Request(url, {
        headers: { authorization: authorization(secret) },
        method: 'POST',
      }),
      {},
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      accessToken: string
      expiresIn: number
    }
    expect(body.accessToken).toMatch(/^oa_omega_/)
    expect(body.expiresIn).toBe(900)
    await expect(
      readOmegaNostrSession(store, body.accessToken),
    ).resolves.toEqual(owner)
  })

  test('rejects another key, a wrong URL, a stale proof, and replay', async () => {
    const ownerSecret = generateSecretKey()
    const otherSecret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => getPublicKey(ownerSecret),
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })
    const run = (auth: string) =>
      handler(
        new Request(url, {
          headers: { authorization: auth },
          method: 'POST',
        }),
        {},
      )

    expect((await run(authorization(otherSecret))).status).toBe(401)
    expect(
      (await run(authorization(ownerSecret, 'https://evil.example/session')))
        .status,
    ).toBe(401)
    expect(
      (
        await run(
          authorization(ownerSecret, url, new Date(now.getTime() - 61_000)),
        )
      ).status,
    ).toBe(401)
    const proof = authorization(ownerSecret)
    expect((await run(proof)).status).toBe(200)
    expect((await run(proof)).status).toBe(409)
  })

  test('atomically permits one mint when the same proof arrives concurrently', async () => {
    const secret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })
    const proof = authorization(secret)
    const responses = await Promise.all(
      Array.from({ length: 8 }, () =>
        handler(
          new Request(url, {
            headers: { authorization: proof },
            method: 'POST',
          }),
          {},
        ),
      ),
    )

    expect(responses.filter(response => response.status === 200)).toHaveLength(
      1,
    )
    expect(responses.filter(response => response.status === 409)).toHaveLength(
      7,
    )
  })

  test('mints the normal session for an existing linked key while self-provisioning is off', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const store = makeMemoryAuthKvStore()
    const canonical = {
      userId: 'github:canonical-owner',
      email: 'owner@example.com',
    }
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => undefined,
      now: () => now,
      resolveLinked: async (_env: unknown, candidate: string) =>
        candidate === pubkey ? canonical : undefined,
      resolveOwner: async () => undefined,
    })
    const response = await handler(
      new Request(url, {
        headers: { authorization: authorization(secret) },
        method: 'POST',
      }),
      {},
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      accessToken: string
      user: { userId: string }
    }
    expect(body.accessToken).toMatch(/^oa_omega_/u)
    expect(body.user.userId).toBe(canonical.userId)
    await expect(
      readOmegaNostrSession(store, body.accessToken),
    ).resolves.toEqual(canonical)
  })

  test('rejects an unlinked key while self-provisioning is off', async () => {
    const secret = generateSecretKey()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => makeMemoryAuthKvStore(),
      expectedOwnerPubkey: () => undefined,
      now: () => now,
      resolveLinked: async () => undefined,
      resolveOwner: async () => undefined,
    })
    const response = await handler(
      new Request(url, {
        headers: { authorization: authorization(secret) },
        method: 'POST',
      }),
      {},
    )

    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({ error: 'unauthorized' })
  })

  test('refuses an owner key when its canonical account link conflicts', async () => {
    const secret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const linkOwner = vi.fn(async () => false)
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => getPublicKey(secret),
      linkOwner,
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })
    const response = await handler(
      new Request(url, {
        headers: { authorization: authorization(secret) },
        method: 'POST',
      }),
      {},
    )

    expect(response.status).toBe(401)
    expect(linkOwner).toHaveBeenCalledWith(
      {},
      { userId: 'github:owner' },
      getPublicKey(secret),
    )
  })

  test('binds the exact body bytes and rejects duplicate required tags', async () => {
    const secret = generateSecretKey()
    const makeHandler = () =>
      makeOmegaNostrSessionHandler({
        authStore: () => makeMemoryAuthKvStore(),
        expectedOwnerPubkey: () => getPublicKey(secret),
        now: () => now,
        resolveOwner: async () => ({ userId: 'github:owner' }),
      })
    const payload = new TextEncoder().encode('{"deviceRef":"omega-1"}')
    const changedPayload = '{"deviceRef": "omega-1"}'

    const service = makeOmegaNostrSessionService({
      authStore: () => makeMemoryAuthKvStore(),
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })
    const changedBodyResult = await service.issue(
      new Request(url, {
        body: changedPayload,
        headers: { authorization: authorization(secret, url, now, payload) },
        method: 'POST',
      }),
      {},
      new TextEncoder().encode(changedPayload),
    )
    expect(changedBodyResult._tag).toBe('Rejected')
    if (changedBodyResult._tag === 'Rejected') {
      expect(changedBodyResult.response.status).toBe(401)
    }

    const exactBodyResult = await service.issue(
      new Request(url, {
        body: payload,
        headers: { authorization: authorization(secret, url, now, payload) },
        method: 'POST',
      }),
      {},
      payload,
    )
    expect(exactBodyResult._tag).toBe('Issued')

    for (const duplicate of [
      ['u', url],
      ['method', 'POST'],
      ['payload', hashPayloadBytes(new Uint8Array())],
    ]) {
      const response = await makeHandler()(
        new Request(url, {
          headers: {
            authorization: authorization(secret, url, now, new Uint8Array(), [
              duplicate,
            ]),
          },
          method: 'POST',
        }),
        {},
      )
      expect(response.status).toBe(401)
    }
  })
})

// Omega self-provisioning (2026-07-28). The gate that used to reject every
// non-owner key is gone; every OTHER NIP-98 term, plus explicit abuse bounds,
// is what stands between a stranger and owner-funded hosted compute.
describe('Omega Nostr per-install self-provisioning', () => {
  const makeSelfProvisionHandler = (
    overrides: {
      enabled?: boolean
      ownerPubkey?: string | undefined
      provision?: (env: unknown, pubkey: string) => Promise<unknown>
      reserve?: () => Promise<
        | { _tag: 'Allowed'; remaining: [] }
        | {
            _tag: 'RateLimited'
            limit: number
            resetAt: string
            retryAfterSeconds: number
            scope: 'creation_global'
            windowSeconds: number
          }
      >
      userExists?: boolean
    } = {},
  ) => {
    const store = makeMemoryAuthKvStore()
    const provisioned: Array<string> = []
    const reserveCalls: Array<{
      creating: boolean
      ipAddress: string
      pubkey: string
    }> = []
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => overrides.ownerPubkey,
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
      selfProvision: {
        enabled: () => overrides.enabled ?? true,
        provision:
          overrides.provision ??
          (async (_env: unknown, pubkey: string) => {
            provisioned.push(pubkey)

            return { provider: 'nostr', userId: `nostr:${pubkey}` }
          }),
        reserve: async (_env: unknown, input) => {
          reserveCalls.push(input)

          return (
            overrides.reserve?.() ??
            Promise.resolve({ _tag: 'Allowed' as const, remaining: [] })
          )
        },
        userExists: async () => overrides.userExists ?? false,
      },
    })

    return { handler, provisioned, reserveCalls, store }
  }

  const post = (
    handler: ReturnType<typeof makeSelfProvisionHandler>['handler'],
    auth: string,
    headers: Record<string, string> = {},
  ) =>
    handler(
      new Request(url, {
        headers: { authorization: auth, ...headers },
        method: 'POST',
      }),
      {},
    )

  test('a brand-new install mints a session bound to ITS OWN pubkey', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const { handler, provisioned, store } = makeSelfProvisionHandler({
      ownerPubkey: getPublicKey(generateSecretKey()),
    })
    const response = await post(handler, authorization(secret), {
      'x-forwarded-for': '203.0.113.5, 10.0.0.1',
    })
    const body = (await response.json()) as {
      accessToken: string
      user: { userId: string }
    }

    expect(response.status).toBe(200)
    expect(provisioned).toEqual([pubkey])
    // The session subject is the INSTALL, never the admin account the old
    // owner-only path resolved.
    expect(body.user.userId).toBe(`nostr:${pubkey}`)
    expect(body.user.userId).not.toBe('github:owner')
    await expect(
      readOmegaNostrSession(store, body.accessToken),
    ).resolves.toEqual({
      provider: 'nostr',
      userId: `nostr:${pubkey}`,
    })
  })

  test('the configured owner key still resolves the owner and skips the buckets', async () => {
    const ownerSecret = generateSecretKey()
    const { handler, provisioned, reserveCalls } = makeSelfProvisionHandler({
      ownerPubkey: getPublicKey(ownerSecret),
    })
    const response = await post(handler, authorization(ownerSecret))
    const body = (await response.json()) as { user: { userId: string } }

    expect(response.status).toBe(200)
    expect(body.user.userId).toBe('github:owner')
    expect(provisioned).toEqual([])
    expect(reserveCalls).toEqual([])
  })

  test('with the kill switch OFF a non-owner key gets the historical 401', async () => {
    const secret = generateSecretKey()
    const { handler, provisioned } = makeSelfProvisionHandler({
      enabled: false,
      ownerPubkey: getPublicKey(generateSecretKey()),
    })
    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({ error: 'unauthorized' })
    expect(provisioned).toEqual([])
  })

  test('the kill switch does not disturb the owner path', async () => {
    const ownerSecret = generateSecretKey()
    const { handler } = makeSelfProvisionHandler({
      enabled: false,
      ownerPubkey: getPublicKey(ownerSecret),
    })

    expect((await post(handler, authorization(ownerSecret))).status).toBe(200)
  })

  test('every remaining NIP-98 term still rejects a self-provisioning caller', async () => {
    const secret = generateSecretKey()
    const { handler, provisioned } = makeSelfProvisionHandler({
      ownerPubkey: getPublicKey(generateSecretKey()),
    })

    // Wrong `u` tag.
    expect(
      (
        await post(
          handler,
          authorization(secret, 'https://evil.example/session'),
        )
      ).status,
    ).toBe(401)
    // Stale `created_at` beyond the skew window.
    expect(
      (
        await post(
          handler,
          authorization(secret, url, new Date(now.getTime() - 61_000)),
        )
      ).status,
    ).toBe(401)
    // Future `created_at` beyond the skew window.
    expect(
      (
        await post(
          handler,
          authorization(secret, url, new Date(now.getTime() + 61_000)),
        )
      ).status,
    ).toBe(401)
    // Not a Nostr authorization scheme at all.
    expect((await post(handler, 'Bearer oa_omega_nope')).status).toBe(401)
    // Tampered signature: re-sign with one key, then swap in another pubkey.
    const forged = JSON.parse(
      atob(authorization(secret).slice('Nostr '.length)),
    ) as Record<string, unknown>
    forged.pubkey = getPublicKey(generateSecretKey())
    expect(
      (await post(handler, `Nostr ${btoa(JSON.stringify(forged))}`)).status,
    ).toBe(401)
    expect(provisioned).toEqual([])

    // Replay of a good proof is still refused after it succeeds once.
    const proof = authorization(secret)
    expect((await post(handler, proof)).status).toBe(200)
    expect((await post(handler, proof)).status).toBe(409)
  })

  test('the limiter is told whether this mint CREATES an account', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const fresh = makeSelfProvisionHandler({ userExists: false })
    await post(fresh.handler, authorization(secret), {
      'x-forwarded-for': '198.51.100.4',
    })

    expect(fresh.reserveCalls).toEqual([
      { creating: true, ipAddress: '198.51.100.4', pubkey },
    ])

    const returning = makeSelfProvisionHandler({ userExists: true })
    await post(returning.handler, authorization(secret), {
      'x-real-ip': '198.51.100.5',
    })

    expect(returning.reserveCalls).toEqual([
      { creating: false, ipAddress: '198.51.100.5', pubkey },
    ])
  })

  test('an exhausted bucket refuses with 429 and a retry-after, minting nothing', async () => {
    const secret = generateSecretKey()
    const { handler, provisioned } = makeSelfProvisionHandler({
      reserve: async () => ({
        _tag: 'RateLimited' as const,
        limit: 200,
        resetAt: '2026-07-29T00:00:00.000Z',
        retryAfterSeconds: 43_200,
        scope: 'creation_global' as const,
        windowSeconds: 86_400,
      }),
    })
    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(429)
    expect(response.headers.get('retry-after')).toBe('43200')
    expect(await response.json()).toMatchObject({
      error: 'omega_nostr_self_provision_rate_limited',
      limit: 200,
      scope: 'creation_global',
    })
    expect(provisioned).toEqual([])
  })

  test('a refused reservation still consumes the one-time proof', async () => {
    const secret = generateSecretKey()
    let limited = true
    const store = makeMemoryAuthKvStore()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => undefined,
      now: () => now,
      resolveOwner: async () => undefined,
      selfProvision: {
        enabled: () => true,
        provision: async (_env: unknown, pubkey: string) => ({
          userId: `nostr:${pubkey}`,
        }),
        reserve: async () =>
          limited
            ? {
                _tag: 'RateLimited' as const,
                limit: 3,
                resetAt: '2026-07-28T13:00:00.000Z',
                retryAfterSeconds: 3_600,
                scope: 'creation_ip' as const,
                windowSeconds: 3_600,
              }
            : { _tag: 'Allowed' as const, remaining: [] },
        userExists: async () => false,
      },
    })
    const proof = authorization(secret)

    expect(
      (
        await handler(
          new Request(url, {
            headers: { authorization: proof },
            method: 'POST',
          }),
          {},
        )
      ).status,
    ).toBe(429)
    limited = false
    // A later retry needs a new proof, even when the first attempt was limited.
    expect(
      (
        await handler(
          new Request(url, {
            headers: { authorization: proof },
            method: 'POST',
          }),
          {},
        )
      ).status,
    ).toBe(409)
  })

  test('a failed provision is a 503, never a silent owner-session fallback', async () => {
    const secret = generateSecretKey()
    const { handler } = makeSelfProvisionHandler({
      provision: async () => undefined,
    })
    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'omega_nostr_self_provision_unavailable',
    })
  })

  test('self-provisioning alone can arm the endpoint with no owner key configured', async () => {
    const secret = generateSecretKey()
    const { handler } = makeSelfProvisionHandler({ ownerPubkey: undefined })

    expect((await post(handler, authorization(secret))).status).toBe(200)
  })

  test('no owner key and no self-provisioning still 503s before reading the proof', async () => {
    const secret = generateSecretKey()
    const { handler } = makeSelfProvisionHandler({
      enabled: false,
      ownerPubkey: undefined,
    })
    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'omega_nostr_auth_unavailable',
    })
  })
})

/**
 * Regression cover for the 2026-07-31 P0.
 *
 * `POST /api/omega/auth/session` answered a Cloud SQL Auth Proxy cold start —
 * a transient window of 10-70s after every new Cloud Run revision — with the
 * single code `omega_nostr_auth_storage_unavailable`, emitted from four
 * `catch {}` blocks that discarded the error. The owner saw a red banner and
 * the logs carried nothing but a pubkey digest.
 */
describe('Omega Nostr session storage failures are named, not collapsed', () => {
  const connectTimeout = (): Error =>
    Object.assign(
      new Error(
        'write CONNECT_TIMEOUT /cloudsql/openagentsgemini:us-central1:khala-sync-pg/.s.PGSQL.5432',
      ),
      { code: 'CONNECT_TIMEOUT' },
    )

  const postgresRefusal = (): Error => {
    const error = new Error('relation "oa_infra_kv" does not exist')
    error.name = 'PostgresError'
    return Object.assign(error, { code: '42P01' })
  }

  const post = async (
    handler: ReturnType<typeof makeOmegaNostrSessionHandler>,
    auth: string,
  ): Promise<Response> =>
    handler(
      new Request(url, { headers: { authorization: auth }, method: 'POST' }),
      {},
    )

  test('names a cold-start connect timeout as unreachable and retryable', async () => {
    const secret = generateSecretKey()
    const audits: Array<Readonly<Record<string, unknown>>> = []
    const handler = makeOmegaNostrSessionHandler({
      audit: (event, fields) => audits.push({ event, ...fields }),
      authStore: () => makeMemoryAuthKvStore(),
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveLinked: async () => {
        throw connectTimeout()
      },
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })

    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(503)
    expect(response.headers.get('retry-after')).toBe('2')
    expect(await response.json()).toEqual({
      cause: 'connect_unavailable',
      error: 'omega_nostr_auth_storage_unreachable',
      retryable: true,
    })
    // The log must name the cause and the step, not just the identity.
    const failure = audits.find(entry => entry['event'] === 'storage_unavailable')
    expect(failure).toMatchObject({
      failureClass: 'connect_unavailable',
      failureCode: 'CONNECT_TIMEOUT',
      phase: 'resolve_linked',
    })
  })

  test('a Postgres refusal is a distinct, honestly non-retryable code', async () => {
    const secret = generateSecretKey()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => makeMemoryAuthKvStore(),
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveLinked: async () => {
        throw postgresRefusal()
      },
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })

    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(503)
    expect(response.headers.get('retry-after')).toBeNull()
    expect(await response.json()).toEqual({
      cause: 'server_error',
      error: 'omega_nostr_auth_storage_rejected',
      retryable: false,
    })
  })

  test('an unrecognized failure keeps the code shipped clients were written against', async () => {
    const secret = generateSecretKey()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => makeMemoryAuthKvStore(),
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveLinked: async () => {
        throw new Error('something nobody classified')
      },
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })

    expect(await (await post(handler, authorization(secret))).json()).toEqual({
      cause: 'unknown',
      error: 'omega_nostr_auth_storage_unavailable',
      retryable: true,
    })
  })

  test('rides out a proxy cold start instead of showing the owner a 503', async () => {
    const secret = generateSecretKey()
    const owner = { userId: 'github:owner' }
    let attempts = 0
    const audits: Array<string> = []
    const handler = makeOmegaNostrSessionHandler({
      audit: event => audits.push(event),
      authStore: () => makeMemoryAuthKvStore(),
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveLinked: async () => {
        attempts += 1
        if (attempts < 3) throw connectTimeout()
        return undefined
      },
      resolveOwner: async () => owner,
    })

    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(200)
    expect(attempts).toBe(3)
    expect(audits.filter(event => event === 'storage_retry')).toHaveLength(2)
  })

  test('a failing session mint names the mint step, not the identity lookup', async () => {
    const secret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const audits: Array<Readonly<Record<string, unknown>>> = []
    const handler = makeOmegaNostrSessionHandler({
      audit: (event, fields) => audits.push({ event, ...fields }),
      authStore: () => ({
        ...store,
        put: async () => {
          throw connectTimeout()
        },
      }),
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })

    const response = await post(handler, authorization(secret))

    expect(response.status).toBe(503)
    expect(
      audits.find(entry => entry['event'] === 'storage_unavailable'),
    ).toMatchObject({ phase: 'mint_session' })
  })
})
