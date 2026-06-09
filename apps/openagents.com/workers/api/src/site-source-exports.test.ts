import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  recordSiteSourceExport,
  type SiteSourceExportRuntime,
} from './site-source-exports'

type Row = Record<string, unknown>

class SourceExportStore {
  exports: Array<Row> = []
  versions: Array<Row> = [
    {
      artifact_manifest_r2_key: 'site-artifacts/site_version_1/manifest.json',
      build_status: 'saved',
      id: 'site_version_1',
      site_id: 'site_project_1',
      source_archive_r2_key: 'site-source/site_version_1/source.txt',
      worker_module_r2_key: null,
    },
  ]
}

class SourceExportStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: SourceExportStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_source_exports')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.exports.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM site_versions')) {
      const versionId = String(this.values[0])
      const siteId = String(this.values[1])
      const row =
        this.store.versions.find(
          item => item.id === versionId && item.site_id === siteId,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first query: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO site_source_exports')) {
      this.store.exports.push({
        actor_user_id: this.values[6] as string | null,
        approved_by_user_id: this.values[7] as string | null,
        archived_at: null,
        artifact_manifest_r2_key: this.values[15] as string | null,
        created_at: String(this.values[24]),
        destination_branch: this.values[11] as string | null,
        destination_owner: this.values[9] as string | null,
        destination_provider: String(this.values[8]),
        destination_pull_request_url: this.values[12] as string | null,
        destination_repository: this.values[10] as string | null,
        destination_url: this.values[13] as string | null,
        export_kind: String(this.values[5]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        receipt_json: String(this.values[23]),
        secret_scan_ref: this.values[22] as string | null,
        secret_scan_status: String(this.values[21]),
        site_id: String(this.values[2]),
        source_archive_r2_key: this.values[14] as string | null,
        source_artifact_ref: this.values[17] as string | null,
        status: String(this.values[4]),
        token_expires_at: this.values[20] as string | null,
        token_hash: this.values[19] as string | null,
        token_ref: this.values[18] as string | null,
        updated_at: String(this.values[25]),
        version_id: String(this.values[3]),
        worker_module_r2_key: this.values[16] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run query: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected all query: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return Promise.reject(new Error(`Unexpected raw query: ${this.query}`))
  }
}

const dbFor = (store: SourceExportStore): D1Database => ({
  batch: () => Promise.reject(new Error('unused')),
  dump: () => Promise.reject(new Error('unused')),
  exec: () => Promise.reject(new Error('unused')),
  prepare: query => new SourceExportStatement(query, store),
  withSession: () => {
    throw new Error('unused')
  },
})

const runtime: SiteSourceExportRuntime = {
  makeExportId: () => 'site_source_export_1',
  makeTokenHash: () => 'site_source_export_token_hash_1',
  makeTokenRef: () => 'site_source_export_token_1',
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

describe('site source exports', () => {
  test('records an approved GitHub pull request export receipt with expiring token ref', async () => {
    const store = new SourceExportStore()
    const receipt = await Effect.runPromise(
      recordSiteSourceExport(
        dbFor(store),
        {
          actorUserId: 'github:operator',
          approve: true,
          destination: {
            branch: 'openagents/site-v1',
            owner: 'OpenAgentsInc',
            provider: 'github',
            pullRequestUrl: 'https://github.com/OpenAgentsInc/site/pull/1',
            repository: 'site',
          },
          exportKind: 'github_pull_request',
          expiresInSeconds: 3_600,
          idempotencyKey: 'site-source-export:1',
          receipt: { githubExport: 'reviewed' },
          secretScan: {
            scannerRef: 'secret-scan:site-version-1',
            status: 'passed',
            summary: 'No secrets found.',
          },
          siteId: 'site_project_1',
          versionId: 'site_version_1',
        },
        runtime,
      ),
    )

    expect(receipt).toMatchObject({
      approvedByUserId: 'github:operator',
      destination: {
        branch: 'openagents/site-v1',
        owner: 'OpenAgentsInc',
        provider: 'github',
        pullRequestUrl: 'https://github.com/OpenAgentsInc/site/pull/1',
        repository: 'site',
      },
      exportKind: 'github_pull_request',
      sourceArchiveR2Key: 'site-source/site_version_1/source.txt',
      status: 'approved',
      tokenExpiresAt: '2026-06-05T13:00:00.000Z',
      tokenRef: 'site_source_export_token_1',
    })
    expect(receipt).not.toHaveProperty('tokenHash')
    expect(store.exports[0]).toMatchObject({
      token_hash: 'site_source_export_token_hash_1',
    })
  })

  test('is idempotent by key', async () => {
    const store = new SourceExportStore()
    const input = {
      destination: { provider: 'download' as const },
      exportKind: 'download_token' as const,
      idempotencyKey: 'site-source-export:idempotent',
      secretScan: { status: 'passed' as const },
      siteId: 'site_project_1',
      versionId: 'site_version_1',
    }

    const first = await Effect.runPromise(
      recordSiteSourceExport(dbFor(store), input, runtime),
    )
    const second = await Effect.runPromise(
      recordSiteSourceExport(dbFor(store), input, {
        ...runtime,
        makeExportId: () => 'site_source_export_2',
      }),
    )

    expect(first.id).toBe(second.id)
    expect(store.exports).toHaveLength(1)
  })

  test('rejects failed secret scans and secret-shaped receipts', async () => {
    const store = new SourceExportStore()

    await expect(
      Effect.runPromise(
        recordSiteSourceExport(dbFor(store), {
          destination: { provider: 'download' },
          exportKind: 'download_token',
          idempotencyKey: 'site-source-export:failed-scan',
          secretScan: { status: 'failed' },
          siteId: 'site_project_1',
          versionId: 'site_version_1',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'SiteSourceExportValidationError',
      reason: 'source export requires a passed secret scan.',
    })

    await expect(
      Effect.runPromise(
        recordSiteSourceExport(dbFor(store), {
          destination: { provider: 'download' },
          exportKind: 'download_token',
          idempotencyKey: 'site-source-export:secret-receipt',
          receipt: { access_token: 'secret' },
          secretScan: { status: 'passed' },
          siteId: 'site_project_1',
          versionId: 'site_version_1',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'SiteSourceExportValidationError',
    })
  })

  test('rejects missing versions and versions with no exportable source refs', async () => {
    const store = new SourceExportStore()
    store.versions = [
      {
        artifact_manifest_r2_key: null,
        build_status: 'saved',
        id: 'site_version_empty',
        site_id: 'site_project_1',
        source_archive_r2_key: null,
        worker_module_r2_key: null,
      },
    ]

    await expect(
      Effect.runPromise(
        recordSiteSourceExport(dbFor(store), {
          destination: { provider: 'download' },
          exportKind: 'download_token',
          idempotencyKey: 'site-source-export:not-found',
          secretScan: { status: 'passed' },
          siteId: 'site_project_1',
          versionId: 'site_version_missing',
        }),
      ),
    ).rejects.toMatchObject({
      reason: 'site version was not found.',
    })

    await expect(
      Effect.runPromise(
        recordSiteSourceExport(dbFor(store), {
          destination: { provider: 'download' },
          exportKind: 'download_token',
          idempotencyKey: 'site-source-export:no-source',
          secretScan: { status: 'passed' },
          siteId: 'site_project_1',
          versionId: 'site_version_empty',
        }),
      ),
    ).rejects.toMatchObject({
      reason:
        'source export requires a source archive, artifact manifest, worker module, or explicit source artifact ref.',
    })
  })
})
