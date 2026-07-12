/**
 * User-configured MCP servers — IPC + projection contract (I2, EP250 wave-2).
 *
 * The FROZEN persisted server shape lives in `fable-local-contract.ts`
 * (`FableLocalMcpServerConfigSchema`); this module owns only the settings-UI
 * boundary around it: the additive host channels, the request decoders, and
 * the PUBLIC-SAFE projection the renderer is allowed to see.
 *
 * Security boundary law (mirrors the codex-connect contract):
 * - The full config (including `env` / `headers` / `args` VALUES, which are
 *   user-provided and may be sensitive) is persisted and read ONLY in the main
 *   process, and handed to the fable-local runtime through a main-side getter.
 * - The renderer projection carries identification only — name, transport,
 *   enabled, the stdio `command` / http `url`, and the COUNTS of args/env/
 *   headers — never their values. Secret values cross renderer -> main once
 *   (the user typed them into the Add form) and never travel back.
 * - Add carries one frozen `FableLocalMcpServerConfig`; remove/toggle carry
 *   only a schema-validated server name. Main re-validates everything against
 *   the frozen schema and the reserved/duplicate/transport rules before it
 *   writes, so a compromised renderer cannot persist an out-of-contract entry.
 */
import { Exit, Schema } from "@effect-native/core/effect"

import { decode } from "./chat-contract.ts"
import {
  FABLE_LOCAL_MCP_NAME_PATTERN,
  FABLE_LOCAL_MCP_SERVER_LIMIT,
  FableLocalMcpServerConfigSchema,
  type FableLocalMcpServerConfig,
} from "./fable-local-contract.ts"

export const McpConfigListChannel = "openagents-desktop/mcp-config-list" as const
export const McpConfigAddChannel = "openagents-desktop/mcp-config-add" as const
export const McpConfigRemoveChannel = "openagents-desktop/mcp-config-remove" as const
export const McpConfigToggleChannel = "openagents-desktop/mcp-config-toggle" as const

/** The name grammar, re-exported so the renderer validates before dispatch. */
export const mcpServerNamePattern = FABLE_LOCAL_MCP_NAME_PATTERN
export const mcpServerListCap = FABLE_LOCAL_MCP_SERVER_LIMIT
/** RESERVED for the internal delegate SDK-MCP server. */
export const mcpReservedServerName = "codex"

// ---------------------------------------------------------------------------
// Public-safe projection (main -> renderer). No secret values.
// ---------------------------------------------------------------------------

export const McpConfigServerViewSchema = Schema.Struct({
  name: Schema.String,
  transport: Schema.Literals(["stdio", "http"]),
  enabled: Schema.Boolean,
  /** stdio: the executable (identification only). */
  command: Schema.optional(Schema.String),
  /** http: the endpoint (identification only). */
  url: Schema.optional(Schema.String),
  /** Counts only — values never cross back to the renderer. */
  argsCount: Schema.Number,
  envCount: Schema.Number,
  headersCount: Schema.Number,
})
export type McpConfigServerView = typeof McpConfigServerViewSchema.Type

export const McpConfigListResultSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("ok"),
    servers: Schema.Array(McpConfigServerViewSchema),
    /** How many stored rows were dropped as schema-invalid on the last read. */
    dropped: Schema.Number,
  }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String }),
])
export type McpConfigListResult = typeof McpConfigListResultSchema.Type

export const McpConfigMutationResultSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("ok"),
    servers: Schema.Array(McpConfigServerViewSchema),
    dropped: Schema.Number,
  }),
  Schema.Struct({ state: Schema.Literal("rejected"), reason: Schema.String }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String }),
])
export type McpConfigMutationResult = typeof McpConfigMutationResultSchema.Type

// ---------------------------------------------------------------------------
// Requests (renderer -> main).
// ---------------------------------------------------------------------------

/** Add carries exactly one frozen server config. */
export const McpConfigAddRequestSchema = FableLocalMcpServerConfigSchema
export type McpConfigAddRequest = FableLocalMcpServerConfig

export const McpConfigNameRequestSchema = Schema.Struct({
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
})
export type McpConfigNameRequest = typeof McpConfigNameRequestSchema.Type

export const McpConfigToggleRequestSchema = Schema.Struct({
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  enabled: Schema.Boolean,
})
export type McpConfigToggleRequest = typeof McpConfigToggleRequestSchema.Type

export const decodeMcpConfigAddRequest = (value: unknown): McpConfigAddRequest | null =>
  decode(McpConfigAddRequestSchema, value) as McpConfigAddRequest | null

export const decodeMcpConfigNameRequest = (value: unknown): McpConfigNameRequest | null =>
  decode(McpConfigNameRequestSchema, value) as McpConfigNameRequest | null

export const decodeMcpConfigToggleRequest = (value: unknown): McpConfigToggleRequest | null =>
  decode(McpConfigToggleRequestSchema, value) as McpConfigToggleRequest | null

/** Project one full config to its public-safe renderer view (drops values). */
export const toMcpConfigServerView = (
  config: FableLocalMcpServerConfig,
): McpConfigServerView => ({
  name: config.name,
  transport: config.transport,
  enabled: config.enabled,
  ...(config.transport === "stdio" && config.command !== undefined
    ? { command: config.command }
    : {}),
  ...(config.transport === "http" && config.url !== undefined ? { url: config.url } : {}),
  argsCount: config.args?.length ?? 0,
  envCount: config.env === undefined ? 0 : Object.keys(config.env).length,
  headersCount: config.headers === undefined ? 0 : Object.keys(config.headers).length,
})

// ---------------------------------------------------------------------------
// Renderer-side decoders (defense in depth; every response is schema-checked
// before it touches state).
// ---------------------------------------------------------------------------

export const decodeMcpConfigListResult = (value: unknown): McpConfigListResult => {
  const decoded = Schema.decodeUnknownExit(McpConfigListResultSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { state: "unavailable", message: "The MCP server list is unavailable." }
  }
  if (decoded.value.state === "unavailable") {
    return { state: "unavailable", message: decoded.value.message.slice(0, 200) }
  }
  return {
    state: "ok",
    dropped: decoded.value.dropped,
    servers: decoded.value.servers.filter((server) => mcpServerNamePattern.test(server.name)),
  }
}

export const decodeMcpConfigMutationResult = (value: unknown): McpConfigMutationResult => {
  const decoded = Schema.decodeUnknownExit(McpConfigMutationResultSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { state: "unavailable", message: "The MCP server change is unavailable." }
  }
  if (decoded.value.state === "rejected") {
    return { state: "rejected", reason: decoded.value.reason.slice(0, 200) }
  }
  if (decoded.value.state === "unavailable") {
    return { state: "unavailable", message: decoded.value.message.slice(0, 200) }
  }
  return {
    state: "ok",
    dropped: decoded.value.dropped,
    servers: decoded.value.servers.filter((server) => mcpServerNamePattern.test(server.name)),
  }
}
