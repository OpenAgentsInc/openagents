import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./gpt-oss20b-production-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

const sse = (frames: ReadonlyArray<string>) =>
  new Response(
    `${frames.map(frame => `data: ${frame}\n\n`).join('')}data: [DONE]\n\n`,
    {
      headers: { 'content-type': 'text/event-stream' },
    },
  )

describe('GPT-OSS 20B production smoke', () => {
  test('checks readiness, model catalog, nonstreaming, and streaming Hydralisk disclosure', async () => {
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
            data: [{ id: 'openai/gpt-oss-20b' }],
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
              '{"choices":[{"delta":{"content":"ADY"},"finish_reason":null}],"openagents":{"lane":"open","requested_model":"openai/gpt-oss-20b","served_model":"openai/gpt-oss-20b","supply_lane":"hydralisk","worker":"hydralisk-vllm"}}',
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
            model: 'openai/gpt-oss-20b',
            openagents: {
              lane: 'open',
              requested_model: 'openai/gpt-oss-20b',
              served_model: 'openai/gpt-oss-20b',
              supply_lane: 'hydralisk',
              worker: 'hydralisk-vllm',
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

    const output = await smoke.runGptOss20bProductionSmoke({
      approveLiveSpend: true,
      baseUrl: 'https://openagents.com',
      fetchImpl,
      token: 'oa_agent_test',
    })

    expect(output.ok).toBe(true)
    expect(output.model).toBe('openai/gpt-oss-20b')
    expect(output.nonstream).toMatchObject({
      openagents: {
        requested_model: 'openai/gpt-oss-20b',
        supply_lane: 'hydralisk',
      },
      responseId: 'chatcmpl_test',
      totalTokens: 8,
    })
    expect(output.stream.openagents).toMatchObject({
      requested_model: 'openai/gpt-oss-20b',
      supply_lane: 'hydralisk',
      worker: 'hydralisk-vllm',
    })
    expect(output.checks.map((check: { name: string }) => check.name)).toEqual([
      'readiness_endpoint_200',
      'readiness_has_servable_model',
      'models_endpoint_200',
      'models_lists_gpt_oss20b',
      'nonstream_completion_200',
      'nonstream_infrastructure_guard_clean',
      'nonstream_usage_present',
      'nonstream_hydralisk_disclosure_present',
      'stream_completion_200',
      'stream_done_seen',
      'stream_frames_present',
      'stream_infrastructure_guard_clean',
      'stream_hydralisk_disclosure_present',
    ])
  })

  test('supports readiness-only mode without a token', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 2, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openai/gpt-oss-20b' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await smoke.runGptOss20bProductionSmoke({
      fetchImpl,
      readinessOnly: true,
    })

    expect(output.ok).toBe(true)
    expect(output.model).toBe('openai/gpt-oss-20b')
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })

  test('refuses completion smoke without explicit live-spend approval', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openai/gpt-oss-20b' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runGptOss20bProductionSmoke({
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('Refusing authenticated completion smoke')
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })
})
