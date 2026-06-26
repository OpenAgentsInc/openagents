import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./khala-glm-reap-production-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

const sse = (frames: ReadonlyArray<string>) =>
  new Response(
    `${frames.map(frame => `data: ${frame}\n\n`).join('')}data: [DONE]\n\n`,
    {
      headers: { 'content-type': 'text/event-stream' },
    },
  )

const armedEnv = {
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret-token',
  HYDRALISK_GLM_52_REAP_504B_BASE_URL: 'https://hydralisk.example.test',
  HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF: 'glm-reap-504b-g4-tp4-mtp2-rp105',
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF: 'receipt.public.glm_reap_504b.smoke',
}

const poolEnv = {
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BASE_URL:
    'https://hydralisk-primary.example.test',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BEARER_TOKEN: 'secret-primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BENCHMARK_RESERVED: 'true',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_PREFLIGHT_REF:
    'preflight.hydralisk.glm_52_reap_504b.primary.v1',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_RECEIPT_REF:
    'receipt.hydralisk.glm_52_reap_504b.primary.v1',
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'primary,second',
  HYDRALISK_GLM_52_REAP_504B_SECOND_BASE_URL:
    'https://hydralisk-second.example.test',
  HYDRALISK_GLM_52_REAP_504B_SECOND_BEARER_TOKEN: 'secret-second',
  HYDRALISK_GLM_52_REAP_504B_SECOND_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_SECOND_PREFLIGHT_REF:
    'preflight.hydralisk.glm_52_reap_504b.second.v1',
  HYDRALISK_GLM_52_REAP_504B_SECOND_PROFILE_REF:
    'profile.hydralisk.glm_52_reap_504b.second.v1',
  HYDRALISK_GLM_52_REAP_504B_SECOND_RECEIPT_REF:
    'receipt.hydralisk.glm_52_reap_504b.second.v1',
}

describe('Khala GLM REAP production smoke', () => {
  test('skips cleanly without the GLM arming env and does not fetch', async () => {
    const fetchImpl = vi.fn()

    const output = await smoke.runKhalaGlmReapProductionSmoke({
      env: {},
      fetchImpl,
    })

    expect(output).toMatchObject({
      ok: true,
      reason: 'glm_reap_lane_not_armed',
      skipped: true,
    })
    expect(output.arming.blockerRefs).toContain(
      'HYDRALISK_GLM_52_REAP_504B_ENABLED',
    )
    expect(fetchImpl).not.toHaveBeenCalled()
  })

  test('readiness-only uses deployed public readiness and catalog evidence when local GLM arming is absent', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/api/v1/gateway/readiness') {
        return json({
          hiddenModelCount: 0,
          lanes: [
            {
              armed: true,
              hiddenModelCount: 0,
              lane: 'hydralisk',
              servableModelCount: 1,
            },
          ],
          reasonRefs: ['gateway.readiness.ready.all_models_servable'],
          servableModelCount: 1,
          status: 'ready',
          totalModelCount: 1,
        })
      }

      if (url.pathname === '/api/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }], object: 'list' })
      }

      return json(
        { error: 'unexpected spend path', path: url.pathname },
        { status: 500 },
      )
    })

    const output = await smoke.runKhalaGlmReapProductionSmoke({
      env: {},
      fetchImpl,
      readinessOnly: true,
    })

    expect(output).toMatchObject({
      arming: {
        armed: false,
        evidenceSource: 'deployed_public_readiness_catalog',
      },
      counter: null,
      ok: true,
      reason: 'local_glm_reap_lane_not_armed_read_deployed_public_evidence',
      skipped: false,
    })
    expect(output.arming.blockerRefs).toContain(
      'HYDRALISK_GLM_52_REAP_504B_ENABLED',
    )
    expect(output.readiness).toMatchObject({
      lanes: [
        {
          armed: true,
          lane: 'hydralisk',
          servableModelCount: 1,
        },
      ],
      servableModelCount: 1,
      status: 'ready',
    })
    expect(fetchImpl).toHaveBeenCalledTimes(2)
    expect(
      fetchImpl.mock.calls.map(call => new URL(String(call[0])).pathname),
    ).toEqual(['/api/v1/gateway/readiness', '/api/v1/models'])
  })

  test('readiness-only rejects deployed evidence that does not prove a servable Hydralisk lane', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/api/v1/gateway/readiness') {
        return json({
          lanes: [
            {
              armed: true,
              hiddenModelCount: 0,
              lane: 'fireworks',
              servableModelCount: 1,
            },
          ],
          servableModelCount: 1,
          status: 'ready',
        })
      }

      if (url.pathname === '/api/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }], object: 'list' })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runKhalaGlmReapProductionSmoke({
        env: {},
        fetchImpl,
        readinessOnly: true,
      }),
    ).rejects.toThrow(
      'deployed_glm_reap_readiness_public_evidence_present failed',
    )
  })

  test('resolves a named pool with the benchmark primary excluded and the second replica eligible', () => {
    const arming = smoke.resolveGlmReapSmokeArming(poolEnv)

    expect(arming).toMatchObject({
      armed: true,
      eligibleReplicaRefs: ['replica.hydralisk.glm_52_reap_504b.second'],
      excludedReplicaRefs: ['replica.hydralisk.glm_52_reap_504b.primary'],
      poolMode: 'named_pool',
    })
    expect(arming.replicas).toHaveLength(2)
    expect(JSON.stringify(arming)).not.toContain('secret')
    expect(JSON.stringify(arming)).not.toContain('https://')
  })

  test('checks /api/v1 catalog hygiene, GLM disclosures, receipts, stream, and counter movement', async () => {
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
          return json({ servableModelCount: 1, status: 'ready' })
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
              '{"choices":[{"delta":{"content":"ADY"},"finish_reason":null}],"openagents":{"lane":"open","receipt":"receipt.glm.stream","receipt_url":"https://openagents.com/receipts/glm-stream","requested_model":"openagents/khala","routing":{"selected_replica_id":"primary","selected_replica_ref":"replica.hydralisk.glm_52_reap_504b.primary"},"served_model":"openagents/glm-5.2-reap-504b","supply_lane":"hydralisk","worker":"hydralisk-vllm-glm-5p2-reap-504b"}}',
            ])
          }

          return json({
            choices: [{ message: { content: 'READY', role: 'assistant' } }],
            id: 'chatcmpl_glm',
            model: 'openagents/khala',
            openagents: {
              lane: 'open',
              receipt: 'receipt.glm.nonstream',
              receipt_url: 'https://openagents.com/receipts/glm-nonstream',
              requested_model: 'openagents/khala',
              routing: {
                selected_replica_id: 'primary',
                selected_replica_ref:
                  'replica.hydralisk.glm_52_reap_504b.primary',
              },
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

        if (url.pathname === '/receipts/glm-nonstream') {
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
              receiptRef: 'receipt.glm.nonstream',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        if (url.pathname === '/receipts/glm-stream') {
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
              receiptRef: 'receipt.glm.stream',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        return json({ error: 'not found', path: url.pathname }, { status: 404 })
      },
    )

    const output = await smoke.runKhalaGlmReapProductionSmoke({
      approveLiveSpend: true,
      counterPollMs: 1,
      env: armedEnv,
      fetchImpl,
      sleep: () => Promise.resolve(),
      token: 'oa_agent_test',
    })

    expect(output).toMatchObject({
      arming: { armed: true },
      counter: {
        after: 120,
        before: 100,
        delta: 20,
        requiredDelta: 15,
        servedTokens: 20,
      },
      model: 'openagents/khala',
      ok: true,
      skipped: false,
    })
    expect(output.nonstream.openagents).toMatchObject({
      requested_model: 'openagents/khala',
      routing: {
        selected_replica_id: 'primary',
        selected_replica_ref: 'replica.hydralisk.glm_52_reap_504b.primary',
      },
      served_model: 'openagents/glm-5.2-reap-504b',
      supply_lane: 'hydralisk',
      worker: 'hydralisk-vllm-glm-5p2-reap-504b',
    })
    expect(output.stream.openagents).toMatchObject({
      requested_model: 'openagents/khala',
      routing: {
        selected_replica_id: 'primary',
        selected_replica_ref: 'replica.hydralisk.glm_52_reap_504b.primary',
      },
      served_model: 'openagents/glm-5.2-reap-504b',
      supply_lane: 'hydralisk',
      worker: 'hydralisk-vllm-glm-5p2-reap-504b',
    })
  })

  test('checks the whole named pool while proving the reserved primary receives no smoke traffic', async () => {
    let counterReads = 0
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/api/public/khala-tokens-served') {
          counterReads += 1
          return json({ tokensServed: counterReads === 1 ? 500 : 528 })
        }

        if (url.pathname === '/api/v1/gateway/readiness') {
          return json({ servableModelCount: 1, status: 'ready' })
        }

        if (url.pathname === '/api/v1/models') {
          return json({ data: [{ id: 'openagents/khala' }], object: 'list' })
        }

        if (url.pathname === '/api/v1/chat/completions') {
          const body = JSON.parse(String(init?.body || '{}')) as {
            stream?: boolean
          }
          const openagents = {
            lane: 'open',
            receipt: body.stream
              ? 'receipt.glm.pool.stream'
              : 'receipt.glm.pool.nonstream',
            receipt_url: body.stream
              ? 'https://openagents.com/receipts/glm-pool-stream'
              : 'https://openagents.com/receipts/glm-pool-nonstream',
            requested_model: 'openagents/khala',
            routing: {
              selected_replica_id: 'second',
              selected_replica_ref: 'replica.hydralisk.glm_52_reap_504b.second',
            },
            served_model: 'openagents/glm-5.2-reap-504b',
            supply_lane: 'hydralisk',
            worker: 'hydralisk-vllm-glm-5p2-reap-504b',
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
            id: 'chatcmpl_glm_pool',
            model: 'openagents/khala',
            openagents,
            usage: { completion_tokens: 3, prompt_tokens: 9, total_tokens: 12 },
          })
        }

        if (
          url.pathname === '/receipts/glm-pool-nonstream' ||
          url.pathname === '/receipts/glm-pool-stream'
        ) {
          return json({
            receipt: {
              kind: 'charge',
              ledgerState: 'paid',
              modelEvidence: {
                requested_model: 'openagents/khala',
                served_model: 'openagents/glm-5.2-reap-504b',
                supply_lane: 'hydralisk',
                total_tokens: 14,
                worker: 'hydralisk-vllm-glm-5p2-reap-504b',
              },
              receiptRef: 'receipt.glm.pool',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        return json({ error: 'not found', path: url.pathname }, { status: 404 })
      },
    )

    const output = await smoke.runKhalaGlmReapProductionSmoke({
      approveLiveSpend: true,
      counterPollMs: 1,
      env: poolEnv,
      expectedReplicaId: 'second',
      fetchImpl,
      sleep: () => Promise.resolve(),
      token: 'oa_agent_test',
    })

    expect(output).toMatchObject({
      ok: true,
      pool: {
        eligibleReplicaRefs: ['replica.hydralisk.glm_52_reap_504b.second'],
        excludedReplicaRefs: ['replica.hydralisk.glm_52_reap_504b.primary'],
        mode: 'named_pool',
      },
      skipped: false,
    })
    expect(output.nonstream.openagents.routing).toMatchObject({
      selected_replica_id: 'second',
      selected_replica_ref: 'replica.hydralisk.glm_52_reap_504b.second',
    })
    expect(output.stream.openagents.routing).toMatchObject({
      selected_replica_id: 'second',
      selected_replica_ref: 'replica.hydralisk.glm_52_reap_504b.second',
    })
  })

  test('fails the pool smoke if traffic lands on the benchmark-reserved primary replica', async () => {
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
          return json({ data: [{ id: 'openagents/khala' }], object: 'list' })
        }

        if (url.pathname === '/api/v1/chat/completions') {
          JSON.parse(String(init?.body || '{}'))
          return json({
            choices: [{ message: { content: 'READY', role: 'assistant' } }],
            id: 'chatcmpl_glm_pool_reserved',
            model: 'openagents/khala',
            openagents: {
              receipt_url: 'https://openagents.com/receipts/glm-reserved',
              requested_model: 'openagents/khala',
              routing: {
                selected_replica_id: 'primary',
                selected_replica_ref:
                  'replica.hydralisk.glm_52_reap_504b.primary',
              },
              served_model: 'openagents/glm-5.2-reap-504b',
              supply_lane: 'hydralisk',
              worker: 'hydralisk-vllm-glm-5p2-reap-504b',
            },
            usage: { total_tokens: 8 },
          })
        }

        return json({ error: 'not found' }, { status: 404 })
      },
    )

    await expect(
      smoke.runKhalaGlmReapProductionSmoke({
        approveLiveSpend: true,
        env: poolEnv,
        expectedReplicaId: 'second',
        fetchImpl,
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('nonstream_replica_routing_present failed')
  })

  test('fails when the public catalog exposes the raw GLM model id', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/api/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/api/v1/models') {
        return json({
          data: [
            { id: 'openagents/khala' },
            { id: 'openagents/glm-5.2-reap-504b' },
          ],
          object: 'list',
        })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    await expect(
      smoke.runKhalaGlmReapProductionSmoke({
        env: armedEnv,
        fetchImpl,
        readinessOnly: true,
      }),
    ).rejects.toThrow('models_public_surface_closed failed')
  })

  test('fails when the public counter does not move enough for served usage', async () => {
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
          return json({ data: [{ id: 'openagents/khala' }], object: 'list' })
        }

        if (url.pathname === '/api/v1/chat/completions') {
          const body = JSON.parse(String(init?.body || '{}')) as {
            stream?: boolean
          }
          if (body.stream) {
            return sse([
              '{"choices":[{"delta":{"content":"READY"},"finish_reason":null}],"openagents":{"receipt_url":"https://openagents.com/receipts/stream","requested_model":"openagents/khala","routing":{"selected_replica_id":"primary","selected_replica_ref":"replica.hydralisk.glm_52_reap_504b.primary"},"served_model":"openagents/glm-5.2-reap-504b","supply_lane":"hydralisk","worker":"hydralisk-vllm-glm-5p2-reap-504b"}}',
            ])
          }
          return json({
            choices: [{ message: { content: 'READY', role: 'assistant' } }],
            id: 'chatcmpl_glm',
            model: 'openagents/khala',
            openagents: {
              receipt_url: 'https://openagents.com/receipts/nonstream',
              requested_model: 'openagents/khala',
              routing: {
                selected_replica_id: 'primary',
                selected_replica_ref:
                  'replica.hydralisk.glm_52_reap_504b.primary',
              },
              served_model: 'openagents/glm-5.2-reap-504b',
              supply_lane: 'hydralisk',
              worker: 'hydralisk-vllm-glm-5p2-reap-504b',
            },
            usage: { total_tokens: 8 },
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
                total_tokens: 8,
                worker: 'hydralisk-vllm-glm-5p2-reap-504b',
              },
              receiptRef: 'receipt.glm',
              schemaVersion: 'openagents.inference.receipt.v1',
            },
          })
        }

        return json({ error: 'not found' }, { status: 404 })
      },
    )

    await expect(
      smoke.runKhalaGlmReapProductionSmoke({
        approveLiveSpend: true,
        counterPollMs: 1,
        counterPolls: 2,
        env: armedEnv,
        fetchImpl,
        sleep: () => Promise.resolve(),
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('tokens_served_counter_delta_matches_glm_usage failed')
  })
})
