import { Context, Layer, Effect } from "effect"
import * as Schema from "@effect/schema/Schema"

/**
 * Output format for Claude Code responses
 * @since 1.0.0
 */
export type OutputFormat = "text" | "json" | "json_stream"

/**
 * Claude Code configuration schema
 * @since 1.0.0
 */
export const ClaudeCodeConfigSchema = Schema.Struct({
  /** Model to use (defaults to claude-3-opus-20240229) */
  model: Schema.optional(Schema.String),
  
  /** Custom system prompt to prepend */
  systemPrompt: Schema.optional(Schema.String),
  
  /** Additional system prompt to append */
  appendSystemPrompt: Schema.optional(Schema.String),
  
  /** Output format preference */
  outputFormat: Schema.optional(Schema.Literal("text", "json", "json_stream")),
  
  /** Path to claude CLI executable (defaults to 'claude' in PATH) */
  cliPath: Schema.optional(Schema.String),
  
  /** Allowed MCP tools (empty array means all allowed) */
  allowedTools: Schema.optional(Schema.Array(Schema.String)),
  
  /** Default timeout for commands in milliseconds */
  defaultTimeout: Schema.optional(Schema.Number)
})

/**
 * Claude Code configuration type
 * @since 1.0.0
 */
export interface ClaudeCodeConfig extends Schema.Schema.Type<typeof ClaudeCodeConfigSchema> {}

/**
 * Claude Code configuration service
 * @since 1.0.0
 */
export const ClaudeCodeConfig = Context.GenericTag<ClaudeCodeConfig>("ai/ClaudeCodeConfig")

/**
 * Default Claude Code configuration
 * @since 1.0.0
 */
export const ClaudeCodeConfigDefault = Layer.succeed(
  ClaudeCodeConfig,
  {
    model: "claude-3-opus-20240229",
    outputFormat: "json",
    cliPath: "claude",
    allowedTools: [],
    defaultTimeout: 60000 // 60 seconds
  }
)

/**
 * Create a Claude Code configuration layer
 * @since 1.0.0
 */
export const makeClaudeCodeConfig = (config: Partial<ClaudeCodeConfig>) =>
  Layer.succeed(
    ClaudeCodeConfig,
    {
      model: config.model ?? "claude-3-opus-20240229",
      outputFormat: config.outputFormat ?? "json",
      cliPath: config.cliPath ?? "claude",
      allowedTools: config.allowedTools ?? [],
      defaultTimeout: config.defaultTimeout ?? 60000,
      systemPrompt: config.systemPrompt,
      appendSystemPrompt: config.appendSystemPrompt
    }
  )