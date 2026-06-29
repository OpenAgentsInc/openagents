import { Cause, Effect, Exit, Option, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type PassthroughAdapterConfig,
  type PassthroughFetch,
  makePassthroughAdapter,
} from './passthrough-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'

const run = <A>(
  effect: Effect.Effect<A, InferenceAdapterError>,
): Promise<Exit.Exit<A, InferenceAdapterError>> => Effect.runPromiseExit(effect)

const successValue = <A>(exit: Exit.Exit<A, InferenceAdapterError>): A => {
  if (!Exit.isSuccess(exit)) {
    throw new Error(`expected success, got: ${String(exit.cause)}`)
  }
  return exit.value
}

const failureError = (
  exit: Exit.Exit<unknown, InferenceAdapterError>,
): InferenceAdapterError => {
  expect(Exit.isFailure(exit)).toBe(true)
  const failure = Exit.isFailure(exit)
    ? Cause.findErrorOption(exit.cause)
    : Option.none()
  if (Option.isNone(failure)) {
    throw new Error('expected a typed InferenceAdapterError failure')
  }
  expect(failure.value).toBeInstanceOf(InferenceAdapterError)
  return failure.value
}

// Capture the last fetch call so we can assert request mapping.
type Captured = { url: string; init: RequestInit }

const fetchReturning = (
  status: number,
  body: unknown,
  captured?: Array<Captured>,
): PassthroughFetch =>
  async (url, init) => {
    captured?.push({ url, init })
    return new Response(JSON.stringify(body), {
      headers: { 'content-type': 'application/json' },
      status,
    })
  }

const fetchRawBody = (
  status: number,
  rawBody: string,
  captured?: Array<Captured>,
): PassthroughFetch =>
  async (url, init) => {
    captured?.push({ url, init })
    return new Response(rawBody, { status })
  }

const openAiConfig = (
  fetch: PassthroughFetch,
  overrides: Partial<PassthroughAdapterConfig> = {},
): PassthroughAdapterConfig => ({
  apiKey: Redacted.make('sk-openai-test'),
  baseUrl: 'https://api.openai.com',
  fetch,
  id: 'passthrough-openai',
  wireFormat: 'openai',
  ...overrides,
})

const anthropicConfig = (
  fetch: PassthroughFetch,
  overrides: Partial<PassthroughAdapterConfig> = {},
): PassthroughAdapterConfig => ({
  apiKey: Redacted.make('sk-ant-test'),
  baseUrl: 'https://api.anthropic.com',
  fetch,
  id: 'passthrough-anthropic',
  wireFormat: 'anthropic',
  ...overrides,
})

const request = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'hello world', role: 'user' }],
  model: 'gpt-test',
  passthroughParams: {},
  stream: false,
  ...overrides,
})

const openAiPayload = {
  choices: [
    {
      finish_reason: 'stop',
      index: 0,
      message: { content: 'hi there', role: 'assistant' },
    },
  ],
  model: 'gpt-test-served',
  usage: {
    completion_tokens: 7,
    prompt_tokens: 12,
    prompt_tokens_details: { cached_tokens: 4 },
    total_tokens: 19,
  },
}

const anthropicPayload = {
  content: [
    { text: 'hi ', type: 'text' },
    { text: 'there', type: 'text' },
    { input: {}, name: 'noop', type: 'tool_use' },
  ],
  model: 'claude-test-served',
  stop_reason: 'end_turn',
  usage: { cache_read_input_tokens: 5, input_tokens: 20, output_tokens: 8 },
}

describe('passthrough adapter — OpenAI wire format', () => {
  test('maps the request to OpenAI Chat Completions and extracts usage', async () => {
    const captured: Array<Captured> = []
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    )

    const exit = await run(
      adapter.complete(
        request({
          messages: [
            { content: 'be terse', role: 'system' },
            { content: 'hello world', role: 'user' },
          ],
          passthroughParams: { max_tokens: 256, temperature: 0.3, top_p: 0.9 },
        }),
      ),
    )
    const result = successValue(exit)

    // Endpoint + auth header mapping.
    expect(captured[0]?.url).toBe('https://api.openai.com/v1/chat/completions')
    const headers = captured[0]?.init.headers as Record<string, string>
    expect(headers.authorization).toBe('Bearer sk-openai-test')

    // Body mapping: model, messages (system preserved), allow-listed params.
    const body = JSON.parse(String(captured[0]?.init.body))
    expect(body.model).toBe('gpt-test')
    expect(body.max_tokens).toBe(256)
    expect(body.temperature).toBe(0.3)
    expect(body.top_p).toBe(0.9)
    expect(body.messages).toEqual([
      { content: 'be terse', role: 'system' },
      { content: 'hello world', role: 'user' },
    ])

    // Receipt-first usage, including the cached-input dimension.
    expect(result.content).toBe('hi there')
    expect(result.finishReason).toBe('stop')
    expect(result.servedModel).toBe('gpt-test-served')
    expect(result.usage.promptTokens).toBe(12)
    expect(result.usage.completionTokens).toBe(7)
    expect(result.usage.totalTokens).toBe(19)
    expect(result.usage.cachedPromptTokens).toBe(4)
  })

  test('falls back to default max_tokens and omits unset params', async () => {
    const captured: Array<Captured> = []
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured), {
        defaultMaxTokens: 2_048,
      }),
    )

    await run(adapter.complete(request()))

    const body = JSON.parse(String(captured[0]?.init.body))
    expect(body.max_tokens).toBe(2_048)
    expect('temperature' in body).toBe(false)
    expect('seed' in body).toBe(false)
  })
})

describe('passthrough adapter — Anthropic wire format', () => {
  test('maps the request to Anthropic Messages, splitting out system', async () => {
    const captured: Array<Captured> = []
    const adapter = makePassthroughAdapter(
      anthropicConfig(fetchReturning(200, anthropicPayload, captured)),
    )

    const exit = await run(
      adapter.complete(
        request({
          messages: [
            { content: 'be terse', role: 'system' },
            { content: 'hello', role: 'user' },
            { content: 'hi', role: 'assistant' },
            { content: 'more', role: 'user' },
          ],
          model: 'claude-test',
          passthroughParams: { max_tokens: 512, top_k: 5 },
        }),
      ),
    )
    const result = successValue(exit)

    // Endpoint + Anthropic auth/version headers.
    expect(captured[0]?.url).toBe('https://api.anthropic.com/v1/messages')
    const headers = captured[0]?.init.headers as Record<string, string>
    expect(headers['x-api-key']).toBe('sk-ant-test')
    expect(headers['anthropic-version']).toBe('2023-06-01')
    expect(headers.authorization).toBeUndefined()

    // system is hoisted out of messages; only user/assistant turns remain.
    const body = JSON.parse(String(captured[0]?.init.body))
    expect(body.model).toBe('claude-test')
    expect(body.max_tokens).toBe(512)
    expect(body.top_k).toBe(5)
    expect(body.system).toBe('be terse')
    expect(body.messages).toEqual([
      { content: 'hello', role: 'user' },
      { content: 'hi', role: 'assistant' },
      { content: 'more', role: 'user' },
    ])

    // Text blocks concatenated; tool_use ignored. Usage from input/output.
    expect(result.content).toBe('hi there')
    expect(result.finishReason).toBe('stop')
    expect(result.servedModel).toBe('claude-test-served')
    expect(result.usage.promptTokens).toBe(20)
    expect(result.usage.completionTokens).toBe(8)
    expect(result.usage.totalTokens).toBe(28)
    expect(result.usage.cachedPromptTokens).toBe(5)
  })

  test('maps Anthropic stop_reason max_tokens to finish_reason length', async () => {
    const adapter = makePassthroughAdapter(
      anthropicConfig(
        fetchReturning(200, { ...anthropicPayload, stop_reason: 'max_tokens' }),
      ),
    )
    const exit = await run(adapter.complete(request({ model: 'claude-test' })))
    expect(successValue(exit).finishReason).toBe('length')
  })
})

describe('passthrough adapter — error mapping', () => {
  test('maps a 429 to a retryable adapter error', async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(429, { error: 'slow down' })),
    )
    const exit = await run(adapter.complete(request()))
    const error = failureError(exit)
    expect(error.adapterId).toBe('passthrough-openai')
    expect(error.reason).toContain('retryable')
    expect(error.reason).toContain('429')
  })

  test('maps a 503 to a retryable adapter error', async () => {
    const adapter = makePassthroughAdapter(
      anthropicConfig(fetchReturning(503, { error: 'overloaded' })),
    )
    const exit = await run(adapter.complete(request({ model: 'claude-test' })))
    const error = failureError(exit)
    expect(error.reason).toContain('retryable')
    expect(error.reason).toContain('503')
  })

  test('maps a 400 to a non-retryable adapter error', async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(400, { error: 'bad model' })),
    )
    const exit = await run(adapter.complete(request()))
    const error = failureError(exit)
    expect(error.reason).toContain('400')
    expect(error.reason).not.toContain('retryable')
  })

  test('maps a transport throw to a retryable adapter error', async () => {
    const throwingFetch: PassthroughFetch = async () => {
      throw new TypeError('network down')
    }
    const adapter = makePassthroughAdapter(openAiConfig(throwingFetch))
    const exit = await run(adapter.complete(request()))
    const error = failureError(exit)
    expect(error.reason).toContain('retryable')
    expect(error.reason).toContain('transport')
  })

  test('maps a non-JSON body to an adapter error', async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchRawBody(200, '<html>nope</html>')),
    )
    const exit = await run(adapter.complete(request()))
    const error = failureError(exit)
    expect(error.reason).toContain('non-JSON')
  })
})

describe('passthrough adapter — streaming', () => {
  test('forces a non-streamed partner call and emits content + terminal usage frames', async () => {
    const captured: Array<Captured> = []
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    )

    const exit = await run(adapter.stream(request({ stream: true })))
    const chunks = successValue(exit)

    // The partner is always asked for a non-streamed response so we settle
    // metering from real usage rather than reconstructing it from deltas.
    const body = JSON.parse(String(captured[0]?.init.body))
    expect(body.stream).toBe(false)

    expect(chunks).toHaveLength(2)
    expect(chunks[0]?.contentDelta).toBe('hi there')
    expect(chunks[0]?.usage).toBeUndefined()
    expect(chunks[1]?.contentDelta).toBe('')
    expect(chunks[1]?.finishReason).toBe('stop')
    expect(chunks[1]?.usage?.totalTokens).toBe(19)
    expect(chunks[1]?.usage?.completionTokens).toBe(7)
  })

  test('propagates a partner failure through stream as a retryable error', async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(500, { error: 'boom' })),
    )
    const exit = await run(adapter.stream(request({ stream: true })))
    expect(Exit.isFailure(exit)).toBe(true)
  })
})
