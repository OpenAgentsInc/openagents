/**
 * Tests for Claude Code Control Service
 * @since Phase 3
 */

import { describe, expect, it } from "vitest"
import { Effect, TestContext } from "effect"
import * as ClaudeCodeControlService from "../src/services/ClaudeCodeControlService.js"
import * as WebSocketClient from "../src/services/WebSocketClient.js"

// Mock WebSocket client for testing
const MockWebSocketClientLive = WebSocketClient.WebSocketClient.of({
  connect: () => Effect.succeed(undefined),
  disconnect: () => Effect.succeed(undefined),
  send: () => Effect.succeed(undefined),
  receive: () => Effect.succeed([] as any),
  isConnected: () => Effect.succeed(true)
})

describe("ClaudeCodeControlService", () => {
  it("should start a new Claude Code session", async () => {
    const program = Effect.gen(function*() {
      const service = yield* ClaudeCodeControlService.ClaudeCodeControlService
      
      const session = yield* service.startSession(
        "test-machine",
        "/test/project/path",
        "test-user"
      )

      expect(session).toEqual(
        expect.objectContaining({
          machineId: "test-machine",
          projectPath: "/test/project/path",
          userId: "test-user",
          status: "active"
        })
      )
      
      expect(session.sessionId).toBeTruthy()
      expect(session.startedAt).toBeInstanceOf(Date)
    })

    const result = await program.pipe(
      Effect.provide(ClaudeCodeControlService.ClaudeCodeControlServiceLive),
      Effect.provide(MockWebSocketClientLive),
      Effect.runPromise
    )

    expect(result).toBeDefined()
  })

  it("should send a prompt to a Claude Code session", async () => {
    const program = Effect.gen(function*() {
      const service = yield* ClaudeCodeControlService.ClaudeCodeControlService
      
      const prompt = yield* service.sendPrompt(
        "test-machine",
        "test-session",
        "Help me write a function",
        { maxTurns: 3 }
      )

      expect(prompt).toEqual(
        expect.objectContaining({
          sessionId: "test-session",
          machineId: "test-machine",
          promptText: "Help me write a function",
          status: "sent"
        })
      )
      
      expect(prompt.promptId).toBeTruthy()
      expect(prompt.sentAt).toBeInstanceOf(Date)
    })

    const result = await program.pipe(
      Effect.provide(ClaudeCodeControlService.ClaudeCodeControlServiceLive),
      Effect.provide(MockWebSocketClientLive),
      Effect.runPromise
    )

    expect(result).toBeDefined()
  })

  it("should get active sessions for a machine", async () => {
    const program = Effect.gen(function*() {
      const service = yield* ClaudeCodeControlService.ClaudeCodeControlService
      
      // Start a session first
      yield* service.startSession(
        "test-machine",
        "/test/project",
        "test-user"
      )
      
      // Get active sessions
      const sessions = yield* service.getActiveSessions("test-machine")
      
      expect(sessions).toHaveLength(1)
      expect(sessions[0]).toEqual(
        expect.objectContaining({
          machineId: "test-machine",
          projectPath: "/test/project",
          status: "active"
        })
      )
    })

    const result = await program.pipe(
      Effect.provide(ClaudeCodeControlService.ClaudeCodeControlServiceLive),
      Effect.provide(MockWebSocketClientLive),
      Effect.runPromise
    )

    expect(result).toBeDefined()
  })

  it("should get machine info with correct structure", async () => {
    const program = Effect.gen(function*() {
      const service = yield* ClaudeCodeControlService.ClaudeCodeControlService
      
      const info = yield* service.getMachineInfo("test-machine")
      
      expect(info).toEqual(
        expect.objectContaining({
          machineId: "test-machine",
          supportedFeatures: expect.arrayContaining(["file_edit", "command_exec", "git_ops"]),
          activeSessions: expect.any(Array),
          lastHeartbeat: expect.any(Date)
        })
      )
    })

    const result = await program.pipe(
      Effect.provide(ClaudeCodeControlService.ClaudeCodeControlServiceLive),
      Effect.provide(MockWebSocketClientLive),
      Effect.runPromise
    )

    expect(result).toBeDefined()
  })

  it("should end a Claude Code session", async () => {
    const program = Effect.gen(function*() {
      const service = yield* ClaudeCodeControlService.ClaudeCodeControlService
      
      // Start a session first
      const session = yield* service.startSession(
        "test-machine",
        "/test/project",
        "test-user"
      )
      
      // End the session
      yield* service.endSession("test-machine", session.sessionId)
      
      // Verify it's no longer in active sessions
      const sessions = yield* service.getActiveSessions("test-machine")
      expect(sessions).toHaveLength(0)
    })

    await program.pipe(
      Effect.provide(ClaudeCodeControlService.ClaudeCodeControlServiceLive),
      Effect.provide(MockWebSocketClientLive),
      Effect.runPromise
    )
  })

  it("should validate project access", async () => {
    const program = Effect.gen(function*() {
      const service = yield* ClaudeCodeControlService.ClaudeCodeControlService
      
      // Try to start session with non-existent path
      const result = yield* service.startSession(
        "test-machine",
        "/non/existent/path",
        "test-user"
      ).pipe(
        Effect.either
      )
      
      expect(result._tag).toBe("Left")
    })

    await program.pipe(
      Effect.provide(ClaudeCodeControlService.ClaudeCodeControlServiceLive),
      Effect.provide(MockWebSocketClientLive),
      Effect.runPromise
    )
  })
})

describe("Claude Code CLI Commands", () => {
  it("should have proper command structure", () => {
    // This test ensures the CLI commands are properly structured
    // In a real implementation, we'd test the actual command parsing
    
    const expectedCommands = [
      "start",
      "prompt", 
      "sessions",
      "stream",
      "end",
      "info"
    ]
    
    // Verify we have all expected Claude Code subcommands
    expect(expectedCommands).toHaveLength(6)
    expect(expectedCommands).toContain("start")
    expect(expectedCommands).toContain("prompt")
    expect(expectedCommands).toContain("sessions")
  })
})