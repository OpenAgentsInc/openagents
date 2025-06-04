import { Context, Effect, Layer, Stream, Chunk, Fiber } from "effect"
import { CommandExecutor, Command } from "@effect/platform"
import { Schema } from "@effect/schema"
import { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"
import {
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
  ) => Effect.Effect<ClaudeCodeJsonResponse | ClaudeCodeTextResponse, ClaudeCodeExecutionError | ClaudeCodeParseError, never>

  /**
   * Continue a conversation with a session ID
   */
  readonly continueSession: (
    sessionId: string,
    prompt: string,
    options?: PromptOptions
  ) => Effect.Effect<ClaudeCodeJsonResponse | ClaudeCodeTextResponse, ClaudeCodeExecutionError | ClaudeCodeParseError | ClaudeCodeSessionError, never>

  /**
   * Resume the most recent conversation
   */
  readonly continueRecent: (
    prompt: string,
    options?: PromptOptions
  ) => Effect.Effect<ClaudeCodeJsonResponse | ClaudeCodeTextResponse, ClaudeCodeExecutionError | ClaudeCodeParseError, never>

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
  readonly checkAvailability: () => Effect.Effect<boolean, ClaudeCodeNotFoundError, never>
}

/**
 * Claude Code client service tag
 * @since 1.0.0
 */
export const ClaudeCodeClient = Context.GenericTag<ClaudeCodeClient>("ai/ClaudeCodeClient")

/**
 * Build command arguments for Claude CLI
 */
const buildArgs = (
  config: ClaudeCodeConfig,
  prompt: string,
  options?: PromptOptions,
  sessionId?: string
): Array<string> => {
  const args: Array<string> = []

  // Non-interactive mode
  args.push("--print", prompt)

  // Model selection
  const model = options?.model ?? config.model
  if (model) {
    args.push("--model", model)
  }

  // System prompt
  const systemPrompt = options?.systemPrompt ?? config.systemPrompt
  if (systemPrompt) {
    args.push("--system-prompt", systemPrompt)
  }

  // Append system prompt
  const appendSystemPrompt = options?.appendSystemPrompt ?? config.appendSystemPrompt
  if (appendSystemPrompt) {
    args.push("--append-system-prompt", appendSystemPrompt)
  }

  // Output format
  const outputFormat = options?.outputFormat ?? config.outputFormat
  if (outputFormat && outputFormat !== "text") {
    args.push("--output-format", outputFormat)
  }

  // Session management
  if (sessionId) {
    args.push("--resume", sessionId)
  }

  // Allowed tools
  const allowedTools = options?.allowedTools ?? config.allowedTools
  if (allowedTools && allowedTools.length > 0) {
    args.push("--allowedTools", allowedTools.join(","))
  }

  return args
}

/**
 * Parse Claude Code output based on format
 */
const parseOutput = (
  output: string,
  format: string
): Effect.Effect<ClaudeCodeJsonResponse | ClaudeCodeTextResponse, ClaudeCodeParseError> => {
  if (format === "text") {
    // For text format, we just return the content
    // Session ID would need to be extracted from a different mechanism
    return Effect.succeed({
      content: output.trim()
    })
  }

  // Parse JSON output
  return Effect.try({
    try: () => {
      const lines = output.trim().split('\n')
      // Handle streaming JSON by taking the last complete JSON object
      for (let i = lines.length - 1; i >= 0; i--) {
        try {
          const parsed = JSON.parse(lines[i])
          return Schema.decodeUnknownSync(ClaudeCodeJsonResponse)(parsed)
        } catch {
          // Continue to next line
        }
      }
      throw new Error("No valid JSON found in output")
    },
    catch: (error) => new ClaudeCodeParseError({
      output,
      format,
      cause: error
    })
  })
}

/**
 * Claude Code client implementation
 * @since 1.0.0
 */
export const ClaudeCodeClientLive = Layer.effect(
  ClaudeCodeClient,
  Effect.gen(function* (_) {
    const config = yield* _(ClaudeCodeConfig)
    const executor = yield* _(CommandExecutor)

    const executeCommand = (
      args: Array<string>,
      timeout?: number
    ) =>
      Effect.gen(function* () {
        const command = Command.make(config.cliPath ?? "claude", ...args)
        const process = yield* executor.start(command)

        // Set up timeout
        const timeoutMs = timeout ?? config.defaultTimeout ?? 60000
        const exitCodeFiber = yield* Effect.fork(
          process.exitCode.pipe(
            Effect.timeout(timeoutMs),
            Effect.orElse(() => Effect.succeed(1))
          )
        )

        // Collect output
        const output = yield* process.stdout.pipe(
          Stream.decodeText(),
          Stream.runCollect,
          Effect.map((chunks: Chunk.Chunk<string>) => Chunk.toReadonlyArray(chunks).join(""))
        )

        const exitCode = yield* Fiber.join(exitCodeFiber)

        if (exitCode !== 0) {
          const stderr = yield* process.stderr.pipe(
            Stream.decodeText(),
            Stream.runCollect,
            Effect.map((chunks: Chunk.Chunk<string>) => Chunk.toReadonlyArray(chunks).join(""))
          )

          return yield* Effect.fail(new ClaudeCodeExecutionError({
            command: `${config.cliPath} ${args.join(" ")}`,
            exitCode,
            stderr
          }))
        }

        return output
      }).pipe(
        Effect.mapError(error =>
          error._tag === "ClaudeCodeExecutionError"
            ? error
            : new ClaudeCodeExecutionError({
                command: `${config.cliPath} ${args.join(" ")}`,
                exitCode: -1,
                stderr: String(error)
              })
        )
      )

    const checkAvailability = () =>
      Effect.gen(function* () {
        const command = Command.make(config.cliPath ?? "claude", "--version")
        const process = yield* executor.start(command).pipe(
          Effect.catchAll((error) => 
            Effect.fail(new ClaudeCodeNotFoundError({
              message: `Claude CLI not found at: ${config.cliPath}`,
              cause: error
            }))
          )
        )
        const exitCode = yield* process.exitCode
        return exitCode === 0
      })

    const prompt = (text: string, options?: PromptOptions) =>
      Effect.gen(function* () {
        const args = buildArgs(config, text, options)
        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      })

    const continueSession = (sessionId: string, text: string, options?: PromptOptions) =>
      Effect.gen(function* () {
        const args = buildArgs(config, text, options, sessionId)
        const output = yield* executeCommand(args, options?.timeout).pipe(
          Effect.catchIf(
            (error): error is ClaudeCodeExecutionError => 
              error._tag === "ClaudeCodeExecutionError" && 
              (error.stderr.includes("session") || error.stderr.includes("resume")),
            () => Effect.fail(new ClaudeCodeSessionError({
              sessionId,
              message: "Invalid or expired session"
            }))
          )
        )
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      })

    const continueRecent = (text: string, options?: PromptOptions) =>
      Effect.gen(function* () {
        const args = ["--continue", ...buildArgs(config, text, options).slice(2)] // Skip --print and prompt
        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      })

    const streamPrompt = (text: string, options?: PromptOptions): Stream.Stream<string, ClaudeCodeExecutionError, never> =>
      Stream.unwrapScoped(
        Effect.gen(function* () {
          const args = buildArgs(config, text, { ...options, outputFormat: "json_stream" })
          const command = Command.make(config.cliPath ?? "claude", ...args)
          const process = yield* executor.start(command).pipe(
            Effect.mapError(error => new ClaudeCodeExecutionError({
              command: `${config.cliPath} ${args.join(" ")}`,
              exitCode: -1,
              stderr: `Failed to start process: ${error}`
            }))
          )

          return process.stdout.pipe(
            Stream.decodeText(),
            Stream.splitLines,
            Stream.filter(line => line.trim().length > 0),
            Stream.mapEffect((line: string) =>
              Effect.try(() => {
                const parsed = JSON.parse(line)
                return parsed.content || ""
              }).pipe(
                Effect.mapError(() => new ClaudeCodeExecutionError({
                  command: `${config.cliPath} ${args.join(" ")}`,
                  exitCode: -1,
                  stderr: `Failed to parse streaming output: ${line}`
                }))
              )
            )
          )
        })
      )

    return {
      prompt,
      continueSession,
      continueRecent,
      streamPrompt,
      checkAvailability
    } satisfies ClaudeCodeClient
  })
)