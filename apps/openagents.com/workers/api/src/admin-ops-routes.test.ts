import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  ADMIN_OPS_HEALTH_PATH,
  ADMIN_OPS_RUNS_PATH,
  type AdminCaller,
  makeAdminOpsRoutes,
} from './admin-ops-routes'

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

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db.prepare(this.sql).all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const SCHEMA = `
CREATE TABLE token_usage_events (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  observed_at TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  producer_system TEXT NOT NULL,
  source_route TEXT NOT NULL,
  actor_user_id TEXT,
  run_ref TEXT,
  task_ref TEXT,
  provider TEXT,
  model TEXT,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  usage_truth TEXT NOT NULL,
  cost_amount REAL,
  currency TEXT,
  demand_kind TEXT NOT NULL DEFAULT 'unlabeled',
  demand_source TEXT
);
CREATE TABLE push_device_tokens (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  expo_push_token TEXT NOT NULL,
  platform TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, device_id)
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>

const makeRoutes = (
  db: D1Database,
  adminUserId: string | undefined,
  fetchImpl?: (input: string, init?: RequestInit) => Promise<Response>,
) =>
  makeAdminOpsRoutes<Env>({
    db: env => env.OPENAGENTS_DB,
    ...(fetchImpl === undefined ? {} : { fetch: fetchImpl }),
    nowIso: () => '2026-07-06T12:00:00.000Z',
    requireAdminCaller: async (): Promise<AdminCaller | undefined> =>
      adminUserId === undefined ? undefined : { userId: adminUserId },
  })

const fakeCtx = {} as ExecutionContext

describe('Aiur ops routes — auth matrix (fail closed)', () => {
  test('runs and health both 401 without an admin caller', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, undefined)

    const runs = await routes.handleAdminOpsRunsApi(
      new Request(`https://openagents.com${ADMIN_OPS_RUNS_PATH}`),
      env,
      fakeCtx,
    )
    expect(runs.status).toBe(401)

    const health = await routes.handleAdminOpsHealthApi(
      new Request(`https://openagents.com${ADMIN_OPS_HEALTH_PATH}`),
      env,
      fakeCtx,
    )
    expect(health.status).toBe(401)
  })
})

describe('Aiur ops routes — runs (exact usage receipts)', () => {
  test('reads recent org-cloud turns filtered to the exact demand-source tag', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    await db
      .prepare(
        `INSERT INTO token_usage_events
           (id, idempotency_key, observed_at, ingested_at, producer_system,
            source_route, actor_user_id, run_ref, task_ref, provider, model,
            total_tokens, usage_truth, cost_amount, currency, demand_kind, demand_source)
         VALUES (?, ?, ?, ?, 'omega', 'omega_hosted_gemini', ?, ?, ?, ?, ?, ?, 'exact', 0.01, 'usd', 'external', ?)`,
      )
      .bind(
        'evt_1',
        'idem_1',
        '2026-07-06T11:00:00.000Z',
        '2026-07-06T11:00:00.000Z',
        'user_1',
        'thread_1',
        'turn_1',
        'gemini',
        'gemini-2.5-pro',
        100,
        'khala_mobile_org_cloud_runtime',
      )
      .run()
    // A DIFFERENT demand_source (e.g. an unrelated inference charge) must
    // never leak into the ops runs view.
    await db
      .prepare(
        `INSERT INTO token_usage_events
           (id, idempotency_key, observed_at, ingested_at, producer_system,
            source_route, actor_user_id, run_ref, task_ref, provider, model,
            total_tokens, usage_truth, demand_kind, demand_source)
         VALUES (?, ?, ?, ?, 'khala', 'chat', ?, ?, ?, ?, ?, ?, 'exact', 'external', 'khala_chat')`,
      )
      .bind(
        'evt_2',
        'idem_2',
        '2026-07-06T11:05:00.000Z',
        '2026-07-06T11:05:00.000Z',
        'user_2',
        'thread_2',
        'turn_2',
        'openrouter',
        'some-model',
        50,
      )
      .run()

    const response = await routes.handleAdminOpsRunsApi(
      new Request(`https://openagents.com${ADMIN_OPS_RUNS_PATH}`),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      runs: ReadonlyArray<{ userId: string; threadId: string; totalTokens: number }>
      liveViaKhalaSync: boolean
    }
    expect(body.runs).toHaveLength(1)
    expect(body.runs[0]?.userId).toBe('user_1')
    expect(body.runs[0]?.threadId).toBe('thread_1')
    expect(body.runs[0]?.totalTokens).toBe(100)
    // Honest: this is not (yet) a live Khala Sync feed.
    expect(body.liveViaKhalaSync).toBe(false)
  })

  test('returns an empty list (not fabricated data) when there are no org-cloud turns yet', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleAdminOpsRunsApi(
      new Request(`https://openagents.com${ADMIN_OPS_RUNS_PATH}`),
      env,
      fakeCtx,
    )
    const body = (await response.json()) as { runs: ReadonlyArray<unknown> }
    expect(body.runs).toEqual([])
  })
})

describe('Aiur ops routes — health strip (honest states)', () => {
  test('reports not_measured for lastOrgCloudTurnCompletedAt when no turns exist', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner', async () => new Response('{}', { status: 200 }))

    const response = await routes.handleAdminOpsHealthApi(
      new Request(`https://openagents.com${ADMIN_OPS_HEALTH_PATH}`),
      env,
      fakeCtx,
    )
    const body = (await response.json()) as {
      checks: {
        lastOrgCloudTurnCompletedAt: { status: string }
        pushDeviceTokensRegistered: { status: string; value: string }
        khalaPublicStatsReachable: { status: string }
      }
    }
    expect(body.checks.lastOrgCloudTurnCompletedAt.status).toBe('not_measured')
    expect(body.checks.pushDeviceTokensRegistered.status).toBe('ok')
    expect(body.checks.pushDeviceTokensRegistered.value).toBe('0')
    expect(body.checks.khalaPublicStatsReachable.status).toBe('ok')
  })

  test('reports a real timestamp for lastOrgCloudTurnCompletedAt once a turn exists', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    await db
      .prepare(
        `INSERT INTO token_usage_events
           (id, idempotency_key, observed_at, ingested_at, producer_system,
            source_route, total_tokens, usage_truth, demand_kind, demand_source)
         VALUES ('evt_1', 'idem_1', '2026-07-06T11:00:00.000Z', '2026-07-06T11:00:00.000Z',
                 'omega', 'omega_hosted_gemini', 100, 'exact', 'external',
                 'khala_mobile_org_cloud_runtime')`,
      )
      .run()
    const routes = makeRoutes(db, 'user_owner', async () => new Response('{}', { status: 200 }))

    const response = await routes.handleAdminOpsHealthApi(
      new Request(`https://openagents.com${ADMIN_OPS_HEALTH_PATH}`),
      env,
      fakeCtx,
    )
    const body = (await response.json()) as {
      checks: { lastOrgCloudTurnCompletedAt: { status: string; value?: string } }
    }
    expect(body.checks.lastOrgCloudTurnCompletedAt.status).toBe('ok')
    expect(body.checks.lastOrgCloudTurnCompletedAt.value).toBe('2026-07-06T11:00:00.000Z')
  })

  test('reports an error state (not a crash) when the Khala public stats reachability check fails', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner', async () => new Response('', { status: 500 }))

    const response = await routes.handleAdminOpsHealthApi(
      new Request(`https://openagents.com${ADMIN_OPS_HEALTH_PATH}`),
      env,
      fakeCtx,
    )
    const body = (await response.json()) as {
      checks: { khalaPublicStatsReachable: { status: string } }
    }
    expect(body.checks.khalaPublicStatsReachable.status).toBe('error')
  })

  test('a thrown network error is caught and reported as an error state, never an uncaught rejection', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner', async () => {
      throw new Error('network down')
    })

    const response = await routes.handleAdminOpsHealthApi(
      new Request(`https://openagents.com${ADMIN_OPS_HEALTH_PATH}`),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      checks: { khalaPublicStatsReachable: { status: string; messageSafe?: string } }
    }
    expect(body.checks.khalaPublicStatsReachable.status).toBe('error')
    expect(body.checks.khalaPublicStatsReachable.messageSafe).toContain('network down')
  })
})
