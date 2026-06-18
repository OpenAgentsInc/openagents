import { describe, expect, test } from 'vitest'

import {
  cleanProductRouteRedirectLocation,
  githubWriteResultRedirectLocation,
} from './routing/redirect-policy'

describe('redirect URL policy', () => {
  test('keeps GitHub write OAuth result state out of the public app URL', () => {
    const location = githubWriteResultRedirectLocation('https://openagents.com')

    expect(location).toBe('https://openagents.com')

    const redirectUrl = new URL(location)
    expect(redirectUrl.search).toBe('')
    expect(redirectUrl.hash).toBe('')
    expect(redirectUrl.searchParams.has('github_write')).toBe(false)
  })

  test('canonicalizes stale product URLs that already contain result params', () => {
    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/?github_write=connected'),
      ),
    ).toBe('https://openagents.com/')

    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/login?github_write=connected'),
      ),
    ).toBe('https://openagents.com/')

    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/onboarding?checkout=complete'),
      ),
    ).toBe('https://openagents.com/onboarding')

    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/billing?checkout=complete'),
      ),
    ).toBe('https://openagents.com/billing')

    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/order?checkout=complete'),
      ),
    ).toBe('https://openagents.com/order')

    expect(
      cleanProductRouteRedirectLocation(
        new URL(
          'https://openagents.com/orders/software_order_1?checkout=complete',
        ),
      ),
    ).toBe('https://openagents.com/orders/software_order_1')
  })

  test('does not strip protocol query params from callback or API routes', () => {
    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/auth/callback?code=abc&state=def'),
      ),
    ).toBeUndefined()

    expect(
      cleanProductRouteRedirectLocation(
        new URL('https://openagents.com/api/provider-accounts?cursor=abc'),
      ),
    ).toBeUndefined()
  })
})
