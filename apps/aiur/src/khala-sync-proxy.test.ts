import { describe, expect, test } from 'vitest'

import { AIUR_ACCESS_COOKIE } from './auth/cookies'
import type { AiurAuthClientLike } from './auth/session'
import {
  AIUR_SYNC_BOOTSTRAP_PATH,
  AIUR_SYNC_CONNECT_PATH,
  routeAiurKhalaSyncProxyRequest,
} from './khala-sync-proxy'

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

describe('routeAiurKhalaSyncProxyRequest — fail closed', () => {
  test('returns undefined for unrelated paths (falls through)', () => {
    const request = new Request('https://aiur.openagents.com/some/other/path')
    expect(
      routeAiurKhalaSyncProxyRequest(request, {}, { client: fakeDeniedClient() }),
    ).toBeUndefined()
  })

  test('bootstrap without any session cookie => 401 unauthenticated, never forwarded', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_SYNC_BOOTSTRAP_PATH}`,
      { method: 'POST', body: '{}' },
    )
    let forwarded = false
    const response = await routeAiurKhalaSyncProxyRequest(
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

  test('bootstrap with a valid session but empty allowlist => 401, never forwarded (fail closed)', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_SYNC_BOOTSTRAP_PATH}`,
      {
        method: 'POST',
        body: '{}',
        headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` },
      },
    )
    let forwarded = false
    const response = await routeAiurKhalaSyncProxyRequest(
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

  test('bootstrap with an owner session forwards upstream with a Bearer header', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_SYNC_BOOTSTRAP_PATH}`,
      {
        method: 'POST',
        body: '{"scope":"scope.public.tokens-served"}',
        headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` },
      },
    )
    let capturedAuth: string | null = null
    const response = await routeAiurKhalaSyncProxyRequest(
      request,
      { AIUR_OWNER_USER_IDS: 'user_owner', KHALA_SYNC_UPSTREAM_BASE_URL: 'https://upstream.example' },
      {
        client: fakeOwnerClient('user_owner'),
        fetch: async (input, init) => {
          capturedAuth = new Headers(init?.headers).get('authorization')
          expect(input).toBe('https://upstream.example/api/sync/bootstrap')
          return new Response(JSON.stringify({ ok: true }))
        },
      },
    )
    expect(response?.status).toBe(200)
    expect(capturedAuth).toBe('Bearer validtoken')
  })

  test('connect upgrade without a WebSocket upgrade header is rejected 426', async () => {
    const request = new Request(
      `https://aiur.openagents.com${AIUR_SYNC_CONNECT_PATH}?scope=scope.public.tokens-served`,
      { headers: { cookie: `${AIUR_ACCESS_COOKIE}=validtoken` } },
    )
    const response = await routeAiurKhalaSyncProxyRequest(
      request,
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      { client: fakeOwnerClient('user_owner') },
    )
    expect(response?.status).toBe(426)
  })
})
