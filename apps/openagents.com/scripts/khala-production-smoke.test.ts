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

        if (url.pathname === '/receipts/test') {
          return json({
            receipt: {
              kind: 'charge',
              ledgerState: 'paid',
              modelEvidence: {
                requested_model: 'openagents/khala',
                served_model: 'accounts/fireworks/models/deepseek-v4-flash',
                supply_lane: 'fireworks',
                total_tokens: 8,
                worker: 'fireworks',
              },
              receiptRef: 'inference.receipt.test',
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
                served_model: 'accounts/fireworks/models/deepseek-v4-flash',
                supply_lane: 'fireworks',
                total_tokens: 8,
                worker: 'fireworks',
              },
              receiptRef: 'inference.receipt.stream',
              schemaVersion: 'openagents.inference.receipt.v1',
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
      receipt: {
        modelEvidence: {
          requested_model: 'openagents/khala',
          served_model: 'accounts/fireworks/models/deepseek-v4-flash',
          supply_lane: 'fireworks',
          total_tokens: 8,
          worker: 'fireworks',
        },
        url: 'https://openagents.com/receipts/test',
      },
      totalTokens: 8,
    })
    expect(output.stream.openagents).toMatchObject({
      receipt: 'inference.receipt.stream',
      requested_model: 'openagents/khala',
      served_model: 'accounts/fireworks/models/deepseek-v4-flash',
      supply_lane: 'fireworks',
      worker: 'fireworks',
    })
    expect(output.stream.receipt).toMatchObject({
      modelEvidence: {
        requested_model: 'openagents/khala',
        served_model: 'accounts/fireworks/models/deepseek-v4-flash',
        supply_lane: 'fireworks',
        total_tokens: 8,
        worker: 'fireworks',
      },
      url: 'https://openagents.com/receipts/stream',
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
      'nonstream_receipt_ref_present',
      'nonstream_receipt_endpoint_200',
      'nonstream_receipt_schema_present',
      'nonstream_receipt_backing_evidence_present',
      'nonstream_receipt_usage_present',
      'nonstream_receipt_redaction_guard_clean',
      'stream_completion_200',
      'stream_done_seen',
      'stream_frames_present',
      'stream_public_model_preserved',
      'stream_infrastructure_guard_clean',
      'stream_backing_disclosure_present',
      'stream_receipt_ref_present',
      'stream_receipt_endpoint_200',
      'stream_receipt_schema_present',
      'stream_receipt_backing_evidence_present',
      'stream_receipt_usage_present',
      'stream_receipt_redaction_guard_clean',
    ])
  })

  test('accepts operator-credit zero-debit receipts without dereferencing charge receipts', async () => {
    let receiptReads = 0
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/v1/gateway/readiness') {
          return json({ servableModelCount: 1, status: 'ready' })
        }

        if (url.pathname === '/v1/models') {
          return json({ data: [{ id: 'openagents/khala' }] })
        }

        if (url.pathname === '/v1/chat/completions') {
          const body = JSON.parse(String(init?.body || '{}')) as {
            stream?: boolean
          }
          const openagents = {
            lane: 'open',
            requested_model: 'openagents/khala',
            served_model: 'accounts/fireworks/models/deepseek-v4-flash',
            supply_lane: 'fireworks',
            telemetry: {
              detailRef: body.stream
                ? '/api/public/inference/receipts/receipt.inference.operator_credit.stream'
                : '/api/public/inference/receipts/receipt.inference.operator_credit.nonstream',
              totalTokens: 8,
            },
            worker: 'fireworks',
          }

          if (body.stream) {
            return sse([
              '{"choices":[{"delta":{"content":"READY"},"finish_reason":null}]}',
              JSON.stringify({
                choices: [{ delta: {}, finish_reason: null }],
                openagents,
              }),
            ])
          }

          return json({
            choices: [{ message: { content: 'READY', role: 'assistant' } }],
            id: 'chatcmpl_operator_credit',
            model: 'openagents/khala',
            openagents,
            usage: { total_tokens: 8 },
          })
        }

        if (url.pathname.startsWith('/api/public/inference/receipts/')) {
          receiptReads += 1
        }

        return json({ error: 'not found' }, { status: 404 })
      },
    )

    const output = await smoke.runKhalaProductionSmoke({
      approveLiveSpend: true,
      fetchImpl,
      token: 'oa_agent_test',
    })

    expect(receiptReads).toBe(0)
    expect(output.nonstream.receipt).toMatchObject({
      kind: 'operator_credit',
      ledgerState: 'zero_debit_operator_exempt',
      receiptRef: 'receipt.inference.operator_credit.nonstream',
      zeroDebit: true,
    })
    expect(output.stream.receipt).toMatchObject({
      kind: 'operator_credit',
      ledgerState: 'zero_debit_operator_exempt',
      receiptRef: 'receipt.inference.operator_credit.stream',
      zeroDebit: true,
    })
    expect(output.checks.map((check: { name: string }) => check.name)).toContain(
      'nonstream_operator_credit_zero_debit',
    )
    expect(output.checks.map((check: { name: string }) => check.name)).toContain(
      'stream_operator_credit_zero_debit',
    )
  })

  test('fails when a completion has no dereferenceable receipt ref', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      if (url.pathname === '/v1/chat/completions') {
        return json({
          choices: [{ message: { content: 'READY', role: 'assistant' } }],
          id: 'chatcmpl_no_receipt',
          model: 'openagents/khala',
          openagents: {
            requested_model: 'openagents/khala',
            served_model: 'accounts/fireworks/models/deepseek-v4-flash',
            supply_lane: 'fireworks',
            worker: 'fireworks',
          },
          usage: { total_tokens: 8 },
        })
      }

      return json({ error: 'not found' }, { status: 404 })
    })

    await expect(
      smoke.runKhalaProductionSmoke({
        approveLiveSpend: true,
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('nonstream_receipt_ref_present failed')
  })

  test('fails when the dereferenced receipt backing evidence mismatches', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      if (url.pathname === '/v1/chat/completions') {
        return json({
          choices: [{ message: { content: 'READY', role: 'assistant' } }],
          id: 'chatcmpl_bad_receipt',
          model: 'openagents/khala',
          openagents: {
            receipt_url: 'https://openagents.com/receipts/mismatch',
            requested_model: 'openagents/khala',
            served_model: 'accounts/fireworks/models/deepseek-v4-flash',
            supply_lane: 'fireworks',
            worker: 'fireworks',
          },
          usage: { total_tokens: 8 },
        })
      }

      if (url.pathname === '/receipts/mismatch') {
        return json({
          receipt: {
            kind: 'charge',
            ledgerState: 'paid',
            modelEvidence: {
              requested_model: 'openagents/khala',
              served_model: 'openai/gpt-oss-20b',
              supply_lane: 'hydralisk',
              total_tokens: 8,
              worker: 'hydralisk-vllm',
            },
            receiptRef: 'inference.receipt.bad',
            schemaVersion: 'openagents.inference.receipt.v1',
          },
        })
      }

      return json({ error: 'not found' }, { status: 404 })
    })

    await expect(
      smoke.runKhalaProductionSmoke({
        approveLiveSpend: true,
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('nonstream_receipt_backing_evidence_present failed')
  })

  test('fails when the dereferenced receipt exposes raw prompt or secret-shaped material', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      if (url.pathname === '/v1/chat/completions') {
        return json({
          choices: [{ message: { content: 'READY', role: 'assistant' } }],
          id: 'chatcmpl_unsafe_receipt',
          model: 'openagents/khala',
          openagents: {
            receipt_url: 'https://openagents.com/receipts/unsafe',
            requested_model: 'openagents/khala',
            served_model: 'accounts/fireworks/models/deepseek-v4-flash',
            supply_lane: 'fireworks',
            worker: 'fireworks',
          },
          usage: { total_tokens: 8 },
        })
      }

      if (url.pathname === '/receipts/unsafe') {
        return json({
          receipt: {
            kind: 'charge',
            ledgerState: 'paid',
            modelEvidence: {
              requested_model: 'openagents/khala',
              served_model: 'accounts/fireworks/models/deepseek-v4-flash',
              supply_lane: 'fireworks',
              total_tokens: 8,
              worker: 'fireworks',
            },
            raw_prompt: 'secret prompt text',
            receiptRef: 'inference.receipt.unsafe',
            schemaVersion: 'openagents.inference.receipt.v1',
          },
        })
      }

      return json({ error: 'not found' }, { status: 404 })
    })

    await expect(
      smoke.runKhalaProductionSmoke({
        approveLiveSpend: true,
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('nonstream_receipt_redaction_guard_clean failed')
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
