// Fireworks AI provider adapter for the inference gateway (EPIC #5474, #5479).
//
// Fireworks is the managed open-model / cheap supply lane (DeepSeek, Kimi, GLM,
// Qwen, MiniMax, gpt-oss, Nemotron, ...) for models Vertex doesn't carry. Its
// API is OpenAI-compatible, so this adapter is a near drop-in over the
// chat-completions wire format with the base URL + bearer key swapped. See
// `docs/inference/2026-06-19-fireworks-provider.md`.
//
// Boundaries (provider-adapter.ts): this module ONLY translates a normalized
// InferenceRequest into Fireworks chat-completions and returns the normalized
// result + the provider `usage` object (receipt-first). It never touches
// credits, payment, routing, or public projection. Routing (#5482) decides when
// this adapter is dispatched; metering (#5477) reads the `usage` it returns.
//
// Receipt-first: `usage` is taken VERBATIM from the Fireworks response
// (`prompt_tokens` / `completion_tokens` / `total_tokens`), never an estimate
// (INVARIANTS.md "Canonical Token Usage Ledger").
//
// Rate-limit shape for routing/backoff (#5482): a 429 ("Too Many Requests") and
// a 503 ("Service Overloaded") both surface as a typed InferenceAdapterError
// carrying `retryable: true` plus the http status / kind, so the router can back
// off and overflow to another supply lane rather than failing the request.
import { Effect } from 'effect'

import { parseJsonRecord, recordFromUnknown } from '../json-boundary'
import { KHALA_CODE_MODEL_ID, KHALA_MODEL_ID } from './pricing'
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
import {
  inferenceToolCallDeltasFromUnknown,
  inferenceToolCallsFromUnknown,
  openAiWireMessageFromInferenceMessage,
} from './openai-chat-compat'

export const FIREWORKS_ADAPTER_ID = 'fireworks'

export const FIREWORKS_DEFAULT_BASE_URL =
  'https://api.fireworks.ai/inference/v1'

// Fireworks model ids are `accounts/fireworks/models/<id>`. The gateway carries
// short aliases; this prefix is applied when the requested model is a bare id.
// Routing (#5482) owns the real alias table; we only normalize the wire id so a
// bare open-model alias (e.g. "deepseek-v4-pro") reaches the provider.
const FIREWORKS_MODEL_PREFIX = 'accounts/fireworks/models/'
const KHALA_CODE_BACKING_MODEL_ID = `${FIREWORKS_MODEL_PREFIX}kimi-k2p7-code`
export const KHALA_FIREWORKS_BACKING_MODEL_ID = `${FIREWORKS_MODEL_PREFIX}deepseek-v4-flash`
// The strongest frontier GLM coding model the gateway can serve over Fireworks.
// Used only for internal frontier-coding eval load (the MirrorCode gym rung): the
// chat route rewrites a Khala request to this id when it is routed to the
// strong-coding alias lane. Bare-aliased so `toFireworksModelId` applies the
// Fireworks account prefix; priced in `pricing.ts` (`glm-5p2`).
export const KHALA_STRONG_CODING_FIREWORKS_MODEL_ID = 'glm-5p2'

const toFireworksModelId = (model: string): string => {
  const id = model.trim()
  if (id.toLowerCase() === KHALA_MODEL_ID) {
    return KHALA_FIREWORKS_BACKING_MODEL_ID
  }
  if (id.toLowerCase() === KHALA_CODE_MODEL_ID) {
    return KHALA_CODE_BACKING_MODEL_ID
  }
  return id.includes('/') ? id : `${FIREWORKS_MODEL_PREFIX}${id}`
}

// The platform HTTP response shape. Aliased so the adapter does not add raw
// Response-returning surface annotations to the Worker domain layer (HTTP
// response mapping stays in the route/HTTP modules).
type HttpResponse = Response

// Minimal `fetch` surface this adapter needs, so tests inject a mock and the
// Worker passes the platform `fetch`.
export type FetchLike = (
  input: string,
  init: Readonly<{
    method: string
    headers: Record<string, string>
    body: string
  }>,
) => Promise<HttpResponse>

export type FireworksAdapterConfig = Readonly<{
  // Resolves the Fireworks API key at call time. The Worker wires this to the
  // `FIREWORKS_API_KEY` secret; tests inject a fixed value. NEVER logged.
  getApiKey: () => string | undefined
  // OpenAI-compatible base URL. Defaults to the live Fireworks base.
  baseUrl?: string | undefined
  // Injected fetch (tests pass a mock; the Worker passes the platform fetch).
  fetchImpl?: FetchLike | undefined
}>

// HTTP status -> typed retry classification consumed by routing (#5482).
const classifyStatus = (
  status: number,
): Readonly<{ retryable: boolean; kind: string }> => {
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
  input: Readonly<{
    reason: string
    httpStatus?: number | undefined
    retryable?: boolean | undefined
    kind?: string | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: FIREWORKS_ADAPTER_ID,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable,
  })

// Build the Fireworks chat-completions request body from the normalized
// request. Standard sampling params (temperature, top_p, max_tokens, ...) ride
// through `passthroughParams` verbatim; load-bearing fields are set explicitly
// and win over any stray passthrough copy. The `x-session-affinity` key is a
// transport header, not a body field, so it is dropped from the body.
const toRequestBody = (request: InferenceRequest): Record<string, unknown> => {
  const { 'x-session-affinity': _affinity, ...params } =
    request.passthroughParams
  return {
    ...params,
    messages: request.messages.map(openAiWireMessageFromInferenceMessage),
    model: toFireworksModelId(request.model),
    stream: request.stream,
    // RECEIPT-FIRST STREAMING (the "missing terminal usage frame" 524 fix):
    // OpenAI-compatible providers OMIT the `usage` object from streamed
    // responses BY DEFAULT — you must opt in with `stream_options.include_usage`
    // to get a terminal frame carrying real prompt/completion counts. Without it
    // the adapter's receipt-first guard correctly refuses to settle on an
    // estimate and fails with `fireworks stream missing terminal usage frame`
    // (observed on a short streamed prompt at ~1.4s). Asking for it makes a
    // normal streamed completion carry its real usage, so streaming meters
    // exactly like non-streaming. A stray client copy in passthroughParams is
    // overridden here (load-bearing field wins). Only set on streaming requests.
    ...(request.stream ? { stream_options: { include_usage: true } } : {}),
  }
}

const headersFor = (
  apiKey: string,
  request: InferenceRequest,
): Record<string, string> => {
  const headers: Record<string, string> = {
    accept: request.stream ? 'text/event-stream' : 'application/json',
    authorization: `Bearer ${apiKey}`,
    'content-type': 'application/json',
  }
  // Prompt-cache affinity: pin shared-prefix prompts to one replica to maximize
  // cache hit rate (cached input billed ~50%). The router supplies a stable
  // per-session key via `x-session-affinity` in passthroughParams.
  const affinity = request.passthroughParams['x-session-affinity']
  if (typeof affinity === 'string' && affinity.length > 0) {
    headers['x-session-affinity'] = affinity
  }
  return headers
}

// Receipt-first usage extraction. Reads the provider `usage` object verbatim;
// the optional cached-input dimension comes from `prompt_tokens_details` when
// Fireworks reports it (prompt-cache hits, billed ~50% of input).
const extractUsage = (raw: unknown): InferenceUsage | undefined => {
  const record = recordFromUnknown(raw)
  if (record === undefined) {
    return undefined
  }
  const usage = recordFromUnknown(record['usage'])
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
  if (record === undefined) {
    return undefined
  }
  const choices = record['choices']
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

// Parse one SSE line (`data: {...}` or `data: [DONE]`). Returns undefined for
// blank lines, comments, and the terminal sentinel. Raw JSON decoding goes
// through the json-boundary helper (no raw JSON.parse here).
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
  const choice = firstChoice(frame)
  if (choice === undefined) {
    return ''
  }
  const delta = recordFromUnknown(choice['delta'])
  const content = delta?.['content']
  return typeof content === 'string' ? content : ''
}

const deltaReasoningOf = (frame: Record<string, unknown>): string => {
  const choice = firstChoice(frame)
  if (choice === undefined) {
    return ''
  }
  const delta = recordFromUnknown(choice['delta'])
  const direct =
    delta?.['reasoning_content'] ??
    delta?.['reasoning'] ??
    delta?.['reasoning_delta'] ??
    delta?.['reasoningContent']
  return typeof direct === 'string' ? direct : ''
}

const toolCallDeltasOf = (
  frame: Record<string, unknown>,
): InferenceStreamEvent['toolCallDeltas'] => {
  const choice = firstChoice(frame)
  const delta = recordFromUnknown(choice?.['delta'])
  const toolCallDeltas = inferenceToolCallDeltasFromUnknown(
    delta?.['tool_calls'],
  )
  return toolCallDeltas === undefined || toolCallDeltas.length === 0
    ? undefined
    : toolCallDeltas
}

const finishReasonOf = (frame: Record<string, unknown>): string | undefined => {
  const choice = firstChoice(frame)
  const reason = choice?.['finish_reason']
  return typeof reason === 'string' && reason.length > 0 ? reason : undefined
}

// Normalize a parsed SSE frame into the route-facing event shape. Shared by the
// buffered `stream` array path and the incremental `streamSse` pass-through so
// both produce byte-identical normalization.
const eventForFrame = (
  frame: Record<string, unknown>,
): InferenceStreamEvent => {
  const event: {
    contentDelta: string
    reasoningDelta?: string
    toolCallDeltas?: InferenceStreamEvent['toolCallDeltas']
    finishReason?: string
    usage?: InferenceUsage
    servedModel?: string
  } = { contentDelta: deltaContentOf(frame) }
  const reasoningDelta = deltaReasoningOf(frame)
  if (reasoningDelta !== '') {
    event.reasoningDelta = reasoningDelta
  }
  const toolCallDeltas = toolCallDeltasOf(frame)
  if (toolCallDeltas !== undefined) {
    event.toolCallDeltas = toolCallDeltas
  }
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

// Build a true incremental SSE source over the upstream `ReadableStream`. Frames
// are decoded + parsed AS BYTES ARRIVE (partial lines are buffered across reads)
// and yielded one at a time, so the route can pump each to the client and reset
// the edge idle-timer. The running terminal state (finishReason / usage /
// servedModel) is captured during iteration and exposed via `terminal()` once
// the stream is drained — receipt-first, no re-buffering of content. The
// `[DONE]` sentinel and blank/comment lines are skipped by `parseSseData`.
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
        // Emit every complete line currently buffered. SSE frames are newline
        // delimited; a `data:` payload is single-line for OpenAI-compatible
        // chunk framing, so line-at-a-time parsing is sufficient and we never
        // hold a whole frame back waiting for the blank separator line.
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
          // Flush any trailing partial line (some sources omit the final \n).
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

// Resolve the key + transport, failing with a typed (non-retryable) config
// error when the key is absent. NEVER includes key material in the message.
const resolveTransport = (config: FireworksAdapterConfig) =>
  Effect.sync(() => config.getApiKey()).pipe(
    Effect.flatMap(apiKey =>
      apiKey === undefined || apiKey.trim() === ''
        ? Effect.fail(
            adapterError({
              kind: 'configuration_error',
              reason: 'FIREWORKS_API_KEY is not configured',
              retryable: false,
            }),
          )
        : Effect.succeed({
            apiKey,
            baseUrl: config.baseUrl ?? FIREWORKS_DEFAULT_BASE_URL,
            fetchImpl:
              config.fetchImpl ?? ((input, init) => fetch(input, init)),
          }),
    ),
  )

// POST to Fireworks chat-completions, surfacing transport faults as a typed
// retryable error. Returns the platform response for the caller to inspect.
const postChatCompletions = (
  transport: Readonly<{
    apiKey: string
    baseUrl: string
    fetchImpl: FetchLike
  }>,
  request: InferenceRequest,
) =>
  Effect.tryPromise({
    catch: error =>
      adapterError({
        kind: 'transport_error',
        reason: `fireworks request failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
        // Network/transport faults are safe to retry / overflow.
        retryable: true,
      }),
    try: () =>
      transport.fetchImpl(`${transport.baseUrl}/chat/completions`, {
        body: JSON.stringify(toRequestBody(request)),
        headers: headersFor(transport.apiKey, request),
        method: 'POST',
      }),
  })

// Map a non-2xx response into a typed error WITHOUT leaking the request body or
// any header/key material — only the status, classification, and a bounded,
// redacted snippet of the response body.
const failForStatus = (response: HttpResponse) =>
  Effect.gen(function* () {
    const status = response.status
    const { kind, retryable } = classifyStatus(status)
    const text = yield* Effect.tryPromise({
      catch: () => '',
      try: () => response.text(),
    }).pipe(Effect.orElseSucceed(() => ''))
    const snippet = text.slice(0, 200)
    return yield* Effect.fail(
      adapterError({
        httpStatus: status,
        kind,
        reason:
          snippet === ''
            ? `fireworks responded ${status}`
            : `fireworks responded ${status}: ${snippet}`,
        retryable,
      }),
    )
  })

const readBody = (response: HttpResponse) =>
  Effect.tryPromise({
    catch: error =>
      adapterError({
        kind: 'malformed_response',
        reason: `fireworks body unreadable: ${
          error instanceof Error ? error.message : String(error)
        }`,
        retryable: false,
      }),
    try: () => response.text(),
  })

// Build the adapter. The Worker constructs ONE instance and registers it;
// tests construct instances with a mock fetch + fixed key.
export const makeFireworksAdapter = (
  config: FireworksAdapterConfig,
): InferenceProviderAdapter => ({
  complete: (request: InferenceRequest) =>
    Effect.gen(function* () {
      const transport = yield* resolveTransport(config)
      const response = yield* postChatCompletions(transport, {
        ...request,
        stream: false,
      })
      if (!response.ok) {
        return yield* failForStatus(response)
      }
      // Read + decode through the json-boundary helper (no raw JSON.parse).
      const text = yield* readBody(response)
      const raw = parseJsonRecord(text)
      if (raw === undefined) {
        return yield* Effect.fail(
          adapterError({
            kind: 'malformed_response',
            reason: 'fireworks returned unparseable JSON',
            retryable: false,
          }),
        )
      }
      const usage = extractUsage(raw)
      if (usage === undefined) {
        return yield* Effect.fail(
          adapterError({
            kind: 'malformed_response',
            reason: 'fireworks response missing usage object',
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
      return {
        content,
        finishReason,
        servedModel: servedModelFrom(raw, toFireworksModelId(request.model)),
        ...(toolCalls === undefined || toolCalls.length === 0
          ? {}
          : { toolCalls }),
        usage,
      } satisfies InferenceResult
    }),
  id: FIREWORKS_ADAPTER_ID,
  stream: (request: InferenceRequest) =>
    Effect.gen(function* () {
      const transport = yield* resolveTransport(config)
      const response = yield* postChatCompletions(transport, {
        ...request,
        stream: true,
      })
      if (!response.ok) {
        return yield* failForStatus(response)
      }
      // Read the full SSE body and parse it into normalized chunks. The route
      // re-serializes these into OpenAI-compatible frames. (A future iteration
      // can switch to incremental ReadableStream parsing; the chunk contract is
      // identical.)
      const body = yield* readBody(response)
      const frames = body
        .split('\n')
        .map(parseSseData)
        .filter(
          (frame): frame is Record<string, unknown> => frame !== undefined,
        )

      const contentChunks: Array<InferenceStreamChunk> = []
      let finishReason: string | undefined
      let servedModel = toFireworksModelId(request.model)
      let usage: InferenceUsage | undefined

      for (const frame of frames) {
        servedModel = servedModelFrom(frame, servedModel)
        const event = eventForFrame(frame)
        if (
          event.contentDelta !== '' ||
          event.reasoningDelta !== undefined ||
          (event.toolCallDeltas !== undefined &&
            event.toolCallDeltas.length > 0)
        ) {
          contentChunks.push({
            contentDelta: event.contentDelta,
            ...(event.reasoningDelta === undefined
              ? {}
              : { reasoningDelta: event.reasoningDelta }),
            ...(event.toolCallDeltas === undefined
              ? {}
              : { toolCallDeltas: event.toolCallDeltas }),
          })
        }
        if (event.finishReason !== undefined) {
          finishReason = event.finishReason
        }
        if (event.usage !== undefined) {
          usage = event.usage
        }
        if (event.servedModel !== undefined) {
          servedModel = event.servedModel
        }
      }

      // Receipt-first: some Fireworks stream modes omit terminal usage. Return
      // the terminal chunk without usage so the route can disclose an unmetered
      // stream instead of failing or settling on an estimate.
      const terminalChunk: InferenceStreamChunk = {
        contentDelta: '',
        finishReason: finishReason ?? 'stop',
        servedModel,
        ...(usage === undefined ? {} : { usage }),
      }
      return [...contentChunks, terminalChunk]
    }),
  // TRUE PASS-THROUGH STREAM (the long-generation 524 fix). Returns a lazily
  // consumed SSE source over the upstream `response.body` so the route can pump
  // each frame to the client as Fireworks produces it — every chunk resets the
  // Cloudflare edge idle-timer, so a multi-minute generation never 524s. The
  // buffered `stream` above stays for tests/metering reconstruction/overflow.
  // `stream_options.include_usage` (set in toRequestBody) makes the upstream
  // emit a terminal usage frame so metering settles receipt-first.
  streamSse: (request: InferenceRequest) =>
    Effect.gen(function* () {
      const transport = yield* resolveTransport(config)
      const response = yield* postChatCompletions(transport, {
        ...request,
        stream: true,
      })
      if (!response.ok) {
        return yield* failForStatus(response)
      }
      const body = response.body
      if (body === null) {
        return yield* Effect.fail(
          adapterError({
            kind: 'malformed_response',
            reason: 'fireworks stream had no response body',
            retryable: false,
          }),
        )
      }
      return makeSseSource(body, toFireworksModelId(request.model))
    }),
})

// Default registered instance for the Worker (EPIC #5479). The key is resolved
// lazily at call time from the Worker secret `FIREWORKS_API_KEY` (exposed on the
// global/process env at request time); it is NEVER read or logged at
// registration. The gateway stays INERT under INFERENCE_GATEWAY_ENABLED and
// routing (#5482) decides when this adapter is actually dispatched, so an absent
// key only ever surfaces as a typed configuration_error if/when this adapter is
// exercised.
export const fireworksAdapter: InferenceProviderAdapter = makeFireworksAdapter({
  getApiKey: () => {
    const fromProcess =
      typeof process !== 'undefined'
        ? process.env?.['FIREWORKS_API_KEY']
        : undefined
    const fromGlobal = (globalThis as { FIREWORKS_API_KEY?: string })
      .FIREWORKS_API_KEY
    return fromProcess ?? fromGlobal
  },
})
