import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  ADMIN_OPERATOR_OVERVIEW_PATH,
  makeAdminOperatorOverviewHandler,
  redactDeep,
  REDACTED_PLACEHOLDER,
} from './admin-operator-routes'
import { noopExecutionContextTracing } from './execution-context-tracing'

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
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
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
  provider TEXT,
  model TEXT,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  usage_truth TEXT NOT NULL,
  demand_kind TEXT NOT NULL DEFAULT 'unlabeled',
  demand_source TEXT
);
CREATE TABLE pylon_api_registrations (
  pylon_ref TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  status TEXT NOT NULL,
  resource_mode TEXT NOT NULL,
  wallet_ready INTEGER NOT NULL DEFAULT 0,
  latest_heartbeat_at TEXT,
  public_projection_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE pylon_api_assignments (
  assignment_ref TEXT PRIMARY KEY,
  pylon_ref TEXT NOT NULL,
  owner_agent_user_id TEXT NOT NULL,
  job_kind TEXT NOT NULL,
  state TEXT NOT NULL,
  lease_expires_at TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE pylon_api_events (
  event_ref TEXT PRIMARY KEY,
  pylon_ref TEXT NOT NULL,
  assignment_ref TEXT,
  event_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE agent_traces (
  trace_uuid TEXT PRIMARY KEY,
  owner_user_id TEXT NOT NULL,
  agent_ref TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  visibility TEXT NOT NULL,
  step_count INTEGER NOT NULL DEFAULT 0,
  demand_kind TEXT,
  demand_source TEXT,
  created_at TEXT NOT NULL
);
`

const seed = (raw: DatabaseSync): void => {
  raw.exec(SCHEMA)
  raw
    .prepare(
      `INSERT INTO token_usage_events
        (id, idempotency_key, observed_at, provider, model, total_tokens,
         usage_truth, demand_kind, demand_source)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      'tue-1',
      'idem-1',
      '2026-07-22T10:00:00.000Z',
      'pylon-codex-own-capacity',
      'openagents/pylon-codex',
      1200,
      'exact',
      'own_capacity',
      'khala_coding_delegation',
    )
  raw
    .prepare(
      `INSERT INTO token_usage_events
        (id, idempotency_key, observed_at, provider, model, total_tokens,
         usage_truth, demand_kind, demand_source)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      'tue-2',
      'idem-2',
      '2026-07-22T11:00:00.000Z',
      'openagents',
      'openagents/khala',
      800,
      'exact',
      'own_capacity',
      'khala_mobile_org_cloud_runtime',
    )

  raw
    .prepare(
      `INSERT INTO pylon_api_registrations
        (pylon_ref, display_name, status, resource_mode, wallet_ready,
         latest_heartbeat_at, public_projection_json, updated_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .run(
      'pylon:alpha',
      'Alpha node',
      'online',
      'own_capacity',
      1,
      '2026-07-22T11:30:00.000Z',
      JSON.stringify({ pylonRef: 'pylon:alpha', capacity: { codex: 2 } }),
      '2026-07-22T11:30:00.000Z',
    )

  raw
    .prepare(
      `INSERT INTO pylon_api_assignments
        (assignment_ref, pylon_ref, owner_agent_user_id, job_kind, state,
         lease_expires_at, public_projection_json, created_at, updated_at,
         archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .run(
      'assign:1',
      'pylon:alpha',
      'agent:owner',
      'codex_agent_task',
      'running',
      '2026-07-22T12:00:00.000Z',
      // A secret-shaped value planted here to prove the redaction pass masks
      // it even though ingest is supposed to keep this column public-safe.
      JSON.stringify({
        assignmentRef: 'assign:1',
        objective: 'Implement issue #9188',
        leakedToken: 'oa_agent_abcdef0123456789deadbeef',
      }),
      '2026-07-22T11:00:00.000Z',
      '2026-07-22T11:45:00.000Z',
    )

  raw
    .prepare(
      `INSERT INTO pylon_api_events
        (event_ref, pylon_ref, assignment_ref, event_kind, status,
         public_projection_json, created_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .run(
      'event:1',
      'pylon:alpha',
      'assign:1',
      'assignment_progress',
      'ok',
      JSON.stringify({ phase: 'proof-ready' }),
      '2026-07-22T11:40:00.000Z',
    )

  raw
    .prepare(
      `INSERT INTO agent_traces
        (trace_uuid, owner_user_id, agent_ref, schema_version, visibility,
         step_count, demand_kind, demand_source, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      'trace-uuid-1',
      'agent:owner',
      'agent:owner',
      'ATIF-v1.7',
      'owner_only',
      12,
      'own_capacity',
      'khala_coding_delegation',
      '2026-07-22T11:42:00.000Z',
    )
}

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  seed(raw)
  return new SqliteD1(raw) as unknown as D1Database
}

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  tracing: noopExecutionContextTracing,
  props: undefined,
  waitUntil: () => undefined,
})

type TestSession = Readonly<{ user: Readonly<{ email: string }> }>
type Env = Readonly<{ OPENAGENTS_DB: D1Database }>

const makeHandler = (
  session: TestSession | undefined,
  db: D1Database,
  isAdmin: (email: string) => boolean,
) =>
  makeAdminOperatorOverviewHandler<TestSession, Env>({
    appendRefreshedSessionCookies: response => response,
    db: () => db,
    isOpenAgentsAdminEmail: isAdmin,
    nowIso: () => '2026-07-22T12:00:00.000Z',
    requireBrowserSession: () => Promise.resolve(session),
  })

const request = () =>
  new Request(`https://openagents.com${ADMIN_OPERATOR_OVERVIEW_PATH}`, {
    method: 'GET',
  })

describe('admin operator overview (#9188)', () => {
  test('an admin sees the full redacted snapshot', async () => {
    const db = makeDb()
    const handler = makeHandler(
      { user: { email: 'chris@openagents.com' } },
      db,
      email => email === 'chris@openagents.com',
    )

    const response = await handler.handleAdminOperatorOverview(
      request(),
      { OPENAGENTS_DB: db },
      executionContext(),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, any>

    expect(body.ok).toBe(true)
    // Agent chains: the running assignment with its event timeline.
    expect(body.agentChains.recentCount).toBe(1)
    expect(body.agentChains.activeCount).toBe(1)
    expect(body.agentChains.chains[0].assignmentRef).toBe('assign:1')
    expect(body.agentChains.chains[0].state).toBe('running')
    expect(body.agentChains.chains[0].events[0].eventKind).toBe(
      'assignment_progress',
    )

    // Token rollup: totals + last-24h + breakdowns.
    expect(body.tokens.total.events).toBe(2)
    expect(body.tokens.total.tokens).toBe(2000)
    expect(body.tokens.byProvider.length).toBeGreaterThan(0)
    expect(body.tokens.recent.length).toBe(2)

    // Traces + fleet.
    expect(body.traces[0].traceUuid).toBe('trace-uuid-1')
    expect(body.fleet.totalCount).toBe(1)
    expect(body.fleet.onlineCount).toBe(1)
    expect(body.fleet.pylons[0].pylonRef).toBe('pylon:alpha')

    // Cloud health reads the real last org-cloud turn.
    expect(body.cloudHealth.lastOrgCloudTurnAt.status).toBe('ok')

    // Honest client-composed markers.
    expect(body.fullAuto.status).toBe('unavailable')
    expect(body.fleetRuns.liveEndpoint).toBe('/api/fleet-runs')
  })

  test('the snapshot redacts secret-shaped material', async () => {
    const db = makeDb()
    const handler = makeHandler(
      { user: { email: 'chris@openagents.com' } },
      db,
      () => true,
    )
    const response = await handler.handleAdminOperatorOverview(
      request(),
      { OPENAGENTS_DB: db },
      executionContext(),
    )
    const text = await response.text()
    // The planted `oa_agent_...` token must never survive to the wire.
    expect(text).not.toContain('oa_agent_abcdef0123456789deadbeef')
    const body = JSON.parse(text) as Record<string, any>
    expect(body.agentChains.chains[0].projection.leakedToken).toBe(
      REDACTED_PLACEHOLDER,
    )
    // The public-safe fields survive.
    expect(body.agentChains.chains[0].projection.objective).toBe(
      'Implement issue #9188',
    )
  })

  test('a non-admin is refused with 403', async () => {
    const db = makeDb()
    const handler = makeHandler(
      { user: { email: 'stranger@example.com' } },
      db,
      email => email === 'chris@openagents.com',
    )
    const response = await handler.handleAdminOperatorOverview(
      request(),
      { OPENAGENTS_DB: db },
      executionContext(),
    )
    expect(response.status).toBe(403)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.error).toBe('forbidden')
  })

  test('an unauthenticated request is refused with 401', async () => {
    const db = makeDb()
    const handler = makeHandler(undefined, db, () => true)
    const response = await handler.handleAdminOperatorOverview(
      request(),
      { OPENAGENTS_DB: db },
      executionContext(),
    )
    expect(response.status).toBe(401)
  })

  test('a non-GET method is rejected', async () => {
    const db = makeDb()
    const handler = makeHandler(
      { user: { email: 'chris@openagents.com' } },
      db,
      () => true,
    )
    const response = await handler.handleAdminOperatorOverview(
      new Request(`https://openagents.com${ADMIN_OPERATOR_OVERVIEW_PATH}`, {
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )
    expect(response.status).toBe(405)
  })

  test('redactDeep masks secret-shaped keys and values', () => {
    const out = redactDeep({
      access_token: 'whatever',
      email: 'someone@example.com',
      note: 'plain text is fine',
      nested: { mnemonic: 'word word word' },
    }) as Record<string, any>
    expect(out.access_token).toBe(REDACTED_PLACEHOLDER)
    expect(out.email).toBe(REDACTED_PLACEHOLDER)
    expect(out.note).toBe('plain text is fine')
    expect(out.nested.mnemonic).toBe(REDACTED_PLACEHOLDER)
  })
})
