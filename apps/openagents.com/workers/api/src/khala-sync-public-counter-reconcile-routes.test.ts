// KS-6.3 (#8304): route tests for the admin tokens-served reconcile/repair
// surface. The reconcile core is covered in
// khala-sync-public-tokens-served.test.ts; these tests drive the HTTP
// contract: admin gate, GET = read-only reconcile, POST requires an
// explicit `repair: true`, and honest typed failures.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleKhalaSyncTokensServedReconcile,
  KHALA_SYNC_TOKENS_SERVED_RECONCILE_PATH,
} from './khala-sync-public-counter-reconcile-routes'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import type { TokensServedReconcileDeps } from './khala-sync-public-tokens-served'

const url = `https://openagents.com${KHALA_SYNC_TOKENS_SERVED_RECONCILE_PATH}`

// A minimal scripted Postgres fake: answers the projection read and the
// repair transaction statements the reconcile helper issues.
const makeDeps = (
  input: Readonly<{
    exactTotal: number
    projectedTotal: number | null
  }>,
): { deps: TokensServedReconcileDeps; state: { total: number | null; repairs: number } } => {
  const state = { repairs: 0, total: input.projectedTotal }
  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('SELECT total, last_event_at')) {
      return state.total === null
        ? []
        : [{ last_event_at: null, total: state.total }]
    }
    if (text.includes('SELECT total FROM khala_sync_public_counters')) {
      return state.total === null ? [] : [{ total: state.total }]
    }
    if (text.includes('INSERT INTO khala_sync_public_counters')) {
      state.total = Number(values[1])
      return [{ last_event_at: null, total: state.total }]
    }
    if (text.includes('INSERT INTO khala_sync_public_counter_repairs')) {
      state.repairs += 1
      return []
    }
    if (text.includes('INSERT INTO khala_sync_scopes')) {
      return [{ last_version: 1 }]
    }
    if (text.includes('INSERT INTO khala_sync_changelog')) {
      return [{ committed_at: '2026-07-04T12:00:00.000Z' }]
    }
    throw new Error(`unscripted: ${text.slice(0, 60)}`)
  }
  const sql = run as unknown as KhalaSyncPushSqlClient['sql'] & {
    begin: unknown
  }
  ;(sql as { begin: unknown }).begin = async (
    fn: (tx: unknown) => Promise<unknown>,
  ) => fn(run)
  return {
    deps: {
      binding: { connectionString: 'postgres://hyperdrive-fake' },
      makeSqlClient: async () => ({ end: async () => undefined, sql }),
      readExactTokensServed: async () => input.exactTotal,
    },
    state,
  }
}

const call = (
  request: Request,
  deps: TokensServedReconcileDeps,
  authorized = true,
): Promise<Response> =>
  Effect.runPromise(
    handleKhalaSyncTokensServedReconcile(request, {
      reconcileDeps: deps,
      requireOperator: async () => authorized,
    }),
  )

describe(`${KHALA_SYNC_TOKENS_SERVED_RECONCILE_PATH}`, () => {
  test('requires the admin bearer (401 otherwise)', async () => {
    const { deps } = makeDeps({ exactTotal: 10, projectedTotal: 10 })
    const response = await call(new Request(url), deps, false)
    expect(response.status).toBe(401)
  })

  test('GET reconciles read-only and reports drift', async () => {
    const { deps, state } = makeDeps({ exactTotal: 1_000, projectedTotal: 900 })
    const response = await call(new Request(url), deps)
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ok: boolean
      report: Record<string, unknown>
    }
    expect(body.ok).toBe(true)
    expect(body.report).toMatchObject({
      counterId: 'tokens-served',
      driftTokens: 100,
      exactTokensServed: 1_000,
      inSync: false,
      projectedTokensServed: 900,
      repaired: false,
    })
    // Read-only: no repair row, no projection write.
    expect(state.repairs).toBe(0)
    expect(state.total).toBe(900)
  })

  test('POST without repair:true is a 400 — drift is never overwritten implicitly', async () => {
    const { deps, state } = makeDeps({ exactTotal: 1_000, projectedTotal: 900 })
    const response = await call(
      new Request(url, {
        body: JSON.stringify({}),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      deps,
    )
    expect(response.status).toBe(400)
    expect(state.repairs).toBe(0)
  })

  test('POST { repair: true } realigns projection = exact SUM with an audit', async () => {
    const { deps, state } = makeDeps({ exactTotal: 1_000, projectedTotal: 900 })
    const response = await call(
      new Request(url, {
        body: JSON.stringify({
          auditNote: 'operator-approved repair (route test)',
          repair: true,
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      deps,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { report: Record<string, unknown> }
    expect(body.report).toMatchObject({
      previousTotal: 900,
      projectedTokensServed: 1_000,
      repairSource: 'reconcile_repair',
      repaired: true,
    })
    expect(state.repairs).toBe(1)
    expect(state.total).toBe(1_000)
  })

  test('POST { repair: true } against an uninitialized counter is the backfill', async () => {
    const { deps, state } = makeDeps({
      exactTotal: 555_000,
      projectedTotal: null,
    })
    const response = await call(
      new Request(url, {
        body: JSON.stringify({ repair: true }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      deps,
    )
    const body = (await response.json()) as { report: Record<string, unknown> }
    expect(body.report).toMatchObject({
      previousTotal: null,
      projectedTokensServed: 555_000,
      repairSource: 'backfill',
      repaired: true,
    })
    expect(state.total).toBe(555_000)
  })

  test('honest 503 when the KHALA_SYNC_DB binding is absent', async () => {
    const response = await call(new Request(url), {
      binding: undefined,
      readExactTokensServed: async () => 0,
    })
    expect(response.status).toBe(503)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).toBe('no_binding')
  })

  test('rejects non-GET/POST methods', async () => {
    const { deps } = makeDeps({ exactTotal: 1, projectedTotal: 1 })
    const response = await call(new Request(url, { method: 'DELETE' }), deps)
    expect(response.status).toBe(405)
  })
})
