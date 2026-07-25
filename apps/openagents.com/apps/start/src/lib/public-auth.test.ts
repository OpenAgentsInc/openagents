import { describe, expect, test, vi } from 'vitest'

import { publicAuthAction, readPublicAuthState } from './public-auth'

describe('public auth state', () => {
  test('shows the admin analytics action for an authenticated admin', async () => {
    const fetcher = vi.fn(async () =>
      Response.json({
        authenticated: true,
        bootstrap: { isAdmin: true },
      }),
    )

    const state = await readPublicAuthState(fetcher)

    expect(fetcher).toHaveBeenCalledWith('/api/auth/session', {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    expect(publicAuthAction(state)).toEqual({
      href: '/admin/analytics',
      label: 'Analytics',
    })
  })

  test('fails soft to the anonymous login action', async () => {
    const state = await readPublicAuthState(async () => {
      throw new Error('offline')
    })

    expect(publicAuthAction(state)).toEqual({
      href: '/login',
      label: 'Log In',
    })
  })
})
