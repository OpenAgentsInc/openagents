// OpenRouter Khala fallback adapter (#6313).
//
// OpenRouter is used only as a hidden Khala overflow lane. The public customer
// model remains `openagents/khala`; the OpenRouter upstream model id is supplied
// by Worker env at registration time and is never added to the public model
// catalog. The adapter speaks the OpenAI-compatible Chat Completions shape and
// normalizes back into the shared provider-adapter contract.
import { Effect, Redacted } from 'effect'

import { parseJsonRecord, recordFromUnknown } from '../json-boundary'
import {
  inferenceToolCallsFromUnknown,
  openAiWireMessageFromInferenceMessage,
} from './openai-chat-compat'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceToolCall,
  type InferenceToolCallDelta,
  type InferenceUsage,
} from './provider-adapter'

type OpenRouterResponse = Response

export type OpenRouterFetch = (
  input: string,
  init: Readonly<{
    body: string
    headers: Record<string, string>
    method: string
    signal?: AbortSignal | undefined
  }>,
) => Promise<OpenRouterResponse>

export type OpenRouterAdapterConfig = Readonly<{
  id: string
  apiKey: Redacted.Redacted<string>
  baseUrl: string
  upstreamModel: string
  fetchImpl?: OpenRouterFetch | undefined
  timeoutMs?: number | undefined
}>

export const OPENROUTER_DEFAULT_TIMEOUT_MS = 60_000
export const OPENROUTER_DEFAULT_BASE_URL = 'https://openrouter.ai/api/v1'
export const OPENROUTER_KHALA_FALLBACK_MODEL_ID = 'openrouter/free'

const OPENAI_FORWARDED_PARAMS = [
  'temperature',
  'top_p',
  'frequency_penalty',
  'presence_penalty',
  'stop',
  'seed',
  'tools',
  'tool_choice',
  'parallel_tool_calls',
] as const

const numberParam = (
  params: Readonly<Record<string, unknown>>,
  key: string,
): number | undefined => {
  const value = params[key]
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

const forwardParams = (
  params: Readonly<Record<string, unknown>>,
): Record<string, unknown> => {
  const out: Record<string, unknown> = {}
  for (const key of OPENAI_FORWARDED_PARAMS) {
    if (params[key] !== undefined) {
      out[key] = params[key]
    }
  }
  return out
}

const requestBody = (
  config: OpenRouterAdapterConfig,
  request: InferenceRequest,
): Record<string, unknown> => ({
  ...forwardParams(request.passthroughParams),
  messages: request.messages.map(openAiWireMessageFromInferenceMessage),
  model: config.upstreamModel,
  ...(numberParam(request.passthroughParams, 'max_tokens') === undefined
    ? {}
    : { max_tokens: numberParam(request.passthroughParams, 'max_tokens') }),
  stream: false,
})

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
  return { kind: 'request_rejected', retryable: false }
}

const adapterError = (
  config: OpenRouterAdapterConfig,
  input: Readonly<{
    reason: string
    httpStatus?: number | undefined
    kind?: string | undefined
    retryable?: boolean | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: config.id,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable,
  })

const safeSignal = (timeoutMs: number): AbortSignal | undefined => {
  try {
    return AbortSignal.timeout(timeoutMs)
  } catch {
    return undefined
  }
}

const endpointFor = (baseUrl: string): string =>
  `${baseUrl.replace(/\/+$/u, '')}/chat/completions`

const postChatCompletions = (
  config: OpenRouterAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<OpenRouterResponse, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: error =>
      adapterError(config, {
        kind: 'transport_error',
        reason: `retryable: openrouter transport error (${
          error instanceof Error ? error.name : 'unknown'
        })`,
        retryable: true,
      }),
    try: () => {
      const fetcher = config.fetchImpl ?? (globalThis.fetch as OpenRouterFetch)
      const apiKey = request.callerProviderKey ?? config.apiKey
      const signal = safeSignal(
        config.timeoutMs ?? OPENROUTER_DEFAULT_TIMEOUT_MS,
      )
      return fetcher(endpointFor(config.baseUrl), {
        body: JSON.stringify(requestBody(config, request)),
        headers: {
          accept: 'application/json',
          authorization: `Bearer ${Redacted.value(apiKey)}`,
          'content-type': 'application/json',
        },
        method: 'POST',
        ...(signal === undefined ? {} : { signal }),
      })
    },
  })

const readBody = (
  config: OpenRouterAdapterConfig,
  response: OpenRouterResponse,
): Effect.Effect<string, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: () =>
      adapterError(config, {
        kind: 'malformed_response',
        reason: 'openrouter returned an unreadable response body',
        retryable: false,
      }),
    try: () => response.text(),
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
  return typeof model === 'string' && model.trim() !== '' ? model : fallback
}

const normalizeResult = (
  config: OpenRouterAdapterConfig,
  raw: unknown,
): Effect.Effect<InferenceResult, InferenceAdapterError> => {
  const usage = extractUsage(raw)
  if (usage === undefined) {
    return Effect.fail(
      adapterError(config, {
        kind: 'malformed_response',
        reason: 'openrouter response missing usage object',
        retryable: false,
      }),
    )
  }
  const choice = firstChoice(raw)
  const message = recordFromUnknown(choice?.['message'])
  const toolCalls = inferenceToolCallsFromUnknown(message?.['tool_calls'])
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
    servedModel: servedModelFrom(raw, config.upstreamModel),
    ...(toolCalls === undefined || toolCalls.length === 0 ? {} : { toolCalls }),
    usage,
  })
}

const runCompletion = (
  config: OpenRouterAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<InferenceResult, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postChatCompletions(config, request)
    const body = yield* readBody(config, response)
    const raw = parseJsonRecord(body)
    if (raw === undefined) {
      return yield* Effect.fail(
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'openrouter returned unparseable JSON',
          retryable: false,
        }),
      )
    }
    if (!response.ok) {
      const classified = classifyStatus(response.status)
      return yield* Effect.fail(
        adapterError(config, {
          httpStatus: response.status,
          kind: classified.kind,
          reason:
            response.status === 429
              ? 'retryable: openrouter rate limited (429)'
              : `openrouter rejected request (${response.status})`,
          retryable: classified.retryable,
        }),
      )
    }
    return yield* normalizeResult(config, raw)
  })

const toolCallDeltasFromResult = (
  toolCalls: ReadonlyArray<InferenceToolCall> | undefined,
): ReadonlyArray<InferenceToolCallDelta> | undefined => {
  if (toolCalls === undefined || toolCalls.length === 0) {
    return undefined
  }
  return toolCalls.map((toolCall, index) => ({
    function: {
      arguments: toolCall.function.arguments,
      name: toolCall.function.name,
    },
    id: toolCall.id,
    index,
    type: toolCall.type,
  }))
}

export const makeOpenRouterAdapter = (
  config: OpenRouterAdapterConfig,
): InferenceProviderAdapter => ({
  complete: request => runCompletion(config, { ...request, stream: false }),
  id: config.id,
  stream: request =>
    runCompletion(config, { ...request, stream: false }).pipe(
      Effect.map((result): ReadonlyArray<InferenceStreamChunk> => {
        const toolCallDeltas = toolCallDeltasFromResult(result.toolCalls)
        return [
          {
            contentDelta: result.content,
            ...(toolCallDeltas === undefined ? {} : { toolCallDeltas }),
          },
          {
            contentDelta: '',
            finishReason: result.finishReason,
            servedModel: result.servedModel,
            usage: result.usage,
          },
        ]
      }),
    ),
})
