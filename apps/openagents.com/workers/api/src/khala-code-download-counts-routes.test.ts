import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KHALA_CODE_DOWNLOAD_COUNTS_ENDPOINT,
  handlePublicKhalaCodeDownloadCountsApi,
  khalaCodeDownloadCountRowFromSql,
  makeD1KhalaCodeDownloadCountsStore,
  type KhalaCodeDownloadCountRow,
  type KhalaCodeDownloadCountsStore,
} from './khala-code-download-counts-routes'

const nowIso = '2026-07-04T12:00:00.000Z'

const request = (method = 'GET'): Request =>
  new Request(`https://openagents.com${KHALA_CODE_DOWNLOAD_COUNTS_ENDPOINT}`, {
    method,
  })

const storeWithRows = (
  rows: ReadonlyArray<KhalaCodeDownloadCountRow>,
): KhalaCodeDownloadCountsStore => ({
  readCounts: () => Effect.succeed(rows),
})

const storeFailure = (error: unknown): KhalaCodeDownloadCountsStore => ({
  readCounts: () => Effect.fail(error),
})

const fakeDownloadCountsDb = (
  rows: ReadonlyArray<Record<string, unknown>>,
): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    all: <T>(): Promise<{ results: ReadonlyArray<T> }> =>
      Promise.resolve({ results: rows as ReadonlyArray<T> }),
  })

  return { prepare } as unknown as D1Database
}

describe('GET /api/public/khala-code/download-counts', () => {
  test('returns exact public-safe aggregate rows from the store', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaCodeDownloadCountsApi(request(), {
        nowIso: () => nowIso,
        store: storeWithRows([
          {
            artifactKind: 'npm_cli',
            channel: 'stable',
            exactRows: 7,
            lastCountedAt: '2026-07-04T10:00:00.000Z',
          },
        ]),
      }),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as {
      schemaVersion: string
      product: string
      promiseId: string
      counts: ReadonlyArray<KhalaCodeDownloadCountRow>
      blockerRefs: ReadonlyArray<string>
      sourceRefs: ReadonlyArray<string>
      staleness: { composition: string; maxStalenessSeconds: number }
    }

    expect(body.schemaVersion).toBe(
      'openagents.khala_code.public_download_counts.v1',
    )
    expect(body.product).toBe('khala-code')
    expect(body.promiseId).toBe('khala_code.desktop_codex_wrapper.v1')
    expect(body.counts).toEqual([
      {
        artifactKind: 'npm_cli',
        channel: 'stable',
        exactRows: 7,
        lastCountedAt: '2026-07-04T10:00:00.000Z',
      },
    ])
    expect(body.blockerRefs).toEqual([])
    expect(body.sourceRefs).toEqual(['table:khala_code_download_events'])
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
  })

  test('returns counts: [] rather than a synthesized number when no rows exist', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaCodeDownloadCountsApi(request(), {
        nowIso: () => nowIso,
        store: storeWithRows([]),
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      counts: ReadonlyArray<unknown>
      blockerRefs: ReadonlyArray<string>
    }
    expect(body.counts).toEqual([])
    expect(body.blockerRefs).toContain(
      'blocker.public.khala_code_download_counts.no_rows',
    )
  })

  test('returns counts: [] rather than a synthesized number when the table is missing', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaCodeDownloadCountsApi(request(), {
        nowIso: () => nowIso,
        store: storeFailure(
          new Error('D1_ERROR: no such table: khala_code_download_events'),
        ),
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      counts: ReadonlyArray<unknown>
      blockerRefs: ReadonlyArray<string>
      sourceRefs: ReadonlyArray<string>
    }
    expect(body.counts).toEqual([])
    expect(body.blockerRefs).toContain(
      'blocker.public.khala_code_download_counts.table_missing',
    )
    expect(body.sourceRefs).toEqual([
      'table:khala_code_download_events:missing',
    ])
  })

  test('production D1 store normalizes count rows and skips invalid rows', async () => {
    const store = makeD1KhalaCodeDownloadCountsStore(
      fakeDownloadCountsDb([
        {
          artifact_kind: 'npm_cli',
          channel: 'stable',
          download_count: '5',
          last_counted_at: '2026-07-04T09:00:00.000Z',
        },
        {
          artifact_kind: 'desktop_dmg',
          channel: 'rc',
          download_count: 2,
          last_counted_at: null,
        },
        {
          artifact_kind: 'desktop_dmg',
          channel: '',
          download_count: 999,
          last_counted_at: null,
        },
      ]),
    )

    const rows = await Effect.runPromise(store.readCounts())

    expect(rows).toEqual([
      {
        artifactKind: 'desktop_dmg',
        channel: 'rc',
        exactRows: 2,
        lastCountedAt: null,
      },
      {
        artifactKind: 'npm_cli',
        channel: 'stable',
        exactRows: 5,
        lastCountedAt: '2026-07-04T09:00:00.000Z',
      },
    ])
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaCodeDownloadCountsApi(request('POST'), {
        nowIso: () => nowIso,
        store: storeWithRows([]),
      }),
    )

    expect(response.status).toBe(405)
  })

  test('row normalization rejects unknown artifact kinds and synthetic counts', () => {
    expect(
      khalaCodeDownloadCountRowFromSql({
        artifact_kind: 'other',
        channel: 'stable',
        download_count: 1,
        last_counted_at: null,
      }),
    ).toBeUndefined()
    expect(
      khalaCodeDownloadCountRowFromSql({
        artifact_kind: 'npm_cli',
        channel: 'stable',
        download_count: -1,
        last_counted_at: null,
      }),
    ).toBeUndefined()
  })
})
