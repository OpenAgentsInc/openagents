import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./khala-glm-reap-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

const sse = (frames: ReadonlyArray<string>) =>
  new Response(
    `${frames.map(frame => `data: ${frame}\n\n`).join('')}data: [DONE]\n\n`,
    {
      headers: { 'content-type': 'text/event-stream' },
    },
  )

const armedEnv = {
  HYDRALISK_GLM_52_REAP_504B_BASE_URL:
    'https://hydralisk-glm-52-reap-504b.example.test',
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret-route-token',
  HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF:
    'preflight.hydralisk.glm_52_reap_504b.g4.mtp2.v1',
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF:
    'receipt.hydralisk.glm_52_reap_504b.g4.mtp2_smoke.v1',
}

describe('Khala GLM REAP smoke', () => {
  test('skips cleanly when the GLM REAP arming env is absent', async () => {
    const fetchImpl = vi.fn()

    const output = await smoke.runKhalaGlmReapSmoke({
      env: {},
      fetchImpl,
    })

    expect(output).toMatchObject({
      arming: {
        armed: false,
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.route_not_ready',
          'blocker.hydralisk_glm_52_reap_504b.base_url_missing',
          'blocker.hydralisk_glm_52_reap_504b.bearer_missing',
          'blocker.hydralisk_glm_52_reap_504b.preflight_ref_missing',
          'blocker.hydralisk_glm_52_reap_504b.receipt_ref_missing',
        ],
      },
      ok: true,
      state: 'skipped',
    })
    expect(fetchImpl).not.toHaveBeenCalled()
  })

  test('checks canonical API calls, GLM receipts, model catalog closure, and token counter movement', async () => {
    let counterReads = 0
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/api/public/khala-tokens-served') {
          counterReads += 1
          return json({
            schemaVersion: 'openagents.public_khala_tokens_served.v1',
            tokensServed: counterReads === 1 ? 100 : 120,
          })
        }

        if (url.pathname === '/api/v1/gateway/readiness') {
          return json({
            servableModelCount: 1,
            status: 'ready',
          })
        }

        if (url.pathname === '/api/v1/models') {
          return json({
            data: [{ id: 'openagents/khala' }],
            object: 'list',
          })
        }

        if (url.pathname === '/api/v1/chat/completions') {
          const headers = new Headers(init?.headers)
          expect(headers.get('authorization')).toBe('Bearer oa_agent_test')
          const body = JSON.parse(String(init?.body || '{}')) as {
            stream?: boolean
          }

          if (body.stream) {
            return sse([
              '{"choices":[{"delta":{"content":"RE"},"finish_reason":null}]}',
              '{"choices":[{"delta":{"content":"ADY"},"finish_reason":null}],"openagents":{"lane":"open","receipt_url":"https://openagents.com/receipts/stream","requested_model":"openagents/khala","served_model":"openagents/glm-5.2-reap-504b","supply_lane":"hydralisk","worker":"hydralisk-vllm-glm-5p2-reap-504b"}}',
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
            id: 'chatcmpl_glm_test',
            model: 'openagents/khala',
            openagents: {
              receipt_url: 'https://openagents.com/receipts/nonstream',
              requested_model: 'openagents/khala',
              served_model: 'openagents/glm-5.2-reap-504b',
              supply_lane: 'hydralisk',
              worker: 'hydralisk-vllm-glm-5p2-reap-504b',
            },
            usage: {
              completion_tokens: 2,
              prompt_tokens: 7,
              total_tokens: 9,
            },
          })
        }

        if (url.pathname === '/receipts/nonstream') {
          return json({
            receipt: {
              kind: 'charge',
              ledgerState: 'paid',
              modelEvidence: {
                requested_model: 'openagents/khala',
                served_model: 'openagents/glm-5.2-reap-504b',
                supply_lane: 'hydralisk',
                total_tokens: 9,
                worker: 'hydralisk-vllm-glm-5p2-reap-504b',
              },
              receiptRef: 'inference.receipt.glm.nonstream',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        if (url.pathname === '/receipts/stream') {
          return json({
            receipt: {
              kind: 'charge',
              ledgerState: 'paid',
              modelEvidence: {
                requested_model: 'openagents/khala',
                served_model: 'openagents/glm-5.2-reap-504b',
                supply_lane: 'hydralisk',
                total_tokens: 11,
                worker: 'hydralisk-vllm-glm-5p2-reap-504b',
              },
              receiptRef: 'inference.receipt.glm.stream',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        return json({ error: 'not found', path: url.pathname }, { status: 404 })
      },
    )

    const output = await smoke.runKhalaGlmReapSmoke({
      approveLiveSpend: true,
      counterSettleMs: 0,
      env: armedEnv,
      fetchImpl,
      token: 'oa_agent_test',
    })

    expect(output).toMatchObject({
      arming: {
        armed: true,
        evidenceRefs: [
          'preflight.hydralisk.glm_52_reap_504b.g4.mtp2.v1',
          'receipt.hydralisk.glm_52_reap_504b.g4.mtp2_smoke.v1',
        ],
      },
      nonstream: {
        openagents: {
          served_model: 'openagents/glm-5.2-reap-504b',
          supply_lane: 'hydralisk',
          worker: 'hydralisk-vllm-glm-5p2-reap-504b',
        },
      },
      ok: true,
      publicTokensServed: {
        afterTokens: 120,
        beforeTokens: 100,
        counterDelta: 20,
        expectedCounterDelta: 20,
      },
      state: 'ok',
    })
    expect(output.checks.map((check: { name: string }) => check.name)).toContain(
      'public_tokens_counter_delta',
    )
  })

  test('fails when the public counter does not move by the receipt token sum', async () => {
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/api/public/khala-tokens-served') {
          return json({ tokensServed: 100 })
        }

        if (url.pathname === '/api/v1/gateway/readiness') {
          return json({ servableModelCount: 1, status: 'ready' })
        }

        if (url.pathname === '/api/v1/models') {
          return json({ data: [{ id: 'openagents/khala' }] })
        }

        if (url.pathname === '/api/v1/chat/completions') {
          const body = JSON.parse(String(init?.body || '{}')) as {
            stream?: boolean
          }
          if (body.stream) {
            return sse([
              '{"choices":[{"delta":{"content":"READY"},"finish_reason":"stop"}],"openagents":{"receipt_url":"https://openagents.com/receipts/stream","requested_model":"openagents/khala","served_model":"openagents/glm-5.2-reap-504b","supply_lane":"hydralisk","worker":"hydralisk-vllm-glm-5p2-reap-504b"}}',
            ])
          }
          return json({
            choices: [{ message: { content: 'READY', role: 'assistant' } }],
            model: 'openagents/khala',
            openagents: {
              receipt_url: 'https://openagents.com/receipts/nonstream',
              requested_model: 'openagents/khala',
              served_model: 'openagents/glm-5.2-reap-504b',
              supply_lane: 'hydralisk',
              worker: 'hydralisk-vllm-glm-5p2-reap-504b',
            },
            usage: { total_tokens: 9 },
          })
        }

        if (
          url.pathname === '/receipts/nonstream' ||
          url.pathname === '/receipts/stream'
        ) {
          return json({
            receipt: {
              kind: 'charge',
              ledgerState: 'paid',
              modelEvidence: {
                requested_model: 'openagents/khala',
                served_model: 'openagents/glm-5.2-reap-504b',
                supply_lane: 'hydralisk',
                total_tokens: 9,
                worker: 'hydralisk-vllm-glm-5p2-reap-504b',
              },
              receiptRef: 'inference.receipt.glm',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        return json({ error: 'not found' }, { status: 404 })
      },
    )

    await expect(
      smoke.runKhalaGlmReapSmoke({
        approveLiveSpend: true,
        counterSettleMs: 0,
        env: armedEnv,
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('public_tokens_counter_delta failed')
  })
})
