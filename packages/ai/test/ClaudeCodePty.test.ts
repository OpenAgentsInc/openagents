import { Effect } from "effect"
import { describe, expect, it } from "vitest"
import { ClaudeCodeConfigDefault } from "../src/config/ClaudeCodeConfig.js"
import { ClaudeCodeClient } from "../src/providers/ClaudeCodeClient.js"
import { ClaudeCodePtyClientLive, makeClaudeCodePtyClient } from "../src/providers/ClaudeCodePty.js"

describe("ClaudeCodePtyClient", () => {
  describe("checkAvailability", () => {
    it.skipIf(process.env.CI)("should return a boolean", () =>
      Effect.gen(function*() {
        const client = yield* ClaudeCodeClient
        const result = yield* client.checkAvailability()

        expect(typeof result).toBe("boolean")
      }).pipe(
        Effect.provide(ClaudeCodePtyClientLive),
        Effect.provide(ClaudeCodeConfigDefault),
        Effect.runPromise
      ))
  })

  describe("makeClaudeCodePtyClient", () => {
    it("should create a client with all methods", () => {
      const config = { outputFormat: "json" as const }
      const client = makeClaudeCodePtyClient(config)

      expect(client.prompt).toBeDefined()
      expect(client.continueSession).toBeDefined()
      expect(client.continueRecent).toBeDefined()
      expect(client.streamPrompt).toBeDefined()
      expect(client.checkAvailability).toBeDefined()
    })
  })
})
