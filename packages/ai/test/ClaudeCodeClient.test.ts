import { CommandExecutor } from "@effect/platform"
import { Effect, Exit, Layer } from "effect"
import { describe, expect, it, vi } from "vitest"
import { ClaudeCodeClient, ClaudeCodeClientLive, ClaudeCodeConfig, ClaudeCodeConfigDefault } from "../src/index.js"

// Mock CommandExecutor for testing
const mockExecutor = {
  start: vi.fn()
}

const MockCommandExecutorLayer = Layer.succeed(CommandExecutor, mockExecutor as any)

describe("ClaudeCodeClient", () => {
  describe("checkAvailability", () => {
    it("should return true when claude CLI is available", () =>
      Effect.gen(function*() {
        // Mock successful version check
        mockExecutor.start.mockResolvedValueOnce({
          exitCode: Effect.succeed(0),
          stdout: { pipe: () => Effect.succeed("") },
          stderr: { pipe: () => Effect.succeed("") }
        })

        const client = yield* ClaudeCodeClient
        const result = yield* client.checkAvailability()

        expect(result).toBe(true)
        expect(mockExecutor.start).toHaveBeenCalledWith(
          expect.objectContaining({
            command: "claude",
            args: ["--version"]
          })
        )
      }).pipe(
        Effect.provide(ClaudeCodeClientLive),
        Effect.provide(ClaudeCodeConfigDefault),
        Effect.provide(MockCommandExecutorLayer),
        Effect.runPromise
      ))

    it("should return false when claude CLI is not found", () =>
      Effect.gen(function*() {
        // Mock command not found
        mockExecutor.start.mockRejectedValueOnce(new Error("command not found"))

        const client = yield* ClaudeCodeClient
        const exit = yield* Effect.exit(client.checkAvailability())

        expect(Exit.isFailure(exit)).toBe(true)
        if (Exit.isFailure(exit)) {
          const error = exit.cause._tag === "Fail" ? exit.cause.error : null
          expect(error?._tag).toBe("ClaudeCodeNotFoundError")
        }
      }).pipe(
        Effect.provide(ClaudeCodeClientLive),
        Effect.provide(ClaudeCodeConfigDefault),
        Effect.provide(MockCommandExecutorLayer),
        Effect.runPromise
      ))
  })

  describe("prompt", () => {
    it("should execute a prompt and parse JSON response", () =>
      Effect.gen(function*() {
        const mockResponse = {
          content: "Hello from Claude!",
          model: "claude-3-opus-20240229",
          usage: {
            input_tokens: 10,
            output_tokens: 20,
            total_tokens: 30
          }
        }

        // Mock successful command execution
        mockExecutor.start.mockResolvedValueOnce({
          exitCode: Effect.succeed(0),
          stdout: {
            pipe: vi.fn().mockReturnValue(
              Effect.succeed([JSON.stringify(mockResponse)])
            )
          },
          stderr: { pipe: () => Effect.succeed([]) }
        })

        const client = yield* ClaudeCodeClient
        const result = yield* client.prompt("Say hello")

        expect(result).toMatchObject({
          content: "Hello from Claude!",
          model: "claude-3-opus-20240229"
        })
      }).pipe(
        Effect.provide(ClaudeCodeClientLive),
        Effect.provide(ClaudeCodeConfigDefault),
        Effect.provide(MockCommandExecutorLayer),
        Effect.runPromise
      ))

    it("should handle text format response", () =>
      Effect.gen(function*() {
        const mockResponse = "Hello from Claude!"

        // Mock successful command execution
        mockExecutor.start.mockResolvedValueOnce({
          exitCode: Effect.succeed(0),
          stdout: {
            pipe: vi.fn().mockReturnValue(
              Effect.succeed([mockResponse])
            )
          },
          stderr: { pipe: () => Effect.succeed([]) }
        })

        const textConfig = Layer.succeed(ClaudeCodeConfig, {
          outputFormat: "text",
          cliPath: "claude"
        })

        const program = Effect.gen(function*() {
          const client = yield* ClaudeCodeClient
          const result = yield* client.prompt("Say hello")

          expect(result).toMatchObject({
            content: "Hello from Claude!"
          })
        })

        yield* program.pipe(
          Effect.provide(ClaudeCodeClientLive),
          Effect.provide(textConfig),
          Effect.provide(MockCommandExecutorLayer)
        )
      }).pipe(Effect.runPromise))
  })

  describe("error handling", () => {
    it("should handle command execution errors", () =>
      Effect.gen(function*() {
        // Mock command failure
        mockExecutor.start.mockResolvedValueOnce({
          exitCode: Effect.succeed(1),
          stdout: { pipe: () => Effect.succeed([]) },
          stderr: {
            pipe: vi.fn().mockReturnValue(
              Effect.succeed(["Error: API rate limit exceeded"])
            )
          }
        })

        const client = yield* ClaudeCodeClient
        const exit = yield* Effect.exit(client.prompt("Say hello"))

        expect(Exit.isFailure(exit)).toBe(true)
        if (Exit.isFailure(exit)) {
          const error = exit.cause._tag === "Fail" ? exit.cause.error : null
          expect(error?._tag).toBe("ClaudeCodeExecutionError")
          expect(error?.stderr).toContain("API rate limit exceeded")
        }
      }).pipe(
        Effect.provide(ClaudeCodeClientLive),
        Effect.provide(ClaudeCodeConfigDefault),
        Effect.provide(MockCommandExecutorLayer),
        Effect.runPromise
      ))

    it("should handle JSON parse errors", () =>
      Effect.gen(function*() {
        // Mock successful command but invalid JSON
        mockExecutor.start.mockResolvedValueOnce({
          exitCode: Effect.succeed(0),
          stdout: {
            pipe: vi.fn().mockReturnValue(
              Effect.succeed(["Invalid JSON response"])
            )
          },
          stderr: { pipe: () => Effect.succeed([]) }
        })

        const client = yield* ClaudeCodeClient
        const exit = yield* Effect.exit(client.prompt("Say hello"))

        expect(Exit.isFailure(exit)).toBe(true)
        if (Exit.isFailure(exit)) {
          const error = exit.cause._tag === "Fail" ? exit.cause.error : null
          expect(error?._tag).toBe("ClaudeCodeParseError")
        }
      }).pipe(
        Effect.provide(ClaudeCodeClientLive),
        Effect.provide(ClaudeCodeConfigDefault),
        Effect.provide(MockCommandExecutorLayer),
        Effect.runPromise
      ))
  })
})
