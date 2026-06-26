import { Schema as S } from "effect"

export const DEFAULT_BASE_URL = "https://openagents.com"
export const KHALA_MODEL_ID = "openagents/khala"

export const KHALA_CHAT_MAX_MESSAGE_CHARS = 8_000
export const KHALA_CHAT_MAX_MESSAGES = 40
export const KHALA_CHAT_MAX_TOTAL_CHARS = 24_000

export const KhalaChatRole = S.Literals(["user", "assistant"])
export type KhalaChatRole = typeof KhalaChatRole.Type

export const KhalaChatMessage = S.Struct({
  role: KhalaChatRole,
  content: S.String,
})
export type KhalaChatMessage = typeof KhalaChatMessage.Type

export const KhalaPublicChatRequest = S.Struct({
  messages: S.Array(KhalaChatMessage),
})
export type KhalaPublicChatRequest = typeof KhalaPublicChatRequest.Type

export const PublicDeltaPayload = S.Struct({
  text: S.String,
})

export const PublicDonePayload = S.Struct({
  done: S.Boolean,
})

export const PublicErrorPayload = S.Struct({
  error: S.String,
  code: S.optional(S.String),
})

export const OpenAiStreamPayload = S.Struct({
  choices: S.Array(
    S.Struct({
      delta: S.Struct({
        content: S.optional(S.String),
      }),
    }),
  ),
})

export const OpenAiModelsResponse = S.Record(S.String, S.Unknown)
export const FreeKeyResponse = S.Record(S.String, S.Unknown)

export const KhalaFeedbackResponse = S.Struct({
  schemaVersion: S.String,
  feedbackRef: S.String,
  traceRef: S.NullOr(S.String),
  createdAt: S.String,
})
export type KhalaFeedbackResponse = typeof KhalaFeedbackResponse.Type

export class KhalaCliError extends S.TaggedErrorClass<KhalaCliError>()("KhalaCliError", {
  reason: S.String,
  code: S.optional(S.String),
  statusCode: S.optional(S.Number),
}) {}

export type ChatMode = "public" | "api"

export interface ChatClientOptions {
  readonly mode: ChatMode
  readonly baseUrl: string
  readonly token?: string | undefined
  readonly fetch?: typeof fetch | undefined
  readonly onRetry?: ((event: KhalaRetryEvent) => void) | undefined
}

export interface ChatTurnOptions extends ChatClientOptions {
  readonly messages: ReadonlyArray<KhalaChatMessage>
  readonly onDelta?: ((text: string) => void) | undefined
}

export interface ChatTurnResult {
  readonly text: string
  readonly assistantMessage: KhalaChatMessage
  readonly traceRef?: string | undefined
}

export interface KhalaRetryEvent {
  readonly retry: number
  readonly maxRetries: number
  readonly delayMs: number
  readonly error: KhalaCliError
}

export interface KhalaFeedbackSubmitOptions {
  readonly baseUrl: string
  readonly fetch?: typeof fetch | undefined
  readonly feedback: string
  readonly traceRef?: string | undefined
  readonly source: string
  readonly clientVersion: string
}
