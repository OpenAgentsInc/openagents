import { Buffer } from 'node:buffer'
import { execFile, execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { promisify } from 'node:util'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeD1ForgeCoordinationStore } from './forge-coordination-store'
import { makeD1ForgeGitCanonicalStore } from './forge-git-canonical-store'
import {
  FORGE_GIT_RECEIVE_PACK_ADVERTISEMENT_CONTENT_TYPE,
  FORGE_GIT_RECEIVE_PACK_REQUEST_CONTENT_TYPE,
  FORGE_GIT_RECEIVE_PACK_RESULT_CONTENT_TYPE,
  makeForgeGitIntakeRoutes,
} from './forge-git-intake-routes'
import { makeD1R2ForgeGitPackfileArchiveStore } from './forge-git-packfile-archive-store'
import { makeD1ForgeTenantGitAuthStore } from './forge-tenant-git-auth-store'

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

  async run(): Promise<{ success: true; results: []; meta: { changes: number } }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, results: [], success: true }
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

const migration = (name: string): string =>
  readFileSync(new URL(`../migrations/${name}`, import.meta.url), 'utf8')

const migrations = [
  '0251_forge_coordination_source_of_truth.sql',
  '0252_forge_git_packfile_archives.sql',
  '0253_forge_tenant_git_access_tokens.sql',
  '0254_forge_control_plane_receipts.sql',
  '0255_forge_git_canonical_store.sql',
].map(migration)

const textEncoder = new TextEncoder()
const execFileAsync = promisify(execFile)
const zeroSha1 = '0'.repeat(40)
const nowIso = '2026-06-28T18:00:00.000Z'
const tenantRef = 'tenant.openagents'
const repositoryRef = 'repo.openagents.openagents'
const subjectRef = 'agent.forge.test'
const gitToken = 'oa_forge_git_0123456789abcdef0123456789abcdef0123456789abcdef'

const pktLine = (payload: string): Uint8Array => {
  const payloadBytes = textEncoder.encode(payload)
  const header = textEncoder.encode(
    (payloadBytes.byteLength + 4).toString(16).padStart(4, '0'),
  )
  const bytes = new Uint8Array(header.byteLength + payloadBytes.byteLength)
  bytes.set(header, 0)
  bytes.set(payloadBytes, header.byteLength)
  return bytes
}

const concatBytes = (chunks: ReadonlyArray<Uint8Array>): Uint8Array => {
  const bytes = new Uint8Array(
    chunks.reduce((total, chunk) => total + chunk.byteLength, 0),
  )
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  return bytes
}

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer => {
  const copy = new Uint8Array(bytes.byteLength)
  copy.set(bytes)
  return copy.buffer
}

const receivePackBody = (
  input: Readonly<{
    oldObjectId: string
    newObjectId: string
    refName?: string
    packfile?: Uint8Array
  }>,
): Uint8Array =>
  concatBytes([
    pktLine(
      `${input.oldObjectId} ${input.newObjectId} ${
        input.refName ?? 'refs/heads/main'
      }\0report-status object-format=sha1\n`,
    ),
    textEncoder.encode('0000'),
    ...(input.packfile === undefined ? [] : [input.packfile]),
  ])

const packfileBytes = (label: string): Uint8Array =>
  textEncoder.encode(`PACK${label}`)

type Harness = Awaited<ReturnType<typeof makeHarness>>

const makeHarness = async (scope: 'git:receive-pack' | 'git:upload-pack' = 'git:receive-pack') => {
  const sqlite = new DatabaseSync(':memory:')
  sqlite.exec('PRAGMA foreign_keys = ON')
  for (const sql of migrations) {
    sqlite.exec(sql)
  }
  const db = new SqliteD1(sqlite) as unknown as D1Database
  const bucket = new MemoryR2Bucket()
  const authStore = makeD1ForgeTenantGitAuthStore(db)
  const canonicalStore = makeD1ForgeGitCanonicalStore(db)
  const coordinationStore = makeD1ForgeCoordinationStore(db)
  const archiveStore = makeD1R2ForgeGitPackfileArchiveStore(
    db,
    bucket as unknown as R2Bucket,
  )

  await authStore.upsertTenant({
    displayName: 'OpenAgents',
    state: 'active',
    tenantRef,
    nowIso,
  })
  await authStore.mintGitAccessToken(
    {
      expiresAt: '2026-06-29T18:00:00.000Z',
      repositoryRef,
      scopes: [scope],
      sourceRefs: ['issue.public.github.OpenAgentsInc.openagents.6771'],
      subjectRef,
      tenantRef,
      tokenRef: `token.${scope.replace(':', '.')}`,
      nowIso,
    },
    { makeToken: () => gitToken },
  )

  const routes = makeForgeGitIntakeRoutes({
    makeArchiveStore: () => archiveStore,
    makeCanonicalStore: () => canonicalStore,
    makeCoordinationStore: () => coordinationStore,
    makeTenantGitAuthStore: () => authStore,
    nowIso: () => nowIso,
  })
  const run = (request: Request) => {
    const effect = routes.routeForgeGitIntakeRequest(request, {})
    if (effect === undefined) {
      throw new Error(`unmatched Forge git route: ${request.url}`)
    }
    return Effect.runPromise(effect)
  }

  return { archiveStore, bucket, canonicalStore, coordinationStore, run }
}

const gitHeaders = (): HeadersInit => ({
  authorization: `Bearer ${gitToken}`,
  'content-type': FORGE_GIT_RECEIVE_PACK_REQUEST_CONTENT_TYPE,
})

const postReceivePack = (body: Uint8Array): Request =>
  new Request(
    `https://openagents.com/git/${tenantRef}/${repositoryRef}.git/git-receive-pack`,
    {
      body: arrayBufferFromBytes(body),
      headers: gitHeaders(),
      method: 'POST',
    },
  )

const advertiseRequest = (): Request =>
  new Request(
    `https://openagents.com/git/${tenantRef}/${repositoryRef}.git/info/refs?service=git-receive-pack`,
    { headers: gitHeaders() },
  )

const jsonError = async (response: Response): Promise<{ error: string }> =>
  (await response.json()) as { error: string }

const requestFromIncoming = async (
  incoming: import('node:http').IncomingMessage,
  port: number,
): Promise<Request> => {
  const headers = new Headers()
  for (const [name, value] of Object.entries(incoming.headers)) {
    if (Array.isArray(value)) {
      for (const entry of value) {
        headers.append(name, entry)
      }
    } else if (value !== undefined) {
      headers.set(name, value)
    }
  }

  const method = incoming.method ?? 'GET'
  if (method === 'GET' || method === 'HEAD') {
    return new Request(`http://127.0.0.1:${port}${incoming.url ?? '/'}`, {
      headers,
      method,
    })
  }

  const chunks: Buffer[] = []
  for await (const chunk of incoming) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))
  }
  const body =
    chunks.length === 0 ? undefined : Buffer.concat(chunks)

  return new Request(`http://127.0.0.1:${port}${incoming.url ?? '/'}`, {
    ...(body === undefined ? {} : { body: arrayBufferFromBytes(body) }),
    headers,
    method,
  })
}

describe('Forge smart-Git receive-pack intake routes', () => {
  test('advertises receive-pack and accepts a real parsed packfile into R2, canonical refs, and coordination rows', async () => {
    const {
      archiveStore,
      bucket,
      canonicalStore,
      coordinationStore,
      run,
    }: Harness = await makeHarness()
    const advertisement = await run(advertiseRequest())
    await expect(advertisement.text()).resolves.toContain('capabilities^{}')
    expect(advertisement.headers.get('content-type')).toBe(
      FORGE_GIT_RECEIVE_PACK_ADVERTISEMENT_CONTENT_TYPE,
    )

    const newObjectId = 'a'.repeat(40)
    const response = await run(
      postReceivePack(
        receivePackBody({
          newObjectId,
          oldObjectId: zeroSha1,
          packfile: packfileBytes('first'),
        }),
      ),
    )
    const changeRef = response.headers.get('x-openagents-forge-change-ref')
    const packfileRef = response.headers.get('x-openagents-forge-packfile-ref')
    const statusBody = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe(
      FORGE_GIT_RECEIVE_PACK_RESULT_CONTENT_TYPE,
    )
    expect(statusBody).toContain('unpack ok')
    expect(statusBody).toContain('ok refs/heads/main')
    expect(bucket.objects.size).toBe(1)

    const ref = await canonicalStore.readRef(
      tenantRef,
      repositoryRef,
      'refs/heads/main',
    )
    const object = await canonicalStore.readObject(
      tenantRef,
      repositoryRef,
      newObjectId,
    )
    const archive = await archiveStore.readPackfile(tenantRef, packfileRef!)
    const changes = await coordinationStore.listChanges(tenantRef, 10)
    const statuses = await coordinationStore.listStatuses(
      tenantRef,
      10,
      changeRef!,
    )

    expect(ref?.object_id).toBe(newObjectId)
    expect(ref?.state).toBe('active')
    expect(object?.packfile_ref).toBe(packfileRef)
    expect(archive?.packfile_bytes).toBe(packfileBytes('first').byteLength)
    expect(changes.map(change => change.change_ref)).toEqual([changeRef])
    expect(statuses.map(status => status.subject_ref)).toEqual([changeRef])

    const postPushAdvertisement = await run(advertiseRequest())
    await expect(postPushAdvertisement.text()).resolves.toContain(
      `${newObjectId} refs/heads/main`,
    )
  })

  test('rejects tokens without receive-pack scope before archiving', async () => {
    const { bucket, run }: Harness = await makeHarness('git:upload-pack')
    const response = await run(
      postReceivePack(
        receivePackBody({
          newObjectId: 'b'.repeat(40),
          oldObjectId: zeroSha1,
          packfile: packfileBytes('wrong-scope'),
        }),
      ),
    )
    const body = await jsonError(response)

    expect(response.status).toBe(401)
    expect(body.error).toBe('forge_git_unauthorized')
    expect(bucket.objects.size).toBe(0)
  })

  test('malformed pkt-lines fail closed before R2 archive or coordination writes', async () => {
    const { bucket, coordinationStore, run }: Harness = await makeHarness()
    const response = await run(postReceivePack(textEncoder.encode('zzzz')))
    const body = await jsonError(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('forge_git_receive_pack_malformed')
    expect(bucket.objects.size).toBe(0)
    await expect(coordinationStore.listChanges(tenantRef, 10)).resolves.toEqual([])
  })

  test('unsafe stale ref updates fail closed before archiving a second packfile', async () => {
    const { bucket, canonicalStore, run }: Harness = await makeHarness()
    const firstObjectId = 'c'.repeat(40)
    const secondObjectId = 'd'.repeat(40)

    await run(
      postReceivePack(
        receivePackBody({
          newObjectId: firstObjectId,
          oldObjectId: zeroSha1,
          packfile: packfileBytes('first-stale'),
        }),
      ),
    )
    const response = await run(
      postReceivePack(
        receivePackBody({
          newObjectId: secondObjectId,
          oldObjectId: zeroSha1,
          packfile: packfileBytes('stale'),
        }),
      ),
    )
    const body = await jsonError(response)

    expect(response.status).toBe(409)
    expect(body.error).toBe('forge_git_unsafe_ref_update')
    expect(bucket.objects.size).toBe(1)
    await expect(
      canonicalStore.readRef(tenantRef, repositoryRef, 'refs/heads/main'),
    ).resolves.toMatchObject({ object_id: firstObjectId })
  })

  test('delete-only receive-pack poison paths fail closed', async () => {
    const { bucket, coordinationStore, run }: Harness = await makeHarness()
    const response = await run(
      postReceivePack(
        receivePackBody({
          newObjectId: zeroSha1,
          oldObjectId: 'e'.repeat(40),
        }),
      ),
    )
    const body = await jsonError(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('forge_git_delete_only_push_rejected')
    expect(bucket.objects.size).toBe(0)
    await expect(coordinationStore.listChanges(tenantRef, 10)).resolves.toEqual([])
  })

  test('accepts an actual git push over smart HTTP', async () => {
    const {
      bucket,
      canonicalStore,
      coordinationStore,
      run,
    }: Harness = await makeHarness()
    const server = createServer(async (incoming, outgoing) => {
      try {
        const address = server.address()
        const port =
          typeof address === 'object' && address !== null ? address.port : 0
        const request = await requestFromIncoming(incoming, port)
        const response = await run(request)
        const headers: Record<string, string> = {}
        response.headers.forEach((value, key) => {
          headers[key] = value
        })
        outgoing.writeHead(response.status, headers)
        outgoing.end(Buffer.from(await response.arrayBuffer()))
      } catch (error) {
        outgoing.writeHead(500, { 'content-type': 'text/plain' })
        outgoing.end(error instanceof Error ? error.message : String(error))
      }
    })

    await new Promise<void>(resolve => {
      server.listen(0, '127.0.0.1', () => resolve())
    })
    const address = server.address()
    const port =
      typeof address === 'object' && address !== null ? address.port : 0
    const repoDir = mkdtempSync(join(tmpdir(), 'forge-real-push-'))

    try {
      execFileSync('git', ['init'], { cwd: repoDir })
      execFileSync('git', ['config', 'user.email', 'forge@example.test'], {
        cwd: repoDir,
      })
      execFileSync('git', ['config', 'user.name', 'Forge Test'], {
        cwd: repoDir,
      })
      writeFileSync(join(repoDir, 'README.md'), 'hello forge\n')
      execFileSync('git', ['add', 'README.md'], { cwd: repoDir })
      execFileSync('git', ['commit', '-m', 'forge push smoke'], {
        cwd: repoDir,
      })
      const head = execFileSync('git', ['rev-parse', 'HEAD'], {
        cwd: repoDir,
        encoding: 'utf8',
      }).trim()

      await execFileAsync(
        'git',
        [
          '-c',
          `http.extraHeader=Authorization: Bearer ${gitToken}`,
          'push',
          `http://127.0.0.1:${port}/git/${tenantRef}/${repositoryRef}.git`,
          'HEAD:refs/heads/main',
        ],
        {
          cwd: repoDir,
          timeout: 10_000,
        },
      )

      const ref = await canonicalStore.readRef(
        tenantRef,
        repositoryRef,
        'refs/heads/main',
      )
      const changes = await coordinationStore.listChanges(tenantRef, 10)

      expect(bucket.objects.size).toBe(1)
      expect(ref?.object_id).toBe(head)
      expect(changes).toHaveLength(1)
    } finally {
      await new Promise<void>(resolve => {
        server.close(() => resolve())
      })
      rmSync(repoDir, { force: true, recursive: true })
    }
  })
})
