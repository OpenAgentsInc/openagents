import { describe, expect, test } from 'vitest'

import { SECURITY_HEADERS } from '../shared-surface'
import { aiurEnvFromProcessEnv } from './env'
import {
  createAiurCloudRunFetchHandler,
  sanitizeCloseCode,
  withForwardedProto,
} from './server'

const SHELL = '<!DOCTYPE html><html><body data-shell="aiur"></body></html>'

const makeHandler = (env = {}) =>
  createAiurCloudRunFetchHandler({
    env,
    clientDir: '/nonexistent/dist/client',
    shellHtml: SHELL,
  })

describe('aiurEnvFromProcessEnv', () => {
  test('picks only known AiurEnv fields and trims', () => {
    const env = aiurEnvFromProcessEnv({
      OPENAUTH_CLIENT_ID: ' openagents-web ',
      OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      KHALA_SYNC_UPSTREAM_BASE_URL: 'https://openagents.com',
      AIUR_OWNER_USER_IDS: 'github:14167547',
      HOME: '/root',
      PORT: '8080',
    })
    expect(env).toEqual({
      OPENAUTH_CLIENT_ID: 'openagents-web',
      OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      KHALA_SYNC_UPSTREAM_BASE_URL: 'https://openagents.com',
      AIUR_OWNER_USER_IDS: 'github:14167547',
    })
  })

  test('empty owner allowlist stays undefined (owner gate fails closed)', () => {
    const env = aiurEnvFromProcessEnv({ AIUR_OWNER_USER_IDS: '   ' })
    expect(env.AIUR_OWNER_USER_IDS).toBeUndefined()
  })
})

describe('createAiurCloudRunFetchHandler', () => {
  test('serves the SPA shell for page routes with security headers', async () => {
    const response = await makeHandler()(
      new Request('https://aiur.openagents.com/'),
    )
    expect(response.status).toBe(200)
    expect(await response.text()).toBe(SHELL)
    for (const [key, value] of Object.entries(SECURITY_HEADERS)) {
      expect(response.headers.get(key)).toBe(value)
    }
  })

  test('routes /api/aiur/access through the shared surface (signed out)', async () => {
    const response = await makeHandler()(
      new Request('https://aiur.openagents.com/api/aiur/access'),
    )
    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({ kind: 'signed_out' })
  })

  test('sync proxy without a session is 401 (fail closed), even with an allowlist set', async () => {
    const response = await makeHandler({
      AIUR_OWNER_USER_IDS: 'github:14167547',
    })(
      new Request('https://aiur.openagents.com/api/sync/bootstrap', {
        method: 'POST',
        body: '{}',
      }),
    )
    expect(response.status).toBe(401)
  })

  test('admin credits proxy without a session is 401 (fail closed)', async () => {
    const response = await makeHandler()(
      new Request(
        'https://aiur.openagents.com/api/admin/credits/balance?userId=user_1',
      ),
    )
    expect(response.status).toBe(401)
  })

  test('unknown /api/ paths are 404, not the shell', async () => {
    const response = await makeHandler()(
      new Request('https://aiur.openagents.com/api/nope'),
    )
    expect(response.status).toBe(404)
  })

  test('non-GET page requests are 405, not the shell', async () => {
    const response = await makeHandler()(
      new Request('https://aiur.openagents.com/', { method: 'POST' }),
    )
    expect(response.status).toBe(405)
  })

  test('/auth/github/start redirects to the OpenAuth issuer', async () => {
    const response = await makeHandler()(
      new Request('https://aiur.openagents.com/auth/github/start'),
    )
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toContain(
      'https://auth.openagents.com/authorize',
    )
  })
})

describe('withForwardedProto', () => {
  test('rewrites http URLs to https when the Cloud Run proxy says so', () => {
    const rewritten = withForwardedProto(
      new Request('http://aiur.openagents.com/auth/github/start', {
        headers: { 'x-forwarded-proto': 'https' },
      }),
    )
    expect(new URL(rewritten.url).protocol).toBe('https:')
  })

  test('leaves requests without the forwarded header untouched', () => {
    const request = new Request('http://localhost:8080/')
    expect(withForwardedProto(request)).toBe(request)
  })

  test('OpenAuth redirect derives an https redirect_uri behind the proxy', async () => {
    const response = await makeHandler()(
      new Request('http://aiur.openagents.com/auth/github/start', {
        headers: { 'x-forwarded-proto': 'https' },
      }),
    )
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toContain(
      encodeURIComponent('https://aiur.openagents.com/auth/callback'),
    )
  })
})

describe('sanitizeCloseCode', () => {
  test('passes through valid application close codes', () => {
    expect(sanitizeCloseCode(1000)).toBe(1000)
    expect(sanitizeCloseCode(4001)).toBe(4001)
  })

  test('replaces reserved/invalid codes with 1000', () => {
    expect(sanitizeCloseCode(undefined)).toBe(1000)
    expect(sanitizeCloseCode(1005)).toBe(1000)
    expect(sanitizeCloseCode(1006)).toBe(1000)
    expect(sanitizeCloseCode(999)).toBe(1000)
    expect(sanitizeCloseCode(5000)).toBe(1000)
  })
})
