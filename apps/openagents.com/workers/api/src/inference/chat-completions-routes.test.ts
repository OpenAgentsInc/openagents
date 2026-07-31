import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ChatCompletionsDeps,
  INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER,
  type InferenceAuth,
  handleChatCompletions,
} from './chat-completions-routes'
import {
  FIREWORKS_ADAPTER_ID,
  PASSTHROUGH_OPENAI_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  selectAdapterPlan,
} from './model-router'
import { makePassthroughAdapter } from './passthrough-adapter'
import {
  GEMINI_FLASH_MODEL_ID,
  GPT_56_LUNA_MODEL_ID,
  KHALA_MODEL_ID,
  KIMI_K3_FIREWORKS_MODEL_ID,
} from './pricing'
import {
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
  type InferenceRequest,
} from './provider-adapter'
import { stubEchoAdapter } from './stub-echo-adapter'

const authOk: InferenceAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: InferenceAuth = async () => undefined

const deps = (
  overrides: Partial<ChatCompletionsDeps> = {},
): ChatCompletionsDeps => {
  const registry = new InferenceProviderRegistry()
  registry.register(stubEchoAdapter)
  return {
    authenticate: authOk,
    enabled: true,
    nowEpochMillis: () => 0,
    registry,
    ...overrides,
  }
}

const request = (headers?: HeadersInit): Request =>
  new Request('https://openagents.com/v1/chat/completions', {
    body: JSON.stringify({
      messages: [{ content: 'hello', role: 'user' }],
      model: KHALA_MODEL_ID,
    }),
    ...(headers === undefined ? {} : { headers }),
    method: 'POST',
  })

const hostedLaneArming = {
  fireworks: true,
  hydralisk: false,
  openrouter: false,
  'openagents-network': false,
  'vertex-anthropic': true,
  'vertex-gemini': true,
} as const

const hostedRequest = (model: string): Request =>
  new Request('https://openagents.com/v1/chat/completions', {
    body: JSON.stringify({
      messages: [{ content: 'hello', role: 'user' }],
      model,
    }),
    method: 'POST',
  })

const makeHostedRegistry = () => {
  const calls: Array<string> = []
  const registry = new InferenceProviderRegistry()
  const adapter = (adapterId: string): InferenceProviderAdapter => ({
    complete: inferenceRequest => {
      calls.push(adapterId)
      return Effect.succeed({
        content: `served by ${adapterId}`,
        finishReason: 'stop',
        servedModel: inferenceRequest.model,
        usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
      })
    },
    id: adapterId,
    stream: inferenceRequest =>
      Effect.succeed([
        {
          contentDelta: `served by ${adapterId}`,
          finishReason: 'stop',
          servedModel: inferenceRequest.model,
          usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
        },
      ]),
  })
  registry.register(adapter(VERTEX_GEMINI_ADAPTER_ID))
  registry.register(adapter(FIREWORKS_ADAPTER_ID))
  return { calls, registry }
}

describe('chat completions no-spend admission', () => {
  test('rejects platform-funded inference instead of converting it into free capacity', async () => {
    const response = await Effect.runPromise(
      handleChatCompletions(request(), deps()),
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toMatchObject({
      error: 'platform_funding_unavailable',
      model: KHALA_MODEL_ID,
    })
  })

  test('retains the explicit organization runtime no-meter lane', async () => {
    const secret = 'org-runtime-secret'
    const response = await Effect.runPromise(
      handleChatCompletions(
        request({ [INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER]: secret }),
        deps({ orgCloudRuntimeNoMeterSecret: secret }),
      ),
    )

    expect(response.status).toBe(200)
    expect(await response.json()).toMatchObject({ model: KHALA_MODEL_ID })
  })

  test('retains ordinary inference authentication', async () => {
    const response = await Effect.runPromise(
      handleChatCompletions(request(), deps({ authenticate: authNone })),
    )

    expect(response.status).toBe(401)
  })

  test('allows an internal account to select Gemini Flash or Kimi K3', async () => {
    const calls: Array<{
      adapterId: string
      request: InferenceRequest
    }> = []
    const registry = new InferenceProviderRegistry()
    const adapter = (adapterId: string): InferenceProviderAdapter => ({
      complete: inferenceRequest => {
        calls.push({ adapterId, request: inferenceRequest })
        return Effect.succeed({
          content: `served by ${adapterId}`,
          finishReason: 'stop',
          servedModel: inferenceRequest.model,
          usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
        })
      },
      id: adapterId,
      stream: inferenceRequest =>
        Effect.succeed([
          {
            contentDelta: `served by ${adapterId}`,
            finishReason: 'stop',
            servedModel: inferenceRequest.model,
            usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
          },
        ]),
    })
    registry.register(adapter(VERTEX_GEMINI_ADAPTER_ID))
    registry.register(adapter(FIREWORKS_ADAPTER_ID))

    const responses = await Promise.all(
      [GEMINI_FLASH_MODEL_ID, KIMI_K3_FIREWORKS_MODEL_ID].map(model =>
        Effect.runPromise(
          handleChatCompletions(
            new Request('https://openagents.com/v1/chat/completions', {
              body: JSON.stringify({
                max_tokens: 131_072,
                messages: [
                  {
                    content:
                      model === KIMI_K3_FIREWORKS_MODEL_ID
                        ? [
                            {
                              text: 'Can you describe this image?',
                              type: 'text',
                            },
                            {
                              image_url: {
                                url: 'https://images.example.test/photo.jpg',
                              },
                              type: 'image_url',
                            },
                          ]
                        : 'Hello',
                    role: 'user',
                  },
                ],
                model,
                top_k: 40,
              }),
              method: 'POST',
            }),
            deps({
              internalAccountRefs: new Set(['agent:test-user']),
              laneArming: {
                fireworks: true,
                hydralisk: false,
                openrouter: false,
                'openagents-network': false,
                'vertex-anthropic': true,
                'vertex-gemini': true,
              },
              lanePlan: selectAdapterPlan,
              registry,
            }),
          ),
        ),
      ),
    )
    expect(responses.map(response => response.status)).toEqual([200, 200])

    expect(calls.map(call => call.adapterId)).toEqual([
      VERTEX_GEMINI_ADAPTER_ID,
      FIREWORKS_ADAPTER_ID,
    ])
    expect(calls[1]?.request.model).toBe(KIMI_K3_FIREWORKS_MODEL_ID)
    expect(calls[1]?.request.messages[0]?.contentParts).toHaveLength(2)
    expect(calls[1]?.request.passthroughParams).toMatchObject({
      max_tokens: 131_072,
      top_k: 40,
    })
  })

  test('allows an OpenAuth session to select Kimi K3 without agent token or BYOK', async () => {
    const calls: Array<{
      adapterId: string
      request: InferenceRequest
    }> = []
    const registry = new InferenceProviderRegistry()
    registry.register({
      complete: inferenceRequest => {
        calls.push({ adapterId: FIREWORKS_ADAPTER_ID, request: inferenceRequest })
        return Effect.succeed({
          content: 'served by fireworks',
          finishReason: 'stop',
          servedModel: inferenceRequest.model,
          usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
        })
      },
      id: FIREWORKS_ADAPTER_ID,
      stream: inferenceRequest =>
        Effect.succeed([
          {
            contentDelta: 'served by fireworks',
            finishReason: 'stop',
            servedModel: inferenceRequest.model,
            usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
          },
        ]),
    })

    const authOpenAuth: InferenceAuth = async () => ({
      accountRef: 'openauth:omega-desktop-user',
    })

    const response = await Effect.runPromise(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify({
            messages: [{ content: 'hello', role: 'user' }],
            model: 'kimi-k3',
            stream: false,
          }),
          method: 'POST',
        }),
        deps({
          authenticate: authOpenAuth,
          laneArming: {
            fireworks: true,
            hydralisk: false,
            openrouter: false,
            'openagents-network': false,
            'vertex-anthropic': false,
            'vertex-gemini': true,
          },
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.adapterId).toBe(FIREWORKS_ADAPTER_ID)
    expect(calls[0]?.request.model).toBe('kimi-k3')
  })

  test('rejects an OpenAuth session for non-hosted public models without funding', async () => {
    const authOpenAuth: InferenceAuth = async () => ({
      accountRef: 'openauth:omega-desktop-user',
    })

    const response = await Effect.runPromise(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify({
            messages: [{ content: 'hello', role: 'user' }],
            model: KHALA_MODEL_ID,
          }),
          method: 'POST',
        }),
        deps({ authenticate: authOpenAuth }),
      ),
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toMatchObject({
      error: 'platform_funding_unavailable',
    })
  })

  test('allows a verified OpenAuth account to select Gemini Flash', async () => {
    const { calls, registry } = makeHostedRegistry()
    const response = await Effect.runPromise(
      handleChatCompletions(
        hostedRequest(GEMINI_FLASH_MODEL_ID),
        deps({
          authenticate: async () => ({ accountRef: 'openauth:test-user' }),
          laneArming: hostedLaneArming,
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    expect(calls).toEqual([VERTEX_GEMINI_ADAPTER_ID])
  })

  test('allows an OpenAuth session to select gpt-5.6-luna over passthrough-openai', async () => {
    const calls: Array<{ adapterId: string; request: InferenceRequest }> = []
    const registry = new InferenceProviderRegistry()
    registry.register({
      complete: inferenceRequest => {
        calls.push({
          adapterId: PASSTHROUGH_OPENAI_ADAPTER_ID,
          request: inferenceRequest,
        })
        return Effect.succeed({
          content: 'served by passthrough-openai',
          finishReason: 'stop',
          servedModel: inferenceRequest.model,
          usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
        })
      },
      id: PASSTHROUGH_OPENAI_ADAPTER_ID,
      stream: inferenceRequest =>
        Effect.succeed([
          {
            contentDelta: 'served by passthrough-openai',
            finishReason: 'stop',
            servedModel: inferenceRequest.model,
            usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
          },
        ]),
    })

    const response = await Effect.runPromise(
      handleChatCompletions(
        hostedRequest(GPT_56_LUNA_MODEL_ID),
        deps({
          authenticate: async () => ({
            accountRef: 'openauth:omega-desktop-user',
          }),
          laneArming: { ...hostedLaneArming, passthroughOpenAi: true },
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.adapterId).toBe(PASSTHROUGH_OPENAI_ADAPTER_ID)
    expect(calls[0]?.request.model).toBe(GPT_56_LUNA_MODEL_ID)
  })

  // REGRESSION — omega#160. The hosted Luna lane rendered NOTHING in Omega:
  // the turn was accepted, the upstream succeeded, no error was raised, and the
  // client showed a spinner and then an empty message. A coding client always
  // sends tools, `gpt-5.6-luna` answers such a turn with `content: null` and
  // the whole answer in `tool_calls`, and the streamed projection of the
  // partner result carried content only — so the SSE body had no payload at
  // all. This exercises the REAL passthrough adapter (not a stub) end to end
  // through the route, because the seam that broke was between the two.
  //
  // The upstream fixture is now the partner's REAL SSE, because the adapter now
  // asks for `stream: true` and pumps frames through as they arrive (the "luna
  // does not stream" P0). The tool call's arguments are split across frames the
  // way the partner actually sends them, so this also asserts the client sees
  // PROGRESSIVE frames rather than one materialized block.
  test('streams a gpt-5.6-luna tool call to the client instead of an empty body', async () => {
    const registry = new InferenceProviderRegistry()
    const lunaSseFrames = [
      {
        choices: [
          {
            delta: {
              tool_calls: [
                {
                  function: { arguments: '', name: 'read_file' },
                  id: 'call_luna_1',
                  index: 0,
                  type: 'function',
                },
              ],
            },
            index: 0,
          },
        ],
        model: GPT_56_LUNA_MODEL_ID,
      },
      {
        choices: [
          {
            delta: {
              tool_calls: [{ function: { arguments: '{"pa' }, index: 0 }],
            },
            index: 0,
          },
        ],
      },
      {
        choices: [
          {
            delta: {
              tool_calls: [
                { function: { arguments: 'th":"src/main.rs"}' }, index: 0 },
              ],
            },
            index: 0,
          },
        ],
      },
      {
        choices: [{ delta: {}, finish_reason: 'tool_calls', index: 0 }],
        model: GPT_56_LUNA_MODEL_ID,
        usage: {
          completion_tokens: 18,
          prompt_tokens: 141,
          total_tokens: 159,
        },
      },
    ]
    registry.register(
      makePassthroughAdapter({
        apiKey: Redacted.make('sk-openai-test'),
        baseUrl: 'https://api.openai.com',
        fetch: async () =>
          new Response(
            [
              ...lunaSseFrames.map(frame => `data: ${JSON.stringify(frame)}\n\n`),
              'data: [DONE]\n\n',
            ].join(''),
            { headers: { 'content-type': 'text/event-stream' }, status: 200 },
          ),
        id: PASSTHROUGH_OPENAI_ADAPTER_ID,
        wireFormat: 'openai',
      }),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify({
            max_completion_tokens: 128_000,
            messages: [{ content: 'what is in src/main.rs?', role: 'user' }],
            model: GPT_56_LUNA_MODEL_ID,
            stream: true,
            tools: [
              {
                function: {
                  description: 'Read a file from the project',
                  name: 'read_file',
                  parameters: {
                    properties: { path: { type: 'string' } },
                    required: ['path'],
                    type: 'object',
                  },
                },
                type: 'function',
              },
            ],
          }),
          method: 'POST',
        }),
        deps({
          authenticate: async () => ({
            accountRef: 'openauth:omega-desktop-user',
          }),
          laneArming: { ...hostedLaneArming, passthroughOpenAi: true },
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    const body = await response.text()
    // The tool call the model actually made reaches the client.
    expect(body).toContain('"tool_calls"')
    expect(body).toContain('call_luna_1')
    expect(body).toContain('read_file')
    expect(body).toContain('"finish_reason":"tool_calls"')
    expect(body).toContain('data: [DONE]')

    // PROGRESSIVE, not all-at-once: the argument fragments the partner streamed
    // reach the client as separate SSE frames, in order, unassembled. A body
    // carrying one pre-joined `{"path":"src/main.rs"}` would mean the route had
    // buffered the whole completion before emitting a byte — the P0 defect.
    const dataFrames = body
      .split('\n')
      .filter(line => line.startsWith('data: ') && !line.includes('[DONE]'))
    expect(dataFrames.length).toBeGreaterThan(2)
    const argumentFragments = dataFrames.flatMap(line => {
      const parsed = JSON.parse(line.slice('data: '.length)) as {
        choices?: ReadonlyArray<{
          delta?: {
            tool_calls?: ReadonlyArray<{ function?: { arguments?: string } }>
          }
        }>
      }
      return (parsed.choices?.[0]?.delta?.tool_calls ?? []).map(
        call => call.function?.arguments,
      )
    })
    expect(argumentFragments).toEqual(['', '{"pa', 'th":"src/main.rs"}'])
  })

  test('fails closed for gpt-5.6-luna when the passthrough-openai arming is absent', async () => {
    const { calls, registry } = makeHostedRegistry()
    const response = await Effect.runPromise(
      handleChatCompletions(
        hostedRequest(GPT_56_LUNA_MODEL_ID),
        deps({
          authenticate: async () => ({
            accountRef: 'openauth:omega-desktop-user',
          }),
          // No `passthroughOpenAi` field: absent must mean unarmed.
          laneArming: hostedLaneArming,
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({
      error: 'model_unavailable',
      model: GPT_56_LUNA_MODEL_ID,
    })
    expect(calls).toHaveLength(0)
  })

  test('denies hosted lanes to a non-internal agent account', async () => {
    const { calls, registry } = makeHostedRegistry()
    const response = await Effect.runPromise(
      handleChatCompletions(
        hostedRequest(GEMINI_FLASH_MODEL_ID),
        deps({
          laneArming: hostedLaneArming,
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({
      error: 'model_unavailable',
      model: GEMINI_FLASH_MODEL_ID,
    })
    expect(calls).toHaveLength(0)
  })

  test.each([
    [
      GEMINI_FLASH_MODEL_ID,
      { ...hostedLaneArming, 'vertex-gemini': false },
    ],
    [
      KIMI_K3_FIREWORKS_MODEL_ID,
      { ...hostedLaneArming, fireworks: false },
    ],
  ])(
    'fails closed when the %s hosted lane is disabled',
    async (model, laneArming) => {
      const { calls, registry } = makeHostedRegistry()
      const response = await Effect.runPromise(
        handleChatCompletions(
          hostedRequest(model),
          deps({
            authenticate: async () => ({ accountRef: 'openauth:test-user' }),
            laneArming,
            lanePlan: selectAdapterPlan,
            registry,
          }),
        ),
      )

      expect(response.status).toBe(400)
      expect(await response.json()).toMatchObject({
        error: 'model_unavailable',
        model,
      })
      expect(calls).toHaveLength(0)
    },
  )
})
