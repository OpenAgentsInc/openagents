import { Effect, Schema as S } from "effect"
import { validatePublicConversation } from "./bounds.js"
import { decodeOpenAiFrame, decodePublicFrame, readSseFrames } from "./sse.js"
import {
  DEFAULT_BASE_URL,
  FreeKeyResponse,
  KHALA_MODEL_ID,
  KhalaCliError,
  KhalaPublicChatRequest,
  OpenAiModelsResponse,
  type ChatClientOptions,
  type ChatTurnOptions,
  type ChatTurnResult,
} from "./types.js"

export function runChatTurn(options: ChatTurnOptions): Effect.Effect<ChatTurnResult, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* postChat(options)
    const text = yield* consumeStream(response, options.mode, options.onDelta)
    return { text, assistantMessage: { role: "assistant" as const, content: text } }
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

function postChat(options: ChatTurnOptions): Effect.Effect<Response, KhalaCliError> {
  return Effect.gen(function* () {
    if (options.mode === "public") {
      validatePublicConversation(options.messages)
      const body = S.encodeSync(KhalaPublicChatRequest)({ messages: [...options.messages] })
      return yield* request({
        fetch: options.fetch,
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
}): Effect.Effect<Response, KhalaCliError> {
  return Effect.gen(function* () {
    const fetchImpl = input.fetch ?? fetch
    const response = yield* Effect.tryPromise({
      try: () => fetchImpl(input.url, input.init),
      catch: (error) => toKhalaCliError(error, `Request failed for ${input.url}.`),
    })
    if (!response.ok) {
      const envelope = yield* readErrorEnvelope(response)
      return yield* new KhalaCliError({
        reason: envelope.reason,
        ...(envelope.code === undefined ? {} : { code: envelope.code }),
        statusCode: response.status,
      })
    }
    return response
  })
}

function readJsonResponse(response: Response, label: string): Effect.Effect<unknown, KhalaCliError> {
  return Effect.tryPromise({
    try: () => response.json(),
    catch: (error) => toKhalaCliError(error, `Could not parse ${label}.`),
  })
}

function readErrorEnvelope(response: Response): Effect.Effect<{ readonly reason: string; readonly code?: string | undefined }, KhalaCliError> {
  return Effect.tryPromise({
    try: async () => {
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
          code: typeof payload.code === "string" ? payload.code : undefined,
        }
      } catch {
        return { reason: text.trim() }
      }
    },
    catch: (error) => toKhalaCliError(error, "Could not read Khala error response."),
  })
}

function urlFor(baseUrl: string | undefined, path: string): string {
  const base = (baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "")
  return `${base}${path}`
}

export function toKhalaCliError(error: unknown, fallback: string): KhalaCliError {
  if (error instanceof KhalaCliError) return error
  return new KhalaCliError({
    reason: error instanceof Error ? `${fallback} ${error.message}` : `${fallback} ${String(error)}`,
    code: "runtime_error",
  })
}
