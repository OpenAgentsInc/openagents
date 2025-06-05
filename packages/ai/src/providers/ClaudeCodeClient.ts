import { Schema } from "@effect/schema"
import type { Effect, Stream } from "effect"
import { Context } from "effect"
import type {
  ClaudeCodeExecutionError,
  ClaudeCodeNotFoundError,
  ClaudeCodeParseError,
  ClaudeCodeSessionError
} from "../errors/index.js"

/**
 * Claude Code prompt options
 * @since 1.0.0
 */
export interface PromptOptions {
  /** Override the model for this prompt */
  readonly model?: string
  /** Override system prompt for this prompt */
  readonly systemPrompt?: string
  /** Append to system prompt for this prompt */
  readonly appendSystemPrompt?: string
  /** Override output format for this prompt */
  readonly outputFormat?: "text" | "json" | "json_stream"
  /** Specific tools to allow for this prompt */
  readonly allowedTools?: ReadonlyArray<string>
  /** Timeout in milliseconds */
  readonly timeout?: number
}

/**
 * Claude Code response for JSON format
 * @since 1.0.0
 */
export const ClaudeCodeJsonResponse = Schema.Struct({
  content: Schema.String,
  model: Schema.String,
  stop_reason: Schema.optional(Schema.String),
  session_id: Schema.optional(Schema.String),
  usage: Schema.optional(Schema.Struct({
    input_tokens: Schema.Number,
    output_tokens: Schema.Number,
    total_tokens: Schema.Number
  })),
  metadata: Schema.optional(Schema.Struct({
    cost_usd: Schema.optional(Schema.Number),
    duration_ms: Schema.optional(Schema.Number),
    num_turns: Schema.optional(Schema.Number)
  }))
})

export type ClaudeCodeJsonResponse = Schema.Schema.Type<typeof ClaudeCodeJsonResponse>

/**
 * Claude Code text response
 * @since 1.0.0
 */
export interface ClaudeCodeTextResponse {
  readonly content: string
  readonly sessionId?: string
}

/**
 * Claude Code client service interface
 * @since 1.0.0
 */
export interface ClaudeCodeClient {
  /**
   * Execute a single prompt
   */
  readonly prompt: (
    text: string,
    options?: PromptOptions
  ) => Effect.Effect<
    ClaudeCodeJsonResponse | ClaudeCodeTextResponse,
    ClaudeCodeExecutionError | ClaudeCodeParseError,
    never
  >

  /**
   * Continue a conversation with a session ID
   */
  readonly continueSession: (
    sessionId: string,
    prompt: string,
    options?: PromptOptions
  ) => Effect.Effect<
    ClaudeCodeJsonResponse | ClaudeCodeTextResponse,
    | ClaudeCodeExecutionError
    | ClaudeCodeParseError
    | ClaudeCodeSessionError,
    never
  >

  /**
   * Resume the most recent conversation
   */
  readonly continueRecent: (
    prompt: string,
    options?: PromptOptions
  ) => Effect.Effect<
    ClaudeCodeJsonResponse | ClaudeCodeTextResponse,
    ClaudeCodeExecutionError | ClaudeCodeParseError,
    never
  >

  /**
   * Stream a prompt response
   */
  readonly streamPrompt: (
    text: string,
    options?: PromptOptions
  ) => Stream.Stream<string, ClaudeCodeExecutionError, never>

  /**
   * Check if Claude CLI is available
   */
  readonly checkAvailability: () => Effect.Effect<
    boolean,
    ClaudeCodeNotFoundError,
    never
  >
}

/**
 * Claude Code client service tag
 * @since 1.0.0
 */
export const ClaudeCodeClient = Context.GenericTag<ClaudeCodeClient>("ai/ClaudeCodeClient")
