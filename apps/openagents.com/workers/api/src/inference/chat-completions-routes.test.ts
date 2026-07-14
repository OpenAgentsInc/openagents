import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER,
  type ChatCompletionsDeps,
  type InferenceAuth,
  handleChatCompletions,
} from './chat-completions-routes'
import { KHALA_MODEL_ID } from './pricing'
import { InferenceProviderRegistry } from './provider-adapter'
import { stubEchoAdapter } from './stub-echo-adapter'

const authOk: InferenceAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: InferenceAuth = async () => undefined

const deps = (overrides: Partial<ChatCompletionsDeps> = {}): ChatCompletionsDeps => {
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
    headers,
    method: 'POST',
  })

describe('chat completions no-spend admission', () => {
  test('rejects platform-funded inference instead of converting it into free capacity', async () => {
    const response = await Effect.runPromise(handleChatCompletions(request(), deps()))

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
})
