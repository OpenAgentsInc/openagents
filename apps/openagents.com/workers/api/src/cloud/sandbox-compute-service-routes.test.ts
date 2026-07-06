import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type SandboxAuth,
  type SandboxComputeServiceDeps,
  type SandboxMeteringHook,
  type SandboxRuntimeAdapter,
  MAX_SANDBOX_TTL_SECONDS,
  SandboxAdapterError,
  DEFAULT_SANDBOX_IMAGE,
  handleSandboxGet,
  handleSandboxRequest,
  isSandboxComputeServiceEnabled,
  makeD1SandboxRuntimeAdapter,
  makeLedgerSandboxMeteringHook,
  sandboxRentalReceiptRef,
  stubSandboxAdapter,
} from './sandbox-compute-service-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const authOk: SandboxAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: SandboxAuth = async () => undefined

const baseDeps = (
  overrides: Partial<SandboxComputeServiceDeps> = {},
): SandboxComputeServiceDeps => ({
  authenticate: authOk,
  enabled: true,
  newId: () => 'sbx_fixed',
  ...overrides,
})

const sandboxRequest = (body: unknown, init: RequestInit = {}): Request =>
  new Request('https://openagents.com/v1/sandboxes', {
    body: JSON.stringify(body),
    method: 'POST',
    ...init,
  })

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as Array<T>
    return { results }
  }

  async run<T = Row>(): Promise<{ success: true; results: Array<T> }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { results: [], success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

const SANDBOX_SCHEMA = `
CREATE TABLE cloud_sandbox_sessions (
  sandbox_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  image TEXT NOT NULL,
  ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds > 0),
  status TEXT NOT NULL CHECK (status IN ('provisioning', 'ready', 'stopped', 'expired', 'failed')),
  connection_ref TEXT,
  usage_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at_hint TEXT,
  completed_at TEXT
);
CREATE INDEX idx_cloud_sandbox_sessions_account
  ON cloud_sandbox_sessions (account_ref, created_at DESC);
`

const makeSandboxDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SANDBOX_SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

describe('sandbox compute service feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isSandboxComputeServiceEnabled(undefined)).toBe(false)
    expect(isSandboxComputeServiceEnabled('')).toBe(false)
    expect(isSandboxComputeServiceEnabled('false')).toBe(false)
    expect(isSandboxComputeServiceEnabled('0')).toBe(false)
    expect(isSandboxComputeServiceEnabled('true')).toBe(true)
    expect(isSandboxComputeServiceEnabled('1')).toBe(true)
    expect(isSandboxComputeServiceEnabled('on')).toBe(true)
    expect(isSandboxComputeServiceEnabled('yes')).toBe(true)
  })
})

describe('POST /v1/sandboxes', () => {
  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('sandbox_compute_service_disabled')
  })

  test('rejects an unauthenticated request with 401 + WWW-Authenticate', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ authenticate: authNone })),
    )
    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBe('Bearer')
  })

  test('rejects non-POST with 405', async () => {
    const response = await run(
      handleSandboxRequest(
        new Request('https://openagents.com/v1/sandboxes', { method: 'GET' }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(405)
  })

  test('rejects invalid JSON with 400', async () => {
    const response = await run(
      handleSandboxRequest(
        new Request('https://openagents.com/v1/sandboxes', {
          body: '{bad',
          method: 'POST',
        }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe('invalid_json')
  })

  test('accepts an empty body and applies image/ttl defaults', async () => {
    const response = await run(
      handleSandboxRequest(
        new Request('https://openagents.com/v1/sandboxes', { body: '', method: 'POST' }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('sandbox')
    expect(body.image).toBe(DEFAULT_SANDBOX_IMAGE)
    expect(body.status).toBe('provisioning')
    // Scaffold never returns a usable connection.
    expect(body.connection_ref).toBeNull()
  })

  test('rejects an over-ceiling TTL with 400 (abuse control) before provisioning', async () => {
    const response = await run(
      handleSandboxRequest(
        sandboxRequest({ ttlSeconds: MAX_SANDBOX_TTL_SECONDS + 1 }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string; maxTtlSeconds: number }
    expect(body.error).toBe('invalid_ttl')
    expect(body.maxTtlSeconds).toBe(MAX_SANDBOX_TTL_SECONDS)
  })

  test('rejects a non-positive TTL with 400', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({ ttlSeconds: 0 }), baseDeps()),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe('invalid_ttl')
  })

  test('stub metering reports metered:false / null receipt (honest, not live)', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({ image: 'custom' }), baseDeps()),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(false)
    expect(body.receipt_ref).toBeNull()
  })

  test('maps a runtime adapter failure to 502', async () => {
    const failing: SandboxRuntimeAdapter = {
      id: 'failing',
      provision: () =>
        Effect.fail(new SandboxAdapterError({ adapterId: 'failing', reason: 'no_capacity' })),
      get: () =>
        Effect.fail(new SandboxAdapterError({ adapterId: 'failing', reason: 'no_capacity' })),
    }
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ adapter: failing })),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('runtime_error')
    expect(body.reason).toBe('no_capacity')
  })

  test('a live metering hook can project a receipt ref', async () => {
    const liveHook: SandboxMeteringHook = context =>
      Effect.succeed({
        metered: true,
        receiptRef: sandboxRentalReceiptRef(context.sandboxId),
      })
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ meteringHook: liveHook })),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(true)
    // The advertised receipt ref is the SAME ref the ledger writes and the
    // public receipt route dereferences (cloudChargeReceiptRef shape).
    expect(body.receipt_ref).toBe(
      'receipt.cloud.sandbox_compute.rental.charge.sbx_fixed',
    )
  })

  test('the stub adapter never returns a usable connection', async () => {
    const sandbox = await run(
      Effect.orDie(
        stubSandboxAdapter.provision({
          sandboxId: 's1',
          accountRef: 'agent:x',
          request: { image: 'i', ttlSeconds: 60, options: {} },
        }),
      ),
    )
    expect(sandbox.status).toBe('provisioning')
    expect(sandbox.connectionRef).toBeNull()
  })
})

const sandboxGetRequest = (): Request =>
  new Request('https://openagents.com/v1/sandboxes/sbx_fixed', { method: 'GET' })

describe('GET /v1/sandboxes/:sandboxId', () => {
  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleSandboxGet(sandboxGetRequest(), 'sbx_fixed', baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('sandbox_compute_service_disabled')
  })

  test('rejects an unauthenticated read with 401', async () => {
    const response = await run(
      handleSandboxGet(sandboxGetRequest(), 'sbx_fixed', baseDeps({ authenticate: authNone })),
    )
    expect(response.status).toBe(401)
  })

  test('the stub adapter has no persistence, so a read is 404 not_found', async () => {
    const response = await run(
      handleSandboxGet(sandboxGetRequest(), 'sbx_fixed', baseDeps()),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('not_found')
  })

  test('projects a resolved sandbox for the owning account', async () => {
    const adapter: SandboxRuntimeAdapter = {
      id: 'persisted',
      provision: () => Effect.fail(new SandboxAdapterError({ adapterId: 'persisted', reason: 'n/a' })),
      get: ({ sandboxId, accountRef }) =>
        Effect.succeed(
          accountRef === 'agent:test-user'
            ? {
                sandboxId,
                accountRef,
                image: DEFAULT_SANDBOX_IMAGE,
                ttlSeconds: 900,
                status: 'ready' as const,
                connectionRef: 'session:scoped-ref',
                createdAt: '2026-06-19T00:00:00.000Z',
                expiresAtHint: null,
              }
            : undefined,
        ),
    }
    const response = await run(
      handleSandboxGet(sandboxGetRequest(), 'sbx_fixed', baseDeps({ adapter })),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('sandbox')
    expect(body.id).toBe('sbx_fixed')
    expect(body.status).toBe('ready')
  })

  test('enforces cross-account isolation (a sandbox is 404 for a different account)', async () => {
    const adapter: SandboxRuntimeAdapter = {
      id: 'persisted',
      provision: () => Effect.fail(new SandboxAdapterError({ adapterId: 'persisted', reason: 'n/a' })),
      get: ({ accountRef }) =>
        Effect.succeed(accountRef === 'agent:owner' ? ({} as never) : undefined),
    }
    const response = await run(
      handleSandboxGet(sandboxGetRequest(), 'sbx_fixed', baseDeps({ adapter })),
    )
    expect(response.status).toBe(404)
  })
})

describe('D1 sandbox runtime adapter', () => {
  test('runs fixture work in isolation, persists lifecycle, and reports usage', async () => {
    const db = makeSandboxDb()
    const adapter = makeD1SandboxRuntimeAdapter(db)
    const meteringCalls: Array<Record<string, unknown>> = []
    const meteringHook: SandboxMeteringHook = context => {
      meteringCalls.push(context)
      return Effect.succeed({
        metered: true,
        receiptRef: sandboxRentalReceiptRef(context.sandboxId),
      })
    }

    const submit = await run(
      handleSandboxRequest(
        sandboxRequest({ image: 'oa-sandbox-base', ttlSeconds: 60 }),
        baseDeps({ adapter, meteringHook }),
      ),
    )
    expect(submit.status).toBe(200)
    const submitted = (await submit.json()) as Record<string, unknown>
    expect(submitted.status).toBe('ready')
    expect(submitted.connection_ref).toBe('sandbox.session.sbx_fixed')
    expect(submitted.expires_at_hint).toEqual(expect.any(String))
    expect(submitted.usage).toMatchObject({ wallSeconds: 60 })
    expect(submitted.metered).toBe(true)
    expect(meteringCalls[0]).toMatchObject({
      sandboxId: 'sbx_fixed',
      usage: { wallSeconds: 60 },
    })

    const read = await run(
      handleSandboxGet(sandboxGetRequest(), 'sbx_fixed', baseDeps({ adapter })),
    )
    expect(read.status).toBe(200)
    const lifecycle = (await read.json()) as Record<string, unknown>
    expect(lifecycle.status).toBe('ready')
    expect(lifecycle.connection_ref).toBe('sandbox.session.sbx_fixed')
    expect(lifecycle.usage).toMatchObject({ wallSeconds: 60 })
  })

  test('persisted sandbox sessions remain account-isolated', async () => {
    const db = makeSandboxDb()
    const adapter = makeD1SandboxRuntimeAdapter(db)
    await run(
      handleSandboxRequest(
        sandboxRequest({ image: 'oa-sandbox-base', ttlSeconds: 60 }),
        baseDeps({ adapter }),
      ),
    )

    const readAsOtherAccount = await run(
      handleSandboxGet(
        sandboxGetRequest(),
        'sbx_fixed',
        baseDeps({
          adapter,
          authenticate: async () => ({ accountRef: 'agent:other-user' }),
        }),
      ),
    )
    expect(readAsOtherAccount.status).toBe(404)
  })
})

describe('makeLedgerSandboxMeteringHook', () => {
  test('reports metered:false at provision time (no metered usage yet)', async () => {
    const hook = makeLedgerSandboxMeteringHook({
      ledgerDb: {} as import('../payments-ledger-db').PaymentsLedgerDb,
      priceUsd: () => 1,
      usdToMsat: usd => Math.ceil(usd * 1000),
    })
    const outcome = await run(
      hook({ accountRef: 'agent:x', sandboxId: 's1', image: 'i' }),
    )
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBeNull()
  })

  test('a zero-usd charge is metered with a receipt ref and no debit', async () => {
    const hook = makeLedgerSandboxMeteringHook({
      ledgerDb: {} as import('../payments-ledger-db').PaymentsLedgerDb,
      priceUsd: () => 0,
      usdToMsat: usd => Math.ceil(usd * 1000),
    })
    const outcome = await run(
      hook({
        accountRef: 'agent:x',
        sandboxId: 's1',
        image: 'i',
        usage: { wallSeconds: 30 },
      }),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.receiptRef).toBe(sandboxRentalReceiptRef('s1'))
  })
})
