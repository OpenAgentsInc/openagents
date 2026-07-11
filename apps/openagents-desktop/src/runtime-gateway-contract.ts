import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopRuntimeGatewayInvokeChannel = "openagents-desktop/runtime-gateway/invoke" as const
export const DesktopRuntimeGatewayEventChannel = "openagents-desktop/runtime-gateway/event" as const
export const DesktopRuntimeGatewayProtocolVersion = 3 as const

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
const TimelineTextSchema = Schema.String.check(Schema.isMaxLength(20_000))
const TimelineTypeSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
)
const ArtifactRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(1_024),
)
const TimelineTimestampSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(64),
)
const NonNegativeIntSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
)
const SyncPhaseSchema = Schema.Literals([
  "idle",
  "bootstrapping",
  "catching_up",
  "live",
  "must_refetch",
  "denied",
])

export const DesktopRuntimeCapabilityIdSchema = Schema.Literals([
  "agent-timeline",
  "codex-history",
  "conversation-sync",
  "conversation-stream",
  "git-review",
  "khala-sync",
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
    kind: Schema.Literal("command"),
    commandId: Schema.String,
    command: Schema.Union([
      Schema.Struct({
        id: Schema.Literal("conversation.interrupt"),
        threadRef: Schema.String,
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
  capabilities: Schema.Array(DesktopRuntimeCapabilitySchema),
})

const ConversationStatusSchema = Schema.Struct({
  phase: SyncPhaseSchema,
  cursor: Schema.NullOr(Schema.Number),
  pendingMutationCount: Schema.Number,
})

const ConfirmedThreadSchema = Schema.Struct({
  threadRef: PublicRefSchema,
  title: ConversationTitleSchema,
  messageCount: Schema.Number,
  lastMessageAt: Schema.NullOr(Schema.String),
  updatedAt: Schema.String,
  version: Schema.Number,
})

const ConfirmedMessageSchema = Schema.Struct({
  messageRef: PublicRefSchema,
  threadRef: PublicRefSchema,
  body: ConversationBodySchema,
  createdAt: Schema.String,
  updatedAt: Schema.String,
  version: Schema.Number,
})

const ConfirmedAgentRunSchema = Schema.Struct({
  runRef: PublicRefSchema,
  routeRef: PublicRefSchema,
  status: Schema.Literals(["queued", "running", "waiting_for_input", "completed", "failed", "canceled"]),
  createdAt: TimelineTimestampSchema,
  updatedAt: TimelineTimestampSchema,
  startedAt: Schema.NullOr(TimelineTimestampSchema),
  completedAt: Schema.NullOr(TimelineTimestampSchema),
  failedAt: Schema.NullOr(TimelineTimestampSchema),
  canceledAt: Schema.NullOr(TimelineTimestampSchema),
  version: NonNegativeIntSchema,
})

const ConfirmedAgentTimelineEventSchema = Schema.Struct({
  eventRef: PublicRefSchema,
  runRef: PublicRefSchema,
  sequence: NonNegativeIntSchema,
  eventType: TimelineTypeSchema,
  summary: TimelineTextSchema,
  status: Schema.NullOr(TimelineTypeSchema),
  artifactRefs: Schema.Array(ArtifactRefSchema).check(Schema.isMaxLength(100)),
  createdAt: TimelineTimestampSchema,
  version: NonNegativeIntSchema,
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
    status: ConversationStatusSchema,
    threads: Schema.Array(ConfirmedThreadSchema),
  }),
  Schema.Struct({
    kind: Schema.Literal("conversation_thread"),
    requestId: Schema.String,
    threadRef: PublicRefSchema,
    status: ConversationStatusSchema,
    messages: Schema.Array(ConfirmedMessageSchema),
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
    status: ConversationStatusSchema,
    run: ConfirmedAgentRunSchema,
    events: Schema.Array(ConfirmedAgentTimelineEventSchema).check(Schema.isMaxLength(500)),
  }),
  Schema.Struct({
    kind: Schema.Literal("agent_timeline_unavailable"),
    requestId: Schema.String,
    reason: Schema.Literals(["not_live", "not_found", "read_failed"]),
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
