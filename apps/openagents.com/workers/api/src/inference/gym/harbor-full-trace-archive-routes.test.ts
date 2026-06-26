import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleOperatorHarborFullTraceArchivesApi } from './harbor-full-trace-archive-routes'
import {
  type HarborFullTraceArchiveObject,
  type HarborFullTraceArchivePutInput,
  type HarborFullTraceArchiveRecord,
  type HarborFullTraceArchiveStore,
  harborFullTraceArchiveR2Key,
  makeD1R2HarborFullTraceArchiveStore,
} from './harbor-full-trace-archive-store'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

const digest =
  '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
const runRef = 'run.gym.terminal_bench.khala-live'
const jobRef = 'job.gym.harbor_terminal_bench.khala-tb-1782410587'
const nowIso = '2026-06-26T08:00:00.000Z'

const streamText = async (body: ReadableStream): Promise<string> =>
  new Response(body).text()

const requestBodyText = async (value: unknown): Promise<string> => {
  if (typeof value === 'string') {
    return value
  }
  if (value instanceof ReadableStream) {
    return streamText(value)
  }
  if (value instanceof ArrayBuffer) {
    return new TextDecoder().decode(value)
  }
  if (value instanceof Uint8Array) {
    return new TextDecoder().decode(value)
  }
  return String(value)
}

class MemoryArchiveStore implements HarborFullTraceArchiveStore {
  readonly bodies = new Map<string, string>()
  readonly records = new Map<string, HarborFullTraceArchiveRecord>()

  async listArchives(input?: Readonly<{ limit?: number; runRef?: string }>) {
    const records = [...this.records.values()].filter(record =>
      input?.runRef === undefined ? true : record.runRef === input.runRef,
    )
    return records.slice(0, input?.limit ?? records.length)
  }

  async putArchive(input: HarborFullTraceArchivePutInput) {
    const existing = this.records.get(input.archiveRef)
    if (existing !== undefined) {
      return { created: false, record: existing }
    }

    const artifactR2Key = harborFullTraceArchiveR2Key(input)
    const record: HarborFullTraceArchiveRecord = {
      archiveRef: input.archiveRef,
      runRef: input.runRef,
      jobRef: input.jobRef,
      sourceKind: 'harbor_job_tarball',
      artifactR2Key,
      artifactSha256: input.artifactSha256,
      artifactBytes: input.artifactBytes,
      contentType: input.contentType,
      captureStartedAt: input.captureStartedAt,
      captureCompletedAt: input.captureCompletedAt,
      visibility: 'operator_only',
      containsRawPrompts: true,
      containsRawLogs: true,
      containsPrivateMaterial: true,
      demandKind: 'internal',
      demandSource: 'harbor_terminal_bench',
      createdAt: input.captureCompletedAt,
      updatedAt: input.captureCompletedAt,
    }
    this.records.set(input.archiveRef, record)
    this.bodies.set(input.archiveRef, await streamText(input.body))
    return { created: true, record }
  }

  async readArchive(archiveRef: string) {
    return this.records.get(archiveRef)
  }

  async readArchiveObject(
    archiveRef: string,
  ): Promise<HarborFullTraceArchiveObject | undefined> {
    const record = this.records.get(archiveRef)
    const body = this.bodies.get(archiveRef)
    if (record === undefined || body === undefined) {
      return undefined
    }
    return {
      body: new Response(body).body!,
      contentType: record.contentType,
      record,
      size: new TextEncoder().encode(body).byteLength,
    }
  }
}

const uploadRequest = (
  body = 'tarball-bytes',
  headers: Record<string, string> = {},
) =>
  new Request('https://openagents.com/api/operator/gym/full-trace-archives', {
    body,
    headers: {
      'content-type': 'application/gzip',
      'x-openagents-archive-bytes': String(
        new TextEncoder().encode(body).byteLength,
      ),
      'x-openagents-archive-sha256': digest,
      'x-openagents-capture-completed-at': nowIso,
      'x-openagents-job-ref': jobRef,
      'x-openagents-run-ref': runRef,
      ...headers,
    },
    method: 'POST',
  })

describe('operator Harbor full trace archive route', () => {
  test('rejects upload without an admin token', async () => {
    const store = new MemoryArchiveStore()
    const response = await run(
      handleOperatorHarborFullTraceArchivesApi(uploadRequest(), {
        requireAdminApiToken: () => Promise.resolve(false),
        store,
      }),
    )

    expect(response.status).toBe(401)
    expect(await store.listArchives()).toEqual([])
  })

  test('stores a bounded private archive upload for an admin', async () => {
    const store = new MemoryArchiveStore()
    const response = await run(
      handleOperatorHarborFullTraceArchivesApi(uploadRequest(), {
        nowIso: () => nowIso,
        requireAdminApiToken: () => Promise.resolve(true),
        store,
      }),
    )

    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      archive: {
        artifactBytes: number
        archiveRef: string
        containsPrivateMaterial: boolean
        containsRawLogs: boolean
        containsRawPrompts: boolean
        demandSource: string
        downloadUrl: string
        visibility: string
      }
      created: boolean
      kind: string
    }
    expect(body.kind).toBe('harbor_full_trace_archive_stored')
    expect(body.created).toBe(true)
    expect(body.archive.archiveRef).toBe(
      `archive.gym.harbor_full_trace.${digest.slice(0, 32)}`,
    )
    expect(body.archive.artifactBytes).toBe(13)
    expect(body.archive.visibility).toBe('operator_only')
    expect(body.archive.containsRawPrompts).toBe(true)
    expect(body.archive.containsRawLogs).toBe(true)
    expect(body.archive.containsPrivateMaterial).toBe(true)
    expect(body.archive.demandSource).toBe('harbor_terminal_bench')
    expect(body.archive.downloadUrl).toContain('download=1')
    expect(store.bodies.get(body.archive.archiveRef)).toBe('tarball-bytes')
  })

  test('rejects unsafe archive refs before storage', async () => {
    const store = new MemoryArchiveStore()
    const response = await run(
      handleOperatorHarborFullTraceArchivesApi(
        uploadRequest('tarball-bytes', {
          'x-openagents-archive-ref': '../private/raw/path',
        }),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )

    expect(response.status).toBe(400)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).toContain('archive-ref')
    expect(await store.listArchives()).toEqual([])
  })

  test('lists metadata and downloads bytes only for an admin', async () => {
    const store = new MemoryArchiveStore()
    const stored = await store.putArchive({
      archiveRef: 'archive.gym.harbor_full_trace.manual',
      artifactBytes: 12,
      artifactSha256: digest,
      body: new Response('tarball-body').body!,
      captureCompletedAt: nowIso,
      captureStartedAt: null,
      contentType: 'application/gzip',
      jobRef,
      runRef,
    })

    const listResponse = await run(
      handleOperatorHarborFullTraceArchivesApi(
        new Request(
          'https://openagents.com/api/operator/gym/full-trace-archives?run_ref=run.gym.terminal_bench.khala-live',
        ),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    expect(listResponse.status).toBe(200)
    const listBody = (await listResponse.json()) as {
      archives: ReadonlyArray<{ archiveRef: string; artifactR2Key: string }>
    }
    expect(listBody.archives).toHaveLength(1)
    expect(listBody.archives[0]?.archiveRef).toBe(stored.record.archiveRef)
    expect(listBody.archives[0]?.artifactR2Key).toContain(
      'private/gym/harbor-full-trace-archives/',
    )

    const downloadResponse = await run(
      handleOperatorHarborFullTraceArchivesApi(
        new Request(
          `https://openagents.com/api/operator/gym/full-trace-archives?archive_ref=${stored.record.archiveRef}&download=1`,
        ),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    expect(downloadResponse.status).toBe(200)
    expect(downloadResponse.headers.get('x-openagents-archive-sha256')).toBe(
      digest,
    )
    expect(await downloadResponse.text()).toBe('tarball-body')
  })
})

type Row = Readonly<{
  archive_ref: string
  run_ref: string
  job_ref: string
  source_kind: string
  artifact_r2_key: string
  artifact_sha256: string
  artifact_bytes: number
  content_type: string
  capture_started_at: string | null
  capture_completed_at: string
  visibility: string
  contains_raw_prompts: number
  contains_raw_logs: number
  contains_private_material: number
  demand_kind: string
  demand_source: string
  created_at: string
  updated_at: string
}>

const makeFakeD1 = (): D1Database & { rows: Array<Row> } => {
  const rows: Array<Row> = []

  const statement = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt: D1PreparedStatement = {
      all: async <T>() => {
        const limit = Number(bound.at(-1) ?? 50)
        const filtered = query.includes('WHERE run_ref = ?')
          ? rows.filter(row => row.run_ref === bound[0])
          : rows
        return {
          meta: {} as D1Meta & Record<string, unknown>,
          results: filtered.slice(0, limit) as unknown as Array<T>,
          success: true as const,
        }
      },
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T>() => {
        const value = String(bound[0])
        const row = query.includes('artifact_sha256')
          ? rows.find(candidate => candidate.artifact_sha256 === value)
          : rows.find(candidate => candidate.archive_ref === value)
        return (row ?? null) as T | null
      },
      raw: async () => [] as never,
      run: async <T>() => {
        const [
          archive_ref,
          run_ref,
          job_ref,
          source_kind,
          artifact_r2_key,
          artifact_sha256,
          artifact_bytes,
          content_type,
          capture_started_at,
          capture_completed_at,
          visibility,
          contains_raw_prompts,
          contains_raw_logs,
          contains_private_material,
          demand_kind,
          demand_source,
          created_at,
          updated_at,
        ] = bound as [
          string,
          string,
          string,
          string,
          string,
          string,
          number,
          string,
          string | null,
          string,
          string,
          number,
          number,
          number,
          string,
          string,
          string,
          string,
        ]
        rows.push({
          archive_ref,
          run_ref,
          job_ref,
          source_kind,
          artifact_r2_key,
          artifact_sha256,
          artifact_bytes,
          content_type,
          capture_started_at,
          capture_completed_at,
          visibility,
          contains_raw_prompts,
          contains_raw_logs,
          contains_private_material,
          demand_kind,
          demand_source,
          created_at,
          updated_at,
        })
        return {
          meta: { changes: 1 } as D1Meta & Record<string, unknown>,
          results: [] as unknown as Array<T>,
          success: true as const,
        }
      },
    }
    return stmt
  }

  return {
    batch: async () => [] as never,
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => statement(query),
    rows,
    withSession: () => {
      throw new Error('not implemented')
    },
  } as unknown as D1Database & { rows: Array<Row> }
}

class MemoryR2Bucket {
  readonly objects = new Map<
    string,
    Readonly<{
      body: string
      customMetadata: Record<string, string> | undefined
      httpMetadata: R2HTTPMetadata | undefined
      size: number
    }>
  >()

  async head(key: string) {
    return this.objects.has(key) ? ({ key } as R2Object) : null
  }

  async put(key: string, value: unknown, options?: R2PutOptions) {
    const body = await requestBodyText(value)
    this.objects.set(key, {
      body,
      customMetadata: options?.customMetadata,
      httpMetadata:
        options?.httpMetadata instanceof Headers
          ? undefined
          : options?.httpMetadata,
      size: new TextEncoder().encode(body).byteLength,
    })
    return { key } as R2Object
  }

  async get(key: string) {
    const object = this.objects.get(key)
    if (object === undefined) {
      return null
    }
    return {
      body: new Response(object.body).body!,
      httpEtag: 'etag',
      httpMetadata: object.httpMetadata,
      key,
      size: object.size,
    } as R2ObjectBody
  }
}

describe('D1/R2 Harbor full trace archive store', () => {
  test('stores metadata, private R2 bytes, and returns existing records idempotently', async () => {
    const db = makeFakeD1()
    const bucket = new MemoryR2Bucket()
    const store = makeD1R2HarborFullTraceArchiveStore(
      db as unknown as D1Database,
      bucket as unknown as R2Bucket,
    )
    const input: HarborFullTraceArchivePutInput = {
      archiveRef: 'archive.gym.harbor_full_trace.store',
      artifactBytes: 12,
      artifactSha256: digest,
      body: new Response('archive-body').body!,
      captureCompletedAt: nowIso,
      captureStartedAt: '2026-06-26T07:59:00.000Z',
      contentType: 'application/gzip',
      jobRef,
      runRef,
    }

    const first = await store.putArchive(input)
    const second = await store.putArchive(input)

    expect(first.created).toBe(true)
    expect(second.created).toBe(false)
    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      archive_ref: input.archiveRef,
      artifact_sha256: digest,
      contains_private_material: 1,
      contains_raw_logs: 1,
      contains_raw_prompts: 1,
      demand_kind: 'internal',
      demand_source: 'harbor_terminal_bench',
      visibility: 'operator_only',
    })
    expect(first.record.artifactR2Key).toContain(
      'private/gym/harbor-full-trace-archives/',
    )
    expect(bucket.objects.get(first.record.artifactR2Key)?.body).toBe(
      'archive-body',
    )
    expect(
      bucket.objects.get(first.record.artifactR2Key)?.customMetadata,
    ).toMatchObject({
      archiveRef: input.archiveRef,
      demandSource: 'harbor_terminal_bench',
      runRef,
    })
  })
})
