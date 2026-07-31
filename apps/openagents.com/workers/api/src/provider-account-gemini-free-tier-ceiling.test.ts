// Omega self-provisioning (2026-07-28): the free-tier daily token ceiling is
// ENFORCED for self-provisioned
// (`nostr:`) identities on the hosted-Gemini proxy.
//
// Before this change `dailyTokenCeiling` was advertised by the builtin grant
// and read by nothing: any bearer that satisfied `requireHostedComputeActor`
// could call this proxy in a loop and spend the owner's key without limit.
// These tests pin the new bound AND pin that it does not touch the identity
// classes that already had working flows.

import { describe, expect, test, vi } from 'vitest'

import type { AuthKvStore } from './auth/auth-kv'
import { materializeHttpResult } from './http/responses'
import { makeProviderAccountServiceHandlers } from './provider-account-service-routes'

const model = 'gemini-3.5-flash'
const requestUrl = `https://openagents.com/api/provider-accounts/google-gemini/models/${model}:streamGenerateContent?alt=sse`

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 0,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

/**
 * A D1 double that answers ONLY the daily-token SUM and records every query it
 * was asked, so a test can assert the ledger read did (or did not) happen.
 */
const makeLedgerDb = (
  tokensServedToday: number | string,
  options: Readonly<{ admittedIdentity?: boolean }> = {},
): Readonly<{ db: D1Database; queries: Array<string> }> => {
  const queries: Array<string> = []
  const prepare = (query: string): D1PreparedStatement => {
    queries.push(query)

    const statement = {
      all: () => Promise.resolve({ meta: d1Meta(), results: [], success: true }),
      bind: () => statement,
      first: () =>
        Promise.resolve(
          query.includes('SUM(total_tokens)')
            ? { total: tokensServedToday }
            : query.includes('sarah_voice_alpha_memberships') &&
                options.admittedIdentity === true
              ? { admitted: 1 }
              : null,
        ),
      raw: () => Promise.resolve([]),
      run: () =>
        Promise.resolve({ meta: d1Meta(), results: [], success: true }),
    } as unknown as D1PreparedStatement

    return statement
  }

  return {
    db: {
      batch: () => Promise.resolve([]),
      dump: () => Promise.resolve(new ArrayBuffer(0)),
      exec: () => Promise.resolve({ count: 0, duration: 0 }),
      prepare,
      withSession: () => ({ getBookmark: () => null, prepare }),
    } as unknown as D1Database,
    queries,
  }
}

const makeExecutionContext = (): Readonly<{
  ctx: ExecutionContext
  promises: Array<Promise<unknown>>
}> => {
  const promises: Array<Promise<unknown>> = []

  return {
    ctx: {
      passThroughOnException: () => undefined,
      waitUntil: (promise: Promise<unknown>) => {
        promises.push(Promise.resolve(promise))
      },
    } as unknown as ExecutionContext,
    promises,
  }
}

const handlersForActor = (actorUserId: string) => {
  const typed = makeProviderAccountServiceHandlers({
    readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
    requireHostedComputeActor: () =>
      Promise.resolve({ user: { id: actorUserId } }),
    requireProviderServiceActor: () => Promise.resolve(undefined),
  })

  return async (
    env: Record<string, unknown>,
    ctx: ExecutionContext,
  ): Promise<Response> =>
    materializeHttpResult(
      await typed.handleGoogleGeminiGenerateContentApi(
        new Request(requestUrl, {
          body: JSON.stringify({ contents: [] }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        }),
        // The gate takes an in-flight RESERVATION before reading the ceiling
        // (2026-07-31), so a store is part of the route's environment now.
        // Production always has one, built over `KHALA_SYNC_DB`. Tests that
        // want the missing-store case set `AUTH_KV` explicitly.
        ({ AUTH_KV: makeMemoryAuthKv(), ...env }) as never,
        ctx,
        model,
      ),
    )
}

/**
 * In-memory stand-in for the Postgres-backed reservation store.
 *
 * Reservation behaviour and its overshoot bound are pinned in
 * inference/hosted-compute-token-reservation.test.ts; here it only needs to
 * exist so these ceiling tests exercise the ceiling.
 */
function makeMemoryAuthKv(): AuthKvStore {
  const entries = new Map<string, string>()

  return {
    delete: async key => {
      entries.delete(key)
    },
    get: (async (key: string) => entries.get(key) ?? null) as AuthKvStore['get'],
    listPrefix: async prefix =>
      [...entries.entries()]
        .filter(([key]) => key.startsWith(prefix))
        .map(([key, value]) => ({ key, value })),
    put: async (key, value) => {
      entries.set(key, value)
    },
    putIfAbsent: async (key, value) => {
      if (entries.has(key)) {
        return false
      }
      entries.set(key, value)

      return true
    },
  }
}

const upstreamOk = () =>
  vi
    .spyOn(globalThis, 'fetch')
    .mockResolvedValue(
      new Response('data: [DONE]\n\n', {
        headers: { 'content-type': 'text/event-stream' },
        status: 200,
      }),
    )

const selfProvisionedActor = `nostr:${'a'.repeat(64)}`

describe('hosted Gemini free-tier daily token ceiling', () => {
  test('refuses a self-provisioned install that has spent its daily allowance', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(1_000_000)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor(selfProvisionedActor)(
        { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: ledger.db },
        ctx,
      )

      expect(response.status).toBe(429)
      expect(await response.json()).toMatchObject({
        dailyTokenCeiling: 1_000_000,
        error: 'free_tier_daily_token_ceiling_reached',
        tokensServedToday: 1_000_000,
      })
      // The owner's key was never spent.
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  test('admits a self-provisioned install that is still under the ceiling', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(999_999)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor(selfProvisionedActor)(
        { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: ledger.db },
        ctx,
      )

      expect(response.status).toBe(200)
      expect(upstream).toHaveBeenCalledTimes(1)
      expect(
        ledger.queries.some(query => query.includes('SUM(total_tokens)')),
      ).toBe(true)
    } finally {
      upstream.mockRestore()
    }
  })

  test('honours an owner-configured lower ceiling from env', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(60_000)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor(selfProvisionedActor)(
        {
          GEMINI_API_KEY: 'test-gemini-key',
          OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING: '50000',
          OPENAGENTS_DB: ledger.db,
        },
        ctx,
      )

      expect(response.status).toBe(429)
      expect(await response.json()).toMatchObject({
        dailyTokenCeiling: 50_000,
      })
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  // 2026-07-31 P0. This block previously asserted the OPPOSITE — that a
  // github / email / agent identity was served with NO ledger read at all.
  // That was the vulnerability, not a feature: `POST /api/agents/register` is
  // unauthenticated public self-service, so a stranger could mint an
  // `oa_agent_` token and draw the owner's `GEMINI_API_KEY` without any
  // ceiling. Verified live before the fix: a freshly minted, never-
  // authenticated token returned HTTP 200 and ledgered a real owner-funded
  // draw. The identity classes are now bounded unless explicitly admitted.
  test('bounds a non-admitted agent identity that has spent its allowance', async () => {
    for (const actorUserId of [
      'github:1',
      'email:someone@example.com',
      'agent:test',
      'user_abcdef',
    ]) {
      const upstream = upstreamOk()

      try {
        const ledger = makeLedgerDb(1_000_000)
        const { ctx } = makeExecutionContext()
        const response = await handlersForActor(actorUserId)(
          { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: ledger.db },
          ctx,
        )

        expect(response.status).toBe(429)
        expect(await response.json()).toMatchObject({
          dailyTokenCeiling: 1_000_000,
          error: 'free_tier_daily_token_ceiling_reached',
          tokensServedToday: 1_000_000,
        })
        // The owner's key was never spent.
        expect(upstream).not.toHaveBeenCalled()
      } finally {
        upstream.mockRestore()
      }
    }
  })

  test('serves a non-admitted agent identity that is still under the ceiling', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(10)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor('user_abcdef')(
        { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: ledger.db },
        ctx,
      )

      expect(response.status).toBe(200)
      expect(upstream).toHaveBeenCalledTimes(1)
      expect(
        ledger.queries.some(query => query.includes('SUM(total_tokens)')),
      ).toBe(true)
    } finally {
      upstream.mockRestore()
    }
  })

  // The admitted cohort is how the owner's own Pylon/Khala tooling keeps its
  // unbounded draw. It reuses the EXISTING `INFERENCE_INTERNAL_ACCOUNT_REFS`
  // allowlist rather than a second admission mechanism, and it short-circuits
  // BEFORE the ledger read so no owner workflow gains latency.
  test('exempts an admitted actor without a ledger read', async () => {
    for (const [actorUserId, admittedRef] of [
      ['user_abcdef', 'agent:user_abcdef'],
      ['user_abcdef', 'user_abcdef'],
      ['github:1', 'openauth:github:1'],
    ] as const) {
      const upstream = upstreamOk()

      try {
        // A ledger that would REFUSE if it were consulted.
        const ledger = makeLedgerDb(999_999_999)
        const { ctx } = makeExecutionContext()
        const response = await handlersForActor(actorUserId)(
          {
            GEMINI_API_KEY: 'test-gemini-key',
            INFERENCE_INTERNAL_ACCOUNT_REFS: `other:ref, ${admittedRef}`,
            OPENAGENTS_DB: ledger.db,
          },
          ctx,
        )

        expect(response.status).toBe(200)
        expect(upstream).toHaveBeenCalledTimes(1)
        expect(
          ledger.queries.some(query => query.includes('SUM(total_tokens)')),
        ).toBe(false)
      } finally {
        upstream.mockRestore()
      }
    }
  })

  test('honours an owner-configured ceiling for non-admitted actors', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(60_000)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor('user_abcdef')(
        {
          GEMINI_API_KEY: 'test-gemini-key',
          HOSTED_COMPUTE_DAILY_TOKEN_CEILING: '50000',
          OPENAGENTS_DB: ledger.db,
        },
        ctx,
      )

      expect(response.status).toBe(429)
      expect(await response.json()).toMatchObject({ dailyTokenCeiling: 50_000 })
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  // PRODUCTION CELL SHAPE. `total_tokens` is bigint, so Postgres types
  // `SUM(...)` as numeric and the D1-shaped Cloud SQL adapter (int8 parser
  // only) returns it as a STRING. The previous `typeof === 'number'` guard was
  // therefore always false on Cloud Run and the read always returned 0, which
  // silently made this ceiling — and the chat-completions one — no-ops in
  // production. Verified live 2026-07-31: a refusal reported
  // `tokensServedToday: 0` against a ledger holding 215. Every other test here
  // uses a JS number, which is exactly why none of them caught it.
  test('binds when the ledger cell is numeric-as-string, as Postgres returns it', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb('1000000')
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor('user_abcdef')(
        { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: ledger.db },
        ctx,
      )

      expect(response.status).toBe(429)
      expect(await response.json()).toMatchObject({
        tokensServedToday: 1_000_000,
      })
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  // A PRESENT but unparseable cell is a broken read, not "no usage": it must
  // refuse via the fail-closed path rather than serve on a bad number.
  test('refuses when the ledger cell is present but unparseable', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb('not-a-number')
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor('user_abcdef')(
        { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: ledger.db },
        ctx,
      )

      expect(response.status).toBe(429)
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  // FAIL-CLOSED. A spend path must not fall open during a database outage.
  test('refuses a non-admitted actor when the ledger read throws', async () => {
    const upstream = upstreamOk()

    try {
      const { ctx } = makeExecutionContext()
      const throwingDb = {
        prepare: () => {
          throw new Error('ledger unavailable')
        },
      } as unknown as D1Database
      const response = await handlersForActor('user_abcdef')(
        { GEMINI_API_KEY: 'test-gemini-key', OPENAGENTS_DB: throwingDb },
        ctx,
      )

      expect(response.status).toBe(429)
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  // THE RESERVATION (2026-07-31). The ceiling is no longer a bare read: a
  // marker is taken BEFORE the ledger is read, so a concurrent burst is
  // visible instead of every request racing one stale total. The bound this
  // yields, and its residual, live in
  // inference/hosted-compute-token-reservation.test.ts.
  test('refuses a non-admitted actor when the reservation store is unavailable', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(0)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor('user_abcdef')(
        {
          // No `AUTH_KV` and no `KHALA_SYNC_DB` => the fail-closed store.
          AUTH_KV: undefined,
          GEMINI_API_KEY: 'test-gemini-key',
          OPENAGENTS_DB: ledger.db,
        },
        ctx,
      )

      expect(response.status).toBe(429)
      // The owner's key was never spent on an unbounded request.
      expect(upstream).not.toHaveBeenCalled()
    } finally {
      upstream.mockRestore()
    }
  })

  // BOTH admission paths must skip the reservation, not just the env
  // allowlist. An operator-admitted cohort member is exempt from the ceiling,
  // so it must not become refusable by a reservation store it was never
  // subject to — that would turn a KV blip into an outage for the owner's own
  // admitted identity, which is exactly the failure the cohort check exists to
  // prevent.
  test('an operator-admitted cohort member takes no reservation', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(1_000_000, { admittedIdentity: true })
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor(selfProvisionedActor)(
        {
          // No reservation store at all: an exempt actor must still be served.
          AUTH_KV: undefined,
          GEMINI_API_KEY: 'test-gemini-key',
          OPENAGENTS_DB: ledger.db,
        },
        ctx,
      )

      expect(response.status).toBe(200)
      expect(upstream).toHaveBeenCalledTimes(1)
      expect(
        ledger.queries.some(query => query.includes('SUM(total_tokens)')),
      ).toBe(false)
    } finally {
      upstream.mockRestore()
    }
  })

  // An ADMITTED actor keeps its zero-cost path: no ledger read AND no
  // reservation round-trip, so no owner workflow gains latency or a new
  // dependency.
  test('an admitted actor takes no reservation and reads no ledger', async () => {
    const upstream = upstreamOk()

    try {
      const ledger = makeLedgerDb(1_000_000)
      const { ctx } = makeExecutionContext()
      const response = await handlersForActor('user_owner')(
        {
          AUTH_KV: undefined,
          GEMINI_API_KEY: 'test-gemini-key',
          INFERENCE_INTERNAL_ACCOUNT_REFS: 'agent:user_owner',
          OPENAGENTS_DB: ledger.db,
        },
        ctx,
      )

      expect(response.status).toBe(200)
      expect(upstream).toHaveBeenCalledTimes(1)
      expect(
        ledger.queries.some(query => query.includes('SUM(total_tokens)')),
      ).toBe(false)
    } finally {
      upstream.mockRestore()
    }
  })
})
