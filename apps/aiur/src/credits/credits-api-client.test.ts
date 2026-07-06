// @vitest-environment happy-dom
import { describe, expect, test } from 'vitest'

import { fetchCreditsBalance, mintCreditsActionRef } from './credits-api-client'

describe('mintCreditsActionRef', () => {
  test('mints distinct refs on each call', () => {
    const a = mintCreditsActionRef()
    const b = mintCreditsActionRef()
    expect(a).not.toBe(b)
    expect(a.length).toBeGreaterThan(10)
  })
})

describe('fetchCreditsBalance (URL construction)', () => {
  test('builds a same-origin request with the userId query param', async () => {
    let capturedUrl: string | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      capturedUrl = String(input)
      return new Response(JSON.stringify({ ok: true }), { status: 200 })
    }) as typeof fetch

    try {
      await fetchCreditsBalance({ userId: 'user_1' })
      expect(capturedUrl).toBe('/api/admin/credits/balance?userId=user_1')
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test('builds a same-origin request with the githubLogin query param', async () => {
    let capturedUrl: string | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      capturedUrl = String(input)
      return new Response(JSON.stringify({ ok: true }), { status: 200 })
    }) as typeof fetch

    try {
      await fetchCreditsBalance({ githubLogin: 'octocat' })
      expect(capturedUrl).toBe('/api/admin/credits/balance?githubLogin=octocat')
    } finally {
      globalThis.fetch = originalFetch
    }
  })
})
