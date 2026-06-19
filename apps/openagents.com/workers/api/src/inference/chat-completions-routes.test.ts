import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ChatCompletionsDeps,
  type InferenceAuth,
  type InferenceBalanceReader,
  handleChatCompletions,
  isInferenceGatewayEnabled,
} from './chat-completions-routes'
import {
  type MeteringContext,
  type MeteringHook,
} from './metering-hook'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
} from './provider-adapter'
import { STUB_ECHO_ADAPTER_ID, stubEchoAdapter } from './stub-echo-adapter'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const authOk: InferenceAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: InferenceAuth = async () => undefined
const fundedBalance: InferenceBalanceReader = async () => 100_000
const emptyBalance: InferenceBalanceReader = async () => 0

const registryWithStub = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(stubEchoAdapter)
  return registry
}

const baseDeps = (
  overrides: Partial<ChatCompletionsDeps> = {},
): ChatCompletionsDeps => ({
  authenticate: authOk,
  enabled: true,
  readAvailableMsat: fundedBalance,
  registry: registryWithStub(),
  ...overrides,
})

const chatRequest = (
  body: unknown,
  init: RequestInit = {},
): Request =>
  new Request('https://openagents.com/v1/chat/completions', {
    body: JSON.stringify(body),
    method: 'POST',
    ...init,
  })

const helloBody = {
  messages: [{ content: 'hello world', role: 'user' }],
  model: 'stub-model',
}

describe('inference gateway feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isInferenceGatewayEnabled(undefined)).toBe(false)
    expect(isInferenceGatewayEnabled('')).toBe(false)
    expect(isInferenceGatewayEnabled('false')).toBe(false)
    expect(isInferenceGatewayEnabled('0')).toBe(false)
    expect(isInferenceGatewayEnabled('true')).toBe(true)
    expect(isInferenceGatewayEnabled('TRUE')).toBe(true)
    expect(isInferenceGatewayEnabled('1')).toBe(true)
    expect(isInferenceGatewayEnabled('on')).toBe(true)
  })
})

describe('POST /v1/chat/completions', () => {
  test('is inert (404) when the gateway flag is disabled', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ enabled: false }),
      ),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('inference_gateway_disabled')
  })

  test('rejects an unauthenticated request with 401 + WWW-Authenticate', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ authenticate: authNone }),
      ),
    )
    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBe('Bearer')
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('unauthorized')
  })

  test('rejects with 402 when the credit balance is insufficient', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ readAvailableMsat: emptyBalance }),
      ),
    )
    expect(response.status).toBe(402)
    const body = (await response.json()) as {
      error: string
      availableMsat: number
    }
    expect(body.error).toBe('insufficient_credits')
    expect(body.availableMsat).toBe(0)
  })

  test('rejects a malformed body with 400', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest({ model: 'stub-model', messages: [] }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('invalid_request')
  })

  test('dispatches to the registered stub adapter and returns OpenAI shape', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ newId: () => 'chatcmpl-fixed', nowEpochSeconds: () => 1_700_000_000 }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      id: string
      object: string
      created: number
      model: string
      choices: ReadonlyArray<{
        index: number
        finish_reason: string
        message: { role: string; content: string }
      }>
      usage: {
        prompt_tokens: number
        completion_tokens: number
        total_tokens: number
      }
    }
    expect(body.object).toBe('chat.completion')
    expect(body.id).toBe('chatcmpl-fixed')
    expect(body.created).toBe(1_700_000_000)
    expect(body.model).toBe('stub-model')
    expect(body.choices[0]?.message.role).toBe('assistant')
    expect(body.choices[0]?.message.content).toBe('hello world')
    expect(body.choices[0]?.finish_reason).toBe('stop')
    expect(body.usage.prompt_tokens).toBe(2)
    expect(body.usage.completion_tokens).toBe(2)
    expect(body.usage.total_tokens).toBe(4)
  })

  test('returns model_unavailable when no adapter is registered for the route', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ registry: new InferenceProviderRegistry() }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('model_unavailable')
  })

  test('invokes the metering hook with receipt-first provider usage', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    await run(
      handleChatCompletions(chatRequest(helloBody), baseDeps({ meteringHook })),
    )

    expect(captured).toHaveLength(1)
    const context = captured[0]
    expect(context?.accountRef).toBe('agent:test-user')
    expect(context?.adapterId).toBe(STUB_ECHO_ADAPTER_ID)
    expect(context?.requestedModel).toBe('stub-model')
    expect(context?.streamed).toBe(false)
    expect(context?.usage.totalTokens).toBe(4)
    // Funding kind defaults to card, and the request id is threaded for
    // idempotency-keyed metering.
    expect(context?.fundingKind).toBe('card')
    expect(typeof context?.requestId).toBe('string')
    expect((context?.requestId ?? '').length).toBeGreaterThan(0)
  })

  test('threads the resolved bitcoin funding kind into the metering hook', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          meteringHook,
          resolveFundingKind: async () => 'bitcoin',
        }),
      ),
    )

    expect(captured).toHaveLength(1)
    expect(captured[0]?.fundingKind).toBe('bitcoin')
  })

  test('streams OpenAI-compatible SSE frames and meters from the terminal usage frame', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    const response = await run(
      handleChatCompletions(
        chatRequest({ ...helloBody, stream: true }),
        baseDeps({ meteringHook }),
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')

    const text = await response.text()
    expect(text).toContain('"object":"chat.completion.chunk"')
    expect(text).toContain('"content":"hello world"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)

    expect(captured).toHaveLength(1)
    expect(captured[0]?.streamed).toBe(true)
    expect(captured[0]?.usage.completionTokens).toBe(2)
  })

  test('maps a provider adapter failure to a 502 provider_error', async () => {
    const failingAdapter: InferenceProviderAdapter = {
      complete: () =>
        Effect.fail(
          new InferenceAdapterError({ adapterId: 'boom', reason: 'upstream down' }),
        ),
      id: STUB_ECHO_ADAPTER_ID,
      stream: () =>
        Effect.fail(
          new InferenceAdapterError({ adapterId: 'boom', reason: 'upstream down' }),
        ),
    }
    const registry = new InferenceProviderRegistry()
    registry.register(failingAdapter)

    const response = await run(
      handleChatCompletions(chatRequest(helloBody), baseDeps({ registry })),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('provider_error')
    expect(body.reason).toBe('upstream down')
  })

  // ROUTING & SUPPLY SELECTION (#5482) -------------------------------------
  // The route accepts a multi-lane `lanePlan` and dispatches across it with
  // bounded-backoff overflow. These exercise the route wiring (the pure router
  // logic itself is covered in model-router.test.ts).

  const echoAdapter = (id: string): InferenceProviderAdapter => ({
    ...stubEchoAdapter,
    id,
  })
  const failing = (
    id: string,
    retryable: boolean,
  ): InferenceProviderAdapter => ({
    complete: () =>
      Effect.fail(
        new InferenceAdapterError({ adapterId: id, reason: `${id} down`, retryable }),
      ),
    id,
    stream: () =>
      Effect.fail(
        new InferenceAdapterError({ adapterId: id, reason: `${id} down`, retryable }),
      ),
  })

  test('overflows to the next lane on a retryable failure and meters the served lane', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })
    const registry = new InferenceProviderRegistry()
    registry.register(failing('primary', true))
    registry.register(echoAdapter('overflow'))

    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          dispatch: { sleep: () => Effect.void },
          lanePlan: () => ['primary', 'overflow'],
          meteringHook,
          registry,
        }),
      ),
    )
    expect(response.status).toBe(200)
    // Metering attributes the request to the lane that actually served it.
    expect(captured).toHaveLength(1)
    expect(captured[0]?.adapterId).toBe('overflow')
  })

  test('surfaces a non-retryable failure as 502 without overflow', async () => {
    const overflow = echoAdapter('overflow')
    let overflowCalls = 0
    const registry = new InferenceProviderRegistry()
    registry.register(failing('primary', false))
    registry.register({
      ...overflow,
      complete: request => {
        overflowCalls += 1
        return overflow.complete(request)
      },
    })

    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          dispatch: { sleep: () => Effect.void },
          lanePlan: () => ['primary', 'overflow'],
          registry,
        }),
      ),
    )
    expect(response.status).toBe(502)
    expect(overflowCalls).toBe(0)
  })

  test('returns model_unavailable when no planned lane is registered', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          lanePlan: () => ['vertex-anthropic'],
          registry: new InferenceProviderRegistry(),
        }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('model_unavailable')
  })
})

describe('inference provider registry seam', () => {
  test('resolves a registered adapter and reports its ids', () => {
    const registry = registryWithStub()
    expect(registry.resolve(STUB_ECHO_ADAPTER_ID)?.id).toBe(STUB_ECHO_ADAPTER_ID)
    expect(registry.resolve('not-registered')).toBeUndefined()
    expect(registry.ids()).toEqual([STUB_ECHO_ADAPTER_ID])
  })
})
