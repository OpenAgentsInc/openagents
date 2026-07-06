import { describe, expect, test } from 'vitest'

import { fetchOpsHealth, fetchOpsRuns } from './ops-api-client'

describe('fetchOpsRuns / fetchOpsHealth (URL construction + result shape)', () => {
  test('fetchOpsRuns hits the runs proxy path with the limit query param', async () => {
    let capturedUrl: string | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      capturedUrl = String(input)
      return new Response(JSON.stringify({ ok: true, runs: [], liveViaKhalaSync: false }), {
        status: 200,
      })
    }) as unknown as typeof fetch

    try {
      const result = await fetchOpsRuns(25)
      expect(capturedUrl).toBe('/api/admin/ops/runs?limit=25')
      expect(result.ok).toBe(true)
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test('fetchOpsHealth reports a typed error on a non-2xx response, never throws', async () => {
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ messageSafe: 'nope' }), { status: 401 })) as unknown as typeof fetch

    try {
      const result = await fetchOpsHealth()
      expect(result.ok).toBe(false)
      if (!result.ok) {
        expect(result.status).toBe(401)
        expect(result.messageSafe).toBe('nope')
      }
    } finally {
      globalThis.fetch = originalFetch
    }
  })
})
