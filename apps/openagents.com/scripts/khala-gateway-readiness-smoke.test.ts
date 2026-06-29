import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./khala-gateway-readiness-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

describe('Khala gateway readiness smoke', () => {
  test('checks readiness, models, completion block, and receipt route', async () => {
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/v1/gateway/readiness') {
          return json({
            servableModelCount: 1,
            status: 'degraded',
          })
        }

        if (url.pathname === '/v1/models') {
          return json({
            data: [{ id: 'openagents/khala-mini' }],
            object: 'list',
          })
        }

        if (url.pathname === '/v1/chat/completions') {
          const headers = new Headers(init?.headers)
          expect(headers.get('authorization')).toBe('Bearer oa_agent_test')
          return json({
            choices: [
              {
                message: {
                  content: 'Gateway is serving.',
                  role: 'assistant',
                },
              },
            ],
            id: 'chatcmpl_test',
            openagents: {
              lane: 'small',
              requested_model: 'openagents/khala-mini',
              served_model: 'gemini-3.5-flash',
              telemetry: {
                detailRef: '/api/public/inference/receipts/receipt_test',
              },
              verification: 'none',
              worker: 'vertex-gemini',
            },
          })
        }

        if (url.pathname === '/api/public/inference/receipts/receipt_test') {
          return json({
            receiptRef: 'receipt_test',
            schemaVersion: 'openagents.inference.receipt.v1',
          })
        }

        return json({ error: 'not found' }, { status: 404 })
      },
    )

    const output = await smoke.runKhalaGatewayReadinessSmoke({
      approveLiveSpend: true,
      baseUrl: 'https://openagents.com',
      fetchImpl,
      token: 'oa_agent_test',
    })

    expect(output.ok).toBe(true)
    expect(output.readiness).toEqual({
      servableModelCount: 1,
      status: 'degraded',
    })
    expect(output.receipt).toMatchObject({
      url: 'https://openagents.com/api/public/inference/receipts/receipt_test',
    })
    expect(output.checks.map((check: { name: string }) => check.name)).toEqual([
      'readiness_endpoint_200',
      'readiness_has_servable_model',
      'models_endpoint_200',
      'models_lists_requested_khala_model',
      'completion_endpoint_200',
      'completion_has_openagents_block',
      'completion_echoes_requested_model',
      'completion_has_dereferenceable_receipt_ref',
      'receipt_endpoint_200',
    ])
  })

  test('supports readiness-only mode without a token', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 2, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala-mini' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await smoke.runKhalaGatewayReadinessSmoke({
      fetchImpl,
      readinessOnly: true,
    })

    expect(output.ok).toBe(true)
    expect(output.completion).toBeNull()
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })

  test('refuses completion smoke without explicit live-spend approval', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'degraded' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala-mini' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runKhalaGatewayReadinessSmoke({
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('Refusing authenticated completion smoke')
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })

  test('fails when the requested Khala model is not advertised', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'degraded' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'gemini-3.5-flash' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runKhalaGatewayReadinessSmoke({ fetchImpl, readinessOnly: true }),
    ).rejects.toThrow('models_lists_requested_khala_model failed')
  })
})
