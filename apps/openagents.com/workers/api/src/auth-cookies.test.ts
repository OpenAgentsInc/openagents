// Cookie values are attacker-controlled, and `decodeURIComponent('%')` throws
// `URIError`. `parseCookies` used to call it unguarded, and two of its callers
// — `handleAuthCallback` (index.ts:5139) and `handleSessionApi` (index.ts:5227)
// — are invoked through `Effect.promise(...)`, which does NOT convert a
// rejection into a typed failure. The throw was therefore a DEFECT reaching the
// `Effect.catchCause` in `index.ts`, answering 500 `internal_server_error` and
// filing a `severity: 'critical'` `unhandled_exception` incident — reachable
// unauthenticated with a single malformed cookie, on the auth surface.

import { describe, expect, test } from 'vitest'

import { ACCESS_COOKIE, parseCookies } from './auth-cookies'

const requestWithCookie = (header: string): Request =>
  new Request('https://openagents.com/', { headers: { cookie: header } })

describe('parseCookies', () => {
  test('decodes well-formed percent escapes', () => {
    const cookies = parseCookies(requestWithCookie(`${ACCESS_COOKIE}=a%2Eb`))

    expect(cookies.get(ACCESS_COOKIE)).toBe('a.b')
  })

  test('does not throw on a malformed escape', () => {
    expect(() =>
      parseCookies(requestWithCookie(`${ACCESS_COOKIE}=%`))
    ).not.toThrow()
  })

  test('keeps an undecodable value raw so callers reject it normally', () => {
    const cookies = parseCookies(requestWithCookie(`${ACCESS_COOKIE}=%`))

    expect(cookies.get(ACCESS_COOKIE)).toBe('%')
  })

  test('a malformed value does not discard other cookies', () => {
    const cookies = parseCookies(
      requestWithCookie(`${ACCESS_COOKIE}=%; other=fine`)
    )

    expect(cookies.get(ACCESS_COOKIE)).toBe('%')
    expect(cookies.get('other')).toBe('fine')
  })

  test('handles values containing an equals sign', () => {
    const cookies = parseCookies(requestWithCookie(`${ACCESS_COOKIE}=a=b`))

    expect(cookies.get(ACCESS_COOKIE)).toBe('a=b')
  })

  test('returns an empty map when no cookie header is present', () => {
    expect(parseCookies(new Request('https://openagents.com/')).size).toBe(0)
  })
})
