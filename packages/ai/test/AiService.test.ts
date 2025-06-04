import { Effect } from "effect"
import { describe, expect, it } from "vitest"
import { AiService, AiServiceLive } from "../src/AiService.js"

describe("AiService", () => {
  it("should say hello", () =>
    Effect.gen(function*() {
      const ai = yield* AiService
      const message = yield* ai.hello("World")

      expect(message).toBe("Hello World from AI Service!")
    }).pipe(
      Effect.provide(AiServiceLive),
      Effect.runPromise
    ))

  it("should complete placeholder prompt", () =>
    Effect.gen(function*() {
      const ai = yield* AiService
      const response = yield* ai.complete("Test prompt")

      expect(response.content).toBe("Response to: Test prompt")
      expect(response.model).toBe("placeholder")
      expect(response.usage.totalTokens).toBe(0)
    }).pipe(
      Effect.provide(AiServiceLive),
      Effect.runPromise
    ))
})
