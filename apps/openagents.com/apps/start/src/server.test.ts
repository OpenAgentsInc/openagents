import { describe, expect, test } from 'vitest'

import { SECURITY_HEADERS, applySecurityHeaders } from './server'

describe('Start Worker server entry', () => {
  test('applies the staging security headers without changing status', () => {
    const response = applySecurityHeaders(
      new Response('ok', {
        status: 203,
        headers: {
          'Content-Type': 'text/plain',
        },
      }),
    )

    expect(response.status).toBe(203)
    expect(response.headers.get('Content-Type')).toContain('text/plain')

    for (const [key, value] of Object.entries(SECURITY_HEADERS)) {
      expect(response.headers.get(key)).toBe(value)
    }
  })
})
