import { Schema } from "@effect/schema"
import { Deferred, Effect, Layer, Queue, Stream } from "effect"
import * as pty from "node-pty"
import { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js"
import type { ClaudeCodeSessionError } from "../errors/index.js"
import { ClaudeCodeExecutionError, ClaudeCodeParseError } from "../errors/index.js"
import { ClaudeCodeClient, ClaudeCodeJsonResponse as ClaudeCodeJsonResponseSchema } from "./ClaudeCodeClient.js"
import type {
  ClaudeCodeClient as ClaudeCodeClientType,
  ClaudeCodeJsonResponse,
  ClaudeCodeTextResponse
} from "./ClaudeCodeClient.js"

/**
 * Claude Code client using node-pty for proper TTY emulation
 * @since 1.0.0
 */
export const makeClaudeCodePtyClient = (
  config: ClaudeCodeConfig
): ClaudeCodeClientType => {
  const executeCommand = (args: Array<string>, timeout?: number) =>
    Effect.gen(function*() {
      const claudePath = config.cliPath ?? "claude"

      // Create a deferred for the final result
      const resultDeferred = yield* Deferred.make<string, ClaudeCodeExecutionError>()
      let outputBuffer = ""
      let processExited = false

      // Spawn the process with node-pty
      const ptyProcess = pty.spawn(claudePath, args, {
        name: "dumb",
        cols: 120,
        rows: 30,
        cwd: process.cwd(),
        env: {
          ...process.env,
          CI: "true",
          TERM: "dumb",
          NO_COLOR: "1",
          NODE_NO_READLINE: "1"
        } as any
      })

      // Collect output
      ptyProcess.onData((data: string) => {
        outputBuffer += data
      })

      // Handle process exit
      ptyProcess.onExit(({ exitCode }) => {
        processExited = true

        if (exitCode !== 0) {
          Deferred.fail(
            resultDeferred,
            new ClaudeCodeExecutionError({
              command: `${claudePath} ${args.join(" ")}`,
              exitCode,
              stderr: outputBuffer
            })
          ).pipe(Effect.runPromise)
        } else {
          Deferred.succeed(resultDeferred, outputBuffer).pipe(Effect.runPromise)
        }
      })

      // Wait for result with timeout
      const result = yield* Deferred.await(resultDeferred).pipe(
        Effect.timeout(timeout ?? 30000),
        Effect.tapError(() =>
          Effect.sync(() => {
            if (!processExited) {
              ptyProcess.kill()
            }
          })
        ),
        Effect.mapError((error) => {
          if (error._tag === "TimeoutException") {
            return new ClaudeCodeExecutionError({
              command: `${claudePath} ${args.join(" ")}`,
              exitCode: -1,
              stderr: "Command timed out"
            })
          }
          return error
        })
      )

      return result
    })

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
        // Clean ANSI escape sequences and control characters
        const cleanedOutput = output
          // eslint-disable-next-line no-control-regex
          .replace(/\x1b\[[0-9;?]*[a-zA-Z]/g, "") // ANSI escape codes
          // eslint-disable-next-line no-control-regex
          .replace(/\x1b\[?[0-9;]*[hl]/g, "") // More ANSI codes
          // eslint-disable-next-line no-control-regex
          .replace(/[\r\u001b]/g, "") // Carriage returns and escape chars
          .trim()

        try {
          // Try to parse the entire output as JSON first (for multi-line JSON)
          const parsed = JSON.parse(cleanedOutput)

          // Handle different response formats
          if (parsed.role === "system" && parsed.result) {
            // Convert system response to expected format
            return {
              content: parsed.result,
              model: "claude-3-5-sonnet-20241022",
              session_id: parsed.session_id,
              usage: {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0
              }
            }
          }

          return Schema.decodeUnknownSync(ClaudeCodeJsonResponseSchema)(parsed)
        } catch {
          // If that fails, try line by line
          const lines = cleanedOutput.split("\n").filter((line) => line.trim().length > 0)

          for (let i = lines.length - 1; i >= 0; i--) {
            if (lines[i].trim().startsWith("{")) {
              try {
                const parsed = JSON.parse(lines[i])
                return Schema.decodeUnknownSync(ClaudeCodeJsonResponseSchema)(parsed)
              } catch {
                // Continue
              }
            }
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
          const args = ["--print", text, "--output-format", "stream-json", "--verbose"]
          const claudePath = config.cliPath ?? "claude"
          const queue = yield* Queue.unbounded<string>()

          const ptyProcess = pty.spawn(claudePath, args, {
            name: "dumb",
            cols: 120,
            rows: 30,
            cwd: process.cwd(),
            env: {
              ...process.env,
              CI: "true",
              TERM: "dumb",
              NO_COLOR: "1",
              NODE_NO_READLINE: "1"
            } as any
          })

          let buffer = ""
          ptyProcess.onData((data: string) => {
            buffer += data

            // Process lines
            let newlineIndex: number
            while ((newlineIndex = buffer.indexOf("\n")) >= 0) {
              const line = buffer.substring(0, newlineIndex)
              buffer = buffer.substring(newlineIndex + 1)

              const cleaned = line
                // eslint-disable-next-line no-control-regex
                .replace(/\x1b\[[0-9;?]*[a-zA-Z]/g, "")
                // eslint-disable-next-line no-control-regex
                .replace(/\x1b\[?[0-9;]*[hl]/g, "")
                .trim()

              if (cleaned && cleaned.startsWith("{")) {
                try {
                  const parsed = JSON.parse(cleaned)
                  if (parsed.content) {
                    Queue.offer(queue, parsed.content).pipe(Effect.runPromise)
                  }
                } catch {
                  // Ignore parse errors
                }
              }
            }
          })

          ptyProcess.onExit(() => {
            Queue.shutdown(queue).pipe(Effect.runPromise)
          })

          // Add cleanup
          yield* Effect.addFinalizer(() =>
            Effect.sync(() => {
              ptyProcess.kill()
            })
          )

          return Stream.fromQueue(queue)
        })
      ) as Stream.Stream<string, ClaudeCodeExecutionError, never>,

    checkAvailability: () =>
      Effect.gen(function*() {
        try {
          const output = yield* executeCommand(["--version"], 5000)
          return output.includes("claude") || output.includes("Claude")
        } catch {
          return false
        }
      }) as Effect.Effect<boolean, never, never>
  }
}

/**
 * Claude Code client layer using node-pty
 * @since 1.0.0
 */
export const ClaudeCodePtyClientLive = Layer.effect(
  ClaudeCodeClient,
  Effect.gen(function*() {
    const config = yield* ClaudeCodeConfig
    return makeClaudeCodePtyClient(config)
  })
)
