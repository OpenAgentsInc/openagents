import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ChatCompletionsDeps,
  INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER,
  type InferenceAuth,
  handleChatCompletions,
} from './chat-completions-routes'
import {
  FIREWORKS_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  selectAdapterPlan,
} from './model-router'
import {
  GEMINI_FLASH_MODEL_ID,
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
})
