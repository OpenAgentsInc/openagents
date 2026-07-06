/**
 * CFG-9 (#8524): unit coverage for the Cloud Run adapter layer — the typed
 * unavailable-binding degrade, the D1 REST bridge, queue delivery semantics,
 * the Postgres OpenAuth storage adapter, request helpers, and background
 * task tracking. No network, no database: every backend is a fake.
 */

import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { Effect } from 'effect'

import { makeAssetsFetcher, resolveAssetPath } from './assets'
import {
  BindingUnavailableError,
  isBindingUnavailableError,
  responseForBindingUnavailable,
  unavailableBinding,
} from './binding-unavailable'
import { d1FromProcessEnv, makeD1HttpDatabase } from './d1-http'
import { makeUnavailableDurableObjectNamespace } from './do-shims'
import { makeBackgroundTasks, makeExecutionContext } from './execution-context'
import { cronAuthorized, withForwardedProto } from './http-utils'
import { deliverLeasedBatch } from './queue-postgres'
import { joinKey } from '@openauthjs/openauth/storage/storage'
import { makePostgresOpenAuthStorage } from '../auth/openauth-storage-postgres'

describe('binding-unavailable', () => {
  it('rejects per-call with a typed error and maps to a 503', async () => {
    const queue = unavailableBinding<Queue>('RUNNER_EVENTS')
    const error = await (
      queue.send({ hello: true }) as unknown as Promise<void>
    ).then(
      () => undefined,
      caught => caught as BindingUnavailableError,
    )

    expect(error).toBeDefined()
    expect(isBindingUnavailableError(error)).toBe(true)
    expect(error?.binding).toBe('RUNNER_EVENTS')
    expect(error?.operation).toBe('send')

    const response = responseForBindingUnavailable(error!)
    expect(response.status).toBe(503)
    const body = (await response.json()) as { error: string; binding: string }
    expect(body.error).toBe('service_unavailable')
    expect(body.binding).toBe('RUNNER_EVENTS')
  })

  it('is not treated as a thenable', async () => {
    const binding = unavailableBinding<{ then?: unknown }>('X')
    expect(binding.then).toBeUndefined()
    await Promise.resolve(binding)
  })

  it('unavailable DO namespaces build ids/stubs sync and reject on use', async () => {
    const namespace = makeUnavailableDurableObjectNamespace('MDK_SIDECAR')
    const stub = namespace.get(namespace.idFromName('checkout'))
    await expect(
      (stub as { fetch: (url: string) => Promise<Response> }).fetch(
        'https://do/x',
      ),
    ).rejects.toMatchObject({ binding: 'MDK_SIDECAR' })
  })
})

describe('d1-http bridge', () => {
  const envelope = (results: Array<Record<string, unknown>>) =>
    new Response(
      JSON.stringify({
        result: [
          {
            meta: { changes: 1, duration: 0.4, last_row_id: 7 },
            results,
            success: true,
          },
        ],
        success: true,
      }),
      { status: 200 },
    )

  it('maps prepare/bind/all/first/raw over the REST envelope', async () => {
    const calls: Array<{ body: string; url: string }> = []
    const db = makeD1HttpDatabase({
      accountId: 'acct',
      apiToken: 'token',
      databaseId: 'db',
      fetchImpl: (async (
        url: Parameters<typeof fetch>[0],
        init?: Parameters<typeof fetch>[1],
      ) => {
        calls.push({ body: String(init?.body), url: String(url) })
        return envelope([
          { id: 1, name: 'a' },
          { id: 2, name: 'b' },
        ])
      }) as typeof fetch,
    })

    const statement = db.prepare('SELECT * FROM t WHERE id > ?').bind(0)
    const all = await statement.all()
    expect(all.success).toBe(true)
    expect(all.results).toHaveLength(2)
    expect(all.meta.last_row_id).toBe(7)

    const first = await statement.first<{ id: number }>()
    expect(first?.id).toBe(1)
    const name = await statement.first<string>('name')
    expect(name).toBe('a')

    const raw = await statement.raw()
    expect(raw).toEqual([
      [1, 'a'],
      [2, 'b'],
    ])
    const rawWithColumns = await statement.raw({ columnNames: true })
    expect(rawWithColumns[0]).toEqual(['id', 'name'])

    expect(calls[0]?.url).toContain('/accounts/acct/d1/database/db/query')
    expect(JSON.parse(calls[0]!.body)).toEqual({
      params: [0],
      sql: 'SELECT * FROM t WHERE id > ?',
    })
  })

  it('rejects blob params instead of silently corrupting them', async () => {
    const db = makeD1HttpDatabase({
      accountId: 'acct',
      apiToken: 'token',
      databaseId: 'db',
      fetchImpl: (async () => envelope([])) as unknown as typeof fetch,
    })
    await expect(
      db.prepare('INSERT INTO t VALUES (?)').bind(new ArrayBuffer(4)).run(),
    ).rejects.toThrow(/blob\/bigint/)
  })

  it('surfaces Cloudflare error bodies (e.g. the 7500 write freeze)', async () => {
    const db = makeD1HttpDatabase({
      accountId: 'acct',
      apiToken: 'token',
      databaseId: 'db',
      fetchImpl: (async () =>
        new Response(
          JSON.stringify({
            errors: [{ code: 7500, message: 'exceeded storage limit' }],
            success: false,
          }),
          { status: 400 },
        )) as unknown as typeof fetch,
    })
    await expect(db.prepare('INSERT 1').run()).rejects.toThrow(/7500/)
  })

  it('degrades to the typed unavailable proxy when unconfigured', async () => {
    const db = d1FromProcessEnv({})
    await expect(db.prepare('SELECT 1').all()).rejects.toMatchObject({
      binding: 'OPENAGENTS_DB',
    })
  })
})

describe('queue delivery', () => {
  type Op = readonly [op: string, id: string]

  const fakeJobQueue = (ops: Array<Op>) =>
    ({
      ack: (jobId: string) =>
        Effect.sync(() => {
          ops.push(['ack', jobId])
        }),
      deadLetters: () => Effect.succeed([]),
      enqueue: () => Effect.succeed('job'),
      lease: () => Effect.succeed([]),
      nack: (jobId: string) =>
        Effect.sync(() => {
          ops.push(['nack', jobId])
        }),
    }) as never

  const ctx = makeExecutionContext(makeBackgroundTasks(() => undefined))

  it('acks handler-acked messages and nacks the rest', async () => {
    const ops: Array<Op> = []
    const outcome = await deliverLeasedBatch({
      ctx,
      env: {},
      handler: async batch => {
        batch.messages[0]?.ack()
        // second message neither acked nor retried => nack (redeliver)
      },
      jobQueue: fakeJobQueue(ops),
      jobs: [
        { attempts: 1, id: 'j1', payload: '{"a":1}', topic: 't' },
        { attempts: 1, id: 'j2', payload: '{"a":2}', topic: 't' },
      ],
      log: () => undefined,
      topic: 't',
    })

    expect(outcome).toEqual({ processed: 1, retried: 1 })
    expect(ops).toEqual([
      ['ack', 'j1'],
      ['nack', 'j2'],
    ])
  })

  it('a thrown batch nacks every message (Workers retry semantics)', async () => {
    const ops: Array<Op> = []
    const outcome = await deliverLeasedBatch({
      ctx,
      env: {},
      handler: async batch => {
        batch.messages[0]?.ack()
        throw new Error('boom')
      },
      jobQueue: fakeJobQueue(ops),
      jobs: [
        { attempts: 1, id: 'j1', payload: '{}', topic: 't' },
        { attempts: 1, id: 'j2', payload: '{}', topic: 't' },
      ],
      log: () => undefined,
      topic: 't',
    })

    expect(outcome).toEqual({ processed: 0, retried: 2 })
    expect(ops.map(([op]) => op)).toEqual(['nack', 'nack'])
  })
})

describe('postgres openauth storage', () => {
  type Executed = { text: string; values: ReadonlyArray<unknown> }

  const makeFakeSql = (rows: Record<string, unknown>) => {
    const executed: Array<Executed> = []
    const store = new Map<string, { value_json: string; expires_at: number | null }>(
      Object.entries(rows).map(([key, value]) => [
        key,
        { expires_at: null, value_json: JSON.stringify(value) },
      ]),
    )
    const sql = async (
      strings: TemplateStringsArray,
      ...values: ReadonlyArray<unknown>
    ) => {
      const text = strings.join('?')
      executed.push({ text, values })
      if (text.includes('SELECT key, value_json')) {
        if (text.includes('LIKE')) {
          const like = String(values[0]).replace(/%$/, '')
          return [...store.entries()]
            .filter(([key]) => key.startsWith(like))
            .map(([key, row]) => ({ key, ...row }))
        }
        const key = String(values[0])
        const row = store.get(key)
        return row === undefined ? [] : [{ key, ...row }]
      }
      if (text.includes('INSERT INTO openauth_storage')) {
        store.set(String(values[0]), {
          expires_at: values[2] as number | null,
          value_json: String(values[1]),
        })
        return []
      }
      if (text.includes('DELETE FROM openauth_storage')) {
        store.delete(String(values[0]))
        return []
      }
      return []
    }
    return { executed, sql, store }
  }

  const runtime = {
    nowIso: () => '2026-07-06T00:00:00.000Z',
    nowMs: () => 1_780_000_000_000,
  }

  it('set/get/remove round-trip against the twin table shape', async () => {
    const fake = makeFakeSql({})
    const storage = makePostgresOpenAuthStorage(
      'postgres://ignored',
      runtime,
      async () => fake.sql as never,
    )

    await storage.set(['oauth', 'refresh', 'abc'], { token: 't1' })
    const got = await storage.get(['oauth', 'refresh', 'abc'])
    expect(got).toEqual({ token: 't1' })

    await storage.remove(['oauth', 'refresh', 'abc'])
    expect(await storage.get(['oauth', 'refresh', 'abc'])).toBeUndefined()
  })

  it('expired rows read as missing and are lazily deleted', async () => {
    const fake = makeFakeSql({})
    const storageKey = joinKey(['signing', 'key'])
    fake.store.set(storageKey, {
      expires_at: runtime.nowMs() - 1000,
      value_json: JSON.stringify({ old: true }),
    })
    const storage = makePostgresOpenAuthStorage(
      'postgres://ignored',
      runtime,
      async () => fake.sql as never,
    )
    expect(await storage.get(['signing', 'key'])).toBeUndefined()
    expect(fake.store.has(storageKey)).toBe(false)
  })

  it('scan yields split keys under a prefix', async () => {
    const fake = makeFakeSql({
      [joinKey(['oauth', 'refresh', 'u1', 't1'])]: { a: 1 },
      [joinKey(['oauth', 'refresh', 'u1', 't2'])]: { a: 2 },
      [joinKey(['signing', 'key'])]: { k: true },
    })
    const storage = makePostgresOpenAuthStorage(
      'postgres://ignored',
      runtime,
      async () => fake.sql as never,
    )
    const seen: Array<string> = []
    for await (const [key] of storage.scan(['oauth', 'refresh', 'u1'])) {
      seen.push(key.join('/'))
    }
    expect(seen.sort()).toEqual([
      'oauth/refresh/u1/t1',
      'oauth/refresh/u1/t2',
    ])
  })
})

describe('assets fetcher', () => {
  const distDir = mkdtempSync(path.join(tmpdir(), 'cfg9-assets-'))
  writeFileSync(path.join(distDir, 'index.html'), '<html>shell</html>')
  mkdirSync(path.join(distDir, 'assets'), { recursive: true })
  writeFileSync(
    path.join(distDir, 'assets', 'app-Abc12345.js'),
    'console.log(1)',
  )

  it('never resolves outside the dist dir', () => {
    // URL pathnames are absolute, so normalization clamps at '/' and the
    // relative remainder is re-rooted INSIDE distDir; the containment guard
    // is the invariant (null only for undecodable/backslash edge cases).
    for (const attempt of [
      '/../etc/passwd',
      '/a/%2e%2e/%2e%2e/%2e%2e/etc/passwd',
      '/a/../../etc/passwd',
      '/%00',
    ]) {
      const resolved = resolveAssetPath(distDir, attempt)
      expect(
        resolved === null || resolved.startsWith(distDir + path.sep),
        `${attempt} must stay inside distDir`,
      ).toBe(true)
    }
    expect(resolveAssetPath(distDir, '/assets/app-Abc12345.js')).toContain(
      distDir,
    )
  })

  it('serves exact assets and falls back to the SPA shell', async () => {
    const fetcher = makeAssetsFetcher(distDir)
    const asset = await fetcher.fetch(
      new Request('https://openagents.com/assets/app-Abc12345.js'),
    )
    expect(asset.status).toBe(200)
    expect(asset.headers.get('cache-control')).toContain('immutable')

    const spa = await fetcher.fetch(
      new Request('https://openagents.com/some/client/route'),
    )
    expect(spa.status).toBe(200)
    expect(await spa.text()).toContain('shell')

    const method = await fetcher.fetch(
      new Request('https://openagents.com/x', { method: 'POST' }),
    )
    expect(method.status).toBe(405)
  })
})

describe('http utils', () => {
  it('rewrites the request origin to https only behind the proxy header', () => {
    const behindProxy = withForwardedProto(
      new Request('http://openagents.com/a', {
        headers: { 'x-forwarded-proto': 'https' },
      }),
    )
    expect(new URL(behindProxy.url).protocol).toBe('https:')

    const direct = withForwardedProto(new Request('http://localhost:8080/a'))
    expect(new URL(direct.url).protocol).toBe('http:')
  })

  it('cron auth requires the exact bearer and a configured token', () => {
    const request = (authorization?: string) =>
      new Request('http://x/internal/cron', {
        headers: authorization === undefined ? {} : { authorization },
        method: 'POST',
      })
    expect(cronAuthorized(request('Bearer t'), 't')).toBe(true)
    expect(cronAuthorized(request('Bearer wrong'), 't')).toBe(false)
    expect(cronAuthorized(request(), 't')).toBe(false)
    expect(cronAuthorized(request('Bearer t'), undefined)).toBe(false)
    expect(cronAuthorized(request('Bearer '), '')).toBe(false)
  })
})

describe('background tasks', () => {
  it('tracks, drains, and never throws on rejections', async () => {
    const logged: Array<string> = []
    const tasks = makeBackgroundTasks(event => logged.push(event))
    const ctx = makeExecutionContext(tasks)

    let resolved = false
    ctx.waitUntil(
      new Promise<void>(resolve =>
        setTimeout(() => {
          resolved = true
          resolve()
        }, 5),
      ),
    )
    ctx.waitUntil(Promise.reject(new Error('background boom')))

    await tasks.drain()
    expect(resolved).toBe(true)
    expect(tasks.size()).toBe(0)
    expect(logged).toContain('background_task_failed')
  })
})
