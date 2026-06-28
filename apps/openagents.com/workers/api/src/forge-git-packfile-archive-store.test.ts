import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  FORGE_GIT_PACKFILE_CONTENT_TYPE,
  forgeGitPackfileObjectFormatForCapabilities,
  makeD1R2ForgeGitPackfileArchiveStore,
  type ForgeGitPackfileArchiveStore,
} from './forge-git-packfile-archive-store'

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

  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }

  async all<T = Record<string, unknown>>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ success: true; results: [] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { results: [], success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

class MemoryR2Bucket {
  readonly objects = new Map<
    string,
    Readonly<{
      body: Uint8Array
      customMetadata: Record<string, string> | undefined
      httpMetadata: R2HTTPMetadata | undefined
      sha256: unknown
      size: number
    }>
  >()

  async head(key: string) {
    return this.objects.has(key) ? ({ key } as R2Object) : null
  }

  async put(key: string, value: unknown, options?: R2PutOptions) {
    const body = await bodyBytes(value)
    this.objects.set(key, {
      body,
      customMetadata: options?.customMetadata,
      httpMetadata:
        options?.httpMetadata instanceof Headers
          ? undefined
          : options?.httpMetadata,
      sha256: options?.sha256,
      size: body.byteLength,
    })
    return { key } as R2Object
  }

  async get(key: string) {
    const object = this.objects.get(key)
    if (object === undefined) {
      return null
    }
    return {
      body: new Response(object.body.slice().buffer as ArrayBuffer).body!,
      httpEtag: '"memory"',
      httpMetadata: object.httpMetadata,
      key,
      size: object.size,
    } as R2ObjectBody
  }
}

const bodyBytes = async (value: unknown): Promise<Uint8Array> => {
  if (typeof value === 'string') {
    return new TextEncoder().encode(value)
  }
  if (value instanceof Uint8Array) {
    return value
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value)
  }
  if (value instanceof Blob) {
    return new Uint8Array(await value.arrayBuffer())
  }
  if (value instanceof ReadableStream) {
    return new Uint8Array(await new Response(value).arrayBuffer())
  }
  throw new Error('unsupported memory R2 test body')
}

const migration = readFileSync(
  new URL('../migrations/0252_forge_git_packfile_archives.sql', import.meta.url),
  'utf8',
)

const makeStore = (): {
  bucket: MemoryR2Bucket
  store: ForgeGitPackfileArchiveStore
} => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration)
  const bucket = new MemoryR2Bucket()
  return {
    bucket,
    store: makeD1R2ForgeGitPackfileArchiveStore(
      new SqliteD1(db) as unknown as D1Database,
      bucket as unknown as R2Bucket,
    ),
  }
}

const packBytes = new TextEncoder().encode('PACKfixture-body')
const packSha256 = createHash('sha256').update(packBytes).digest('hex')
const nowIso = '2026-06-28T17:00:00.000Z'

describe('forge git packfile archive D1/R2 store', () => {
  test('stores private R2 bytes and metadata in D1 idempotently', async () => {
    const { bucket, store } = makeStore()
    const input = {
      body: packBytes.buffer,
      capabilities: ['report-status', 'object-format=sha1'],
      changeRef: 'change.forge.6748',
      objectFormat: forgeGitPackfileObjectFormatForCapabilities([
        'report-status',
        'object-format=sha1',
      ]),
      packfileBytes: packBytes.byteLength,
      packfileRef: 'packfile.forge.6748.main',
      packfileSha256: packSha256,
      receivePackRef: 'receive-pack.forge.6748.main',
      refUpdates: [
        {
          action: 'update' as const,
          newObjectId: 'b'.repeat(40),
          oldObjectId: 'a'.repeat(40),
          refName: 'refs/heads/main',
        },
      ],
      repositoryRef: 'repo.openagents.openagents',
      sourceRefs: ['github:OpenAgentsInc/openagents#6748'],
      tenantRef: 'tenant.openagents',
      nowIso,
    }

    const first = await store.putPackfile(input)
    const second = await store.putPackfile(input)

    expect(first.created).toBe(true)
    expect(second.created).toBe(false)
    expect(first.record.artifact_r2_key).toContain(
      'private/forge/git-packfiles/tenant.openagents/repo.openagents.openagents/',
    )
    expect(first.record.packfile_sha256).toBe(packSha256)
    expect(first.record.content_type).toBe(FORGE_GIT_PACKFILE_CONTENT_TYPE)
    expect(JSON.parse(first.record.ref_updates_json)).toEqual(input.refUpdates)
    expect(bucket.objects).toHaveLength(1)
    expect(bucket.objects.get(first.record.artifact_r2_key)?.customMetadata).toMatchObject({
      packfileRef: input.packfileRef,
      packfileSha256: packSha256,
      repositoryRef: input.repositoryRef,
      visibility: 'operator_only',
    })
    expect(bucket.objects.get(first.record.artifact_r2_key)?.sha256).toBe(
      packSha256,
    )
  })

  test('deduplicates by tenant-scoped SHA-256 digest', async () => {
    const { bucket, store } = makeStore()
    const first = await store.putPackfile({
      body: packBytes.buffer,
      capabilities: ['object-format=sha1'],
      objectFormat: 'sha1',
      packfileBytes: packBytes.byteLength,
      packfileRef: 'packfile.forge.first',
      packfileSha256: packSha256,
      refUpdates: [],
      repositoryRef: 'repo.openagents.openagents',
      sourceRefs: [],
      tenantRef: 'tenant.openagents',
      nowIso,
    })
    const second = await store.putPackfile({
      body: packBytes.buffer,
      capabilities: ['object-format=sha1'],
      objectFormat: 'sha1',
      packfileBytes: packBytes.byteLength,
      packfileRef: 'packfile.forge.second',
      packfileSha256: packSha256,
      refUpdates: [],
      repositoryRef: 'repo.openagents.openagents',
      sourceRefs: [],
      tenantRef: 'tenant.openagents',
      nowIso,
    })

    expect(first.created).toBe(true)
    expect(second.created).toBe(false)
    expect(second.record.packfile_ref).toBe('packfile.forge.first')
    expect(bucket.objects).toHaveLength(1)
  })

  test('reads archived packfile objects and repository-scoped rows', async () => {
    const { store } = makeStore()
    const stored = await store.putPackfile({
      body: packBytes.buffer,
      capabilities: ['object-format=sha256'],
      objectFormat: 'sha256',
      packfileBytes: packBytes.byteLength,
      packfileRef: 'packfile.forge.readback',
      packfileSha256: packSha256,
      refUpdates: [],
      repositoryRef: 'repo.openagents.openagents',
      sourceRefs: [],
      tenantRef: 'tenant.openagents',
      nowIso,
    })

    const object = await store.readPackfileObject(
      'tenant.openagents',
      stored.record.packfile_ref,
    )
    const listed = await store.listPackfiles('tenant.openagents', {
      repositoryRef: 'repo.openagents.openagents',
    })

    expect(object?.contentType).toBe(FORGE_GIT_PACKFILE_CONTENT_TYPE)
    expect(object?.size).toBe(packBytes.byteLength)
    expect(listed.map(row => row.packfile_ref)).toEqual([
      'packfile.forge.readback',
    ])
  })
})
