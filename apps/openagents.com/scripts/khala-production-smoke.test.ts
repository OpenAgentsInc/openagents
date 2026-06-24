import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./khala-production-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

const sse = (frames: ReadonlyArray<string>) =>
  new Response(
    `${frames.map(frame => `data: ${frame}\n\n`).join('')}data: [DONE]\n\n`,
    {
      headers: { 'content-type': 'text/event-stream' },
    },
  )

describe('Khala production smoke', () => {
  test('checks readiness, public catalog, nonstreaming, and streaming backing evidence', async () => {
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/v1/gateway/readiness') {
          return json({
            servableModelCount: 1,
            status: 'ready',
          })
        }

        if (url.pathname === '/v1/models') {
          return json({
            data: [{ id: 'openagents/khala' }],
            object: 'list',
          })
        }

        if (url.pathname === '/v1/chat/completions') {
          const headers = new Headers(init?.headers)
          expect(headers.get('authorization')).toBe('Bearer oa_agent_test')
          const body = JSON.parse(String(init?.body || '{}')) as {
            stream?: boolean
          }

          if (body.stream) {
            return sse([
              '{"choices":[{"delta":{"content":"RE"},"finish_reason":null}]}',
              '{"choices":[{"delta":{"content":"ADY"},"finish_reason":null}],"openagents":{"lane":"open","receipt":"inference.receipt.stream","receipt_url":"https://openagents.com/receipts/stream","requested_model":"openagents/khala","served_model":"accounts/fireworks/models/deepseek-v4-flash","supply_lane":"fireworks","worker":"fireworks"}}',
            ])
          }

          return json({
            choices: [
              {
                message: {
                  content: 'READY',
                  role: 'assistant',
                },
              },
            ],
            id: 'chatcmpl_test',
            model: 'openagents/khala',
            openagents: {
              lane: 'open',
              receipt: 'inference.receipt.test',
              receipt_url: 'https://openagents.com/receipts/test',
              requested_model: 'openagents/khala',
              served_model: 'accounts/fireworks/models/deepseek-v4-flash',
              supply_lane: 'fireworks',
              worker: 'fireworks',
            },
            usage: {
              completion_tokens: 1,
              prompt_tokens: 7,
              total_tokens: 8,
            },
          })
        }

        return json({ error: 'not found' }, { status: 404 })
      },
    )

    const output = await smoke.runKhalaProductionSmoke({
      approveLiveSpend: true,
      baseUrl: 'https://openagents.com',
      fetchImpl,
      token: 'oa_agent_test',
    })

    expect(output.ok).toBe(true)
    expect(output.model).toBe('openagents/khala')
    expect(output.nonstream).toMatchObject({
      openagents: {
        receipt: 'inference.receipt.test',
        requested_model: 'openagents/khala',
        served_model: 'accounts/fireworks/models/deepseek-v4-flash',
        supply_lane: 'fireworks',
        worker: 'fireworks',
      },
      responseId: 'chatcmpl_test',
      totalTokens: 8,
    })
    expect(output.stream.openagents).toMatchObject({
      receipt: 'inference.receipt.stream',
      requested_model: 'openagents/khala',
      served_model: 'accounts/fireworks/models/deepseek-v4-flash',
      supply_lane: 'fireworks',
      worker: 'fireworks',
    })
    expect(output.checks.map((check: { name: string }) => check.name)).toEqual([
      'readiness_endpoint_200',
      'readiness_has_servable_model',
      'models_endpoint_200',
      'models_lists_public_khala',
      'models_public_surface_closed',
      'nonstream_completion_200',
      'nonstream_public_model_preserved',
      'nonstream_infrastructure_guard_clean',
      'nonstream_usage_present',
      'nonstream_backing_disclosure_present',
      'stream_completion_200',
      'stream_done_seen',
      'stream_frames_present',
      'stream_public_model_preserved',
      'stream_infrastructure_guard_clean',
      'stream_backing_disclosure_present',
    ])
  })

  test('supports readiness-only mode without a token', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 2, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await smoke.runKhalaProductionSmoke({
      fetchImpl,
      readinessOnly: true,
    })

    expect(output.ok).toBe(true)
    expect(output.model).toBe('openagents/khala')
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })

  test('fails when the public model catalog exposes raw or split model ids', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 2, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({
          data: [
            { id: 'openagents/khala' },
            { id: 'accounts/fireworks/models/deepseek-v4-flash' },
            { id: 'openagents/khala-mini' },
          ],
        })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runKhalaProductionSmoke({
        fetchImpl,
        readinessOnly: true,
      }),
    ).rejects.toThrow('models_public_surface_closed failed')
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })

  test('refuses completion smoke without explicit live-spend approval', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runKhalaProductionSmoke({
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('Refusing authenticated completion smoke')
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })
})
