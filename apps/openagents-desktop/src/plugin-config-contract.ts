import { Exit, Schema } from "@effect-native/core/effect"

import { decode } from "./chat-contract.ts"

export const PluginConfigListChannel = "openagents-desktop/plugin-config-list" as const
export const PluginConfigChooseChannel = "openagents-desktop/plugin-config-choose" as const
export const PluginConfigToggleChannel = "openagents-desktop/plugin-config-toggle" as const
export const PluginConfigRemoveChannel = "openagents-desktop/plugin-config-remove" as const

export const PluginRefSchema = Schema.String.check(
  Schema.isPattern(/^plugin\.local\.[a-f0-9]{24}$/),
)
export type PluginRef = typeof PluginRefSchema.Type
export const LocalSkillInvocationSchema = Schema.Struct({
  pluginRef: PluginRefSchema,
  name: Schema.String.check(Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/)),
})
export type LocalSkillInvocation = typeof LocalSkillInvocationSchema.Type

export const PluginConfigViewSchema = Schema.Struct({
  ref: PluginRefSchema,
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  provider: Schema.Literal("claude_agent"),
  provenance: Schema.Literal("user_local"),
  scope: Schema.Literal("app"),
  readiness: Schema.Literals(["ready", "missing", "invalid"]),
  enabled: Schema.Boolean,
  restartRequired: Schema.Literal(false),
  perSessionUse: Schema.Literal("next_turn"),
  capabilities: Schema.Array(Schema.Literals(["commands", "agents", "skills", "hooks", "mcp"])),
  skills: Schema.Array(Schema.String.check(Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/))),
})
export type PluginConfigView = typeof PluginConfigViewSchema.Type

export const PluginConfigResultSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("ok"), plugins: Schema.Array(PluginConfigViewSchema), dropped: Schema.Number }),
  Schema.Struct({ state: Schema.Literal("cancelled") }),
  Schema.Struct({ state: Schema.Literal("rejected"), reason: Schema.String }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String }),
])
export type PluginConfigResult = typeof PluginConfigResultSchema.Type

export const PluginRefRequestSchema = Schema.Struct({ ref: PluginRefSchema })
export const PluginToggleRequestSchema = Schema.Struct({ ref: PluginRefSchema, enabled: Schema.Boolean })
export type PluginRefRequest = typeof PluginRefRequestSchema.Type
export type PluginToggleRequest = typeof PluginToggleRequestSchema.Type

export const decodePluginRefRequest = (value: unknown): PluginRefRequest | null =>
  decode(PluginRefRequestSchema, value) as PluginRefRequest | null
export const decodePluginToggleRequest = (value: unknown): PluginToggleRequest | null =>
  decode(PluginToggleRequestSchema, value) as PluginToggleRequest | null

export const decodePluginConfigResult = (value: unknown): PluginConfigResult => {
  const result = Schema.decodeUnknownExit(PluginConfigResultSchema)(value)
  return Exit.isSuccess(result)
    ? result.value
    : { state: "unavailable", message: "The local plugin registry is unavailable." }
}
