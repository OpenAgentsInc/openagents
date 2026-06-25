import { Effect, Redacted } from 'effect'
import { describe, expect, it } from 'vitest'

import { makeHydraliskVllmAdapter } from './hydralisk-adapter'
import { HYDRALISK_ADAPTER_ID } from './model-router'
import { HYDRALISK_GPT_OSS_20B_MODEL_ID } from './pricing'
import type { InferenceRequest } from './provider-adapter'

const request = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'Say READY.', role: 'user' }],
  model: HYDRALISK_GPT_OSS_20B_MODEL_ID,
  passthroughParams: { max_tokens: 8 },
  stream: false,
  ...overrides,
})

const responseBody = {
  choices: [
    {
      finish_reason: 'stop',
      message: { content: 'READY' },
    },
  ],
  model: 'openai/gpt-oss-20b',
  usage: {
    completion_tokens: 1,
    prompt_tokens: 7,
    total_tokens: 8,
  },
}

const RETRYABLE_STATUS_CASES = [
  [429, 'rate_limited'],
  [503, 'service_overloaded'],
  [500, 'upstream_error'],
] as const

describe('hydralisk vLLM adapter', () => {
  it('maps the GPT-OSS model id to the Hydralisk OpenAI-compatible endpoint', async () => {
    let captured:
      | Readonly<{
          input: string
          init: RequestInit
          body: Record<string, unknown>
        }>
      | undefined

    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test/',
      fetchImpl: async (input, init) => {
        captured = {
          body: JSON.parse(String(init.body)) as Record<string, unknown>,
          init,
          input,
        }
        return Response.json(responseBody)
      },
      id: HYDRALISK_ADAPTER_ID,
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.content).toBe('READY')
    expect(result.servedModel).toBe('openai/gpt-oss-20b')
    expect(result.usage).toEqual({
      completionTokens: 1,
      promptTokens: 7,
      totalTokens: 8,
    })
    expect(captured?.input).toBe(
      'https://hydralisk.example.test/v1/chat/completions',
    )
    expect(captured?.body.model).toBe(HYDRALISK_GPT_OSS_20B_MODEL_ID)
    expect(captured?.body.stream).toBe(false)
    expect(
      (captured?.init.headers as Record<string, string>).authorization,
    ).toBe('Bearer hydralisk-token')
  })

  it('preserves OpenAI tool metadata in outbound messages and params', async () => {
    let captured: Record<string, unknown> | undefined

    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test/',
      fetchImpl: async (_input, init) => {
        captured = JSON.parse(String(init.body)) as Record<string, unknown>
        return Response.json(responseBody)
      },
      id: HYDRALISK_ADAPTER_ID,
    })

    await Effect.runPromise(
      adapter.complete(
        request({
          messages: [
            {
              content: '',
              role: 'assistant',
              toolCalls: [
                {
                  function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
                  id: 'call_bash',
                  type: 'function',
                },
              ],
            },
            {
              content: '/tmp/project',
              role: 'tool',
              toolCallId: 'call_bash',
            },
          ],
          passthroughParams: {
            max_tokens: 8,
            tool_choice: 'auto',
            tools: [
              {
                function: {
                  name: 'bash',
                  parameters: { type: 'object' },
                },
                type: 'function',
              },
            ],
          },
        }),
      ),
    )

    const messages = captured?.['messages'] as
      | ReadonlyArray<Record<string, unknown>>
      | undefined
    expect(captured?.['tools']).toHaveLength(1)
    expect(captured?.['tool_choice']).toBe('auto')
    expect(messages?.[0]?.['tool_calls']).toEqual([
      {
        function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
        id: 'call_bash',
        type: 'function',
      },
    ])
    expect(messages?.[1]?.['tool_call_id']).toBe('call_bash')
  })

  it('preserves non-streaming assistant tool calls from the provider response', async () => {
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async () =>
        Response.json({
          ...responseBody,
          choices: [
            {
              finish_reason: 'tool_calls',
              message: {
                content: '',
                tool_calls: [
                  {
                    function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
                    id: 'call_bash',
                    type: 'function',
                  },
                ],
              },
            },
          ],
        }),
      id: HYDRALISK_ADAPTER_ID,
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.finishReason).toBe('tool_calls')
    expect(result.toolCalls).toEqual([
      {
        function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
        id: 'call_bash',
        type: 'function',
      },
    ])
  })

  it('fails closed when terminal usage is absent', async () => {
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async () =>
        Response.json({
          choices: [{ finish_reason: 'stop', message: { content: 'READY' } }],
          model: 'openai/gpt-oss-20b',
        }),
      id: HYDRALISK_ADAPTER_ID,
    })

    const outcome = await Effect.runPromise(
      Effect.result(adapter.complete(request())),
    )

    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('malformed_response')
      expect(outcome.failure.retryable).toBe(false)
      expect(outcome.failure.reason).toBe(
        'hydralisk response missing terminal usage',
      )
    }
  })

  it.each(RETRYABLE_STATUS_CASES)(
    'classifies upstream %s as retryable %s',
    async (status, kind) => {
      const adapter = makeHydraliskVllmAdapter({
        apiKey: Redacted.make('hydralisk-token'),
        baseUrl: 'https://hydralisk.example.test',
        fetchImpl: async () => new Response('{}', { status }),
        id: HYDRALISK_ADAPTER_ID,
      })

      const outcome = await Effect.runPromise(
        Effect.result(adapter.complete(request())),
      )

      expect(outcome._tag).toBe('Failure')
      if (outcome._tag === 'Failure') {
        expect(outcome.failure.httpStatus).toBe(status)
        expect(outcome.failure.kind).toBe(kind)
        expect(outcome.failure.retryable).toBe(true)
      }
    },
  )

  it.each(RETRYABLE_STATUS_CASES)(
    'classifies streaming upstream %s as retryable %s',
    async (status, kind) => {
      const adapter = makeHydraliskVllmAdapter({
        apiKey: Redacted.make('hydralisk-token'),
        baseUrl: 'https://hydralisk.example.test',
        fetchImpl: async () => new Response('{}', { status }),
        id: HYDRALISK_ADAPTER_ID,
      })

      const outcome = await Effect.runPromise(
        Effect.result(adapter.streamSse!(request({ stream: true }))),
      )

      expect(outcome._tag).toBe('Failure')
      if (outcome._tag === 'Failure') {
        expect(outcome.failure.httpStatus).toBe(status)
        expect(outcome.failure.kind).toBe(kind)
        expect(outcome.failure.retryable).toBe(true)
      }
    },
  )

  it('parses streaming SSE deltas and terminal usage', async () => {
    let streamedBody: Record<string, unknown> | undefined
    const encoder = new TextEncoder()
    const sse = [
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"content":"RE"},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"content":"ADY"},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":1,"total_tokens":8}}',
      'data: [DONE]',
    ].join('\n\n')
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async (_input, init) => {
        streamedBody = JSON.parse(String(init.body)) as Record<string, unknown>
        return new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(encoder.encode(sse))
              controller.close()
            },
          }),
          { headers: { 'content-type': 'text/event-stream' }, status: 200 },
        )
      },
      id: HYDRALISK_ADAPTER_ID,
    })

    const source = await Effect.runPromise(
      adapter.streamSse!(request({ stream: true })),
    )
    const deltas: Array<string> = []
    for await (const frame of source.frames) {
      if (frame.contentDelta !== '') {
        deltas.push(frame.contentDelta)
      }
    }

    expect(deltas.join('')).toBe('READY')
    expect(streamedBody?.stream).toBe(true)
    expect(streamedBody?.stream_options).toEqual({ include_usage: true })
    expect(source.terminal()).toEqual({
      finishReason: 'stop',
      servedModel: 'openai/gpt-oss-20b',
      usage: { completionTokens: 1, promptTokens: 7, totalTokens: 8 },
    })
  })

  it('preserves streamed tool_call deltas', async () => {
    const encoder = new TextEncoder()
    const sse = [
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_bash","type":"function","function":{"name":"bash"}}]},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\\"cmd\\":\\"pwd\\"}"}}]},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":7,"completion_tokens":4,"total_tokens":11}}',
      'data: [DONE]',
    ].join('\n\n')
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async () =>
        new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(encoder.encode(sse))
              controller.close()
            },
          }),
          { headers: { 'content-type': 'text/event-stream' }, status: 200 },
        ),
      id: HYDRALISK_ADAPTER_ID,
    })

    const source = await Effect.runPromise(
      adapter.streamSse!(request({ stream: true })),
    )
    const toolCallDeltas: Array<unknown> = []
    for await (const frame of source.frames) {
      if (frame.toolCallDeltas !== undefined) {
        toolCallDeltas.push(...frame.toolCallDeltas)
      }
    }

    expect(toolCallDeltas).toEqual([
      {
        function: { name: 'bash' },
        id: 'call_bash',
        index: 0,
        type: 'function',
      },
      { function: { arguments: '{"cmd":"pwd"}' }, index: 0 },
    ])
    expect(source.terminal()).toMatchObject({
      finishReason: 'tool_calls',
      usage: { completionTokens: 4, promptTokens: 7, totalTokens: 11 },
    })
  })
})
