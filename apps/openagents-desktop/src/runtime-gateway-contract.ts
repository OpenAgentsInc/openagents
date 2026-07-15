import { Exit, Schema } from "@effect-native/core/effect"
import { Schema as CanonicalSchema } from "effect"
import {
  ConfirmedAgentRunSchema as CanonicalConfirmedAgentRunSchema,
  ConfirmedAgentTimelineEventSchema as CanonicalConfirmedAgentTimelineEventSchema,
  ConfirmedChatMessageSchema as CanonicalConfirmedChatMessageSchema,
  ConfirmedChatThreadSchema as CanonicalConfirmedChatThreadSchema,
  KhalaSyncConversationStatusSchema as CanonicalKhalaSyncConversationStatusSchema,
  KhalaConversationLiveUpdateSchema as CanonicalKhalaConversationLiveUpdateSchema,
  type ConfirmedAgentRun,
  type ConfirmedAgentTimelineEvent,
  type ConfirmedChatMessage,
  type ConfirmedChatThread,
  type KhalaSyncConversationStatus,
  type KhalaConversationLiveUpdate,
  type ConfirmedRuntimeInteraction,
} from "@openagentsinc/khala-sync-client"
import {
  RuntimeInteractionDecisionEnvelope as CanonicalRuntimeInteractionDecisionEnvelopeSchema,
  RuntimeInteractionProjection as CanonicalRuntimeInteractionProjectionSchema,
  type RuntimeInteractionDecisionEnvelope,
} from "@openagentsinc/khala-sync"
import { CodexHistoryCatalogSchema, CodexHistoryPageSchema, CodexHistorySearchResponseSchema } from "./codex-history-contract.ts"
import { DesktopOperationContextSchema } from "./desktop-operation-context.ts"

export type { ConfirmedRuntimeInteraction } from "@openagentsinc/khala-sync-client"
export type { RuntimeInteractionDecisionEnvelope } from "@openagentsinc/khala-sync"

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
const KhalaConversationLiveUpdateSchema = canonicalBoundary<KhalaConversationLiveUpdate>(CanonicalKhalaConversationLiveUpdateSchema)
const RuntimeInteractionDecisionEnvelopeSchema = canonicalBoundary<RuntimeInteractionDecisionEnvelope>(CanonicalRuntimeInteractionDecisionEnvelopeSchema)
const ConfirmedRuntimeInteractionSchema = Schema.declare<ConfirmedRuntimeInteraction>(
  (value): value is ConfirmedRuntimeInteraction => {
    try {
      CanonicalSchema.decodeUnknownSync(CanonicalRuntimeInteractionProjectionSchema)(value)
      if (typeof value !== "object" || value === null) return false
      const confirmed = value as Record<string, unknown>
      return Number.isInteger(confirmed.requestedSequence) &&
        Number(confirmed.requestedSequence) >= 0 &&
        typeof confirmed.requestedAt === "string" &&
        /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/.test(confirmed.requestedAt) &&
        Number.isInteger(confirmed.version) &&
        Number(confirmed.version) >= 0
    } catch {
      return false
    }
  },
)

export const DesktopRuntimeGatewayInvokeChannel = "openagents-desktop/runtime-gateway/invoke" as const
export const DesktopRuntimeGatewayEventChannel = "openagents-desktop/runtime-gateway/event" as const
export const DesktopRuntimeGatewayProtocolVersion = 12 as const

// Typed per-harness maintenance (MAINT-1, #8785). The renderer projection is
// public-safe by construction: versions, channel, and advisory only — never
// binary paths, home paths, or command output.
export const DesktopMaintenanceHarnessSchema = Schema.Literals(["codex", "claude_code", "opencode"])
export type DesktopMaintenanceHarness = typeof DesktopMaintenanceHarnessSchema.Type
export const DesktopHarnessMaintenanceEntrySchema = Schema.Struct({
  harness: DesktopMaintenanceHarnessSchema,
  installed: Schema.Boolean,
  installedVersion: Schema.NullOr(Schema.String),
  latestVersion: Schema.NullOr(Schema.String),
  channel: Schema.Literals(["npm-global", "bun-global", "pnpm-global", "homebrew", "native", "unknown"]),
  advisory: Schema.Literals(["current", "behind_latest", "unknown"]),
  updateSupported: Schema.Boolean,
})
export type DesktopHarnessMaintenanceEntry = typeof DesktopHarnessMaintenanceEntrySchema.Type
export const DesktopCodexReleaseNotesSchema = Schema.Struct({
  version: Schema.String,
  title: Schema.String,
  body: Schema.String,
  publishedAt: Schema.NullOr(Schema.String),
})
export type DesktopCodexReleaseNotes = typeof DesktopCodexReleaseNotesSchema.Type
export const DesktopHarnessMaintenanceOutcomeSchema = Schema.Literals([
  "updated",
  "already_current",
  "channel_jump_refused",
  "failed",
])

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
const OperationContextField = { context: Schema.optional(DesktopOperationContextSchema) }
export const DesktopVoiceStateSchema = Schema.Struct({
  protocolVersion: Schema.Literal(1),
  phase: Schema.Literals(["idle", "requesting_permission", "connecting", "live", "muted", "suspended", "denied", "offline", "backpressured", "device_changed", "revoked", "failed"]),
  generation: NonNegativeIntSchema, nextSequence: NonNegativeIntSchema, acknowledgedSequence: NonNegativeIntSchema,
  capture: Schema.Boolean, egress: Schema.Boolean, playback: Schema.Boolean,
  retainedAudio: Schema.Boolean,
  activity: Schema.Literals(["stopped", "permission", "connecting", "listening", "speech_detected", "transcribing", "awaiting_confirmation", "executing", "speaking", "muted", "reconnecting", "degraded", "revoked"]),
  transcript: Schema.optional(Schema.Struct({ utteranceRef: PublicRefSchema, text: Schema.String.check(Schema.isMaxLength(16_384)), final: Schema.Boolean })),
  proposal: Schema.optional(Schema.Struct({ proposalRef: PublicRefSchema, utteranceRef: PublicRefSchema, turnRef: PublicRefSchema, targetRef: PublicRefSchema, commandId: PublicRefSchema, expiresAtMs: NonNegativeIntSchema, state: Schema.Literals(["proposed", "applied", "refused"]) })),
  playbackOutcomeRef: Schema.optional(PublicRefSchema),
  reason: Schema.optional(Schema.Literals(["permission_denied", "network_lost", "gateway_revoked", "helper_crashed", "stale_generation", "backpressure", "device_changed"])),
})

/**
 * Exact confirmed-run lanes a Desktop control intent may target (CUT-16).
 * These mirror the mobile confirmed-runtime derivation; the durable authority
 * rejects a control intent whose lane mismatches the stored turn lane.
 */
export const DesktopRuntimeControlLaneSchema = Schema.Literals([
  "codex_app_server",
  "claude_pylon",
  "hosted_khala",
])
export type DesktopRuntimeControlLane = typeof DesktopRuntimeControlLaneSchema.Type

export const DesktopRuntimeCapabilityIdSchema = Schema.Literals([
  "agent-timeline",
  "agent-graph",
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
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("runtime.bootstrap") }),
  }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("query"), requestId: Schema.String, query: Schema.Struct({ id: Schema.Literal("voice.state") }) }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("conversation.catalog") }),
  }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("query"), requestId: Schema.String, query: Schema.Struct({ id: Schema.Literal("codex.history.catalog") }) }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("query"), requestId: Schema.String, query: Schema.Struct({ id: Schema.Literal("maintenance.harness_status"), harness: Schema.optional(DesktopMaintenanceHarnessSchema) }) }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"), requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("codex.history.page"), threadRef: PublicRefSchema, offset: NonNegativeIntSchema, limit: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 500 })) }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"), requestId: Schema.String,
    query: Schema.Struct({ id: Schema.Literal("codex.history.search"), query: Schema.String.check(Schema.isMaxLength(200)), limit: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 100 })) }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("conversation.thread"),
      threadRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("runtime.interactions"),
      threadRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("conversation.commandOutcome"),
      intentId: PublicRefSchema,
      threadRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("agent.timeline"),
      runRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query"),
    requestId: Schema.String,
    query: Schema.Struct({
      id: Schema.Literal("conversation.timeline"),
      threadRef: PublicRefSchema,
    }),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("command"),
    commandId: Schema.String,
    command: Schema.Union([
      Schema.Struct({
        id: Schema.Literal("conversation.interrupt"),
        commandRef: PublicRefSchema,
        threadRef: PublicRefSchema,
        runRef: PublicRefSchema,
        // Additive (CUT-16): the durable lane fence rejects control intents
        // whose target lane mismatches the stored turn lane, so callers pass
        // the exact confirmed run lane instead of relying on the host default.
        lane: Schema.optional(DesktopRuntimeControlLaneSchema),
        expectedVersion: Schema.optional(NonNegativeIntSchema),
      }),
      Schema.Struct({
        id: Schema.Literals(["conversation.continue", "conversation.retry", "conversation.close"]),
        commandRef: PublicRefSchema,
        threadRef: PublicRefSchema,
        runRef: PublicRefSchema,
        lane: Schema.optional(DesktopRuntimeControlLaneSchema),
        expectedVersion: NonNegativeIntSchema,
      }),
      Schema.Struct({
        id: Schema.Literal("conversation.start"),
        threadRef: PublicRefSchema,
        messageRef: PublicRefSchema,
        runRef: PublicRefSchema,
        lane: Schema.optional(Schema.Literals(["codex_app_server", "claude_pylon"])),
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
      Schema.Struct({
        id: Schema.Literal("runtime.decideInteraction"),
        interactionRef: PublicRefSchema,
        threadRef: PublicRefSchema,
        turnRef: PublicRefSchema,
        envelope: RuntimeInteractionDecisionEnvelopeSchema,
      }),
      Schema.Struct({ id: Schema.Literal("session.sign_in") }),
      Schema.Struct({ id: Schema.Literal("session.sign_out") }),
      Schema.Struct({
        id: Schema.Literal("maintenance.harness_update"),
        harness: DesktopMaintenanceHarnessSchema,
      }),
      Schema.Struct({ id: Schema.Literal("voice.start"), protocolVersion: Schema.Literal(1), threadRef: PublicRefSchema, sessionRef: PublicRefSchema, disclosureRef: PublicRefSchema }),
      Schema.Struct({ id: Schema.Literals(["voice.stop", "voice.mute", "voice.unmute", "voice.suspend", "voice.resume", "voice.revoke"]), protocolVersion: Schema.Literal(1) }),
      Schema.Struct({
        id: Schema.Literal("conversation.subscribe"),
        subscriptionRef: PublicRefSchema,
        generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
        threadRef: PublicRefSchema,
        afterCursor: Schema.optional(Schema.NullOr(NonNegativeIntSchema)),
      }),
      Schema.Struct({
        id: Schema.Literal("conversation.unsubscribe"),
        subscriptionRef: PublicRefSchema,
        generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
      }),
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
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("voice_state"), requestId: Schema.optional(Schema.String), commandId: Schema.optional(Schema.String), state: DesktopVoiceStateSchema }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("query_result"),
    requestId: Schema.String,
    result: DesktopRuntimeBootstrapSchema,
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("conversation_catalog"),
    requestId: Schema.String,
    status: KhalaSyncConversationStatusSchema,
    threads: Schema.Array(ConfirmedChatThreadSchema),
  }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("codex_history_catalog"), requestId: Schema.String, catalog: CodexHistoryCatalogSchema }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("codex_history_page"), requestId: Schema.String, page: CodexHistoryPageSchema }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("codex_history_search"), requestId: Schema.String, search: CodexHistorySearchResponseSchema }),
  Schema.Struct({ ...OperationContextField, kind: Schema.Literal("codex_history_unavailable"), requestId: Schema.String, reason: Schema.Literals(["not_found", "read_failed"]) }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("conversation_thread"),
    requestId: Schema.String,
    threadRef: PublicRefSchema,
    status: KhalaSyncConversationStatusSchema,
    messages: Schema.Array(ConfirmedChatMessageSchema),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("conversation_unavailable"),
    requestId: Schema.String,
    reason: Schema.Literals(["not_live", "not_found", "read_failed"]),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("agent_timeline"),
    requestId: Schema.String,
    runRef: PublicRefSchema,
    status: KhalaSyncConversationStatusSchema,
    run: ConfirmedAgentRunSchema,
    events: Schema.Array(ConfirmedAgentTimelineEventSchema).check(Schema.isMaxLength(500)),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("runtime_interactions"),
    requestId: Schema.String,
    threadRef: PublicRefSchema,
    interactions: Schema.Array(ConfirmedRuntimeInteractionSchema).check(
      Schema.isMaxLength(100),
    ),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("runtime_interactions_unavailable"),
    requestId: Schema.String,
    reason: Schema.Literals(["not_live", "read_failed"]),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("runtime_interaction_decision_outcome"),
    commandId: Schema.String,
    interactionRef: PublicRefSchema,
    threadRef: PublicRefSchema,
    turnRef: PublicRefSchema,
    status: Schema.Literals(["pending_reconcile", "unavailable"]),
    mutationId: Schema.optional(NonNegativeIntSchema),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("conversation_timeline"),
    requestId: Schema.String,
    threadRef: PublicRefSchema,
    status: KhalaSyncConversationStatusSchema,
    run: Schema.NullOr(ConfirmedAgentRunSchema),
    events: Schema.Array(ConfirmedAgentTimelineEventSchema).check(Schema.isMaxLength(500)),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("agent_timeline_unavailable"),
    requestId: Schema.String,
    reason: Schema.Literals(["not_live", "not_found", "read_failed"]),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("runtime_command_status"),
    requestId: Schema.String,
    commandRef: PublicRefSchema,
    threadRef: PublicRefSchema,
    runRef: Schema.NullOr(PublicRefSchema),
    status: Schema.Literals([
      "pending",
      "accepted",
      "settled",
      "expired",
      "failed",
      "canceled",
    ]),
    mutationId: Schema.NullOr(NonNegativeIntSchema),
    version: Schema.NullOr(NonNegativeIntSchema),
    updatedAt: Schema.NullOr(Schema.String),
  }),
  Schema.Struct({
    ...OperationContextField,
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
    ...OperationContextField,
    kind: Schema.Literal("conversation_mutation_outcome"),
    commandId: Schema.String,
    status: Schema.Literals(["pending_reconcile", "unavailable"]),
    mutationId: Schema.optional(Schema.Number),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("command_outcome"),
    commandId: Schema.String,
    status: Schema.Literal("unavailable"),
    reason: Schema.String,
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("session_outcome"),
    commandId: Schema.String,
    status: Schema.Literals(["completed", "cancelled", "unavailable"]),
    phase: Schema.Literals(["session_ready", "signed_out", "unavailable"]),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("harness_maintenance_status"),
    requestId: Schema.String,
    observedAt: Schema.String,
    harnesses: Schema.Array(DesktopHarnessMaintenanceEntrySchema),
    codexReleaseNotes: Schema.NullOr(DesktopCodexReleaseNotesSchema),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("harness_maintenance_outcome"),
    commandId: Schema.String,
    harness: DesktopMaintenanceHarnessSchema,
    status: Schema.Literals(["completed", "unavailable"]),
    outcome: Schema.NullOr(DesktopHarnessMaintenanceOutcomeSchema),
    failureReason: Schema.NullOr(Schema.String),
    beforeVersion: Schema.NullOr(Schema.String),
    afterVersion: Schema.NullOr(Schema.String),
    receiptId: Schema.NullOr(Schema.String),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("conversation_subscription_outcome"),
    commandId: Schema.String,
    subscriptionRef: PublicRefSchema,
    generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
    status: Schema.Literals([
      "subscribed",
      "already_subscribed",
      "unsubscribed",
      "not_found",
      "stale_generation",
      "capacity_exceeded",
      "unavailable",
    ]),
    activeGeneration: Schema.optional(Schema.Number.check(
      Schema.isInt(),
      Schema.isGreaterThan(0),
    )),
  }),
  Schema.Struct({
    ...OperationContextField,
    kind: Schema.Literal("request_rejected"),
    reason: Schema.Literals(["invalid_request", "untrusted_renderer", "gateway_disposed"]),
  }),
])
export type DesktopRuntimeGatewayResponse = typeof DesktopRuntimeGatewayResponseSchema.Type

export const DesktopRuntimeGatewayEventSchema = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("voice.lifecycle"), sequence: Schema.Number, state: DesktopVoiceStateSchema }),
  Schema.Struct({
    kind: Schema.Literal("runtime.lifecycle"),
    protocolVersion: Schema.Literal(DesktopRuntimeGatewayProtocolVersion),
    sequence: Schema.Number,
    phase: Schema.Literals(["ready", "disposed"]),
  }),
  KhalaConversationLiveUpdateSchema,
])
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
