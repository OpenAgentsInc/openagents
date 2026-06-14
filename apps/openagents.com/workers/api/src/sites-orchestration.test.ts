import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type SiteBuilderRuntime,
  createSiteBuilderSession,
} from './sites-builder-sessions'
import {
  SITE_ORCHESTRATION_PHASE_ORDER,
  advanceSiteBuilderOrchestration,
} from './sites-orchestration'

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

    if (table === 'site_builder_repair_attempts') {
      this.store.site_builder_repair_attempts.push({
        archived_at: null,
        attempt_number: Number(this.values[5]),
        completed_at: this.values[13] as string | null,
        created_at: String(this.values[12]),
        failure_kind: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[11]),
        phase_kind: this.values[4] as string | null,
        preview_id: this.values[3] as string | null,
        redacted_summary: String(this.values[9]),
        retry_budget: Number(this.values[6]),
        session_id: String(this.values[2]),
        status: String(this.values[7]),
        stop_reason: this.values[10] as string | null,
      })

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

const runtime = {
  nowIso: () => '2026-06-14T12:00:00.000Z',
  randomId: prefix => `${prefix}_test_${Math.random().toString(36).slice(2, 8)}`,
} satisfies SiteBuilderRuntime

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

const seedSession = (db: D1Database) =>
  run(
    createSiteBuilderSession(
      db,
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

describe('Sites prompt -> build -> deploy orchestration core', () => {
  test('advances an ordinary phase and reports the next phase', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const state = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          idempotencyKey: 'orch:run-1',
          phaseKind: 'planning',
          sessionId: 'site_builder_session_orch',
          summary: 'Planning the customer-visible structure.',
          title: 'Planning',
        },
        runtime,
      ),
    )

    expect(state.outcome).toBe('phase_started')
    expect(state.sessionStatus).toBe('planning')
    expect(state.nextPhase).toBe('foundation')
    expect(state.public.currentPhase?.phaseKind).toBe('planning')
    expect(state.public.currentPhase?.status).toBe('succeeded')
  })

  test('records a ready preview through the preview runner', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const state = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          idempotencyKey: 'orch:preview-1',
          phaseKind: 'preview',
          previewCandidate: {
            candidateKind: 'static_assets',
            previewUrl: 'https://otec.openagents.com',
          },
          sessionId: 'site_builder_session_orch',
          summary: 'Static preview is ready.',
          title: 'Preview',
        },
        runtime,
      ),
    )

    expect(state.outcome).toBe('preview_ready')
    expect(state.sessionStatus).toBe('preview_ready')
    expect(state.preview?.preview.status).toBe('ready')
    expect(state.preview?.selection.tier).toBe('r2_static')
    expect(state.public.activePreview?.status).toBe('ready')
    expect(state.nextPhase).toBe('save')
  })

  test('gates a build-needing preview candidate behind container work', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const state = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          idempotencyKey: 'orch:preview-2',
          phaseKind: 'preview',
          previewCandidate: {
            candidateKind: 'needs_build',
            runtimeNeeds: { buildExecution: true },
          },
          sessionId: 'site_builder_session_orch',
          summary: 'Build is required before preview.',
          title: 'Preview',
        },
        runtime,
      ),
    )

    expect(state.outcome).toBe('preview_pending')
    expect(state.preview?.selection.containerWorkGated).toBe(true)
    expect(state.preview?.preview.status).toBe('requested')
  })

  test('routes a build failure through the repair loop and keeps budget', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const state = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          failure: {
            attemptNumber: 1,
            failureKind: 'build_error',
            failureSummary: 'Type error in src/App.tsx.',
            retryBudget: 3,
          },
          idempotencyKey: 'orch:core-fail',
          phaseKind: 'core',
          sessionId: 'site_builder_session_orch',
          summary: 'Core build failed; scheduling repair.',
          title: 'Core',
        },
        runtime,
      ),
    )

    expect(state.outcome).toBe('build_repair_scheduled')
    // a scheduled repair keeps the session in the build loop, not failed
    expect(state.sessionStatus).toBe('building')
    expect(state.repairAttempt?.attemptNumber).toBe(1)
    expect(state.repairAttempt?.status).toBe('blocked')
    expect(state.public.currentPhase?.status).toBe('blocked')
    expect(state.nextPhase).toBeNull()
  })

  test('marks repair exhausted when the retry budget is reached', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const state = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          failure: {
            attemptNumber: 2,
            failureKind: 'build_error',
            failureSummary: 'Repeated failure on src/App.tsx.',
            retryBudget: 2,
          },
          idempotencyKey: 'orch:core-exhaust',
          phaseKind: 'core',
          sessionId: 'site_builder_session_orch',
          summary: 'Core build failed again; budget exhausted.',
          title: 'Core',
        },
        runtime,
      ),
    )

    expect(state.outcome).toBe('build_repair_exhausted')
    expect(state.sessionStatus).toBe('failed')
    expect(state.repairAttempt?.status).toBe('failed')
    expect(state.public.currentPhase?.status).toBe('failed')
  })

  test('records save then deploy outcomes with result refs', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const saved = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          idempotencyKey: 'orch:save',
          phaseKind: 'save',
          resultRef: 'site_version_42',
          sessionId: 'site_builder_session_orch',
          summary: 'Saved reviewable Site version.',
          title: 'Save',
        },
        runtime,
      ),
    )

    expect(saved.outcome).toBe('saved')
    expect(saved.sessionStatus).toBe('saved')
    expect(saved.nextPhase).toBe('deploy')

    const deployed = await run(
      advanceSiteBuilderOrchestration(
        db,
        {
          idempotencyKey: 'orch:deploy',
          phaseKind: 'deploy',
          resultRef: 'site_deployment_42',
          sessionId: 'site_builder_session_orch',
          summary: 'Deployed the Site.',
          title: 'Deploy',
        },
        runtime,
      ),
    )

    expect(deployed.outcome).toBe('deployed')
    expect(deployed.sessionStatus).toBe('deployed')
    expect(deployed.nextPhase).toBeNull()
  })

  test('rejects a phase outside the canonical plan', async () => {
    const store = new OrchestrationStore()
    const db = orchestrationDb(store)
    await seedSession(db)

    const exit = await Effect.runPromiseExit(
      advanceSiteBuilderOrchestration(
        db,
        {
          idempotencyKey: 'orch:bad',
          // deliberately invalid phase kind
          phaseKind: 'not_a_phase' as never,
          sessionId: 'site_builder_session_orch',
          summary: 'Should fail validation.',
          title: 'Bad',
        },
        runtime,
      ),
    )

    expect(exit._tag).toBe('Failure')
  })

  test('exposes the canonical phase order', () => {
    expect(SITE_ORCHESTRATION_PHASE_ORDER).toEqual([
      'planning',
      'foundation',
      'core',
      'styling',
      'integration',
      'optimization',
      'preview',
      'save',
      'deploy',
    ])
  })
})
