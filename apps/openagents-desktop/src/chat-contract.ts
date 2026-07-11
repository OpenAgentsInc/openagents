import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopThreadsChannel = "openagents-desktop/threads" as const
export const DesktopNewThreadChannel = "openagents-desktop/thread-new" as const
export const DesktopOpenThreadChannel = "openagents-desktop/thread-open" as const
export const DesktopHydrateThreadChannel = "openagents-desktop/thread-hydrate" as const
export const DesktopChatTurnChannel = "openagents-desktop/chat-turn" as const

/**
 * Per-message host metadata (#8712, EP250: "if I click on the message, I see
 * the metadata of the message in the right sidebar"). Additive and optional:
 * only facts the host actually observed are recorded — the fable-local lane
 * stamps the SDK-reported effective model, the lane name, the account ref
 * used, the turn ref, the exact reported token total, and the wall-clock
 * duration. Bounded public-safe strings only; never prompts, paths, tokens,
 * or provider payloads.
 */
export const DesktopMessageMetaSchema = Schema.Struct({
  lane: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  model: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  accountRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  turnRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  requestId: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  totalTokens: Schema.optional(Schema.NullOr(Schema.Number)),
  durationMs: Schema.optional(Schema.Number),
})
export type DesktopMessageMeta = typeof DesktopMessageMetaSchema.Type

export const DesktopMessageSchema = Schema.Struct({
  key: Schema.String,
  role: Schema.Literals(["user", "assistant", "system"]),
  text: Schema.String,
  timestamp: Schema.String,
  meta: Schema.optional(DesktopMessageMetaSchema),
})
export type DesktopMessage = typeof DesktopMessageSchema.Type

export const DesktopThreadSchema = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  createdAt: Schema.optional(Schema.String),
  updatedAt: Schema.String,
  cwd: Schema.optional(Schema.String),
  model: Schema.optional(Schema.String),
  notes: Schema.Array(DesktopMessageSchema),
})
export type DesktopThread = typeof DesktopThreadSchema.Type

export const DesktopThreadRequestSchema = Schema.Struct({ id: Schema.String })
export const DesktopTurnRequestSchema = Schema.Struct({ id: Schema.String, message: Schema.String })
export type DesktopTurnRequest = typeof DesktopTurnRequestSchema.Type

export const decode = (schema: any, value: unknown): unknown | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
