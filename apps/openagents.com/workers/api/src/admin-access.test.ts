/* eslint-disable @typescript-eslint/consistent-type-assertions */
import { describe, expect, test } from 'vitest'

import worker, {
  OPENAGENTS_ADMIN_EMAILS,
  SESSION_MAX_AGE_SECONDS,
  appendClearSessionCookies,
  appendSessionCookies,
  isOpenAgentsAdminEmail,
} from './index'

describe('OpenAgents admin access policy', () => {
  const executionContext = {
    passThroughOnException: () => undefined,
    waitUntil: () => undefined,
  } as never
  const makeSyncRoom = (notifiedScopes: Array<string>) =>
    ({
      getByName: (scope: string) => ({
        fetch: async (request: Request) => {
          notifiedScopes.push(
            request.headers.get('x-openagents-sync-scope') ?? scope,
          )

          return Response.json({ ok: true })
        },
      }),
      idFromName: (scope: string) => scope,
      get: (scope: string) => ({
        fetch: async (request: Request) => {
          notifiedScopes.push(
            request.headers.get('x-openagents-sync-scope') ?? scope,
          )

          return Response.json({ ok: true })
        },
      }),
    }) as never
  const requiredWorkerConfig = {
    GITHUB_CLIENT_ID: 'github-client',
    GITHUB_CLIENT_SECRET: 'github-secret',
    OPENAGENTS_APP_URL: 'https://openagents.com',
    OPENAUTH_CLIENT_ID: 'openagents-web',
    OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
  }

  test('has exactly one configured admin account', () => {
    expect(OPENAGENTS_ADMIN_EMAILS).toEqual(['chris@openagents.com'])
  })

  test('matches the configured admin email case-insensitively', () => {
    expect(isOpenAgentsAdminEmail('chris@openagents.com')).toBe(true)
    expect(isOpenAgentsAdminEmail(' Chris@OpenAgents.com ')).toBe(true)
    expect(isOpenAgentsAdminEmail('chris@openaegnts.com')).toBe(false)
    expect(isOpenAgentsAdminEmail('ben@openagents.com')).toBe(false)
    expect(isOpenAgentsAdminEmail('agent@openagents.com')).toBe(false)
  })

  test('uses max-length browser session cookies for access and refresh tokens', () => {
    const headers = new Headers()

    appendSessionCookies(headers, {
      access: 'access-token',
      expiresIn: 3600,
      refresh: 'refresh-token',
    })

    const cookies = headers.getSetCookie()

    expect(SESSION_MAX_AGE_SECONDS).toBe(34_560_000)
    expect(cookies).toHaveLength(2)
    expect(cookies[0]).toContain('oa_access=access-token')
    expect(cookies[0]).toContain(`Max-Age=${SESSION_MAX_AGE_SECONDS}`)
    expect(cookies[1]).toContain('oa_refresh=refresh-token')
    expect(cookies[1]).toContain(`Max-Age=${SESSION_MAX_AGE_SECONDS}`)
  })

  test('clears host-only, auth-path, and domain session cookie variants', () => {
    const headers = new Headers()

    appendClearSessionCookies(headers, 'openagents.com')

    const cookies = headers.getSetCookie()

    expect(cookies).toHaveLength(12)
    expect(cookies).toContain(
      'oa_access=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax',
    )
    expect(cookies).toContain(
      'oa_refresh=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax',
    )
    expect(cookies).toContain(
      'oa_access=; Max-Age=0; Path=/auth; HttpOnly; Secure; SameSite=Lax',
    )
    expect(cookies).toContain(
      'oa_refresh=; Max-Age=0; Path=/auth; HttpOnly; Secure; SameSite=Lax',
    )
    expect(
      cookies.some(
        cookie =>
          cookie.includes('Domain=openagents.com') &&
          cookie.startsWith('oa_access=;'),
      ),
    ).toBe(true)
    expect(
      cookies.some(
        cookie =>
          cookie.includes('Domain=.openagents.com') &&
          cookie.startsWith('oa_refresh=;'),
      ),
    ).toBe(true)
  })

  test('serves the Vite app shell for product routes instead of Worker-rendered HTML', async () => {
    const appShell = '<!doctype html><div id="root">foldkit app</div>'
    const env = {
      ASSETS: {
        fetch: () =>
          new Response(appShell, {
            headers: { 'content-type': 'text/html; charset=utf-8' },
          }),
      },
      ...requiredWorkerConfig,
    } as never

    for (const path of [
      '/',
      '/teams/openagents-core-team/chat',
      '/teams/openagents-core-team/files',
      '/teams/openagents-core-team/files/file_1',
      '/files/file_personal_1',
      '/dashboard',
      '/settings',
      '/share/123e4567-e89b-42d3-a456-426614174000',
      '/t/123e4567-e89b-42d3-a456-426614174000',
    ]) {
      const response = await worker.fetch(
        new Request(`https://openagents.com${path}`) as never,
        env,
        executionContext,
      )

      expect(response.status).toBe(200)
      expect(await response.text()).toBe(appShell)
    }
  })

  test('serves the login page route through the app shell', async () => {
    const appShell = '<!doctype html><div id="root"></div>'
    const response = await worker.fetch(
      new Request('https://openagents.com/login') as never,
      {
        ASSETS: {
          fetch: () => new Response(appShell),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )

    expect(response.status).toBe(200)
    expect(await response.text()).toBe(appShell)
  })

  test('cleans share product routes before serving the app shell', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/share/123e4567-e89b-42d3-a456-426614174000?utm=ignored',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe(
      'https://openagents.com/share/123e4567-e89b-42d3-a456-426614174000',
    )
  })

  test('stores a clean share return target when starting GitHub login', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?returnTo=%2Fshare%2F123e4567-e89b-42d3-a456-426614174000',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(cookies.some(cookie => cookie.includes('oa_auth_state='))).toBe(true)
    expect(
      cookies.some(cookie =>
        cookie.includes(
          'oa_login_return_to=%2Fshare%2F123e4567-e89b-42d3-a456-426614174000',
        ),
      ),
    ).toBe(true)
  })

  test('stores clean Forum return targets when starting GitHub login', async () => {
    const forumResponse = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?returnTo=%2Fforum',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const topicResponse = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?return_to=%2Fforum%2Ft%2F55555555-5555-4555-8555-555555555555%3FsortDir%3Ddesc',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )

    expect(forumResponse.status).toBe(302)
    expect(
      forumResponse.headers
        .getSetCookie()
        .some(cookie => cookie.includes('oa_login_return_to=%2Fforum')),
    ).toBe(true)
    expect(topicResponse.status).toBe(302)
    expect(
      topicResponse.headers
        .getSetCookie()
        .some(cookie =>
          cookie.includes(
            'oa_login_return_to=%2Fforum%2Ft%2F55555555-5555-4555-8555-555555555555%3FsortDir%3Ddesc',
          ),
        ),
    ).toBe(true)
  })

  test('stores invite accept return targets when starting email login', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/email?returnTo=%2Fapi%2Fteam-workspace-invites%2Faccept%3Ftoken%3Dinvite_token_123%26utm%3Dignored',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(cookies.some(cookie => cookie.includes('oa_auth_state='))).toBe(true)
    expect(
      cookies.some(cookie =>
        cookie.includes(
          'oa_login_return_to=%2Fapi%2Fteam-workspace-invites%2Faccept%3Ftoken%3Dinvite_token_123',
        ),
      ),
    ).toBe(true)
    expect(cookies.join('\n')).not.toContain('utm')
  })

  test('does not store invite accept return targets without a token', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/email?returnTo=%2Fapi%2Fteam-workspace-invites%2Faccept',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(
      cookies.some(cookie => /^oa_login_return_to=[^;]/.test(cookie)),
    ).toBe(false)
    expect(cookies).toContain(
      'oa_login_return_to=; Max-Age=0; Path=/auth; HttpOnly; Secure; SameSite=Lax',
    )
  })

  test('stores a clean agent claim return target when starting GitHub login', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?returnTo=%2Fagents%2Fclaims%2Fagent_claim_claim-1%3Fignored%3D1',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(cookies.some(cookie => cookie.includes('oa_auth_state='))).toBe(true)
    expect(
      cookies.some(cookie =>
        cookie.includes(
          'oa_login_return_to=%2Fagents%2Fclaims%2Fagent_claim_claim-1',
        ),
      ),
    ).toBe(true)
    expect(cookies.join('\n')).not.toContain('ignored')
  })

  test('stores a clean Pylon OpenAgents auth verifier return target when starting GitHub login', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?returnTo=%2Fapi%2Fpylon%2Fauth%2Fopenagents%2Fdevice%2Fverify%3Fattempt%3Dpylon_openauth_attempt-1%26code%3Dabcd-efgh%26ignored%3D1',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(cookies.some(cookie => cookie.includes('oa_auth_state='))).toBe(true)
    expect(
      cookies.some(cookie =>
        cookie.includes(
          'oa_login_return_to=%2Fapi%2Fpylon%2Fauth%2Fopenagents%2Fdevice%2Fverify%3Fattempt%3Dpylon_openauth_attempt-1%26code%3DABCD-EFGH',
        ),
      ),
    ).toBe(true)
    expect(cookies.join('\n')).not.toContain('ignored')
  })

  test('does not store nested agent claim paths as login return targets', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?returnTo=%2Fagents%2Fclaims%2Fagent_claim_claim-1%2Fextra',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(
      cookies.some(cookie => /^oa_login_return_to=[^;]/.test(cookie)),
    ).toBe(false)
    expect(cookies).toContain(
      'oa_login_return_to=; Max-Age=0; Path=/auth; HttpOnly; Secure; SameSite=Lax',
    )
  })

  test('does not store nested Forum paths as login return targets', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/login/github?returnTo=%2Fforum%2Ft%2F55555555-5555-4555-8555-555555555555%2Fextra',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(
      cookies.some(cookie => /^oa_login_return_to=[^;]/.test(cookie)),
    ).toBe(false)
    expect(cookies).toContain(
      'oa_login_return_to=; Max-Age=0; Path=/auth; HttpOnly; Secure; SameSite=Lax',
    )
  })

  test('returns failed GitHub login attempts to the clean Forum target', async () => {
    const response = await worker.fetch(
      new Request('https://openagents.com/auth/callback?error=access_denied', {
        headers: {
          cookie: 'oa_login_return_to=%2Fforum',
        },
      }) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/forum')
    expect(cookies).toContain(
      'oa_login_error=github_login_failed; Max-Age=60; Path=/; Secure; SameSite=Lax',
    )
    expect(
      cookies.find(cookie => cookie.startsWith('oa_login_error=')),
    ).not.toContain('HttpOnly')
  })

  test('does not redirect the deleted personal chat alias', async () => {
    const response = await worker.fetch(
      new Request('https://openagents.com/chat') as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )

    expect(response.status).toBe(404)
  })

  test('logout routes clear browser session cookies and redirect home', async () => {
    for (const path of ['/auth/logout', '/logout']) {
      const response = await worker.fetch(
        new Request(`https://openagents.com${path}`) as never,
        {
          ASSETS: {
            fetch: () => Response.json({ unused: true }),
          },
          ...requiredWorkerConfig,
        } as never,
        executionContext,
      )
      const cookies = response.headers.getSetCookie()

      expect(response.status).toBe(302)
      expect(response.headers.get('location')).toBe('/')
      expect(cookies).toHaveLength(12)
      expect(
        cookies.filter(cookie => cookie.startsWith('oa_access=; Max-Age=0')),
      ).toHaveLength(6)
      expect(
        cookies.filter(cookie => cookie.startsWith('oa_refresh=; Max-Age=0')),
      ).toHaveLength(6)
      expect(cookies.some(cookie => cookie.includes('Path=/auth'))).toBe(true)
      expect(
        cookies.some(cookie => cookie.includes('Domain=openagents.com')),
      ).toBe(true)
      expect(
        cookies.some(cookie => cookie.includes('Domain=.openagents.com')),
      ).toBe(true)
    }
  })

  test('logout can return directly to a team workspace invite accept link', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/auth/logout?returnTo=%2Fapi%2Fteam-workspace-invites%2Faccept%3Ftoken%3Dinvite_token_123%26utm%3Dignored',
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe(
      '/api/team-workspace-invites/accept?token=invite_token_123',
    )
    expect(cookies).toHaveLength(12)
  })

  test('session API clears a surviving refresh cookie and returns logged out', async () => {
    const response = await worker.fetch(
      new Request('https://openagents.com/api/auth/session', {
        headers: { cookie: 'oa_refresh=stale-refresh-token' },
      }) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const cookies = response.headers.getSetCookie()

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({ authenticated: false })
    expect(
      cookies.filter(cookie => cookie.startsWith('oa_access=; Max-Age=0')),
    ).toHaveLength(6)
    expect(
      cookies.filter(cookie => cookie.startsWith('oa_refresh=; Max-Age=0')),
    ).toHaveLength(6)
  })

  test('protects operator SHC routes with the admin API token', async () => {
    const missingSecret = await worker.fetch(
      new Request('https://openagents.com/api/omni/operator/fleet') as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const wrongToken = await worker.fetch(
      new Request('https://openagents.com/api/omni/operator/agent-runs', {
        headers: { authorization: 'Bearer wrong' },
        method: 'POST',
      }) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_ADMIN_API_TOKEN: 'expected',
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const wrongDetailToken = await worker.fetch(
      new Request(
        'https://openagents.com/api/omni/operator/agent-runs/123e4567-e89b-42d3-a456-426614174000?email=chris%40openagents.com',
        {
          headers: { authorization: 'Bearer wrong' },
          method: 'GET',
        },
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_ADMIN_API_TOKEN: 'expected',
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const wrongBillingToken = await worker.fetch(
      new Request('https://openagents.com/api/omni/operator/billing/credits', {
        body: JSON.stringify({
          amountCents: 1000,
          email: 'chris@openagents.com',
        }),
        headers: { authorization: 'Bearer wrong' },
        method: 'POST',
      }) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_ADMIN_API_TOKEN: 'expected',
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )
    const wrongTeamChatToken = await worker.fetch(
      new Request(
        'https://openagents.com/api/omni/operator/team-chat/messages',
        {
          body: JSON.stringify({
            body: '@autopilot Introduce Artanis',
            email: 'chris@openagents.com',
            kind: 'autopilot_intent',
            projectId: 'project_artanis',
            teamId: 'team_openagents_core',
          }),
          headers: { authorization: 'Bearer wrong' },
          method: 'POST',
        },
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_ADMIN_API_TOKEN: 'expected',
        ...requiredWorkerConfig,
      } as never,
      executionContext,
    )

    expect(missingSecret.status).toBe(401)
    expect(missingSecret.headers.get('cache-control')).toBe('no-store')
    expect(wrongToken.status).toBe(401)
    expect(wrongToken.headers.get('cache-control')).toBe('no-store')
    expect(wrongDetailToken.status).toBe(401)
    expect(wrongDetailToken.headers.get('cache-control')).toBe('no-store')
    expect(wrongBillingToken.status).toBe(401)
    expect(wrongBillingToken.headers.get('cache-control')).toBe('no-store')
    expect(wrongTeamChatToken.status).toBe(401)
    expect(wrongTeamChatToken.headers.get('cache-control')).toBe('no-store')
  })

  test('admin sync notify wakes the requested sync scope', async () => {
    const notifiedScopes: Array<string> = []
    const response = await worker.fetch(
      new Request('https://openagents.com/api/admin/sync/notify', {
        body: JSON.stringify({ scope: 'team:team_openagents_core' }),
        headers: { authorization: 'Bearer expected' },
        method: 'POST',
      }) as never,
      {
        OPENAGENTS_ADMIN_API_TOKEN: 'expected',
        ...requiredWorkerConfig,
        SYNC_ROOM: makeSyncRoom(notifiedScopes),
      } as never,
      executionContext,
    )

    await expect(response.json()).resolves.toMatchObject({
      ok: true,
      scopes: ['team:team_openagents_core'],
    })
    expect(notifiedScopes).toEqual(['team:team_openagents_core'])
  })
})
