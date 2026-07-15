import { Exit, Schema } from "@effect-native/core/effect"

export const CodexEcosystemSnapshotChannel = "openagents:codex-ecosystem:snapshot" as const
export const CodexEcosystemMutationChannel = "openagents:codex-ecosystem:mutate" as const

export const CodexEcosystemMutationRequestSchema = Schema.Union([
  Schema.Struct({ operation: Schema.Literal("skill_config"), id: Schema.String, enabled: Schema.Boolean }),
  Schema.Struct({ operation: Schema.Literal("marketplace_add"), source: Schema.String, refName: Schema.optional(Schema.String) }),
  Schema.Struct({ operation: Schema.Literal("marketplace_remove"), name: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("marketplace_upgrade"), name: Schema.NullOr(Schema.String) }),
  Schema.Struct({ operation: Schema.Literal("plugin_install"), pluginName: Schema.String, remoteMarketplaceName: Schema.optional(Schema.String) }),
  Schema.Struct({ operation: Schema.Literal("plugin_uninstall"), id: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("mcp_reload") }),
  Schema.Struct({ operation: Schema.Literal("mcp_oauth"), name: Schema.String, threadId: Schema.NullOr(Schema.String) }),
])
export type CodexEcosystemMutationRequest = typeof CodexEcosystemMutationRequestSchema.Type
export const decodeCodexEcosystemMutationRequest = (value: unknown): CodexEcosystemMutationRequest | null => {
  const decoded = Schema.decodeUnknownExit(CodexEcosystemMutationRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}
