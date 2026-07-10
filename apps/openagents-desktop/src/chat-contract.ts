import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopThreadsChannel = "openagents-desktop/threads" as const
export const DesktopNewThreadChannel = "openagents-desktop/thread-new" as const
export const DesktopOpenThreadChannel = "openagents-desktop/thread-open" as const
export const DesktopChatTurnChannel = "openagents-desktop/chat-turn" as const

export const DesktopMessageSchema = Schema.Struct({
  key: Schema.String,
  role: Schema.Literals(["user", "assistant", "system"]),
  text: Schema.String,
  timestamp: Schema.String,
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
