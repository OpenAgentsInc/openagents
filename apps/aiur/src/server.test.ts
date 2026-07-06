import { withStartRequestContext } from '@openagentsinc/effect-start'
import { describe, expect, test } from 'vitest'

import { routeAiurSharedSurface } from './server'

describe('routeAiurSharedSurface', () => {
  test('routes /auth/github/start to a redirect', async () => {
    const response = await withStartRequestContext(
      {
        request: new Request('https://aiur.openagents.com/auth/github/start'),
        env: {
          OPENAUTH_CLIENT_ID: 'openagents-web',
          OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
        },
      },
      () =>
        routeAiurSharedSurface(
          new Request('https://aiur.openagents.com/auth/github/start'),
        ),
    )
    expect(response?.status).toBe(302)
  })

  test('routes /api/aiur/access to a JSON status response', async () => {
    const response = await withStartRequestContext(
      { request: new Request('https://aiur.openagents.com/api/aiur/access'), env: {} },
      () =>
        routeAiurSharedSurface(
          new Request('https://aiur.openagents.com/api/aiur/access'),
        ),
    )
    expect(response?.status).toBe(200)
    expect(await response?.json()).toEqual({ kind: 'signed_out' })
  })

  test('routes /api/sync/bootstrap through the owner-gated proxy (401 without a session)', async () => {
    const response = await withStartRequestContext(
      { request: new Request('https://aiur.openagents.com/api/sync/bootstrap'), env: {} },
      () =>
        routeAiurSharedSurface(
          new Request('https://aiur.openagents.com/api/sync/bootstrap', {
            method: 'POST',
            body: '{}',
          }),
        ),
    )
    expect(response?.status).toBe(401)
  })

  test('routes /api/admin/credits/balance through the owner-gated proxy (401 without a session)', async () => {
    const response = await withStartRequestContext(
      { request: new Request('https://aiur.openagents.com/api/admin/credits/balance?userId=user_1'), env: {} },
      () =>
        routeAiurSharedSurface(
          new Request('https://aiur.openagents.com/api/admin/credits/balance?userId=user_1'),
        ),
    )
    expect(response?.status).toBe(401)
  })

  test('falls through (returns undefined) for an unrelated path', async () => {
    const response = await withStartRequestContext(
      { request: new Request('https://aiur.openagents.com/'), env: {} },
      () => routeAiurSharedSurface(new Request('https://aiur.openagents.com/')),
    )
    expect(response).toBeUndefined()
  })
})
