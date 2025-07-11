import type { CommandExecutor } from "@effect/platform"
import type { Layer } from "effect"
import type { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"
import type { ClaudeCodeClient, ClaudeCodeClient as ClaudeCodeClientType } from "./ClaudeCodeClient.js"

export { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"
export {
  ClaudeCodeExecutionError,
  ClaudeCodeNotFoundError,
  ClaudeCodeParseError,
  ClaudeCodeSessionError
} from "../errors/index.js"
export type { ClaudeCodeJsonResponse, ClaudeCodeTextResponse, PromptOptions } from "./ClaudeCodeClient.js"
export { ClaudeCodeClient } from "./ClaudeCodeClient.js"
/**
 * Simple implementation of Claude Code client
 * @since 1.0.0
 */
export declare const makeClaudeCodeClient: (
  config: ClaudeCodeConfig,
  executor: CommandExecutor.CommandExecutor
) => ClaudeCodeClientType
/**
 * Claude Code client layer (simplified)
 * @since 1.0.0
 */
export declare const ClaudeCodeClientLive: Layer.Layer<
  ClaudeCodeClient,
  never,
  {
    readonly model?: string | undefined
    readonly systemPrompt?: string | undefined
    readonly appendSystemPrompt?: string | undefined
    readonly outputFormat?: "text" | "json" | "json_stream" | undefined
    readonly cliPath?: string | undefined
    readonly allowedTools?: ReadonlyArray<string> | undefined
    readonly defaultTimeout?: number | undefined
  } | CommandExecutor.CommandExecutor
>
// # sourceMappingURL=ClaudeCodeSimple.d.ts.map
