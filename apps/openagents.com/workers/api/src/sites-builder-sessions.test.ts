import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type SiteBuilderRuntime,
  SiteBuilderSessionValidationError,
  appendSiteBuilderEvent,
  appendSiteBuilderMessage,
  createSiteBuilderSession,
  listSiteBuilderFileSnapshots,
  readLatestSiteBuilderFileSnapshot,
  readSiteBuilderSessionProjection,
  recordSiteBuilderArtifact,
  recordSiteBuilderPhaseRun,
  recordSiteBuilderPreview,
  upsertSiteBuilderFileSnapshot,
} from './sites-builder-sessions'
import {
  SITES_TANSTACK_RULES_METADATA_KEY,
  SITES_TANSTACK_RULES_REF,
} from './sites-tanstack-rules'

type Row = Record<string, unknown>

class BuilderStore {
  site_builder_artifacts: Array<Row> = []
  site_builder_events: Array<Row> = []
  site_builder_file_snapshots: Array<Row> = []
  site_builder_messages: Array<Row> = []
  site_builder_phase_runs: Array<Row> = []
  site_builder_previews: Array<Row> = []
  site_builder_sessions: Array<Row> = []
}

const tableNames = [
  'site_builder_sessions',
  'site_builder_messages',
  'site_builder_events',
  'site_builder_phase_runs',
  'site_builder_file_snapshots',
  'site_builder_previews',
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

const byIdempotency = (
  rows: ReadonlyArray<Row>,
  idempotencyKey: string,
): Row | null =>
  rows.find(row => row.idempotency_key === idempotencyKey && active(row)) ??
  null

const byId = (rows: ReadonlyArray<Row>, id: string): Row | null =>
  rows.find(row => row.id === id && active(row)) ?? null

class BuilderStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: BuilderStore,
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

    if (
      this.query.includes('WHERE session_id = ?') &&
      this.query.includes('AND path = ?')
    ) {
      const sessionId = String(this.values[0])
      const path = String(this.values[1])
      const row =
        rows
          .filter(
            candidate =>
              candidate.session_id === sessionId &&
              candidate.path === path &&
              active(candidate),
          )
          .sort(
            (left, right) => Number(right.sequence) - Number(left.sequence),
          )[0] ?? null

      return Promise.resolve(row as T | null)
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

    if (byIdempotency(this.store[table], idempotencyKey) !== null) {
      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_sessions') {
      this.store.site_builder_sessions.push({
        active_artifact_id: null,
        active_preview_id: null,
        archived_at: null,
        created_at: String(this.values[13]),
        created_by_actor_ref: String(this.values[7]),
        customer_user_id: this.values[6] as string | null,
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[12]),
        order_id: this.values[3] as string | null,
        owner_user_id: String(this.values[5]),
        prompt_summary: String(this.values[9]),
        site_id: this.values[2] as string | null,
        source_revision_id: this.values[11] as string | null,
        source_site_version_id: this.values[10] as string | null,
        status: String(this.values[8]),
        updated_at: String(this.values[14]),
        workroom_id: this.values[4] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_messages') {
      this.store.site_builder_messages.push({
        actor_kind: String(this.values[4]),
        archived_at: null,
        body: String(this.values[6]),
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[7]),
        sequence: Number(this.values[3]),
        session_id: String(this.values[2]),
        visibility: String(this.values[5]),
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

    if (table === 'site_builder_file_snapshots') {
      this.store.site_builder_file_snapshots.push({
        archived_at: null,
        artifact_ref: this.values[9] as string | null,
        byte_size: Number(this.values[7]),
        content_hash: String(this.values[6]),
        created_at: String(this.values[13]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        language: this.values[5] as string | null,
        metadata_json: String(this.values[12]),
        path: String(this.values[3]),
        preview_text: this.values[10] as string | null,
        sequence: Number(this.values[4]),
        session_id: String(this.values[2]),
        source_ref: this.values[8] as string | null,
        updated_at: String(this.values[14]),
        visibility: String(this.values[11]),
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

    this.store.site_builder_artifacts.push({
      archived_at: null,
      artifact_kind: String(this.values[3]),
      artifact_ref: String(this.values[4]),
      byte_size: this.values[6] as number | null,
      content_hash: this.values[5] as string | null,
      created_at: String(this.values[9]),
      id: String(this.values[0]),
      idempotency_key: idempotencyKey,
      manifest_ref: this.values[7] as string | null,
      metadata_json: String(this.values[8]),
      session_id: String(this.values[2]),
    })
    const session = byId(
      this.store.site_builder_sessions,
      String(this.values[2]),
    )
    if (session !== null) {
      session.active_artifact_id = String(this.values[0])
    }

    return Promise.resolve({ success: true } as D1Result<T>)
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableFromQuery(this.query)
    const sessionId = String(this.values[0])
    const results = this.store[table]
      .filter(row => row.session_id === sessionId && active(row))
      .sort((left, right) => {
        if (table === 'site_builder_file_snapshots') {
          const pathOrder = String(left.path).localeCompare(String(right.path))

          return pathOrder === 0
            ? Number(right.sequence) - Number(left.sequence)
            : pathOrder
        }

        return Number(left.sequence ?? 0) - Number(right.sequence ?? 0)
      })

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

const builderDb = (store: BuilderStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new BuilderStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime = {
  nowIso: () => '2026-06-05T23:15:00.000Z',
  randomId: prefix => `${prefix}_test`,
} satisfies SiteBuilderRuntime

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

describe('Sites builder sessions', () => {
  test('creates sessions idempotently', async () => {
    const store = new BuilderStore()
    const db = builderDb(store)

    const first = await run(
      createSiteBuilderSession(
        db,
        {
          createdByActorRef: 'actor_customer_1',
          customerUserId: 'user_customer_1',
          id: 'site_builder_session_1',
          idempotencyKey: 'site-builder-session:order-1',
          metadata: { source: 'self_serve_builder' },
          orderId: 'software_order_1',
          ownerUserId: 'user_owner_1',
          promptSummary: 'Build a small customer-safe landing page.',
          siteId: 'site_project_1',
          workroomId: 'workroom_1',
        },
        runtime,
      ),
    )
    const second = await run(
      createSiteBuilderSession(
        db,
        {
          createdByActorRef: 'actor_customer_1',
          id: 'site_builder_session_different',
          idempotencyKey: 'site-builder-session:order-1',
          ownerUserId: 'user_owner_1',
          promptSummary: 'This should not overwrite the first session.',
        },
        runtime,
      ),
    )

    expect(first.id).toBe('site_builder_session_1')
    expect(second.id).toBe(first.id)
    expect(first.metadata.source).toBe('self_serve_builder')
    expect(first.metadata[SITES_TANSTACK_RULES_METADATA_KEY]).toMatchObject({
      ref: SITES_TANSTACK_RULES_REF,
      version: '2026-07-04.1',
    })
    expect(store.site_builder_sessions).toHaveLength(1)
  })

  test('records messages, events, files, previews, and artifacts into safe projections', async () => {
    const store = new BuilderStore()
    const db = builderDb(store)

    const session = await run(
      createSiteBuilderSession(
        db,
        {
          createdByActorRef: 'actor_customer_1',
          id: 'site_builder_session_1',
          idempotencyKey: 'site-builder-session:order-1',
          orderId: 'software_order_1',
          ownerUserId: 'user_owner_1',
          promptSummary: 'Build a product page for an internal tool.',
          siteId: 'site_project_1',
        },
        runtime,
      ),
    )

    await run(
      appendSiteBuilderMessage(
        db,
        {
          actorKind: 'customer',
          body: 'Please make the product page clearer.',
          id: 'site_builder_message_1',
          idempotencyKey: 'site-builder-message:1',
          sessionId: session.id,
        },
        runtime,
      ),
    )
    await run(
      appendSiteBuilderMessage(
        db,
        {
          actorKind: 'agent',
          body: 'Private build receipt stored separately.',
          id: 'site_builder_message_2',
          idempotencyKey: 'site-builder-message:2',
          sessionId: session.id,
          visibility: 'internal',
        },
        runtime,
      ),
    )
    const event = await run(
      appendSiteBuilderEvent(
        db,
        {
          eventKind: 'phase_started',
          id: 'site_builder_event_1',
          idempotencyKey: 'site-builder-event:1',
          phaseKind: 'planning',
          sessionId: session.id,
          summary: 'Planning the customer-visible structure.',
          title: 'Planning',
        },
        runtime,
      ),
    )
    const phase = await run(
      recordSiteBuilderPhaseRun(
        db,
        {
          id: 'site_builder_phase_1',
          idempotencyKey: 'site-builder-phase:1',
          phaseKind: 'planning',
          sessionId: session.id,
          startedAt: '2026-06-05T23:15:00.000Z',
          status: 'running',
          summary: 'Planning the page structure.',
          title: 'Planning',
        },
        runtime,
      ),
    )
    const file = await run(
      upsertSiteBuilderFileSnapshot(
        db,
        {
          byteSize: 128,
          contentHash: 'sha256:abcdef123456',
          id: 'site_builder_file_1',
          idempotencyKey: 'site-builder-file:1',
          language: 'tsx',
          path: 'src/App.tsx',
          previewText: 'export function App() { return <main /> }',
          sessionId: session.id,
        },
        runtime,
      ),
    )
    await run(
      recordSiteBuilderPreview(
        db,
        {
          artifactRef: 'artifact_preview_1',
          id: 'site_builder_preview_1',
          idempotencyKey: 'site-builder-preview:1',
          previewKind: 'static_r2',
          previewUrl:
            'https://sites.openagents.com/previews/site_builder_session_1',
          sessionId: session.id,
          status: 'ready',
          versionRef: 'site_version_preview_1',
        },
        runtime,
      ),
    )
    await run(
      recordSiteBuilderArtifact(
        db,
        {
          artifactKind: 'source_archive',
          artifactRef: 'artifact_source_archive_1',
          byteSize: 2048,
          contentHash: 'sha256:fedcba654321',
          id: 'site_builder_artifact_1',
          idempotencyKey: 'site-builder-artifact:1',
          manifestRef: 'manifest_source_archive_1',
          sessionId: session.id,
        },
        runtime,
      ),
    )

    const projection = await run(
      readSiteBuilderSessionProjection(db, session.id),
    )

    expect(event.sequence).toBe(1)
    expect(phase.sequence).toBe(1)
    expect(file.path).toBe('src/App.tsx')
    expect(projection.public.messages).toHaveLength(1)
    expect(projection.public.messages[0]?.body).toBe(
      'Please make the product page clearer.',
    )
    expect(projection.public.currentPhase).toMatchObject({
      phaseKind: 'planning',
      status: 'running',
      summary: 'Planning the page structure.',
    })
    expect(projection.public.phases).toHaveLength(1)
    expect(projection.operator.eventCount).toBe(2)
    expect(projection.operator.fileCount).toBe(1)
    expect(projection.operator.phaseCount).toBe(1)
    expect(projection.operator.phaseCurrent).toMatchObject({
      phaseKind: 'planning',
      status: 'running',
    })
    expect(projection.operator.previewCount).toBe(1)
    expect(projection.operator.artifactCount).toBe(1)
    expect(projection.operator.activePreviewId).toBe('site_builder_preview_1')
    expect(projection.operator.activeArtifactId).toBe('site_builder_artifact_1')
  })

  test('lists generated file snapshots and reads the latest active path', async () => {
    const store = new BuilderStore()
    const db = builderDb(store)

    const session = await run(
      createSiteBuilderSession(
        db,
        {
          createdByActorRef: 'actor_customer_1',
          id: 'site_builder_session_1',
          idempotencyKey: 'site-builder-session:files',
          ownerUserId: 'user_owner_1',
          promptSummary: 'Build a product page.',
        },
        runtime,
      ),
    )

    await run(
      upsertSiteBuilderFileSnapshot(
        db,
        {
          byteSize: 16,
          contentHash: 'sha256:older',
          id: 'site_builder_file_1',
          idempotencyKey: 'site-builder-file:1',
          path: 'src/App.tsx',
          previewText: 'older',
          sessionId: session.id,
        },
        runtime,
      ),
    )
    await run(
      upsertSiteBuilderFileSnapshot(
        db,
        {
          byteSize: 18,
          contentHash: 'sha256:newer',
          id: 'site_builder_file_2',
          idempotencyKey: 'site-builder-file:2',
          path: 'src/App.tsx',
          previewText: 'newer',
          sessionId: session.id,
        },
        runtime,
      ),
    )

    const files = await run(
      listSiteBuilderFileSnapshots(db, { sessionId: session.id }),
    )
    const latest = await run(
      readLatestSiteBuilderFileSnapshot(db, {
        path: 'src/App.tsx',
        sessionId: session.id,
      }),
    )

    expect(files).toHaveLength(2)
    expect(latest?.sequence).toBe(2)
    expect(latest?.previewText).toBe('newer')
  })

  test('rejects private runner, provider, and secret-shaped material', async () => {
    const store = new BuilderStore()
    const db = builderDb(store)

    await expect(
      run(
        createSiteBuilderSession(
          db,
          {
            createdByActorRef: 'actor_customer_1',
            idempotencyKey: 'site-builder-session:unsafe',
            metadata: { provider_payload: 'raw' },
            ownerUserId: 'user_owner_1',
            promptSummary: 'Build the page.',
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(SiteBuilderSessionValidationError)

    await run(
      createSiteBuilderSession(
        db,
        {
          createdByActorRef: 'actor_customer_1',
          id: 'site_builder_session_1',
          idempotencyKey: 'site-builder-session:order-1',
          ownerUserId: 'user_owner_1',
          promptSummary: 'Build the page.',
        },
        runtime,
      ),
    )

    await expect(
      run(
        upsertSiteBuilderFileSnapshot(
          db,
          {
            byteSize: 10,
            contentHash: 'sha256:abcdef123456',
            idempotencyKey: 'site-builder-file:unsafe',
            path: '../secrets.env',
            sessionId: 'site_builder_session_1',
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(SiteBuilderSessionValidationError)

    await expect(
      run(
        recordSiteBuilderPreview(
          db,
          {
            idempotencyKey: 'site-builder-preview:unsafe',
            previewKind: 'static_r2',
            previewUrl: 'https://evil.example/preview',
            sessionId: 'site_builder_session_1',
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(SiteBuilderSessionValidationError)
  })
})
