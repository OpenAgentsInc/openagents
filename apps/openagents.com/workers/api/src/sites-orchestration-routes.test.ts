import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type SiteBuilderRuntime,
  createSiteBuilderSession,
} from './sites-builder-sessions'
import { makeSitesOrchestrationRoutes } from './sites-orchestration-routes'

type Row = Record<string, unknown>

class OrchestrationStore {
  site_builder_artifacts: Array<Row> = []
  site_builder_events: Array<Row> = []
  site_builder_file_snapshots: Array<Row> = []
  site_builder_messages: Array<Row> = []
  site_builder_phase_runs: Array<Row> = []
  site_builder_previews: Array<Row> = []
  site_builder_repair_attempts: Array<Row> = []
  site_builder_sessions: Array<Row> = []
}

const tableNames = [
  'site_builder_sessions',
  'site_builder_messages',
  'site_builder_events',
  'site_builder_phase_runs',
  'site_builder_file_snapshots',
  'site_builder_previews',
  'site_builder_repair_attempts',
  'site_builder_artifacts',
] as const

type TableName = (typeof tableNames)[number]

const tableFromQuery = (query: string): TableName => {
  const table = tableNames.find(name => query.includes(name))

  if (table === undefined) {
    throw new Error(`Unknown table for query: ${query}`)
  }

  return table
}

const active = (row: Row): boolean => row.archived_at === null

const byIdempotency = (rows: ReadonlyArray<Row>, key: string): Row | null =>
  rows.find(row => row.idempotency_key === key && active(row)) ?? null

const byId = (rows: ReadonlyArray<Row>, id: string): Row | null =>
  rows.find(row => row.id === id && active(row)) ?? null

class OrchestrationStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OrchestrationStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    const table = tableFromQuery(this.query)
    const rows = this.store[table]

    if (this.query.includes('WHERE id = ?')) {
      return Promise.resolve(byId(rows, String(this.values[0])) as T | null)
    }

    if (this.query.includes('WHERE idempotency_key = ?')) {
      return Promise.resolve(
        byIdempotency(rows, String(this.values[0])) as T | null,
      )
    }

    if (this.query.includes('MAX(sequence)')) {
      const sessionId = String(this.values[0])
      const maxSequence = rows
        .filter(row => row.session_id === sessionId && active(row))
        .reduce((max, row) => Math.max(max, Number(row.sequence ?? 0)), 0)

      return Promise.resolve({ next_sequence: maxSequence + 1 } as T)
    }

    if (this.query.includes('MAX(attempt_number)')) {
      const sessionId = String(this.values[0])
      const maxAttempt = rows
        .filter(row => row.session_id === sessionId && active(row))
        .reduce(
          (max, row) => Math.max(max, Number(row.attempt_number ?? 0)),
          0,
        )

      return Promise.resolve({ next_attempt: maxAttempt + 1 } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableFromQuery(this.query)
    const idempotencyKey = String(this.values[1])

    if (byIdempotency(this.store[table], idempotencyKey) !== null) {
      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_sessions') {
      this.store.site_builder_sessions.push({
        active_artifact_id: null,
        active_preview_id: null,
        archived_at: null,
        created_at: String(this.values[14]),
        created_by_actor_ref: String(this.values[7]),
        customer_user_id: this.values[6] as string | null,
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[13]),
        order_id: this.values[3] as string | null,
        owner_user_id: String(this.values[5]),
        prompt_summary: String(this.values[9]),
        site_id: this.values[2] as string | null,
        source_revision_id: this.values[11] as string | null,
        source_site_version_id: this.values[10] as string | null,
        status: String(this.values[8]),
        updated_at: String(this.values[15]),
        workroom_id: this.values[4] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_events') {
      this.store.site_builder_events.push({
        archived_at: null,
        created_at: String(this.values[12]),
        event_kind: String(this.values[4]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        payload_json: String(this.values[11]),
        phase_kind: this.values[5] as string | null,
        sequence: Number(this.values[3]),
        session_id: String(this.values[2]),
        source_ref: this.values[10] as string | null,
        status: String(this.values[7]),
        summary: String(this.values[9]),
        title: String(this.values[8]),
        visibility: String(this.values[6]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_phase_runs') {
      this.store.site_builder_phase_runs.push({
        archived_at: null,
        completed_at: this.values[9] as string | null,
        created_at: String(this.values[11]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[10]),
        phase_kind: String(this.values[4]),
        sequence: Number(this.values[3]),
        session_id: String(this.values[2]),
        started_at: this.values[8] as string | null,
        status: String(this.values[5]),
        summary: String(this.values[7]),
        title: String(this.values[6]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_previews') {
      const row = {
        archived_at: null,
        artifact_ref: this.values[7] as string | null,
        created_at: String(this.values[10]),
        health_ref: this.values[8] as string | null,
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[9]),
        preview_kind: String(this.values[3]),
        preview_url: this.values[5] as string | null,
        session_id: String(this.values[2]),
        status: String(this.values[4]),
        updated_at: String(this.values[11]),
        version_ref: this.values[6] as string | null,
      }
      this.store.site_builder_previews.push(row)
      const session = byId(
        this.store.site_builder_sessions,
        String(this.values[2]),
      )
      if (session !== null) {
        session.active_preview_id = row.id
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableFromQuery(this.query)
    const sessionId = String(this.values[0])
    const results = this.store[table]
      .filter(row => row.session_id === sessionId && active(row))
      .sort((left, right) => Number(left.sequence ?? 0) - Number(right.sequence ?? 0))

    return Promise.resolve({
      results: results as ReadonlyArray<T>,
      success: true,
    } as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const orchestrationDb = (store: OrchestrationStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OrchestrationStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

type TestSession = Readonly<{
  user: Readonly<{ email: string; userId: string }>
}>

const runtime = {
  nowIso: () => '2026-06-14T12:00:00.000Z',
  randomId: prefix => `${prefix}_test_${Math.random().toString(36).slice(2, 8)}`,
} satisfies SiteBuilderRuntime

const adminSession: TestSession = {
  user: { email: 'chris@openagents.com', userId: 'github:operator' },
}

const makeRoutes = (session: TestSession | null) =>
  makeSitesOrchestrationRoutes<TestSession, { OPENAGENTS_DB: D1Database }>({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
  })

const runRoute = (
  session: TestSession | null,
  store: OrchestrationStore,
  request: Request,
): Promise<Response> => {
  const route = makeRoutes(session).routeSitesOrchestrationRequest(
    request,
    { OPENAGENTS_DB: orchestrationDb(store) },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

const seedSession = (store: OrchestrationStore) =>
  Effect.runPromise(
    createSiteBuilderSession(
      orchestrationDb(store),
      {
        createdByActorRef: 'actor_operator_1',
        id: 'site_builder_session_orch',
        idempotencyKey: 'site-builder-session:orch-1',
        orderId: 'software_order_1',
        ownerUserId: 'user_owner_1',
        promptSummary: 'Build a small customer-safe landing page.',
        siteId: 'site_project_1',
      },
      runtime,
    ),
  )

const advanceRequest = (body: Record<string, unknown>) =>
  new Request(
    'https://openagents.com/api/operator/sites/orchestration/site_builder_session_orch/advance',
    {
      body: JSON.stringify(body),
      headers: { 'content-type': 'application/json' },
      method: 'POST',
    },
  )

describe('Sites orchestration routes', () => {
  test('does not match unrelated paths', () => {
    const route = makeRoutes(adminSession).routeSitesOrchestrationRequest(
      new Request('https://openagents.com/api/operator/sites'),
      { OPENAGENTS_DB: orchestrationDb(new OrchestrationStore()) },
      executionContext(),
    )

    expect(route).toBeUndefined()
  })

  test('rejects an unauthenticated advance', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)

    const response = await runRoute(
      null,
      store,
      advanceRequest({
        idempotencyKey: 'orch:run-1',
        phaseKind: 'planning',
        summary: 'Planning.',
        title: 'Planning',
      }),
    )

    expect(response.status).toBe(401)
  })

  test('rejects a non-admin advance', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)

    const response = await runRoute(
      { user: { email: 'someone@example.com', userId: 'github:other' } },
      store,
      advanceRequest({
        idempotencyKey: 'orch:run-1',
        phaseKind: 'planning',
        summary: 'Planning.',
        title: 'Planning',
      }),
    )

    expect(response.status).toBe(403)
  })

  test('advances a phase for an operator', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)

    const response = await runRoute(
      adminSession,
      store,
      advanceRequest({
        idempotencyKey: 'orch:run-1',
        phaseKind: 'planning',
        summary: 'Planning the customer-visible structure.',
        title: 'Planning',
      }),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    const body = (await response.json()) as Record<string, unknown>
    expect(body.outcome).toBe('phase_started')
    expect(body.sessionStatus).toBe('planning')
    expect(body.nextPhase).toBe('foundation')
  })

  test('drives a ready preview through the advance route', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)

    const response = await runRoute(
      adminSession,
      store,
      advanceRequest({
        idempotencyKey: 'orch:preview',
        phaseKind: 'preview',
        previewCandidate: {
          candidateKind: 'static_assets',
          previewUrl: 'https://otec.openagents.com',
        },
        summary: 'Static preview is ready.',
        title: 'Preview',
      }),
    )

    expect(response.status).toBe(201)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.outcome).toBe('preview_ready')
    expect(body.sessionStatus).toBe('preview_ready')
  })

  test('returns a validation error for a bad phase', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)

    const response = await runRoute(
      adminSession,
      store,
      advanceRequest({
        idempotencyKey: 'orch:bad',
        phaseKind: 'not_a_phase',
        summary: 'Bad.',
        title: 'Bad',
      }),
    )

    expect(response.status).toBe(400)
  })

  test('reads the orchestration state for an operator', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)
    await runRoute(
      adminSession,
      store,
      advanceRequest({
        idempotencyKey: 'orch:run-1',
        phaseKind: 'planning',
        summary: 'Planning.',
        title: 'Planning',
      }),
    )

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        'https://openagents.com/api/operator/sites/orchestration/site_builder_session_orch',
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      operator: { phaseCount: number; status: string }
      public: { currentPhase: { phaseKind: string } | null }
    }
    expect(body.operator.phaseCount).toBe(1)
    expect(body.public.currentPhase?.phaseKind).toBe('planning')
  })

  test('rejects an unauthenticated state read', async () => {
    const store = new OrchestrationStore()
    await seedSession(store)

    const response = await runRoute(
      null,
      store,
      new Request(
        'https://openagents.com/api/operator/sites/orchestration/site_builder_session_orch',
      ),
    )

    expect(response.status).toBe(401)
  })
})
