import { Command, CommandExecutor } from "@effect/platform"
import { Schema } from "@effect/schema"
import { Chunk, Effect, Layer, Stream } from "effect"

import { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"
import { ClaudeCodeExecutionError, ClaudeCodeNotFoundError, ClaudeCodeParseError } from "../errors/index.js"
import type { ClaudeCodeTextResponse } from "./ClaudeCodeClient.js"
import { ClaudeCodeClient, ClaudeCodeJsonResponse } from "./ClaudeCodeClient.js"

// Re-export types and config from the main client
export { ClaudeCodeClient, ClaudeCodeJsonResponse, ClaudeCodeTextResponse, PromptOptions } from "./ClaudeCodeClient.js"

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
  executor: CommandExecutor
): ClaudeCodeClient => {
  const executeCommand = (args: Array<string>, _timeout?: number) => {
    const command = Command.make(config.cliPath ?? "claude", ...args)

    return Effect.gen(function*() {
      const process = yield* executor.start(command)
      const exitCode = yield* process.exitCode

      if (exitCode !== 0) {
        const stderr = yield* process.stderr.pipe(
          Stream.decodeText(),
          Stream.runCollect,
          Effect.map((chunks: Chunk.Chunk<string>) => Chunk.toReadonlyArray(chunks).join(""))
        )

        return yield* Effect.fail(
          new ClaudeCodeExecutionError({
            command: `${config.cliPath} ${args.join(" ")}`,
            exitCode,
            stderr
          })
        )
      }

      const output = yield* process.stdout.pipe(
        Stream.decodeText(),
        Stream.runCollect,
        Effect.map((chunks: Chunk.Chunk<string>) => Chunk.toReadonlyArray(chunks).join(""))
      )

      return output
    }).pipe(
      Effect.mapError((error) =>
        error._tag === "ClaudeCodeExecutionError"
          ? error
          : new ClaudeCodeExecutionError({
            command: `${config.cliPath} ${args.join(" ")}`,
            exitCode: -1,
            stderr: String(error)
          })
      )
    )
  }

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
            return Schema.decodeUnknownSync(ClaudeCodeJsonResponse)(parsed)
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
        return yield* parseOutput(output, format)
      }),

    continueSession: (sessionId, prompt, options) =>
      Effect.gen(function*() {
        const args = ["--resume", sessionId, "--print", prompt]
        if (options?.outputFormat) args.push("--output-format", options.outputFormat)

        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      }),

    continueRecent: (prompt, options) =>
      Effect.gen(function*() {
        const args = ["--continue", "--print", prompt]
        if (options?.outputFormat) args.push("--output-format", options.outputFormat)

        const output = yield* executeCommand(args, options?.timeout)
        const format = options?.outputFormat ?? config.outputFormat ?? "text"
        return yield* parseOutput(output, format)
      }),

    streamPrompt: (text, _options) =>
      Stream.unwrapScoped(
        Effect.gen(function*() {
          const args = ["--print", text, "--output-format", "json_stream"]
          const command = Command.make(config.cliPath ?? "claude", ...args)
          const process = yield* executor.start(command)

          return process.stdout.pipe(
            Stream.decodeText(),
            Stream.splitLines,
            Stream.filter((line) => line.trim().length > 0),
            Stream.mapEffect((line: string) =>
              Effect.try(() => {
                const parsed = JSON.parse(line)
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
      ),

    checkAvailability: () =>
      Effect.gen(function*() {
        const command = Command.make(config.cliPath ?? "claude", "--version")
        const process = yield* executor.start(command).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new ClaudeCodeNotFoundError({
                message: `Claude CLI not found at: ${config.cliPath}`,
                cause: error
              })
            )
          )
        )
        const exitCode = yield* process.exitCode
        return exitCode === 0
      })
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
    const executor = yield* CommandExecutor
    return makeClaudeCodeClient(config, executor)
  })
)
