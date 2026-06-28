import { Effect, Schema as S } from "effect"
import { validatePublicConversation } from "./bounds.js"
import { decodeOpenAiFrame, decodePublicFrame, readSseFrames, type StreamFrameMetadata } from "./sse.js"
import {
  ARTANIS_CHAT_PATH,
  ArtanisChatRequest,
  ArtanisChatResponse,
  BYOK_ACK_HEADER,
  BYOK_KEY_HEADER,
  BYOK_PROVIDER_HEADER,
  DEFAULT_BASE_URL,
  FreeKeyResponse,
  KHALA_MODEL_ID,
  KhalaFeedbackResponse,
  KhalaCliError,
  KhalaPublicChatRequest,
  KhalaTokensResponse,
  OpenAiModelsResponse,
  type ArtanisTurnOptions,
  type ArtanisTurnResult,
  type ChatClientOptions,
  type ChatTurnOptions,
  type ChatTurnResult,
  type ChatTurnMetadata,
  type FreeKeyResponse as FreeKeyResponseType,
  type KhalaFeedbackSubmitOptions,
  type KhalaStreamUsage,
} from "./types.js"

const MAX_REQUEST_RETRIES = 5
const RETRY_DELAYS_MS = [400, 1_000, 2_000, 4_000, 8_000] as const
const KHALA_CHAT_MAX_TOKENS = 8_192
const MAX_LENGTH_CONTINUATIONS = 2
const CONTINUE_AFTER_LENGTH_PROMPT =
  "Continue the previous answer from exactly where it stopped. Do not restart, summarize, or mention the continuation."

export function runChatTurn(options: ChatTurnOptions): Effect.Effect<ChatTurnResult, KhalaCliError> {
  return Effect.gen(function* () {
    const startedAt = Date.now()
    let turnOptions = options
    let stream = yield* consumeOneStream(turnOptions, startedAt)
    let text = stream.text
    let reasoningText = stream.reasoningText
    let byokAck = stream.byokAck
    let traceRef = stream.traceRef

    for (let continuation = 0; shouldContinueAfterLength(stream.metadata) && continuation < MAX_LENGTH_CONTINUATIONS; continuation += 1) {
      turnOptions = {
        ...options,
        messages: [
          ...options.messages,
          { role: "assistant" as const, content: text },
          { role: "user" as const, content: CONTINUE_AFTER_LENGTH_PROMPT },
        ],
      }
      const next = yield* consumeOneStream(turnOptions, startedAt)
      text += next.text
      reasoningText += next.reasoningText
      byokAck = next.byokAck ?? byokAck
      traceRef = next.traceRef ?? traceRef
      stream = {
        ...next,
        metadata: mergeUsageMetadata(next.metadata, stream.metadata),
        reasoningText,
        text,
      }
    }

    const durationMs = Math.max(0, Date.now() - startedAt)
    const metadata = buildTurnMetadata({
      durationMs,
      mode: options.mode,
      streamMetadata: {
        ...stream.metadata,
        traceRef: stream.metadata.traceRef ?? traceRef,
      },
      timings: stream.timings,
      text,
    })
    return {
      text,
      reasoningText,
      assistantMessage: { role: "assistant" as const, content: text },
      metadata,
      traceRef: metadata.traceRef,
      ...(byokAck === undefined ? {} : { byokAck }),
    }
  })
}

function consumeOneStream(
  options: ChatTurnOptions,
  startedAt: number,
): Effect.Effect<{
  readonly byokAck?: string | undefined
  readonly metadata: StreamFrameMetadata
  readonly reasoningText: string
  readonly text: string
  readonly timings: {
    readonly firstTokenAt?: number | undefined
    readonly responseAt: number
    readonly startedAt: number
  }
  readonly traceRef?: string | undefined
}, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* postChat(options)
    const responseAt = Date.now()
    const traceRef = readTraceRef(response)
    const byokAck = response.headers.get(BYOK_ACK_HEADER)?.trim() || undefined
    const stream = yield* consumeStream(
      response,
      options.mode,
      startedAt,
      responseAt,
      options.onDelta,
      options.onReasoning,
    )
    return {
      ...stream,
      ...(byokAck === undefined ? {} : { byokAck }),
      traceRef,
    }
  })
}

// Owner-authenticated Artanis operator channel (#6363, epic #6359).
//
// This is a THIN client of the shared `POST /api/operator/artanis/chat`
// endpoint owned by the core lane. The Worker route is the single home of the
// Artanis operator logic, persona, situational awareness, and memory; the CLI
// only posts the conversation with the owner's bearer token and renders the
// reply. The contract is non-streaming: `{ messages }` in, `{ reply }` out.
//
// Unlike the public Khala paths, an unauthenticated or non-owner caller is
// expected to receive 401/403 here. The CLI surfaces that as a typed,
// graceful "owner-only channel" error rather than falling back to public Khala.
export function runArtanisTurn(options: ArtanisTurnOptions): Effect.Effect<ArtanisTurnResult, KhalaCliError> {
  return Effect.gen(function* () {
    const token = options.token.trim()
    if (!token) {
      return yield* new KhalaCliError({
        reason: "Talking to Artanis requires the owner agent token. Run `khala login` to sign in as the owner (or pass --token / set OPENAGENTS_AGENT_TOKEN).",
        code: "missing_token",
      })
    }

    const body = S.encodeSync(ArtanisChatRequest)({ messages: [...options.messages] })
    const response = yield* request({
      fetch: options.fetch,
      url: urlFor(options.baseUrl, ARTANIS_CHAT_PATH),
      init: {
        method: "POST",
        headers: {
          accept: "application/json",
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
      },
    })
    const payload = yield* readJsonResponse(response, "Artanis response")
    const decoded = yield* Effect.try({
      try: () => S.decodeUnknownSync(ArtanisChatResponse)(payload),
      catch: (error) => new KhalaCliError({
        reason: `Unexpected Artanis response: ${String(error)}`,
        code: "schema_mismatch",
        traceRef: readTraceRef(response),
      }),
    })
    return {
      text: decoded.reply,
      traceRef: decoded.traceRef ?? readTraceRef(response),
    }
  })
}

export function fetchModels(options: Pick<ChatClientOptions, "baseUrl" | "fetch">): Effect.Effect<unknown, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* request({
      fetch: options.fetch,
      url: urlFor(options.baseUrl, "/api/v1/models"),
      init: { method: "GET" },
    })
    const payload = yield* readJsonResponse(response, "models response")
    return yield* Effect.try({
      try: () => S.decodeUnknownSync(OpenAiModelsResponse)(payload),
      catch: (error) => new KhalaCliError({
        reason: `Unexpected models response: ${String(error)}`,
        code: "schema_mismatch",
      }),
    })
  })
}

export function mintFreeKey(options: Pick<ChatClientOptions, "baseUrl" | "fetch">): Effect.Effect<FreeKeyResponseType, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* request({
      fetch: options.fetch,
      url: urlFor(options.baseUrl, "/api/keys/free"),
      init: { method: "POST", headers: { accept: "application/json" } },
    })
    const payload = yield* readJsonResponse(response, "free key response")
    return yield* Effect.try({
      try: () => S.decodeUnknownSync(FreeKeyResponse)(payload),
      catch: (error) => new KhalaCliError({
        reason: `Unexpected free key response: ${String(error)}`,
        code: "schema_mismatch",
      }),
    })
  })
}

export function submitFeedback(options: KhalaFeedbackSubmitOptions): Effect.Effect<KhalaFeedbackResponse, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* request({
      fetch: options.fetch,
      url: urlFor(options.baseUrl, "/api/khala/feedback"),
      init: {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          clientVersion: options.clientVersion,
          feedback: options.feedback,
          source: options.source,
          ...(options.traceRef === undefined ? {} : { traceRef: options.traceRef }),
        }),
      },
    })
    const payload = yield* readJsonResponse(response, "feedback response")
    return yield* Effect.try({
      try: () => S.decodeUnknownSync(KhalaFeedbackResponse)(payload),
      catch: (error) => new KhalaCliError({
        reason: `Unexpected feedback response: ${String(error)}`,
        code: "schema_mismatch",
      }),
    })
  })
}

export function fetchTokensServed(options: Pick<ChatClientOptions, "baseUrl" | "fetch">): Effect.Effect<KhalaTokensResponse, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* request({
      fetch: options.fetch,
      url: urlFor(options.baseUrl, "/api/khala/tokens"),
      init: {
        method: "GET",
        headers: { accept: "application/json" },
      },
    })
    const payload = yield* readJsonResponse(response, "tokens response")
    return yield* Effect.try({
      try: () => S.decodeUnknownSync(KhalaTokensResponse)(payload),
      catch: (error) => new KhalaCliError({
        reason: `Unexpected tokens response: ${String(error)}`,
        code: "schema_mismatch",
      }),
    })
  })
}

function postChat(options: ChatTurnOptions): Effect.Effect<Response, KhalaCliError> {
  return Effect.gen(function* () {
    if (options.mode === "public") {
      validatePublicConversation(options.messages)
      const body = S.encodeSync(KhalaPublicChatRequest)({ messages: [...options.messages] })
      return yield* request({
        fetch: options.fetch,
        onRetry: options.onRetry,
        url: urlFor(options.baseUrl, "/api/khala/chat"),
        init: {
          method: "POST",
          headers: {
            accept: "text/event-stream",
            "content-type": "application/json",
          },
          body: JSON.stringify(body),
        },
      })
    }

    const token = options.token?.trim()
    if (!token) {
      return yield* new KhalaCliError({
        reason: "--api requires an agent token. Run `khala login` to sign in (or pass --token / set OPENAGENTS_AGENT_TOKEN). Use --mint-free-key to request a free key when that endpoint is armed.",
        code: "missing_token",
      })
    }

    const providerKey = options.providerKey?.trim()
    const providerName = options.providerName?.trim()
    const byokHeaders =
      providerKey !== undefined && providerKey.length > 0
        ? {
            [BYOK_KEY_HEADER]: providerKey,
            ...(providerName === undefined || providerName.length === 0
              ? {}
              : { [BYOK_PROVIDER_HEADER]: providerName }),
          }
        : {}

    return yield* request({
      fetch: options.fetch,
      onRetry: options.onRetry,
      url: urlFor(options.baseUrl, "/api/v1/chat/completions"),
      init: {
        method: "POST",
        headers: {
          accept: "text/event-stream",
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
          ...byokHeaders,
        },
        body: JSON.stringify({
          model: KHALA_MODEL_ID,
          messages: options.messages,
          max_tokens: KHALA_CHAT_MAX_TOKENS,
          stream: true,
          stream_options: { include_usage: true },
        }),
      },
    })
  })
}

function consumeStream(
  response: Response,
  mode: "public" | "api",
  startedAt: number,
  responseAt: number,
  onDelta: ((text: string) => void) | undefined,
  onReasoning: ((text: string) => void) | undefined,
): Effect.Effect<{
  readonly metadata: StreamFrameMetadata
  readonly reasoningText: string
  readonly text: string
  readonly timings: {
    readonly firstTokenAt?: number | undefined
    readonly responseAt: number
    readonly startedAt: number
  }
}, KhalaCliError> {
  return Effect.tryPromise({
    try: async () => {
      if (response.body === null) {
        throw new KhalaCliError({ reason: "Khala response did not include a stream body.", code: "missing_stream_body" })
      }
      let assembled = ""
      let reasoning = ""
      let metadata: StreamFrameMetadata = {}
      let firstTokenAt: number | undefined
      for await (const frame of readSseFrames(response.body)) {
        const decoded = mode === "public" ? decodePublicFrame(frame) : decodeOpenAiFrame(frame)
        if (decoded.kind === "done") break
        if (decoded.kind === "meta") {
          metadata = mergeMetadata(metadata, decoded.metadata)
          continue
        }
        metadata = mergeMetadata(metadata, decoded.metadata)
        if (decoded.reasoningText !== undefined && decoded.reasoningText.length > 0) {
          firstTokenAt ??= Date.now()
          reasoning += decoded.reasoningText
          onReasoning?.(decoded.reasoningText)
        }
        if (decoded.text.length > 0) {
          firstTokenAt ??= Date.now()
          assembled += decoded.text
          onDelta?.(decoded.text)
        }
      }
      return {
        metadata,
        reasoningText: reasoning,
        text: assembled,
        timings: {
          firstTokenAt,
          responseAt,
          startedAt,
        },
      }
    },
    catch: (error) => toKhalaCliError(error, "Khala stream failed."),
  })
}

function request(input: {
  readonly fetch?: typeof fetch | undefined
  readonly url: string
  readonly init: RequestInit
  readonly onRetry?: ChatClientOptions["onRetry"] | undefined
}): Effect.Effect<Response, KhalaCliError> {
  return Effect.tryPromise({
    try: async () => {
      const fetchImpl = input.fetch ?? fetch
      for (let retry = 0; retry <= MAX_REQUEST_RETRIES; retry += 1) {
        let response: Response
        try {
          response = await fetchImpl(input.url, input.init)
        } catch (error) {
          const requestError = toKhalaCliError(error, `Request failed for ${input.url}.`)
          if (retry < MAX_REQUEST_RETRIES && shouldRetry(requestError)) {
            await waitBeforeRetry(input, retry + 1, requestError)
            continue
          }
          throw requestError
        }

        if (response.ok) {
          return response
        }

        const envelope = await readErrorEnvelope(response)
        const responseError = new KhalaCliError({
          reason: envelope.reason,
          ...(envelope.code === undefined ? {} : { code: envelope.code }),
          statusCode: response.status,
          traceRef: envelope.traceRef ?? readTraceRef(response),
        })

        if (retry < MAX_REQUEST_RETRIES && shouldRetry(responseError)) {
          await waitBeforeRetry(input, retry + 1, responseError)
          continue
        }

        throw responseError
      }

      throw new KhalaCliError({ reason: "Request retry loop exhausted.", code: "retry_exhausted" })
    },
    catch: (error) => toKhalaCliError(error, `Request failed for ${input.url}.`),
  })
}

async function waitBeforeRetry(
  input: {
    readonly onRetry?: ChatClientOptions["onRetry"] | undefined
  },
  retry: number,
  error: KhalaCliError,
): Promise<void> {
  const delayMs = RETRY_DELAYS_MS[retry - 1] ?? RETRY_DELAYS_MS[RETRY_DELAYS_MS.length - 1]
  input.onRetry?.({ delayMs, error, maxRetries: MAX_REQUEST_RETRIES, retry })
  await new Promise(resolve => setTimeout(resolve, delayMs))
}

function shouldRetry(error: KhalaCliError): boolean {
  if (error.statusCode === 429 || error.statusCode === 502 || error.statusCode === 503 || error.statusCode === 504) {
    return true
  }
  return error.code === "runtime_error" || error.code === "request_failed"
}

async function readErrorEnvelope(response: Response): Promise<{
  readonly code?: string | undefined
  readonly reason: string
  readonly traceRef?: string | undefined
}> {
  const text = await response.text()
  if (text.trim().length === 0) {
    return { reason: `Khala returned HTTP ${response.status}.`, traceRef: readTraceRef(response) }
  }
  try {
    const payload = JSON.parse(text) as {
      readonly code?: unknown
      readonly error?: unknown
      readonly message?: unknown
      readonly reason?: unknown
      readonly traceRef?: unknown
    }
    const reason = typeof payload.error === "string"
      ? typeof payload.reason === "string" ? payload.reason : payload.error
      : typeof payload.message === "string"
        ? payload.message
        : `Khala returned HTTP ${response.status}.`
    return {
      reason,
      code: typeof payload.code === "string" ? payload.code : typeof payload.error === "string" ? payload.error : undefined,
      traceRef: typeof payload.traceRef === "string" ? payload.traceRef : readTraceRef(response),
    }
  } catch {
    return { reason: text.trim(), traceRef: readTraceRef(response) }
  }
}

function readJsonResponse(response: Response, label: string): Effect.Effect<unknown, KhalaCliError> {
  return Effect.tryPromise({
    try: () => response.json(),
    catch: (error) => toKhalaCliError(error, `Could not parse ${label}.`),
  })
}

function urlFor(baseUrl: string | undefined, path: string): string {
  const base = (baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "")
  return `${base}${path}`
}

function readTraceRef(response: Response): string | undefined {
  return response.headers.get("x-openagents-trace-ref") ??
    response.headers.get("x-oa-trace-ref") ??
    response.headers.get("x-trace-id") ??
    undefined
}

export function toKhalaCliError(error: unknown, fallback: string): KhalaCliError {
  if (error instanceof KhalaCliError) return error
  return new KhalaCliError({
    reason: error instanceof Error ? `${fallback} ${error.message}` : `${fallback} ${String(error)}`,
    code: "runtime_error",
  })
}

function mergeMetadata(
  previous: StreamFrameMetadata,
  next: StreamFrameMetadata | undefined,
): StreamFrameMetadata {
  if (next === undefined) return previous
  return {
    ...previous,
    ...withoutUndefined(next),
    usage: next.usage ?? previous.usage,
  }
}

function shouldContinueAfterLength(metadata: StreamFrameMetadata): boolean {
  const finishReason = metadata.finishReason?.trim().toLowerCase()
  return finishReason === "length" || finishReason === "max_tokens" || finishReason === "max_tokens_reached"
}

function mergeUsageMetadata(
  latest: StreamFrameMetadata,
  previous: StreamFrameMetadata,
): StreamFrameMetadata {
  if (latest.usage === undefined || previous.usage === undefined) {
    return latest
  }
  return {
    ...latest,
    usage: {
      cachedPromptTokens:
        latest.usage.cachedPromptTokens === undefined && previous.usage.cachedPromptTokens === undefined
          ? undefined
          : (latest.usage.cachedPromptTokens ?? 0) + (previous.usage.cachedPromptTokens ?? 0),
      completionTokens: latest.usage.completionTokens + previous.usage.completionTokens,
      promptTokens: latest.usage.promptTokens + previous.usage.promptTokens,
      totalTokens: latest.usage.totalTokens + previous.usage.totalTokens,
    },
  }
}

function withoutUndefined(record: StreamFrameMetadata): Partial<StreamFrameMetadata> {
  return Object.fromEntries(
    Object.entries(record).filter(([, value]) => value !== undefined),
  ) as Partial<StreamFrameMetadata>
}

function buildTurnMetadata(input: {
  readonly durationMs: number
  readonly mode: "public" | "api"
  readonly streamMetadata: StreamFrameMetadata
  readonly timings: {
    readonly firstTokenAt?: number | undefined
    readonly responseAt: number
    readonly startedAt: number
  }
  readonly text: string
}): ChatTurnMetadata {
  const estimatedUsage = input.streamMetadata.usage === undefined
  const usage = input.streamMetadata.usage ?? estimateUsage(input.text)
  const streamDurationMs = input.timings.firstTokenAt === undefined
    ? undefined
    : Math.max(0, input.timings.startedAt + input.durationMs - input.timings.firstTokenAt)
  const seconds = (streamDurationMs ?? input.durationMs) / 1_000
  const tokensPerSecond = seconds > 0 ? usage.completionTokens / seconds : undefined
  return {
    adapterRouteMetadata: input.streamMetadata.adapterRouteMetadata,
    durationMs: input.durationMs,
    estimatedUsage,
    fallbackReason: input.streamMetadata.fallbackReason,
    finishReason: input.streamMetadata.finishReason,
    mode: input.mode,
    primaryAdapterId: input.streamMetadata.primaryAdapterId,
    requestedModel: input.streamMetadata.requestedModel,
    servedAdapterId: input.streamMetadata.servedAdapterId,
    servedModel: input.streamMetadata.servedModel,
    streamDurationMs,
    timeToFirstByteMs: Math.max(0, input.timings.responseAt - input.timings.startedAt),
    timeToFirstTokenMs: input.timings.firstTokenAt === undefined
      ? undefined
      : Math.max(0, input.timings.firstTokenAt - input.timings.startedAt),
    tokensPerSecond,
    traceRef: input.streamMetadata.traceRef,
    traceUrl: input.streamMetadata.traceUrl,
    traceUuid: input.streamMetadata.traceUuid,
    usage,
  }
}

function estimateUsage(text: string): KhalaStreamUsage {
  const completionTokens = Math.max(1, Math.ceil(text.trim().length / 4))
  return {
    completionTokens,
    promptTokens: 0,
    totalTokens: completionTokens,
  }
}
