/**
 * CFG-9 (#8524): unit coverage for the Cloud Run adapter layer — the typed
 * unavailable-binding degrade, the D1 REST bridge, the assets fetcher,
 * request helpers, and background task tracking. (OpenAuth/auth-KV storage
 * is CFG-3's KvStore surface — covered by its own suites.) No network, no database: every backend is a fake.
 */

import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'

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
