import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopRuntimeGatewayInvokeChannel = "openagents-desktop/runtime-gateway/invoke" as const
export const DesktopRuntimeGatewayEventChannel = "openagents-desktop/runtime-gateway/event" as const
export const DesktopRuntimeGatewayProtocolVersion = 1 as const

export const DesktopRuntimeCapabilityIdSchema = Schema.Literals([
  "codex-history",
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
    kind: Schema.Literal("command"),
    commandId: Schema.String,
    command: Schema.Union([
      Schema.Struct({
        id: Schema.Literal("conversation.interrupt"),
        threadRef: Schema.String,
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

export const DesktopRuntimeGatewayResponseSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("query_result"),
    requestId: Schema.String,
    result: DesktopRuntimeBootstrapSchema,
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
