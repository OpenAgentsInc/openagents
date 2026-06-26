import { Effect, Schema as S } from "effect"
import { validatePublicConversation } from "./bounds.js"
import { decodeOpenAiFrame, decodePublicFrame, readSseFrames } from "./sse.js"
import {
  DEFAULT_BASE_URL,
  FreeKeyResponse,
  KHALA_MODEL_ID,
  KhalaFeedbackResponse,
  KhalaCliError,
  KhalaPublicChatRequest,
  OpenAiModelsResponse,
  type ChatClientOptions,
  type ChatTurnOptions,
  type ChatTurnResult,
  type KhalaFeedbackSubmitOptions,
} from "./types.js"

const MAX_REQUEST_RETRIES = 2
const RETRY_DELAYS_MS = [400, 1_000] as const

export function runChatTurn(options: ChatTurnOptions): Effect.Effect<ChatTurnResult, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* postChat(options)
    const text = yield* consumeStream(response, options.mode, options.onDelta)
    return {
      text,
      assistantMessage: { role: "assistant" as const, content: text },
      traceRef: readTraceRef(response),
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
        }),
      },
    })
  })
}

function consumeStream(
  response: Response,
  mode: "public" | "api",
  onDelta: ((text: string) => void) | undefined,
): Effect.Effect<string, KhalaCliError> {
  return Effect.tryPromise({
    try: async () => {
      if (response.body === null) {
        throw new KhalaCliError({ reason: "Khala response did not include a stream body.", code: "missing_stream_body" })
      }
      let assembled = ""
      for await (const frame of readSseFrames(response.body)) {
        const decoded = mode === "public" ? decodePublicFrame(frame) : decodeOpenAiFrame(frame)
        if (decoded.kind === "done") break
        if (decoded.text.length > 0) {
          assembled += decoded.text
          onDelta?.(decoded.text)
        }
      }
      return assembled
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

async function readErrorEnvelope(response: Response): Promise<{ readonly reason: string; readonly code?: string | undefined }> {
  const text = await response.text()
  if (text.trim().length === 0) {
    return { reason: `Khala returned HTTP ${response.status}.` }
  }
  try {
    const payload = JSON.parse(text) as { readonly error?: unknown; readonly code?: unknown; readonly message?: unknown }
    const reason = typeof payload.error === "string"
      ? payload.error
      : typeof payload.message === "string"
        ? payload.message
        : `Khala returned HTTP ${response.status}.`
    return {
      reason,
      code: typeof payload.code === "string" ? payload.code : typeof payload.error === "string" ? payload.error : undefined,
    }
  } catch {
    return { reason: text.trim() }
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
