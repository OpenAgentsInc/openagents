import { Command, CommandExecutor } from "@effect/platform"
import { Schema } from "@effect/schema"
import { Effect, Layer, Stream } from "effect"
import { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"
import type { ClaudeCodeSessionError } from "../errors/index.js"
import { ClaudeCodeExecutionError, ClaudeCodeParseError } from "../errors/index.js"
import { ClaudeCodeClient, ClaudeCodeJsonResponse as ClaudeCodeJsonResponseSchema } from "./ClaudeCodeClient.js"
import type {
  ClaudeCodeClient as ClaudeCodeClientType,
  ClaudeCodeJsonResponse,
  ClaudeCodeTextResponse
} from "./ClaudeCodeClient.js"

// Re-export types
export type { ClaudeCodeJsonResponse, ClaudeCodeTextResponse, PromptOptions } from "./ClaudeCodeClient.js"

// Re-export the service tag
export { ClaudeCodeClient } from "./ClaudeCodeClient.js"

export {
  ClaudeCodeExecutionError,
  ClaudeCodeNotFoundError,
  ClaudeCodeParseError,
  ClaudeCodeSessionError
} from "../errors/index.js"

export { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"

/**
 * Simple implementation of Claude Code client
 * @since 1.0.0
 */
export const makeClaudeCodeClient = (
  config: ClaudeCodeConfig,
  executor: CommandExecutor.CommandExecutor
): ClaudeCodeClientType => {
  const executeCommand = (args: Array<string>, timeout?: number) =>
    Effect.gen(function*() {
      // Create command directly without shell wrapper
      const command = Command.make(config.cliPath ?? "claude", ...args).pipe(
        Command.env({
          CI: "true",
          TERM: "dumb",
          NO_COLOR: "1",
          NODE_NO_READLINE: "1"
        })
      )

      const result = yield* executor.start(command)
      const output = yield* result.stdout.pipe(
        Stream.decodeText(),
        Stream.runFold("", (acc, chunk) => acc + chunk)
      )

      const exitCode = yield* result.exitCode

      if (exitCode !== 0) {
        const stderr = yield* result.stderr.pipe(
          Stream.decodeText(),
          Stream.runFold("", (acc, chunk) => acc + chunk)
        )
        return yield* Effect.fail(
          new ClaudeCodeExecutionError({
            command: `${config.cliPath ?? "claude"} ${args.join(" ")}`,
            exitCode,
            stderr
          })
        )
      }

      return output
    }).pipe(
      // Add timeout if specified (default to 30 seconds)
      Effect.timeout(timeout ?? 30000),
      Effect.mapError((error) => {
        if (error._tag === "TimeoutException") {
          return new ClaudeCodeExecutionError({
            command: `${config.cliPath ?? "claude"} ${args.join(" ")}`,
            exitCode: -1,
            stderr: "Command timed out"
          })
        }
        return error
      })
    )

  const parseOutput = (
    output: string,
    format: string
  ): Effect.Effect<ClaudeCodeJsonResponse | ClaudeCodeTextResponse, ClaudeCodeParseError> => {
    if (format === "text") {
      return Effect.succeed({
        content: output.trim()
      })
    }

    return Effect.try({
      try: () => {
        const lines = output.trim().split("\n")
        for (let i = lines.length - 1; i >= 0; i--) {
          try {
            const parsed = JSON.parse(lines[i])
            return Schema.decodeUnknownSync(ClaudeCodeJsonResponseSchema)(parsed)
          } catch {
            // Continue
          }
        }
        throw new Error("No valid JSON found in output")
      },
      catch: (error) =>
        new ClaudeCodeParseError({
          output,
          format,
          cause: error
        })
    })
  }

  return {
    prompt: (text, options) =>
      Effect.gen(function*() {
        const args = ["--print", text]
        if (options?.outputFormat) args.push("--output-format", options.outputFormat)

        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        const parsed = yield* parseOutput(output, format)
        return parsed
      }) as Effect.Effect<
        ClaudeCodeJsonResponse | ClaudeCodeTextResponse,
        ClaudeCodeExecutionError | ClaudeCodeParseError,
        never
      >,

    continueSession: (sessionId, prompt, options) =>
      Effect.gen(function*() {
        const args = ["--resume", sessionId, "--print", prompt]
        if (options?.outputFormat) args.push("--output-format", options.outputFormat)

        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      }) as Effect.Effect<
        ClaudeCodeJsonResponse | ClaudeCodeTextResponse,
        ClaudeCodeExecutionError | ClaudeCodeParseError | ClaudeCodeSessionError,
        never
      >,

    continueRecent: (prompt, options) =>
      Effect.gen(function*() {
        const args = ["--continue", "--print", prompt]
        if (options?.outputFormat) args.push("--output-format", options.outputFormat)

        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      }) as Effect.Effect<
        ClaudeCodeJsonResponse | ClaudeCodeTextResponse,
        ClaudeCodeExecutionError | ClaudeCodeParseError,
        never
      >,

    streamPrompt: (text, _options) =>
      Stream.unwrapScoped(
        Effect.gen(function*() {
          const args = ["--print", text, "--output-format", "json_stream"]
          const command = Command.make(config.cliPath ?? "claude", ...args)
          const process = yield* Command.start(command).pipe(
            Effect.provideService(CommandExecutor.CommandExecutor, executor)
          )

          return process.stdout.pipe(
            Stream.decodeText(),
            Stream.splitLines,
            Stream.filter((line) => line.trim().length > 0),
            Stream.mapEffect((line) =>
              Effect.try(() => {
                const parsed = JSON.parse(line as string)
                return parsed.content || ""
              }).pipe(
                Effect.mapError(() =>
                  new ClaudeCodeExecutionError({
                    command: `${config.cliPath} ${args.join(" ")}`,
                    exitCode: -1,
                    stderr: `Failed to parse streaming output: ${line}`
                  })
                )
              )
            )
          )
        })
      ) as Stream.Stream<string, ClaudeCodeExecutionError, never>,

    checkAvailability: () =>
      Effect.gen(function*() {
        const command = Command.make(config.cliPath ?? "claude", "--version")
        const result = yield* executor.start(command)
        const exitCode = yield* result.exitCode
        return exitCode === 0
      }).pipe(
        Effect.catchAll(() => Effect.succeed(false))
      ) as Effect.Effect<boolean, never, never>
  }
}

/**
 * Claude Code client layer (simplified)
 * @since 1.0.0
 */
export const ClaudeCodeClientLive = Layer.effect(
  ClaudeCodeClient,
  Effect.gen(function*() {
    const config = yield* ClaudeCodeConfig
    const executor = yield* CommandExecutor.CommandExecutor
    return makeClaudeCodeClient(config, executor)
  })
)
