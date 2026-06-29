import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { recordSiteBuilderRepairAttempt } from './sites-builder-repair-loop'
import {
  type SiteBuilderRuntime,
  SiteBuilderSessionStorageError,
  SiteBuilderSessionValidationError,
  createSiteBuilderSession,
} from './sites-builder-sessions'

type Row = Record<string, unknown>

class RepairStore {
  site_builder_artifacts: Array<Row> = []
  site_builder_events: Array<Row> = []
  site_builder_file_snapshots: Array<Row> = []
  site_builder_messages: Array<Row> = []
  site_builder_phase_runs: Array<Row> = []
  site_builder_previews: Array<Row> = []
  site_builder_repair_attempts: Array<Row> = []
  site_builder_sessions: Array<Row> = []
}

const tables = [
  'site_builder_repair_attempts',
  'site_builder_sessions',
  'site_builder_messages',
  'site_builder_events',
  'site_builder_phase_runs',
  'site_builder_file_snapshots',
  'site_builder_previews',
  'site_builder_artifacts',
] as const

type Table = (typeof tables)[number]

const tableFromQuery = (query: string): Table => {
  const table = tables.find(name => query.includes(name))

  if (table === undefined) {
    throw new Error(`Unknown table: ${query}`)
  }

  return table
}

const active = (row: Row): boolean => row.archived_at === null

class RepairStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: RepairStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    const table = tableFromQuery(this.query)
    const rows = this.store[table]

    if (this.query.includes('WHERE id = ?')) {
      return Promise.resolve(
        (rows.find(row => row.id === this.values[0] && active(row)) ??
          null) as T | null,
      )
    }

    if (this.query.includes('WHERE idempotency_key = ?')) {
      return Promise.resolve(
        (rows.find(
          row => row.idempotency_key === this.values[0] && active(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('MAX(attempt_number)')) {
      const sessionId = String(this.values[0])
      const maxAttempt = this.store.site_builder_repair_attempts
        .filter(row => row.session_id === sessionId && active(row))
        .reduce((max, row) => Math.max(max, Number(row.attempt_number ?? 0)), 0)

      return Promise.resolve({ next_attempt: maxAttempt + 1 } as T)
    }

    if (this.query.includes('MAX(sequence)')) {
      const sessionId = String(this.values[0])
      const maxSequence = rows
        .filter(row => row.session_id === sessionId && active(row))
        .reduce((max, row) => Math.max(max, Number(row.sequence ?? 0)), 0)

      return Promise.resolve({ next_sequence: maxSequence + 1 } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableFromQuery(this.query)
    const idempotencyKey = String(this.values[1])

    if (
      this.store[table].some(
        row => row.idempotency_key === idempotencyKey && active(row),
      )
    ) {
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

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableFromQuery(this.query)
    const sessionId = String(this.values[0])
    const rows = this.store[table].filter(
      row => row.session_id === sessionId && active(row),
    )

    return Promise.resolve({
      results: rows as ReadonlyArray<T>,
      success: true,
    } as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.resolve([])
  }
}

const db = (store: RepairStore): D1Database => ({
  batch: () => Promise.reject(new Error('batch not used')),
  dump: () => Promise.reject(new Error('dump not used')),
  exec: () => Promise.reject(new Error('exec not used')),
  prepare: query => new RepairStatement(query, store),
  withSession: () => {
    throw new Error('session not used')
  },
})

const runtime = {
  nowIso: () => '2026-06-05T23:55:00.000Z',
  randomId: prefix => `${prefix}_test`,
} satisfies SiteBuilderRuntime

const createSession = async (store: RepairStore) =>
  Effect.runPromise(
    createSiteBuilderSession(
      db(store),
      {
        createdByActorRef: 'user:user_owner',
        id: 'site_builder_session_1',
        idempotencyKey: 'site-builder-session:repair',
        ownerUserId: 'user_owner',
        promptSummary: 'Build a repairable Site.',
      },
      runtime,
    ),
  )

describe('Sites builder repair loop', () => {
  test('records redacted repair attempts and customer-visible events', async () => {
    const store = new RepairStore()
    const session = await createSession(store)
    const attempt = await Effect.runPromise(
      recordSiteBuilderRepairAttempt(
        db(store),
        {
          failureKind: 'build_error',
          failureSummary: 'Vite build failed: missing import in src/App.tsx',
          id: 'site_builder_repair_1',
          idempotencyKey: 'site-builder-repair:1',
          phaseKind: 'preview',
          retryBudget: 2,
          sessionId: session.id,
          status: 'running',
        },
        runtime,
      ),
    )

    expect(attempt.attemptNumber).toBe(1)
    expect(attempt.retryBudget).toBe(2)
    expect(attempt.redactedSummary).toContain('missing import')
    expect(store.site_builder_repair_attempts).toHaveLength(1)
    expect(store.site_builder_events).toHaveLength(1)
    expect(store.site_builder_events[0]).toMatchObject({
      event_kind: 'phase_updated',
      phase_kind: 'preview',
      visibility: 'customer',
    })
  })

  test('rejects retry attempts past the bounded repair budget', async () => {
    const store = new RepairStore()
    const session = await createSession(store)

    await expect(
      Effect.runPromise(
        recordSiteBuilderRepairAttempt(
          db(store),
          {
            attemptNumber: 3,
            failureKind: 'runtime_error',
            failureSummary: 'Preview returned 500.',
            idempotencyKey: 'site-builder-repair:budget',
            retryBudget: 2,
            sessionId: session.id,
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(SiteBuilderSessionValidationError)
  })

  test('rejects private runner and secret-shaped failure material', async () => {
    const store = new RepairStore()
    const session = await createSession(store)

    await expect(
      Effect.runPromise(
        recordSiteBuilderRepairAttempt(
          db(store),
          {
            failureKind: 'preview_error',
            failureSummary: 'runner_payload included gho_abc123456789',
            idempotencyKey: 'site-builder-repair:secret',
            retryBudget: 2,
            sessionId: session.id,
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(SiteBuilderSessionValidationError)
  })

  test('does not create repair attempts for missing sessions', async () => {
    const store = new RepairStore()

    await expect(
      Effect.runPromise(
        recordSiteBuilderRepairAttempt(
          db(store),
          {
            failureKind: 'unknown',
            failureSummary: 'No session exists.',
            idempotencyKey: 'site-builder-repair:missing',
            retryBudget: 1,
            sessionId: 'site_builder_session_missing',
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(SiteBuilderSessionStorageError)
    expect(store.site_builder_repair_attempts).toHaveLength(0)
  })
})
