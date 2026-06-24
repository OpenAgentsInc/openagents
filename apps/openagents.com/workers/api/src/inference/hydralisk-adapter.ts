// Hydralisk vLLM provider adapter for the inference gateway.
//
// Hydralisk is the OpenAgents-owned Python/NVIDIA lane. It exposes an
// OpenAI-compatible API, but it is not a generic partner passthrough: the
// gateway routes only the bounded GPT-OSS model id to this adapter and arms it
// only when public-safe preflight + receipt refs are configured. This adapter
// keeps the existing provider seam receipt-first by requiring terminal usage on
// non-streaming responses and by parsing terminal usage from streaming SSE.
import { Effect, Redacted } from 'effect'

import { parseJsonRecord, recordFromUnknown } from '../json-boundary'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

type HydraliskResponse = Response

export type HydraliskFetch = (
  input: string,
  init: RequestInit,
) => Promise<HydraliskResponse>

export type HydraliskAdapterConfig = Readonly<{
  id: string
  apiKey: Redacted.Redacted<string>
  baseUrl: string
  fetchImpl?: HydraliskFetch | undefined
  upstreamModel?: string | undefined
}>

const requestUrl = (config: HydraliskAdapterConfig): string =>
  `${config.baseUrl.replace(/\/+$/u, '')}/v1/chat/completions`

const classifyStatus = (
  status: number,
): Readonly<{ kind: string; retryable: boolean }> => {
  if (status === 429) {
    return { kind: 'rate_limited', retryable: true }
  }
  if (status === 503) {
    return { kind: 'service_overloaded', retryable: true }
  }
  if (status >= 500) {
    return { kind: 'upstream_error', retryable: true }
  }
  // Lane-level auth/routing failures (401 unauthorized, 403 forbidden, 404 not
  // found) mean THIS serving lane is unavailable or misconfigured — not that the
  // request itself is bad. Treat them as retryable so `dispatchWithOverflow`
  // overflows to the next lane (e.g. the Khala Vertex Gemini fallback) instead of
  // failing the whole request. Genuine client errors (400/422) stay non-retryable.
  if (status === 401 || status === 403 || status === 404) {
    return { kind: 'lane_unavailable', retryable: true }
  }
  return { kind: 'request_rejected', retryable: false }
}

const adapterError = (
  config: HydraliskAdapterConfig,
  input: Readonly<{
    reason: string
    httpStatus?: number | undefined
    retryable?: boolean | undefined
    kind?: string | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: config.id,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable,
  })

const streamOptions = (
  passthroughParams: Readonly<Record<string, unknown>>,
): Record<string, unknown> => ({
  ...(recordFromUnknown(passthroughParams['stream_options']) ?? {}),
  include_usage: true,
})

const toRequestBody = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Record<string, unknown> => {
  const { 'x-session-affinity': _affinity, ...params } =
    request.passthroughParams
  return {
    ...params,
    messages: request.messages.map(message => ({
      content: message.content,
      role: message.role,
    })),
    model: config.upstreamModel ?? request.model,
    stream: request.stream,
    ...(request.stream
      ? { stream_options: streamOptions(request.passthroughParams) }
      : {}),
  }
}

const headersFor = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Record<string, string> => ({
  accept: request.stream ? 'text/event-stream' : 'application/json',
  authorization: `Bearer ${Redacted.value(config.apiKey)}`,
  'content-type': 'application/json',
})

const postChatCompletions = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<HydraliskResponse, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: error =>
      adapterError(config, {
        kind: 'transport_error',
        reason: `retryable: hydralisk transport error (${
          error instanceof Error ? error.name : 'unknown'
        })`,
        retryable: true,
      }),
    try: () => {
      const fetcher = config.fetchImpl ?? globalThis.fetch
      return fetcher(requestUrl(config), {
      body: JSON.stringify(toRequestBody(config, request)),
        headers: headersFor(config, request),
        method: 'POST',
      })
    },
  })

const failForStatus = (
  config: HydraliskAdapterConfig,
  response: HydraliskResponse,
): Effect.Effect<never, InferenceAdapterError> => {
  const classified = classifyStatus(response.status)
  return Effect.fail(
    adapterError(config, {
      httpStatus: response.status,
      kind: classified.kind,
      reason:
        response.status === 429
          ? 'hydralisk rate limited request (429)'
          : `hydralisk rejected request (${response.status})`,
      retryable: classified.retryable,
    }),
  )
}

const parseResponseJson = (
  config: HydraliskAdapterConfig,
  response: HydraliskResponse,
): Effect.Effect<Record<string, unknown>, InferenceAdapterError> =>
  Effect.gen(function* () {
    const value = yield* Effect.tryPromise({
      catch: () =>
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'hydralisk returned a non-JSON response',
          retryable: false,
        }),
      try: () => response.json(),
    })
    const parsed = recordFromUnknown(value)
    if (parsed === undefined) {
      return yield* Effect.fail(
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'hydralisk returned a non-JSON response',
          retryable: false,
        }),
      )
    }
    return parsed
  })

const extractUsage = (raw: unknown): InferenceUsage | undefined => {
  const record = recordFromUnknown(raw)
  const usage = recordFromUnknown(record?.['usage'])
  if (usage === undefined) {
    return undefined
  }
  const promptTokens = Number(usage['prompt_tokens'])
  const completionTokens = Number(usage['completion_tokens'])
  const totalTokens = Number(usage['total_tokens'])
  if (
    !Number.isFinite(promptTokens) ||
    !Number.isFinite(completionTokens) ||
    !Number.isFinite(totalTokens)
  ) {
    return undefined
  }
  const details = recordFromUnknown(usage['prompt_tokens_details'])
  const cached =
    details === undefined ? Number.NaN : Number(details['cached_tokens'])
  return {
    completionTokens,
    promptTokens,
    totalTokens,
    ...(Number.isFinite(cached) ? { cachedPromptTokens: cached } : {}),
  }
}

const firstChoice = (raw: unknown): Record<string, unknown> | undefined => {
  const record = recordFromUnknown(raw)
  const choices = record?.['choices']
  if (!Array.isArray(choices) || choices.length === 0) {
    return undefined
  }
  return recordFromUnknown(choices[0])
}

const servedModelFrom = (raw: unknown, fallback: string): string => {
  const record = recordFromUnknown(raw)
  const model = record?.['model']
  return typeof model === 'string' && model.length > 0 ? model : fallback
}

const parseSseData = (line: string): Record<string, unknown> | undefined => {
  const trimmed = line.trim()
  if (!trimmed.startsWith('data:')) {
    return undefined
  }
  const payload = trimmed.slice('data:'.length).trim()
  if (payload === '' || payload === '[DONE]') {
    return undefined
  }
  return parseJsonRecord(payload)
}

const deltaContentOf = (frame: Record<string, unknown>): string => {
  const delta = recordFromUnknown(firstChoice(frame)?.['delta'])
  const content = delta?.['content']
  return typeof content === 'string' ? content : ''
}

const finishReasonOf = (frame: Record<string, unknown>): string | undefined => {
  const reason = firstChoice(frame)?.['finish_reason']
  return typeof reason === 'string' && reason.length > 0 ? reason : undefined
}

const eventForFrame = (
  frame: Record<string, unknown>,
): InferenceStreamEvent => {
  const event: {
    contentDelta: string
    finishReason?: string
    usage?: InferenceUsage
    servedModel?: string
  } = { contentDelta: deltaContentOf(frame) }
  const reason = finishReasonOf(frame)
  if (reason !== undefined) {
    event.finishReason = reason
  }
  const usage = extractUsage(frame)
  if (usage !== undefined) {
    event.usage = usage
  }
  const model = frame['model']
  if (typeof model === 'string' && model.length > 0) {
    event.servedModel = model
  }
  return event
}

const makeSseSource = (
  body: ReadableStream<Uint8Array>,
  fallbackModel: string,
): InferenceStreamSource => {
  let finishReason: string | undefined
  let usage: InferenceUsage | undefined
  let servedModel: string | undefined = fallbackModel

  const frames = (async function* (): AsyncIterable<InferenceStreamEvent> {
    const reader = body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    try {
      for (;;) {
        const { done, value } = await reader.read()
        if (value !== undefined) {
          buffer += decoder.decode(value, { stream: true })
        }
        let newlineIndex = buffer.indexOf('\n')
        while (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex)
          buffer = buffer.slice(newlineIndex + 1)
          const frame = parseSseData(line)
          if (frame !== undefined) {
            const event = eventForFrame(frame)
            if (event.finishReason !== undefined) {
              finishReason = event.finishReason
            }
            if (event.usage !== undefined) {
              usage = event.usage
            }
            if (event.servedModel !== undefined) {
              servedModel = event.servedModel
            }
            yield event
          }
          newlineIndex = buffer.indexOf('\n')
        }
        if (done) {
          const tail = parseSseData(buffer)
          if (tail !== undefined) {
            const event = eventForFrame(tail)
            if (event.finishReason !== undefined) {
              finishReason = event.finishReason
            }
            if (event.usage !== undefined) {
              usage = event.usage
            }
            if (event.servedModel !== undefined) {
              servedModel = event.servedModel
            }
            yield event
          }
          break
        }
      }
    } finally {
      reader.releaseLock()
    }
  })()

  return {
    frames,
    terminal: () => ({ finishReason, servedModel, usage }),
  }
}

const toResult = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
  raw: Record<string, unknown>,
): Effect.Effect<InferenceResult, InferenceAdapterError> => {
  const usage = extractUsage(raw)
  if (usage === undefined) {
    return Effect.fail(
      adapterError(config, {
        kind: 'malformed_response',
        reason: 'hydralisk response missing terminal usage',
        retryable: false,
      }),
    )
  }
  const choice = firstChoice(raw)
  const message = recordFromUnknown(choice?.['message'])
  const content =
    typeof message?.['content'] === 'string'
      ? (message['content'] as string)
      : ''
  const finishReason =
    typeof choice?.['finish_reason'] === 'string'
      ? (choice['finish_reason'] as string)
      : 'stop'
  return Effect.succeed({
    content,
    finishReason,
    servedModel: servedModelFrom(raw, request.model),
    usage,
  })
}

const complete = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<InferenceResult, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postChatCompletions(config, {
      ...request,
      stream: false,
    })
    if (!response.ok) {
      return yield* failForStatus(config, response)
    }
    const raw = yield* parseResponseJson(config, response)
    return yield* toResult(config, request, raw)
  })

const streamChunks = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<ReadonlyArray<InferenceStreamChunk>, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postChatCompletions(config, {
      ...request,
      stream: true,
    })
    if (!response.ok) {
      return yield* failForStatus(config, response)
    }
    const body = yield* Effect.tryPromise({
      catch: () =>
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'hydralisk returned an unreadable stream',
          retryable: false,
        }),
      try: () => response.text(),
    })
    const frames = body
      .split('\n')
      .map(parseSseData)
      .filter((frame): frame is Record<string, unknown> => frame !== undefined)

    const contentChunks: Array<InferenceStreamChunk> = []
    let finishReason: string | undefined
    let servedModel = request.model
    let usage: InferenceUsage | undefined

    for (const frame of frames) {
      servedModel = servedModelFrom(frame, servedModel)
      const delta = deltaContentOf(frame)
      if (delta !== '') {
        contentChunks.push({ contentDelta: delta })
      }
      const reason = finishReasonOf(frame)
      if (reason !== undefined) {
        finishReason = reason
      }
      const frameUsage = extractUsage(frame)
      if (frameUsage !== undefined) {
        usage = frameUsage
      }
    }

    return [
      ...contentChunks,
      {
        contentDelta: '',
        finishReason: finishReason ?? 'stop',
        servedModel,
        ...(usage === undefined ? {} : { usage }),
      },
    ]
  })

export const makeHydraliskVllmAdapter = (
  config: HydraliskAdapterConfig,
): InferenceProviderAdapter => ({
  complete: request => complete(config, request),
  id: config.id,
  stream: request => streamChunks(config, request),
  streamSse: request =>
    Effect.gen(function* () {
      const response = yield* postChatCompletions(config, {
        ...request,
        stream: true,
      })
      if (!response.ok) {
        return yield* failForStatus(config, response)
      }
      if (response.body === null) {
        return yield* Effect.fail(
          adapterError(config, {
            kind: 'malformed_response',
            reason: 'hydralisk stream had no response body',
            retryable: false,
          }),
        )
      }
      return makeSseSource(response.body, request.model)
    }),
})
