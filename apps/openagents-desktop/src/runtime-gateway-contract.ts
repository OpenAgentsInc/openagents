import { Exit, Schema } from "@effect-native/core/effect"
import { Schema as CanonicalSchema } from "effect"
import {
  ConfirmedAgentRunSchema as CanonicalConfirmedAgentRunSchema,
  ConfirmedAgentTimelineEventSchema as CanonicalConfirmedAgentTimelineEventSchema,
  ConfirmedChatMessageSchema as CanonicalConfirmedChatMessageSchema,
  ConfirmedChatThreadSchema as CanonicalConfirmedChatThreadSchema,
  KhalaSyncConversationStatusSchema as CanonicalKhalaSyncConversationStatusSchema,
  type ConfirmedAgentRun,
  type ConfirmedAgentTimelineEvent,
  type ConfirmedChatMessage,
  type ConfirmedChatThread,
  type KhalaSyncConversationStatus,
} from "@openagentsinc/khala-sync-client"
import { CodexHistoryCatalogSchema, CodexHistoryPageSchema } from "./codex-history-contract.ts"

/**
 * Effect Native currently pins a newer Effect build than Khala Sync. Keep the
 * canonical schema as the sole decoder and adapt it to the host's Effect
 * version with an opaque declaration instead of re-declaring its fields.
 */
const canonicalBoundary = <Value>(schema: unknown) =>
  Schema.declare<Value>((value): value is Value => {
    try {
      CanonicalSchema.decodeUnknownSync(schema as never)(value)
      return true
    } catch {
      return false
    }
  })

const ConfirmedAgentRunSchema = canonicalBoundary<ConfirmedAgentRun>(CanonicalConfirmedAgentRunSchema)
const ConfirmedAgentTimelineEventSchema = canonicalBoundary<ConfirmedAgentTimelineEvent>(CanonicalConfirmedAgentTimelineEventSchema)
const ConfirmedChatMessageSchema = canonicalBoundary<ConfirmedChatMessage>(CanonicalConfirmedChatMessageSchema)
const ConfirmedChatThreadSchema = canonicalBoundary<ConfirmedChatThread>(CanonicalConfirmedChatThreadSchema)
const KhalaSyncConversationStatusSchema = canonicalBoundary<KhalaSyncConversationStatus>(CanonicalKhalaSyncConversationStatusSchema)

export const DesktopRuntimeGatewayInvokeChannel = "openagents-desktop/runtime-gateway/invoke" as const
export const DesktopRuntimeGatewayEventChannel = "openagents-desktop/runtime-gateway/event" as const
export const DesktopRuntimeGatewayProtocolVersion = 6 as const

const PublicRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const ConversationTitleSchema = Schema.String.check(Schema.isMaxLength(160))
const ConversationBodySchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(20_000),
)
const NonNegativeIntSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
)

export const DesktopRuntimeCapabilityIdSchema = Schema.Literals([
  "agent-timeline",
  "codex-history",
  "conversation-sync",
  "conversation-stream",
  "git-review",
  "khala-sync",
  "local-identity",
  "openagents-session",
  "provider-accounts",
  "workspace",
])
export type DesktopRuntimeCapabilityId = typeof DesktopRuntimeCapabilityIdSchema.Type

export const DesktopRuntimeCapabilitySchema = Schema.Struct({
  id: DesktopRuntimeCapabilityIdSchema,
  state: Schema.Literals(["available", "unavailable"]),
  reason: Schema.optional(Schema.String),
})

export const DesktopRuntimeGatewayRequestSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("runtime.bootstrap") }),
  }),
  Schema.Struct({
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("conversation.catalog") }),
  }),
  Schema.Struct({ kind: Schema.Literal("query"), requestId: Schema.String, query: Schema.Struct({ id: Schema.Literal("codex.history.catalog") }) }),
  Schema.Struct({
    kind: Schema.Literal("query"), requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("codex.history.page"), threadRef: PublicRefSchema, offset: NonNegativeIntSchema, limit: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 500 })) }),
  }),
  Schema.Struct({
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("conversation.thread"),
      threadRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("agent.timeline"),
      runRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("conversation.timeline"),
      threadRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("command"),
    commandId: Schema.String,
    command: Schema.Union([
      Schema.Struct({
        id: Schema.Literal("conversation.interrupt"),
        commandRef: PublicRefSchema,
        threadRef: PublicRefSchema,
        runRef: PublicRefSchema,
      }),
      Schema.Struct({
        id: Schema.Literal("conversation.start"),
        threadRef: PublicRefSchema,
        messageRef: PublicRefSchema,
        runRef: PublicRefSchema,
      }),
      Schema.Struct({
        id: Schema.Literal("conversation.create"),
        threadRef: PublicRefSchema,
        title: ConversationTitleSchema,
      }),
      Schema.Struct({
        id: Schema.Literal("conversation.append"),
        threadRef: PublicRefSchema,
        messageRef: PublicRefSchema,
        body: ConversationBodySchema,
      }),
      Schema.Struct({ id: Schema.Literal("session.sign_in") }),
      Schema.Struct({ id: Schema.Literal("session.sign_out") }),
    ]),
  }),
])
export type DesktopRuntimeGatewayRequest = typeof DesktopRuntimeGatewayRequestSchema.Type

const DesktopRuntimeBootstrapSchema = Schema.Struct({
  kind: Schema.Literal("runtime.bootstrap"),
  protocolVersion: Schema.Literal(DesktopRuntimeGatewayProtocolVersion),
  lifecycle: Schema.Literals(["starting", "ready", "disposed"]),
  sessionPhase: Schema.Literals(["signed_out", "unverified", "session_ready", "denied", "unavailable"]),
  identityTier:Schema.Literals(["local_only","account_linked","local_unavailable"]),
  capabilities: Schema.Array(DesktopRuntimeCapabilitySchema),
})

export const DesktopRuntimeGatewayResponseSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("query_result"),
    requestId: Schema.String,
    result: DesktopRuntimeBootstrapSchema,
  }),
  Schema.Struct({
    kind: Schema.Literal("conversation_catalog"),
    requestId: Schema.String,
    status: KhalaSyncConversationStatusSchema,
    threads: Schema.Array(ConfirmedChatThreadSchema),
  }),
  Schema.Struct({ kind: Schema.Literal("codex_history_catalog"), requestId: Schema.String, catalog: CodexHistoryCatalogSchema }),
  Schema.Struct({ kind: Schema.Literal("codex_history_page"), requestId: Schema.String, page: CodexHistoryPageSchema }),
  Schema.Struct({ kind: Schema.Literal("codex_history_unavailable"), requestId: Schema.String, reason: Schema.Literals(["not_found", "read_failed"]) }),
  Schema.Struct({
    kind: Schema.Literal("conversation_thread"),
    requestId: Schema.String,
    threadRef: PublicRefSchema,
    status: KhalaSyncConversationStatusSchema,
    messages: Schema.Array(ConfirmedChatMessageSchema),
  }),
  Schema.Struct({
    kind: Schema.Literal("conversation_unavailable"),
    requestId: Schema.String,
    reason: Schema.Literals(["not_live", "read_failed"]),
  }),
  Schema.Struct({
    kind: Schema.Literal("agent_timeline"),
    requestId: Schema.String,
    runRef: PublicRefSchema,
    status: KhalaSyncConversationStatusSchema,
    run: ConfirmedAgentRunSchema,
    events: Schema.Array(ConfirmedAgentTimelineEventSchema).check(Schema.isMaxLength(500)),
  }),
  Schema.Struct({
    kind: Schema.Literal("conversation_timeline"),
    requestId: Schema.String,
    threadRef: PublicRefSchema,
    status: KhalaSyncConversationStatusSchema,
    run: Schema.NullOr(ConfirmedAgentRunSchema),
    events: Schema.Array(ConfirmedAgentTimelineEventSchema).check(Schema.isMaxLength(500)),
  }),
  Schema.Struct({
    kind: Schema.Literal("agent_timeline_unavailable"),
    requestId: Schema.String,
    reason: Schema.Literals(["not_live", "not_found", "read_failed"]),
  }),
  Schema.Struct({
    kind: Schema.Literal("runtime_command_outcome"),
    commandId: Schema.String,
    threadRef: PublicRefSchema,
    runRef: PublicRefSchema,
    messageRef: Schema.optional(PublicRefSchema),
    status: Schema.Literals(["accepted", "unknown_pending_reconcile", "rejected", "unavailable"]),
    mutationId: Schema.optional(NonNegativeIntSchema),
    reason: Schema.optional(Schema.String),
  }),
  Schema.Struct({
    kind: Schema.Literal("conversation_mutation_outcome"),
    commandId: Schema.String,
    status: Schema.Literals(["pending_reconcile", "unavailable"]),
    mutationId: Schema.optional(Schema.Number),
  }),
  Schema.Struct({
    kind: Schema.Literal("command_outcome"),
    commandId: Schema.String,
    status: Schema.Literal("unavailable"),
    reason: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("session_outcome"),
    commandId: Schema.String,
    status: Schema.Literals(["completed", "cancelled", "unavailable"]),
    phase: Schema.Literals(["session_ready", "signed_out", "unavailable"]),
  }),
  Schema.Struct({
    kind: Schema.Literal("request_rejected"),
    reason: Schema.Literals(["invalid_request", "untrusted_renderer", "gateway_disposed"]),
  }),
])
export type DesktopRuntimeGatewayResponse = typeof DesktopRuntimeGatewayResponseSchema.Type

export const DesktopRuntimeGatewayEventSchema = Schema.Struct({
  kind: Schema.Literal("runtime.lifecycle"),
  protocolVersion: Schema.Literal(DesktopRuntimeGatewayProtocolVersion),
  sequence: Schema.Number,
  phase: Schema.Literals(["ready", "disposed"]),
})
export type DesktopRuntimeGatewayEvent = typeof DesktopRuntimeGatewayEventSchema.Type

const decode = (schema: any, value: unknown): unknown | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeDesktopRuntimeGatewayRequest = (value: unknown): DesktopRuntimeGatewayRequest | null =>
  decode(DesktopRuntimeGatewayRequestSchema, value) as DesktopRuntimeGatewayRequest | null

export const decodeDesktopRuntimeGatewayResponse = (value: unknown): DesktopRuntimeGatewayResponse | null =>
  decode(DesktopRuntimeGatewayResponseSchema, value) as DesktopRuntimeGatewayResponse | null

export const decodeDesktopRuntimeGatewayEvent = (value: unknown): DesktopRuntimeGatewayEvent | null =>
  decode(DesktopRuntimeGatewayEventSchema, value) as DesktopRuntimeGatewayEvent | null

export const invalidDesktopRuntimeGatewayResponse = (): DesktopRuntimeGatewayResponse => ({
  kind: "request_rejected",
  reason: "invalid_request",
})
