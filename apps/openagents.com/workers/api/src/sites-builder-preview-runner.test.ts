import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  recordSiteBuilderPreviewCandidate,
  selectSiteBuilderPreviewTier,
} from './sites-builder-preview-runner'
import {
  type SiteBuilderRuntime,
  createSiteBuilderSession,
} from './sites-builder-sessions'

type Row = Record<string, unknown>

class PreviewRunnerStore {
  site_builder_artifacts: Array<Row> = []
  site_builder_events: Array<Row> = []
  site_builder_file_snapshots: Array<Row> = []
  site_builder_messages: Array<Row> = []
  site_builder_previews: Array<Row> = []
  site_builder_sessions: Array<Row> = []
}

const tables = [
  'site_builder_sessions',
  'site_builder_messages',
  'site_builder_events',
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

class PreviewRunnerStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: PreviewRunnerStore,
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

    if (table === 'site_builder_previews') {
      this.store.site_builder_previews.push({
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

const db = (store: PreviewRunnerStore): D1Database => ({
  batch: () => Promise.reject(new Error('batch not used')),
  dump: () => Promise.reject(new Error('dump not used')),
  exec: () => Promise.reject(new Error('exec not used')),
  prepare: query => new PreviewRunnerStatement(query, store),
  withSession: () => {
    throw new Error('session not used')
  },
})

const runtime = {
  nowIso: () => '2026-06-05T23:45:00.000Z',
  randomId: prefix => `${prefix}_test`,
} satisfies SiteBuilderRuntime

const createSession = async (store: PreviewRunnerStore) =>
  Effect.runPromise(
    createSiteBuilderSession(
      db(store),
      {
        createdByActorRef: 'user:user_owner',
        id: 'site_builder_session_1',
        idempotencyKey: 'site-builder-session:preview',
        ownerUserId: 'user_owner',
        promptSummary: 'Build a previewable Site.',
      },
      runtime,
    ),
  )

describe('Sites builder preview runner', () => {
  test('selects the static R2 tier for static candidates', () => {
    expect(
      selectSiteBuilderPreviewTier({ candidateKind: 'static_assets' }),
    ).toMatchObject({
      containerWorkGated: false,
      previewKind: 'static_r2',
      reason: 'static_candidate',
      tier: 'r2_static',
    })
  })

  test('selects the WFP staging tier for Worker module candidates', () => {
    expect(
      selectSiteBuilderPreviewTier({
        candidateKind: 'worker_module',
        workerModulePath: 'dist/worker.mjs',
      }),
    ).toMatchObject({
      containerWorkGated: false,
      previewKind: 'workers_for_platforms',
      reason: 'worker_module_ready',
      tier: 'wfp_staging',
    })
  })

  test('selects the metered Container tier only when execution is required', () => {
    expect(
      selectSiteBuilderPreviewTier({
        candidateKind: 'needs_build',
        runtimeNeeds: { dependencyInstall: true },
      }),
    ).toMatchObject({
      containerWorkGated: true,
      previewKind: 'container',
      reason: 'build_or_runtime_execution_required',
      tier: 'container_metered',
    })
  })

  test('records preview selection and customer-visible event receipt', async () => {
    const store = new PreviewRunnerStore()
    const session = await createSession(store)
    const result = await Effect.runPromise(
      recordSiteBuilderPreviewCandidate(
        db(store),
        {
          candidate: {
            artifactRef: 'artifact_preview_1',
            candidateKind: 'static_assets',
            previewUrl: 'https://sites.openagents.com/previews/static-1',
          },
          id: 'site_builder_preview_1',
          idempotencyKey: 'site-builder-preview:static',
          sessionId: session.id,
        },
        runtime,
      ),
    )

    expect(result.selection.tier).toBe('r2_static')
    expect(result.preview.previewKind).toBe('static_r2')
    expect(result.preview.status).toBe('ready')
    expect(store.site_builder_previews).toHaveLength(1)
    expect(store.site_builder_events).toHaveLength(1)
    expect(store.site_builder_events[0]).toMatchObject({
      event_kind: 'preview_created',
      phase_kind: 'preview',
      status: 'succeeded',
      visibility: 'customer',
    })
  })
})
