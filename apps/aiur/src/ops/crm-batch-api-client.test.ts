import { describe, expect, test } from 'vitest'

import { fetchCrmBatchQueue, postCrmBatchApprove } from './crm-batch-api-client'

describe('fetchCrmBatchQueue / postCrmBatchApprove', () => {
  test('fetchCrmBatchQueue hits the queue proxy path with status filter', async () => {
    let capturedUrl: string | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      capturedUrl = String(input)
      return new Response(
        JSON.stringify({
          ok: true,
          queue: { total: 0, groups: [] },
        }),
        { status: 200 },
      )
    }) as unknown as typeof fetch

    try {
      const result = await fetchCrmBatchQueue({ status: 'proposed' })
      expect(capturedUrl).toBe(
        '/api/admin/ops/crm/batch-queue?status=proposed',
      )
      expect(result.ok).toBe(true)
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test('postCrmBatchApprove posts commandIds to the approve proxy path', async () => {
    let capturedUrl: string | undefined
    let capturedInit: RequestInit | undefined
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      capturedUrl = String(input)
      capturedInit = init
      return new Response(
        JSON.stringify({
          ok: true,
          result: {
            batchRef: 'crm_batch_1',
            requestedCount: 2,
            executedCount: 2,
            failedCount: 0,
            notPendingCount: 0,
            notFoundCount: 0,
            cappedCount: 0,
            items: [
              { commandId: 'crm_cmd_1', disposition: 'executed' },
              { commandId: 'crm_cmd_2', disposition: 'executed' },
            ],
          },
        }),
        { status: 200 },
      )
    }) as unknown as typeof fetch

    try {
      const result = await postCrmBatchApprove({
        commandIds: ['crm_cmd_1', 'crm_cmd_2'],
      })
      expect(capturedUrl).toBe('/api/admin/ops/crm/batch-approve')
      expect(capturedInit?.method).toBe('POST')
      expect(JSON.parse(String(capturedInit?.body))).toEqual({
        commandIds: ['crm_cmd_1', 'crm_cmd_2'],
      })
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.value.result.executedCount).toBe(2)
      }
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test('reports a typed error on a non-2xx response, never throws', async () => {
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ messageSafe: 'nope' }), {
        status: 401,
      })) as unknown as typeof fetch

    try {
      const result = await fetchCrmBatchQueue()
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
