// Partner passthrough provider adapter for the inference gateway
// (EPIC #5474, #5481).
//
// This adapter is the breadth + burst lane behind the gateway: it forwards a
// normalized InferenceRequest directly to a partner LLM API and returns a
// receipt-first usage object (the inference gateway business doc §3c — "Direct
// Anthropic, OpenAI, and others as breadth + burst capacity beyond our Vertex
// quota"). It is the overflow target routing (#5482) falls through to when our
// own Vertex (#5480) / Fireworks (#5479) quota is exhausted or doesn't cover a
// model.
//
// Two partner wire formats are supported, selected per registered adapter id:
//   - OpenAI Chat Completions (`POST {base}/v1/chat/completions`)
//   - Anthropic Messages       (`POST {base}/v1/messages`)
// Both normalize OUT of our shared InferenceRequest and normalize the partner
// response back IN to our shared InferenceResult / InferenceStreamChunk, so the
// route and metering hook never see partner-specific shapes.
//
// Keys come from Worker secrets (ANTHROPIC_API_KEY / OPENAI_API_KEY), injected
// as Redacted values at registration time; this module never reads process env,
// never commits a key, and never logs key material. Transport problems and
// 429/5xx partner responses surface as a typed retryable InferenceAdapterError
// so routing can fail over to another adapter rather than 500-ing the request.

import { Effect, Redacted } from 'effect'

import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceUsage,
} from './provider-adapter'

// Partner HTTP response. Aliased so the adapter's transport types stay distinct
// from the Worker's own Response-returning route surfaces (those are budgeted by
// the zero-debt architecture check; this is a partner client, not a route).
type PartnerResponse = Response

// Injected fetch so tests can pass a mock without a real network. Matches the
// Worker global `fetch` signature closely enough for our POST-only use.
export type PassthroughFetch = (
  input: string,
  init: RequestInit,
) => Promise<PartnerResponse>

// Partner wire format the adapter speaks.
export type PassthroughWireFormat = 'anthropic' | 'openai'

export type PassthroughAdapterConfig = Readonly<{
  // Stable adapter id, e.g. "passthrough-anthropic" / "passthrough-openai".
  id: string
  wireFormat: PassthroughWireFormat
  // Partner API key from a Worker secret, kept Redacted so it can't be logged.
  apiKey: Redacted.Redacted<string>
  // Partner API origin (no trailing slash), e.g. "https://api.anthropic.com"
  // or "https://api.openai.com". The wire-format-specific path is appended.
  baseUrl: string
  // Injected fetcher; defaults to the Worker global `fetch`.
  fetch?: PassthroughFetch | undefined
  // Request timeout in ms. Defaults to 60s.
  timeoutMs?: number | undefined
  // Anthropic Messages requires an explicit max_tokens; OpenAI treats it as
  // optional. Used as the default when the caller does not pass `max_tokens`
  // in passthroughParams. Defaults to 1024.
  defaultMaxTokens?: number | undefined
  // Anthropic API version header value. Defaults to a known-good date.
  anthropicVersion?: string | undefined
}>

const DEFAULT_TIMEOUT_MS = 60_000
const DEFAULT_MAX_TOKENS = 1_024
const DEFAULT_ANTHROPIC_VERSION = '2023-06-01'

const fail = (
  id: string,
  reason: string,
): Effect.Effect<never, InferenceAdapterError> =>
  Effect.fail(new InferenceAdapterError({ adapterId: id, reason }))

// Build a retryable reason for 429/5xx so callers (routing/overflow) can tell a
// transient partner problem from a permanent one. The reason string is the
// stable surface the route maps to its JSON error; keep it bounded and free of
// key material or prompt content.
const transportFailureReason = (status: number): string =>
  status === 429
    ? 'retryable: partner rate limited (429)'
    : `retryable: partner server error (${status})`

const isRetryableStatus = (status: number): boolean =>
  status === 429 || status >= 500

// Pull a numeric passthrough param (max_tokens, etc.) when present and sane.
const numberParam = (
  params: Readonly<Record<string, unknown>>,
  key: string,
): number | undefined => {
  const value = params[key]
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

// Sampling params we forward verbatim when present. We copy only a known,
// bounded allow-list rather than spreading arbitrary keys, so an unexpected
// field can't change auth/routing/streaming behavior.
const OPENAI_FORWARDED_PARAMS = [
  'temperature',
  'top_p',
  'frequency_penalty',
  'presence_penalty',
  'stop',
  'seed',
] as const

const ANTHROPIC_FORWARDED_PARAMS = [
  'temperature',
  'top_p',
  'top_k',
  'stop_sequences',
] as const

const forwardParams = (
  params: Readonly<Record<string, unknown>>,
  allow: ReadonlyArray<string>,
): Record<string, unknown> => {
  const out: Record<string, unknown> = {}
  for (const key of allow) {
    if (params[key] !== undefined) {
      out[key] = params[key]
    }
  }
  return out
}

// ---- OpenAI Chat Completions mapping ------------------------------------

type OpenAiUsage = Readonly<{
  prompt_tokens?: number
  completion_tokens?: number
  total_tokens?: number
  prompt_tokens_details?: Readonly<{ cached_tokens?: number }>
}>

type OpenAiResponse = Readonly<{
  model?: string
  choices?: ReadonlyArray<
    Readonly<{
      finish_reason?: string | null
      message?: Readonly<{ content?: string | null }>
    }>
  >
  usage?: OpenAiUsage
}>

const openAiBody = (
  request: InferenceRequest,
  defaultMaxTokens: number,
): Record<string, unknown> => ({
  model: request.model,
  messages: request.messages.map(message => ({
    content: message.content,
    role: message.role,
  })),
  max_tokens:
    numberParam(request.passthroughParams, 'max_tokens') ?? defaultMaxTokens,
  stream: request.stream,
  ...forwardParams(request.passthroughParams, OPENAI_FORWARDED_PARAMS),
})

const openAiUsage = (usage: OpenAiUsage | undefined): InferenceUsage => {
  const promptTokens = usage?.prompt_tokens ?? 0
  const completionTokens = usage?.completion_tokens ?? 0
  const cached = usage?.prompt_tokens_details?.cached_tokens
  return {
    promptTokens,
    completionTokens,
    totalTokens: usage?.total_tokens ?? promptTokens + completionTokens,
    ...(typeof cached === 'number' ? { cachedPromptTokens: cached } : {}),
  }
}

const openAiResult = (
  request: InferenceRequest,
  payload: OpenAiResponse,
): InferenceResult => {
  const choice = payload.choices?.[0]
  return {
    content: choice?.message?.content ?? '',
    finishReason: choice?.finish_reason ?? 'stop',
    servedModel: payload.model ?? request.model,
    usage: openAiUsage(payload.usage),
  }
}

// ---- Anthropic Messages mapping -----------------------------------------

type AnthropicUsage = Readonly<{
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
}>

type AnthropicResponse = Readonly<{
  model?: string
  stop_reason?: string | null
  content?: ReadonlyArray<Readonly<{ type?: string; text?: string }>>
  usage?: AnthropicUsage
}>

// Anthropic Messages keeps the `system` prompt out of `messages`; split any
// system turns out so the request maps cleanly.
const anthropicBody = (
  request: InferenceRequest,
  defaultMaxTokens: number,
): Record<string, unknown> => {
  const systemParts: Array<string> = []
  const turns: Array<{ role: string; content: string }> = []
  for (const message of request.messages) {
    if (message.role === 'system') {
      systemParts.push(message.content)
    } else {
      // Anthropic accepts only "user" / "assistant" roles.
      turns.push({
        content: message.content,
        role: message.role === 'assistant' ? 'assistant' : 'user',
      })
    }
  }
  return {
    model: request.model,
    max_tokens:
      numberParam(request.passthroughParams, 'max_tokens') ?? defaultMaxTokens,
    messages: turns,
    stream: request.stream,
    ...(systemParts.length > 0 ? { system: systemParts.join('\n\n') } : {}),
    ...forwardParams(request.passthroughParams, ANTHROPIC_FORWARDED_PARAMS),
  }
}

// Map Anthropic's stop_reason to the OpenAI-style finish_reason our envelope
// uses, so downstream consumers see one vocabulary.
const anthropicFinishReason = (
  stopReason: string | null | undefined,
): string => {
  switch (stopReason) {
    case 'max_tokens':
      return 'length'
    case 'tool_use':
      return 'tool_calls'
    case 'end_turn':
    case 'stop_sequence':
      return 'stop'
    default:
      return stopReason ?? 'stop'
  }
}

const anthropicUsage = (usage: AnthropicUsage | undefined): InferenceUsage => {
  const promptTokens = usage?.input_tokens ?? 0
  const completionTokens = usage?.output_tokens ?? 0
  const cached = usage?.cache_read_input_tokens
  return {
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
    ...(typeof cached === 'number' ? { cachedPromptTokens: cached } : {}),
  }
}

const anthropicText = (
  content: AnthropicResponse['content'] | undefined,
): string =>
  (content ?? [])
    .filter(block => block.type === 'text' && typeof block.text === 'string')
    .map(block => block.text ?? '')
    .join('')

const anthropicResult = (
  request: InferenceRequest,
  payload: AnthropicResponse,
): InferenceResult => ({
  content: anthropicText(payload.content),
  finishReason: anthropicFinishReason(payload.stop_reason),
  servedModel: payload.model ?? request.model,
  usage: anthropicUsage(payload.usage),
})

// ---- HTTP plumbing -------------------------------------------------------

const requestPath = (wireFormat: PassthroughWireFormat): string =>
  wireFormat === 'anthropic' ? '/v1/messages' : '/v1/chat/completions'

// Read the Redacted secret to a string at the network boundary only. The value
// is placed on an outbound header and never logged or returned.
const requestHeaders = (
  config: PassthroughAdapterConfig,
): Record<string, string> => {
  const key = Redacted.value(config.apiKey)
  if (config.wireFormat === 'anthropic') {
    return {
      accept: 'application/json',
      'anthropic-version': config.anthropicVersion ?? DEFAULT_ANTHROPIC_VERSION,
      'content-type': 'application/json',
      'x-api-key': key,
    }
  }
  return {
    accept: 'application/json',
    authorization: `Bearer ${key}`,
    'content-type': 'application/json',
  }
}

const safeSignal = (timeoutMs: number): AbortSignal | undefined => {
  try {
    return AbortSignal.timeout(timeoutMs)
  } catch {
    return undefined
  }
}

const postToPartner = (
  config: PassthroughAdapterConfig,
  body: unknown,
): Effect.Effect<PartnerResponse, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: error =>
      new InferenceAdapterError({
        adapterId: config.id,
        reason: `retryable: partner transport error (${
          error instanceof Error ? error.name : 'unknown'
        })`,
      }),
    try: () => {
      const fetcher = config.fetch ?? (globalThis.fetch as PassthroughFetch)
      const signal = safeSignal(config.timeoutMs ?? DEFAULT_TIMEOUT_MS)
      return fetcher(`${config.baseUrl}${requestPath(config.wireFormat)}`, {
        body: JSON.stringify(body),
        headers: requestHeaders(config),
        method: 'POST',
        ...(signal === undefined ? {} : { signal }),
      })
    },
  })

const parseJson = (
  config: PassthroughAdapterConfig,
  response: PartnerResponse,
): Effect.Effect<unknown, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: () =>
      new InferenceAdapterError({
        adapterId: config.id,
        reason: 'partner returned a non-JSON response',
      }),
    try: () => response.json(),
  })

// ---- Adapter factory -----------------------------------------------------

const buildBody = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
): Record<string, unknown> => {
  const defaultMaxTokens = config.defaultMaxTokens ?? DEFAULT_MAX_TOKENS
  return config.wireFormat === 'anthropic'
    ? anthropicBody(request, defaultMaxTokens)
    : openAiBody(request, defaultMaxTokens)
}

const toResult = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
  payload: unknown,
): InferenceResult =>
  config.wireFormat === 'anthropic'
    ? anthropicResult(request, payload as AnthropicResponse)
    : openAiResult(request, payload as OpenAiResponse)

// Shared request → response path for both complete and (collected) stream.
const runCompletion = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<InferenceResult, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postToPartner(config, buildBody(config, request))

    if (isRetryableStatus(response.status)) {
      return yield* fail(config.id, transportFailureReason(response.status))
    }

    const payload = yield* parseJson(config, response)

    if (!response.ok) {
      return yield* fail(
        config.id,
        `partner rejected request (${response.status})`,
      )
    }

    return toResult(config, request, payload)
  })

// Build a passthrough adapter for one partner. Each registered partner gets one
// adapter id. The adapter is pure data + Effects; it touches the network only
// when `complete`/`stream` actually run, so registering it under a disabled
// flag keeps it fully INERT.
export const makePassthroughAdapter = (
  config: PassthroughAdapterConfig,
): InferenceProviderAdapter => ({
  id: config.id,
  complete: (request: InferenceRequest) => runCompletion(config, request),
  // Streaming maps to a single non-streamed partner call whose result is split
  // into a content frame + a terminal usage frame. We force `stream: false` on
  // the partner request (the route serializes our frames into SSE itself), so
  // we always settle metering from the partner's real, receipt-first usage
  // rather than reconstructing counts from SSE deltas. A future revision can
  // upgrade this to true partner SSE passthrough without changing the contract.
  stream: (request: InferenceRequest) =>
    runCompletion(config, { ...request, stream: false }).pipe(
      Effect.map((result): ReadonlyArray<InferenceStreamChunk> => {
        const contentChunk: InferenceStreamChunk = {
          contentDelta: result.content,
        }
        const terminalChunk: InferenceStreamChunk = {
          contentDelta: '',
          finishReason: result.finishReason,
          servedModel: result.servedModel,
          usage: result.usage,
        }
        return [contentChunk, terminalChunk]
      }),
    ),
})
