import { Schema } from "@effect/schema";
import { Context, Layer } from "effect";
/**
 * Output format for Claude Code responses
 * @since 1.0.0
 */
export type OutputFormat = "text" | "json" | "json_stream";
/**
 * Claude Code configuration schema
 * @since 1.0.0
 */
export declare const ClaudeCodeConfigSchema: Schema.Struct<{
    /** Model to use (defaults to claude-3-opus-20240229) */
    model: Schema.optional<typeof Schema.String>;
    /** Custom system prompt to prepend */
    systemPrompt: Schema.optional<typeof Schema.String>;
    /** Additional system prompt to append */
    appendSystemPrompt: Schema.optional<typeof Schema.String>;
    /** Output format preference */
    outputFormat: Schema.optional<Schema.Literal<["text", "json", "json_stream"]>>;
    /** Path to claude CLI executable (defaults to 'claude' in PATH) */
    cliPath: Schema.optional<typeof Schema.String>;
    /** Allowed MCP tools (empty array means all allowed) */
    allowedTools: Schema.optional<Schema.Array$<typeof Schema.String>>;
    /** Default timeout for commands in milliseconds */
    defaultTimeout: Schema.optional<typeof Schema.Number>;
}>;
/**
 * Claude Code configuration type
 * @since 1.0.0
 */
export type ClaudeCodeConfig = Schema.Schema.Type<typeof ClaudeCodeConfigSchema>;
/**
 * Claude Code configuration service
 * @since 1.0.0
 */
export declare const ClaudeCodeConfig: Context.Tag<{
    readonly model?: string | undefined;
    readonly systemPrompt?: string | undefined;
    readonly appendSystemPrompt?: string | undefined;
    readonly outputFormat?: "text" | "json" | "json_stream" | undefined;
    readonly cliPath?: string | undefined;
    readonly allowedTools?: readonly string[] | undefined;
    readonly defaultTimeout?: number | undefined;
}, {
    readonly model?: string | undefined;
    readonly systemPrompt?: string | undefined;
    readonly appendSystemPrompt?: string | undefined;
    readonly outputFormat?: "text" | "json" | "json_stream" | undefined;
    readonly cliPath?: string | undefined;
    readonly allowedTools?: readonly string[] | undefined;
    readonly defaultTimeout?: number | undefined;
}>;
/**
 * Default Claude Code configuration
 * @since 1.0.0
 */
export declare const ClaudeCodeConfigDefault: Layer.Layer<{
    readonly model?: string | undefined;
    readonly systemPrompt?: string | undefined;
    readonly appendSystemPrompt?: string | undefined;
    readonly outputFormat?: "text" | "json" | "json_stream" | undefined;
    readonly cliPath?: string | undefined;
    readonly allowedTools?: readonly string[] | undefined;
    readonly defaultTimeout?: number | undefined;
}, never, never>;
/**
 * Create a Claude Code configuration layer
 * @since 1.0.0
 */
export declare const makeClaudeCodeConfig: (config: Partial<ClaudeCodeConfig>) => Layer.Layer<{
    readonly model?: string | undefined;
    readonly systemPrompt?: string | undefined;
    readonly appendSystemPrompt?: string | undefined;
    readonly outputFormat?: "text" | "json" | "json_stream" | undefined;
    readonly cliPath?: string | undefined;
    readonly allowedTools?: readonly string[] | undefined;
    readonly defaultTimeout?: number | undefined;
}, never, never>;
//# sourceMappingURL=ClaudeCodeConfig.d.ts.map