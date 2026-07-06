import { describe, expect, test } from 'vitest'

import { AIUR_ACCESS_COOKIE, AIUR_AUTH_STATE_COOKIE, AIUR_REFRESH_COOKIE } from './cookies'
import {
  handleAiurCallback,
  handleAiurLoginStart,
  handleAiurLogout,
} from './routes'

describe('handleAiurLoginStart', () => {
  test('redirects to the shared issuer and sets a state cookie', async () => {
    const request = new Request('https://aiur.openagents.com/auth/github/start')
    const response = await handleAiurLoginStart(request, {
      OPENAUTH_CLIENT_ID: 'openagents-web',
      OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
    })

    expect(response.status).toBe(302)
    const location = response.headers.get('location') ?? ''
    expect(location).toContain('https://auth.openagents.com')
    expect(location).toContain('client_id=openagents-web')
    expect(location).toContain(
      `redirect_uri=${encodeURIComponent('https://aiur.openagents.com/auth/callback')}`,
    )
    const setCookie = response.headers.get('set-cookie') ?? ''
    expect(setCookie).toContain(AIUR_AUTH_STATE_COOKIE)
    expect(setCookie).toContain('HttpOnly')
  })
})

describe('handleAiurCallback (pre-exchange guards)', () => {
  const env = {
    OPENAUTH_CLIENT_ID: 'openagents-web',
    OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
  }

  test('an `error` query param bounces home without a session', async () => {
    const request = new Request(
      'https://aiur.openagents.com/auth/callback?error=access_denied',
    )
    const response = await handleAiurCallback(request, env)
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/')
    expect(response.headers.get('set-cookie')).not.toContain(AIUR_ACCESS_COOKIE)
  })

  test('a missing code bounces home', async () => {
    const request = new Request('https://aiur.openagents.com/auth/callback')
    const response = await handleAiurCallback(request, env)
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/')
  })

  test('a state mismatch (or missing state cookie) bounces home', async () => {
    const request = new Request(
      'https://aiur.openagents.com/auth/callback?code=abc&state=mismatched',
      { headers: { cookie: `${AIUR_AUTH_STATE_COOKIE}=expected` } },
    )
    const response = await handleAiurCallback(request, env)
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/')
    expect(response.headers.get('set-cookie')).not.toContain(AIUR_ACCESS_COOKIE)
  })
})

describe('handleAiurLogout', () => {
  test('clears the session cookies and redirects home', () => {
    const response = handleAiurLogout()
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/')
    const cookies = response.headers.getSetCookie
      ? response.headers.getSetCookie()
      : [response.headers.get('set-cookie') ?? '']
    const joined = cookies.join(';')
    expect(joined).toContain(AIUR_ACCESS_COOKIE)
    expect(joined).toContain(AIUR_REFRESH_COOKIE)
    expect(joined).toContain('Max-Age=0')
  })
})
