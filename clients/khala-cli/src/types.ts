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

export const PublicReasoningPayload = S.Struct({
  text: S.String,
})

export const PublicDonePayload = S.Struct({
  done: S.Boolean,
})

export const PublicErrorPayload = S.Struct({
  error: S.String,
  code: S.optional(S.String),
  reason: S.optional(S.String),
  traceRef: S.optional(S.String),
})

export const KhalaStreamUsage = S.Struct({
  cachedPromptTokens: S.optional(S.Number),
  completionTokens: S.Number,
  promptTokens: S.Number,
  totalTokens: S.Number,
})
export type KhalaStreamUsage = typeof KhalaStreamUsage.Type

export const PublicMetaPayload = S.Struct({
  adapterRouteMetadata: S.optional(S.Unknown),
  fallbackReason: S.optional(S.NullOr(S.String)),
  finishReason: S.optional(S.String),
  primaryAdapterId: S.optional(S.String),
  requestedModel: S.optional(S.String),
  servedAdapterId: S.optional(S.String),
  servedModel: S.optional(S.String),
  traceRef: S.optional(S.String),
  traceUrl: S.optional(S.String),
  traceUuid: S.optional(S.String),
  usage: S.optional(KhalaStreamUsage),
})
export type PublicMetaPayload = typeof PublicMetaPayload.Type

export const OpenAiStreamPayload = S.Struct({
  id: S.optional(S.String),
  model: S.optional(S.String),
  openagents: S.optional(S.Unknown),
  choices: S.Array(
    S.Struct({
      delta: S.Struct({
        content: S.optional(S.String),
        reasoning: S.optional(S.String),
        reasoning_content: S.optional(S.String),
      }),
    }),
  ),
  usage: S.optional(S.Struct({
    cached_tokens: S.optional(S.Number),
    completion_tokens: S.Number,
    prompt_tokens: S.Number,
    total_tokens: S.Number,
  })),
})

export const OpenAiModelsResponse = S.Record(S.String, S.Unknown)
export const FreeKeyResponse = S.Struct({
  credential: S.Struct({
    token: S.String,
  }),
})
export type FreeKeyResponse = typeof FreeKeyResponse.Type

export const KhalaFeedbackResponse = S.Struct({
  schemaVersion: S.String,
  feedbackRef: S.String,
  traceRef: S.NullOr(S.String),
  createdAt: S.String,
})
export type KhalaFeedbackResponse = typeof KhalaFeedbackResponse.Type

export const KhalaTokensResponse = S.Struct({
  schemaVersion: S.String,
  tokensServed: S.Int,
  generatedAt: S.String,
  staleness: S.Unknown,
})
export type KhalaTokensResponse = typeof KhalaTokensResponse.Type

export class KhalaCliError extends S.TaggedErrorClass<KhalaCliError>()("KhalaCliError", {
  reason: S.String,
  code: S.optional(S.String),
  statusCode: S.optional(S.Number),
  traceRef: S.optional(S.String),
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
  readonly onReasoning?: ((text: string) => void) | undefined
}

export interface ChatTurnResult {
  readonly text: string
  readonly reasoningText: string
  readonly assistantMessage: KhalaChatMessage
  readonly metadata: ChatTurnMetadata
  readonly traceRef?: string | undefined
}

export interface ChatTurnMetadata {
  readonly adapterRouteMetadata?: unknown
  readonly durationMs: number
  readonly estimatedUsage: boolean
  readonly fallbackReason?: string | null | undefined
  readonly finishReason?: string | undefined
  readonly mode: ChatMode
  readonly primaryAdapterId?: string | undefined
  readonly requestedModel?: string | undefined
  readonly servedAdapterId?: string | undefined
  readonly servedModel?: string | undefined
  readonly streamDurationMs?: number | undefined
  readonly timeToFirstByteMs?: number | undefined
  readonly timeToFirstTokenMs?: number | undefined
  readonly tokensPerSecond?: number | undefined
  readonly traceRef?: string | undefined
  readonly traceUrl?: string | undefined
  readonly traceUuid?: string | undefined
  readonly usage: KhalaStreamUsage
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
