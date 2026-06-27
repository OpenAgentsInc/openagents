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

// Owner-authenticated Artanis operator channel (#6363, epic #6359).
// Shared contract with the mobile app and the core Worker route:
//   POST /api/operator/artanis/chat
//   body  { messages: [{ role, content }] }
//   ->    { reply }   (owner-only operator persona, NOT public Khala roleplay)
export const ARTANIS_CHAT_PATH = "/api/operator/artanis/chat"

export const ArtanisChatRequest = S.Struct({
  messages: S.Array(KhalaChatMessage),
})
export type ArtanisChatRequest = typeof ArtanisChatRequest.Type

export const ArtanisChatResponse = S.Struct({
  reply: S.String,
  traceRef: S.optional(S.String),
})
export type ArtanisChatResponse = typeof ArtanisChatResponse.Type

// OpenAgents device-auth (`khala login`, #6363, epic #6359).
//
// This mirrors the shared, already-deployed server contract owned by the core
// lane (apps/openagents.com/workers/api/src/pylon-openagents-auth-routes.ts),
// the same flow the Pylon CLI uses. The CLI authenticates with its existing
// agent token (auto-minted free key or --token), starts a link attempt, prints
// the verification URL + user code, and polls the status endpoint until the
// browser sign-in links that token to the owner's OpenAgents account.
//
//   POST /api/pylon/auth/openagents/device/start   (Bearer agent token)
//     -> 201 { status: "pending", attemptId, expiresAt, intervalSeconds,
//              userCode, verificationUrl, linkedAgent: { tokenPrefix } }
//     -> 200 { status: "linked", linkedAgent: { tokenPrefix } } (already linked)
//   GET  /api/pylon/auth/openagents/device/{attemptId}  (Bearer agent token)
//     -> 200 { status: "pending" | "linked", ... }
//     -> 410 { status: "expired", attemptId }
//
// On link the SAME agent token becomes the owner-linked token; the server does
// not mint a new credential here, so the CLI keeps and re-stores the token it
// authenticated with.
export const OPENAGENTS_DEVICE_AUTH_START_PATH =
  "/api/pylon/auth/openagents/device/start"
export const openAgentsDeviceAuthStatusPath = (attemptId: string): string =>
  `/api/pylon/auth/openagents/device/${encodeURIComponent(attemptId)}`

const DeviceAuthLinkedAgent = S.Struct({
  tokenPrefix: S.optional(S.String),
})

export const DeviceAuthStartResponse = S.Struct({
  schema: S.optional(S.String),
  status: S.Literals(["pending", "linked"]),
  attemptId: S.optional(S.String),
  expiresAt: S.optional(S.String),
  intervalSeconds: S.optional(S.Number),
  userCode: S.optional(S.String),
  verificationUrl: S.optional(S.String),
  linkedAgent: S.optional(DeviceAuthLinkedAgent),
})
export type DeviceAuthStartResponse = typeof DeviceAuthStartResponse.Type

export const DeviceAuthStatusResponse = S.Struct({
  schema: S.optional(S.String),
  status: S.Literals(["pending", "linked", "expired"]),
  attemptId: S.optional(S.String),
  expiresAt: S.optional(S.String),
  intervalSeconds: S.optional(S.Number),
  linkedAgent: S.optional(DeviceAuthLinkedAgent),
})
export type DeviceAuthStatusResponse = typeof DeviceAuthStatusResponse.Type

// GET /api/agents/me identity projection (Bearer agent token). Used after a
// successful login to print "Signed in as <displayName>." The agent display
// name and (optional) agent email are what the token resolves to; the owner
// email itself is not exposed on this surface.
export const AGENTS_ME_PATH = "/api/agents/me"

export const AgentMeResponse = S.Struct({
  authenticated: S.optional(S.Boolean),
  agent: S.optional(
    S.Struct({
      user: S.optional(
        S.Struct({
          displayName: S.optional(S.NullOr(S.String)),
          primaryEmail: S.optional(S.NullOr(S.String)),
        }),
      ),
      tokenPrefix: S.optional(S.String),
    }),
  ),
})
export type AgentMeResponse = typeof AgentMeResponse.Type

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

export interface ArtanisTurnOptions {
  readonly baseUrl: string
  readonly token: string
  readonly messages: ReadonlyArray<KhalaChatMessage>
  readonly fetch?: typeof fetch | undefined
}

export interface ArtanisTurnResult {
  readonly text: string
  readonly traceRef?: string | undefined
}
