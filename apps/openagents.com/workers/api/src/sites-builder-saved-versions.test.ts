import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type AutopilotSitesRuntime } from './sites'
import { saveSiteBuilderVersion } from './sites-builder-saved-versions'
import {
  type SiteBuilderRuntime,
  SiteBuilderSessionValidationError,
  createSiteBuilderSession,
} from './sites-builder-sessions'

type Row = Record<string, unknown>

class SaveVersionStore {
  site_builder_artifacts: Array<Row> = []
  site_builder_events: Array<Row> = []
  site_builder_file_snapshots: Array<Row> = []
  site_builder_messages: Array<Row> = []
  site_builder_phase_runs: Array<Row> = []
  site_builder_previews: Array<Row> = []
  site_builder_saved_versions: Array<Row> = []
  site_builder_sessions: Array<Row> = []
  site_deployments: Array<Row> = []
  site_events: Array<Row> = []
  site_projects: Array<Row> = [
    {
      access_mode: 'customer_owner',
      active_deployment_id: null,
      active_version_id: null,
      archived_at: null,
      created_at: '2026-06-05T00:00:00.000Z',
      id: 'site_1',
      owner_user_id: 'user_owner',
      project_id: null,
      prompt: 'Build an OTEC Site.',
      slug: 'otec',
      software_order_id: 'order_1',
      source_repository_name: null,
      source_repository_owner: null,
      source_repository_provider: null,
      source_repository_ref: null,
      status: 'needs_review',
      team_id: null,
      title: 'OTEC',
      updated_at: '2026-06-05T00:00:00.000Z',
      visibility: 'team',
    },
    {
      access_mode: 'customer_owner',
      active_deployment_id: null,
      active_version_id: null,
      archived_at: null,
      created_at: '2026-06-05T00:00:00.000Z',
      id: 'site_2',
      owner_user_id: 'user_owner',
      project_id: null,
      prompt: 'Build a different Site.',
      slug: 'other',
      software_order_id: 'order_2',
      source_repository_name: null,
      source_repository_owner: null,
      source_repository_provider: null,
      source_repository_ref: null,
      status: 'needs_review',
      team_id: null,
      title: 'Other',
      updated_at: '2026-06-05T00:00:00.000Z',
      visibility: 'team',
    },
  ]
  site_storage_bindings: Array<Row> = []
  site_versions: Array<Row> = []
}

const active = (row: Row): boolean => row.archived_at === null

const builderTables = [
  'site_builder_saved_versions',
  'site_builder_sessions',
  'site_builder_messages',
  'site_builder_events',
  'site_builder_phase_runs',
  'site_builder_file_snapshots',
  'site_builder_previews',
  'site_builder_artifacts',
] as const

const builderTableFromQuery = (query: string): (typeof builderTables)[number] => {
  const table = builderTables.find(name => query.includes(name))

  if (table === undefined) {
    throw new Error(`Unknown builder table: ${query}`)
  }

  return table
}

class SaveVersionStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: SaveVersionStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_projects')) {
      return Promise.resolve(
        (this.store.site_projects.find(
          row => row.id === this.values[0] && active(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('FROM site_builder_saved_versions')) {
      return Promise.resolve(
        (this.store.site_builder_saved_versions.find(
          row => row.idempotency_key === this.values[0] && active(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('WHERE id = ?')) {
      const table = builderTableFromQuery(this.query)

      return Promise.resolve(
        (this.store[table].find(row => row.id === this.values[0] && active(row)) ??
          null) as T | null,
      )
    }

    if (this.query.includes('WHERE idempotency_key = ?')) {
      const table = builderTableFromQuery(this.query)

      return Promise.resolve(
        (this.store[table].find(
          row => row.idempotency_key === this.values[0] && active(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('MAX(sequence)')) {
      const table = builderTableFromQuery(this.query)
      const sessionId = String(this.values[0])
      const maxSequence = this.store[table]
        .filter(row => row.session_id === sessionId && active(row))
        .reduce((max, row) => Math.max(max, Number(row.sequence ?? 0)), 0)

      return Promise.resolve({ next_sequence: maxSequence + 1 } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO site_builder_sessions')) {
      this.store.site_builder_sessions.push({
        active_artifact_id: null,
        active_preview_id: null,
        archived_at: null,
        created_at: String(this.values[14]),
        created_by_actor_ref: String(this.values[7]),
        customer_user_id: this.values[6] as string | null,
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
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

    if (this.query.includes('INSERT OR IGNORE INTO site_builder_saved_versions')) {
      if (
        this.store.site_builder_saved_versions.some(
          row => row.idempotency_key === this.values[1] && active(row),
        )
      ) {
        return Promise.resolve({ success: true } as D1Result<T>)
      }

      this.store.site_builder_saved_versions.push({
        archived_at: null,
        artifact_ref: this.values[6] as string | null,
        build_receipt_ref: this.values[7] as string | null,
        created_at: String(this.values[11]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        notes: this.values[9] as string | null,
        preview_id: this.values[5] as string | null,
        session_id: String(this.values[2]),
        site_id: String(this.values[3]),
        site_metadata_json: String(this.values[10]),
        site_version_id: String(this.values[4]),
        source_hash: this.values[8] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_versions')) {
      this.store.site_versions.push({
        artifact_manifest_r2_key: this.values[5] as string | null,
        build_command: this.values[8] as string | null,
        build_log_r2_key: this.values[6] as string | null,
        build_status: String(this.values[7]),
        created_at: String(this.values[16]),
        created_by_run_id: this.values[15] as string | null,
        created_by_user_id: this.values[14] as string | null,
        d1_binding_name: this.values[11] as string | null,
        id: String(this.values[0]),
        metadata_json: String(this.values[13]),
        r2_binding_name: this.values[12] as string | null,
        rejected_at: this.values[18] as string | null,
        saved_at: this.values[17] as string | null,
        site_id: String(this.values[1]),
        source_archive_r2_key: this.values[4] as string | null,
        source_commit_sha: this.values[3] as string | null,
        source_kind: String(this.values[2]),
        static_assets_manifest_json: String(this.values[10]),
        worker_module_r2_key: this.values[9] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_events')) {
      this.store.site_events.push({
        actor_run_id: this.values[7] as string | null,
        actor_user_id: this.values[6] as string | null,
        created_at: String(this.values[8]),
        deployment_id: this.values[3] as string | null,
        id: String(this.values[0]),
        payload_json: this.values[9] as string | null,
        site_id: String(this.values[1]),
        summary: String(this.values[5]),
        type: String(this.values[4]),
        version_id: this.values[2] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO site_builder_events')) {
      this.store.site_builder_events.push({
        archived_at: null,
        created_at: String(this.values[12]),
        event_kind: String(this.values[4]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
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

    if (this.query.includes('INSERT INTO site_storage_bindings')) {
      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_builder_')) {
      const table = builderTableFromQuery(this.query)
      const sessionId = String(this.values[0])
      const rows = this.store[table].filter(
        row => row.session_id === sessionId && active(row),
      )

      return Promise.resolve({
        results: rows as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.resolve([])
  }
}

const db = (store: SaveVersionStore): D1Database => ({
  batch: () => Promise.reject(new Error('batch not used')),
  dump: () => Promise.reject(new Error('dump not used')),
  exec: () => Promise.reject(new Error('exec not used')),
  prepare: query => new SaveVersionStatement(query, store),
  withSession: () => {
    throw new Error('session not used')
  },
})

class ArtifactBucket implements R2Bucket {
  objects = new Map<string, string>()

  put(key: string, value: string): Promise<R2Object> {
    this.objects.set(key, value)

    return Promise.resolve({ key } as R2Object)
  }

  createMultipartUpload(): Promise<R2MultipartUpload> {
    return Promise.reject(new Error('not used'))
  }

  delete(): Promise<void> {
    return Promise.reject(new Error('not used'))
  }

  get(): Promise<R2ObjectBody | null> {
    return Promise.reject(new Error('not used'))
  }

  head(): Promise<R2Object | null> {
    return Promise.reject(new Error('not used'))
  }

  list(): Promise<R2Objects> {
    return Promise.reject(new Error('not used'))
  }

  resumeMultipartUpload(): R2MultipartUpload {
    throw new Error('not used')
  }
}

const sitesRuntime = {
  makeDeploymentId: () => 'site_deployment_test',
  makeDeploymentAttemptId: () => 'site_deployment_attempt_test',
  makeEventId: () => 'site_event_test',
  makeSiteId: () => 'site_test',
  makeVersionId: () => 'site_version_test',
  nowIso: () => '2026-06-05T20:00:00.000Z',
} satisfies AutopilotSitesRuntime

const builderRuntime = {
  nowIso: () => '2026-06-05T20:00:00.000Z',
  randomId: prefix => `${prefix}_test`,
} satisfies SiteBuilderRuntime

const createSession = async (
  store: SaveVersionStore,
  siteId = 'site_1',
): Promise<void> => {
  await Effect.runPromise(
    createSiteBuilderSession(
      db(store),
      {
        createdByActorRef: 'agent:adjutant',
        id: 'site_builder_session_1',
        idempotencyKey: `session:${siteId}`,
        orderId: 'order_1',
        ownerUserId: 'user_owner',
        promptSummary: 'Build a reviewable Site.',
        siteId,
      },
      builderRuntime,
    ),
  )
}

const saveInput = {
  artifactRef: 'r2://builder/session/artifact.zip',
  buildCommand: 'bun run build',
  buildLogText: 'Build completed.',
  buildReceiptRef: 'receipt:builder:1',
  idempotencyKey: 'builder-save:session-1:preview-1',
  notes: 'Ready for customer review.',
  previewId: 'preview_1',
  sessionId: 'site_builder_session_1',
  siteId: 'site_1',
  siteMetadata: {
    siteJson: {
      audience: 'customer',
      title: 'OTEC',
    },
  },
  sourceArchiveText: 'index.html',
  sourceCommitSha: 'abc123',
  sourceHash: 'sha256:source',
  staticAssetsManifest: {
    assets: {
      '/index.html': {
        contentType: 'text/html; charset=utf-8',
        r2Key: 'sites/site_1/builder/index.html',
      },
    },
  },
}

describe('saveSiteBuilderVersion', () => {
  test('saves builder output as a reviewable site version without deployment', async () => {
    const store = new SaveVersionStore()
    const bucket = new ArtifactBucket()
    await createSession(store)

    const result = await Effect.runPromise(
      saveSiteBuilderVersion(
        db(store),
        bucket,
        saveInput,
        sitesRuntime,
        builderRuntime,
      ),
    )

    expect(result.version?.id).toBe('site_version_test')
    expect(result.savedVersion.siteVersionId).toBe('site_version_test')
    expect(store.site_versions).toHaveLength(1)
    expect(store.site_deployments).toHaveLength(0)
    expect(store.site_builder_saved_versions).toHaveLength(1)
    expect(store.site_builder_events).toHaveLength(1)
    expect(store.site_builder_events[0]?.visibility).toBe('customer')
    expect(store.site_events[0]?.type).toBe('site_version.saved')

    const metadata = JSON.parse(String(store.site_versions[0]?.metadata_json))
    expect(metadata.builder.sessionId).toBe('site_builder_session_1')
    expect(metadata.builder.orderId).toBe('order_1')
    expect(metadata.builder.buildReceiptRef).toBe('receipt:builder:1')
    expect(metadata.siteJson.title).toBe('OTEC')
  })

  test('reuses saved mapping for duplicate idempotency keys', async () => {
    const store = new SaveVersionStore()
    const bucket = new ArtifactBucket()
    await createSession(store)

    await Effect.runPromise(
      saveSiteBuilderVersion(
        db(store),
        bucket,
        saveInput,
        sitesRuntime,
        builderRuntime,
      ),
    )
    const replay = await Effect.runPromise(
      saveSiteBuilderVersion(
        db(store),
        bucket,
        saveInput,
        sitesRuntime,
        builderRuntime,
      ),
    )

    expect(replay.version).toBeNull()
    expect(replay.savedVersion.siteVersionId).toBe('site_version_test')
    expect(store.site_versions).toHaveLength(1)
    expect(store.site_builder_saved_versions).toHaveLength(1)
  })

  test('rejects saving a session linked to a different site', async () => {
    const store = new SaveVersionStore()
    await createSession(store, 'site_2')

    const result = await Effect.runPromiseExit(
      saveSiteBuilderVersion(
        db(store),
        new ArtifactBucket(),
        saveInput,
        sitesRuntime,
        builderRuntime,
      ),
    )

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(String(result.cause)).toContain(
        SiteBuilderSessionValidationError.name,
      )
    }
    expect(store.site_versions).toHaveLength(0)
  })
})
