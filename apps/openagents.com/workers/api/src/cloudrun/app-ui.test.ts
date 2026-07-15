import type { Tokens } from '@openauthjs/openauth/client'
import { describe, expect, test, vi } from 'vitest'

import { handleAppUiRequest } from './app-ui'

const ctx = {} as ExecutionContext
const env = { name: 'test' }
const request = (pathname = '/app', method = 'GET') =>
  new Request(`https://openagents.com${pathname}`, { method })

const dependencies = (session: Readonly<{ tokens?: Tokens }> | undefined) => ({
  renderStart: vi.fn(() =>
    Promise.resolve(
      new Response('<!doctype html><title>App</title>', {
        headers: { 'content-type': 'text/html; charset=utf-8' },
      }),
    ),
  ),
  verifySession: vi.fn(() => Promise.resolve(session)),
})

describe('authenticated /app mount', () => {
  test('does not own public or API paths', async () => {
    const deps = dependencies(undefined)

    for (const pathname of ['/', '/astro', '/login', '/api/auth/session']) {
      expect(
        await handleAppUiRequest(request(pathname), env, ctx, deps),
      ).toBeUndefined()
    }
    expect(deps.verifySession).not.toHaveBeenCalled()
  })

  test('redirects an unauthenticated visitor to login with a bounded app return', async () => {
    const deps = dependencies(undefined)
    const response = await handleAppUiRequest(request(), env, ctx, deps)

    expect(response?.status).toBe(302)
    expect(response?.headers.get('location')).toBe('/login?returnTo=%2Fapp')
    expect(response?.headers.get('cache-control')).toBe('private, no-store')
    expect(response?.headers.get('vary')).toContain('Cookie')
    expect(deps.renderStart).not.toHaveBeenCalled()
  })

  test('fails closed when session verification errors', async () => {
    const deps = dependencies(undefined)
    deps.verifySession.mockRejectedValueOnce(new Error('issuer unavailable'))

    const response = await handleAppUiRequest(request(), env, ctx, deps)
    expect(response?.status).toBe(302)
    expect(deps.renderStart).not.toHaveBeenCalled()
  })

  test('serves Start only after verification and preserves refreshed cookies', async () => {
    const deps = dependencies({
      tokens: {
        access: 'refreshed-access',
        refresh: 'refreshed-refresh',
      },
    })
    const response = await handleAppUiRequest(request(), env, ctx, deps)

    expect(response?.status).toBe(200)
    expect(await response?.text()).toContain('<title>App</title>')
    expect(response?.headers.get('cache-control')).toBe('private, no-store')
    expect(response?.headers.get('vary')).toContain('Cookie')
    expect(response?.headers.getSetCookie().join('\n')).toContain(
      'oa_access=refreshed-access',
    )
    expect(response?.headers.getSetCookie().join('\n')).toContain(
      'oa_refresh=refreshed-refresh',
    )
  })

  test('supports HEAD and rejects mutations without invoking auth', async () => {
    const headDeps = dependencies({})
    const head = await handleAppUiRequest(
      request('/app/', 'HEAD'),
      env,
      ctx,
      headDeps,
    )
    expect(head?.status).toBe(200)

    const postDeps = dependencies({})
    const post = await handleAppUiRequest(
      request('/app', 'POST'),
      env,
      ctx,
      postDeps,
    )
    expect(post?.status).toBe(405)
    expect(post?.headers.get('allow')).toBe('GET, HEAD')
    expect(postDeps.verifySession).not.toHaveBeenCalled()
  })
})
