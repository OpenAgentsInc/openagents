import { describe, expect, test } from 'vitest'

import {
  RouteAccessForbidden,
  RouteAccessNotFound,
} from '../thread-access'
import { routeAccessResponse } from './route-access-response'

describe('routeAccessResponse', () => {
  test('maps typed access errors to stable API responses', async () => {
    const forbidden = routeAccessResponse(
      new RouteAccessForbidden({ routeId: 'thread_1' }),
      { surface: 'api' },
    )
    const notFound = routeAccessResponse(
      new RouteAccessNotFound({ routeId: 'thread_1' }),
      { surface: 'api' },
    )

    await expect(forbidden.json()).resolves.toEqual({ error: 'forbidden' })
    await expect(notFound.json()).resolves.toEqual({ error: 'not_found' })
    expect(forbidden.status).toBe(403)
    expect(notFound.status).toBe(404)
    expect(forbidden.headers.get('cache-control')).toBe('no-store')
    expect(notFound.headers.get('cache-control')).toBe('no-store')
  })

  test('keeps product access failures as no-store redirects', () => {
    const response = routeAccessResponse(
      new RouteAccessForbidden({ routeId: 'thread_1' }),
      { href: '/', surface: 'product' },
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/')
    expect(response.headers.get('cache-control')).toBe('no-store')
  })
})
