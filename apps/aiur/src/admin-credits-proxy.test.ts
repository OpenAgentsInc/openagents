import { describe, expect, test } from 'vitest'

import {
  AIUR_ADMIN_CREDITS_BALANCE_PATH,
  AIUR_ADMIN_CREDITS_GRANT_PATH,
  AIUR_ADMIN_OPS_HEALTH_PATH,
  AIUR_ADMIN_OPS_RUNS_PATH,
  routeAiurAdminCreditsProxyRequest,
} from './admin-credits-proxy'
import { AIUR_ACCESS_COOKIE } from './auth/cookies'
import type { AiurAuthClientLike } from './auth/session'

const fakeUser = (userId: string) => ({
  userId,
  provider: 'github' as const,
  email: 'octocat@example.com',
  name: 'Octo Cat',
  avatarUrl: '',
})

const fakeOwnerClient = (userId: string): AiurAuthClientLike => ({
  verify: (async () => ({
    err: undefined,
    subject: { type: 'user', properties: fakeUser(userId) },
    tokens: undefined,
  })) as never,
})

const fakeDeniedClient = (): AiurAuthClientLike => ({
  verify: (async () => ({ err: { message: 'invalid' } })) as never,
})

describe('routeAiurAdminCreditsProxyRequest — fail closed', () => {
  test('returns undefined for unrelated paths (falls through)', () => {
    const request = new Request('https://aiur.openagents.com/some/other/path')
    expect(
      routeAiurAdminCreditsProxyRequest(request, {}, { client: fakeDeniedClient() }),
    ).toBeUndefined()
  })

  test('balance without any session cookie => 401, never forwarded', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_ADMIN_CREDITS_BALANCE_PATH}?userId=user_1`,
    )
    let forwarded = false
    const response = await routeAiurAdminCreditsProxyRequest(
      request,
      {},
      {
        client: fakeOwnerClient('user_owner'),
        fetch: async () => {
          forwarded = true
          return new Response('{}')
        },
      },
    )
    expect(response?.status).toBe(401)
    expect(forwarded).toBe(false)
  })

  test('balance with a valid session but empty allowlist => 401, never forwarded (fail closed)', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_ADMIN_CREDITS_BALANCE_PATH}?userId=user_1`,
      { headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` } },
    )
    let forwarded = false
    const response = await routeAiurAdminCreditsProxyRequest(
      request,
      {},
      {
        client: fakeOwnerClient('user_owner'),
        fetch: async () => {
          forwarded = true
          return new Response('{}')
        },
      },
    )
    expect(response?.status).toBe(401)
    expect(forwarded).toBe(false)
  })

  test('an owner session forwards GET requests upstream with a Bearer header and the original query string', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_ADMIN_CREDITS_BALANCE_PATH}?userId=user_1`,
      { headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` } },
    )
    let capturedUrl: string | undefined
    let capturedAuth: string | null = null
    const response = await routeAiurAdminCreditsProxyRequest(
      request,
      { AIUR_OWNER_USER_IDS: 'user_owner', KHALA_SYNC_UPSTREAM_BASE_URL: 'https://upstream.example' },
      {
        client: fakeOwnerClient('user_owner'),
        fetch: async (input, init) => {
          capturedUrl = input
          capturedAuth = new Headers(init?.headers).get('authorization')
          return new Response(JSON.stringify({ ok: true }))
        },
      },
    )
    expect(response?.status).toBe(200)
    expect(capturedUrl).toBe('https://upstream.example/api/admin/credits/balance?userId=user_1')
    expect(capturedAuth).toBe('Bearer validtoken')
  })

  test('an owner session forwards POST requests upstream with the body and a Bearer header', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_ADMIN_CREDITS_GRANT_PATH}`,
      {
        method: 'POST',
        body: JSON.stringify({ userId: 'user_1', grantRef: 'g1', amountUsdCents: 500, reason: 'x' }),
        headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` },
      },
    )
    let capturedBody: string | undefined
    let capturedAuth: string | null = null
    const response = await routeAiurAdminCreditsProxyRequest(
      request,
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      {
        client: fakeOwnerClient('user_owner'),
        fetch: async (_input, init) => {
          capturedBody = init?.body as string
          capturedAuth = new Headers(init?.headers).get('authorization')
          return new Response(JSON.stringify({ ok: true }), { status: 201 })
        },
      },
    )
    expect(response?.status).toBe(201)
    expect(capturedAuth).toBe('Bearer validtoken')
    expect(JSON.parse(capturedBody ?? '{}')).toEqual({
      userId: 'user_1',
      grantRef: 'g1',
      amountUsdCents: 500,
      reason: 'x',
    })
  })

  test('AIUR-3: ops runs/health paths are also proxied, owner-gated the same way', async () => {
    const runsRequest = new Request(`https://aiur.openagents.com${AIUR_ADMIN_OPS_RUNS_PATH}`)
    const denied = await routeAiurAdminCreditsProxyRequest(
      runsRequest,
      {},
      { client: fakeDeniedClient() },
    )
    expect(denied?.status).toBe(401)

    const healthRequest = new Request(
      `https://aiur.openagents.com${AIUR_ADMIN_OPS_HEALTH_PATH}`,
      { headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` } },
    )
    let forwardedUrl: string | undefined
    const response = await routeAiurAdminCreditsProxyRequest(
      healthRequest,
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      {
        client: fakeOwnerClient('user_owner'),
        fetch: async input => {
          forwardedUrl = input
          return new Response(JSON.stringify({ ok: true }))
        },
      },
    )
    expect(response?.status).toBe(200)
    expect(forwardedUrl).toBe('https://openagents.com/api/admin/ops/health')
  })
})
