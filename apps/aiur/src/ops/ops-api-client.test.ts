import { describe, expect, test } from 'vitest'

import { fetchDailySalesLedger, fetchOpsHealth, fetchOpsRuns } from './ops-api-client'

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

describe('fetchDailySalesLedger (OB-6, #8563)', () => {
  test('hits the daily-sales-ledger proxy path with no query params by default', async () => {
    let capturedUrl: string | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      capturedUrl = String(input)
      return new Response(
        JSON.stringify({
          ok: true,
          ledger: {
            since: '2026-07-01',
            until: '2026-07-01',
            generatedAt: '2026-07-01T00:00:00.000Z',
            segmentRefs: [],
            segmentDays: [],
            deliverabilityDays: [],
            totals: {
              sourced: 0,
              drafted: 0,
              approved: 0,
              sent: 0,
              delivered: 0,
              bounced: 0,
              complained: 0,
              optOuts: 0,
              quoted: 0,
              closedWon: 0,
              closedLost: 0,
            },
            digestLine: '2026-07-01 sales ledger: sourced 0, drafted 0, sent 0, delivered 0, bounced 0, complained 0 (deliverability: not_measured), quoted 0, closes 0.',
            notMeasured: [],
          },
        }),
        { status: 200 },
      )
    }) as unknown as typeof fetch

    try {
      const result = await fetchDailySalesLedger()
      expect(capturedUrl).toBe('/api/admin/ops/daily-sales-ledger')
      expect(result.ok).toBe(true)
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test('passes since/until through as query params', async () => {
    let capturedUrl: string | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      capturedUrl = String(input)
      return new Response(JSON.stringify({ ok: false, messageSafe: 'nope' }), { status: 400 })
    }) as unknown as typeof fetch

    try {
      await fetchDailySalesLedger({ since: '2026-06-01', until: '2026-06-02' })
      expect(capturedUrl).toBe(
        '/api/admin/ops/daily-sales-ledger?since=2026-06-01&until=2026-06-02',
      )
    } finally {
      globalThis.fetch = originalFetch
    }
  })
})
