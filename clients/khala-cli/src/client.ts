import { Effect, Schema as S } from "effect"
import { validatePublicConversation } from "./bounds.js"
import { decodeOpenAiFrame, decodePublicFrame, readSseFrames, type StreamFrameMetadata } from "./sse.js"
import {
  DEFAULT_BASE_URL,
  FreeKeyResponse,
  KHALA_MODEL_ID,
  KhalaFeedbackResponse,
  KhalaCliError,
  KhalaPublicChatRequest,
  KhalaTokensResponse,
  OpenAiModelsResponse,
  type ChatClientOptions,
  type ChatTurnOptions,
  type ChatTurnResult,
  type ChatTurnMetadata,
  type KhalaFeedbackSubmitOptions,
  type KhalaStreamUsage,
} from "./types.js"

const MAX_REQUEST_RETRIES = 5
const RETRY_DELAYS_MS = [400, 1_000, 2_000, 4_000, 8_000] as const

export function runChatTurn(options: ChatTurnOptions): Effect.Effect<ChatTurnResult, KhalaCliError> {
  return Effect.gen(function* () {
    const startedAt = Date.now()
    const response = yield* postChat(options)
    const traceRef = readTraceRef(response)
    const stream = yield* consumeStream(response, options.mode, options.onDelta)
    const durationMs = Math.max(0, Date.now() - startedAt)
    const metadata = buildTurnMetadata({
      durationMs,
      mode: options.mode,
      streamMetadata: {
        ...stream.metadata,
        traceRef: stream.metadata.traceRef ?? traceRef,
      },
      text: stream.text,
    })
    return {
      text: stream.text,
      assistantMessage: { role: "assistant" as const, content: stream.text },
      metadata,
      traceRef: metadata.traceRef,
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

export function mintFreeKey(options: Pick<ChatClientOptions, "baseUrl" | "fetch">): Effect.Effect<unknown, KhalaCliError> {
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
        reason: "--api requires --token or OPENAGENTS_AGENT_TOKEN. Use --mint-free-key to request a free key when that endpoint is armed.",
        code: "missing_token",
      })
    }

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
        },
        body: JSON.stringify({
          model: KHALA_MODEL_ID,
          messages: options.messages,
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
  onDelta: ((text: string) => void) | undefined,
): Effect.Effect<{ readonly metadata: StreamFrameMetadata; readonly text: string }, KhalaCliError> {
  return Effect.tryPromise({
    try: async () => {
      if (response.body === null) {
        throw new KhalaCliError({ reason: "Khala response did not include a stream body.", code: "missing_stream_body" })
      }
      let assembled = ""
      let metadata: StreamFrameMetadata = {}
      for await (const frame of readSseFrames(response.body)) {
        const decoded = mode === "public" ? decodePublicFrame(frame) : decodeOpenAiFrame(frame)
        if (decoded.kind === "done") break
        if (decoded.kind === "meta") {
          metadata = mergeMetadata(metadata, decoded.metadata)
          continue
        }
        metadata = mergeMetadata(metadata, decoded.metadata)
        if (decoded.text.length > 0) {
          assembled += decoded.text
          onDelta?.(decoded.text)
        }
      }
      return { metadata, text: assembled }
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

function withoutUndefined(record: StreamFrameMetadata): Partial<StreamFrameMetadata> {
  return Object.fromEntries(
    Object.entries(record).filter(([, value]) => value !== undefined),
  ) as Partial<StreamFrameMetadata>
}

function buildTurnMetadata(input: {
  readonly durationMs: number
  readonly mode: "public" | "api"
  readonly streamMetadata: StreamFrameMetadata
  readonly text: string
}): ChatTurnMetadata {
  const estimatedUsage = input.streamMetadata.usage === undefined
  const usage = input.streamMetadata.usage ?? estimateUsage(input.text)
  const seconds = input.durationMs / 1_000
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
    tokensPerSecond,
    traceRef: input.streamMetadata.traceRef,
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
