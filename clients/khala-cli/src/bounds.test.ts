import { describe, expect, test } from "bun:test"
import {
  KHALA_CHAT_MAX_MESSAGE_CHARS,
  KHALA_CHAT_MAX_MESSAGES,
  KHALA_CHAT_MAX_TOTAL_CHARS,
  type KhalaChatMessage,
} from "./types.js"
import { prepareUserTurn, totalMessageChars } from "./bounds.js"

describe("Khala public chat bounds", () => {
  test("normalizes a single user prompt", () => {
    expect(prepareUserTurn([], "  hello  ")).toEqual([{ role: "user", content: "hello" }])
  })

  test("rejects empty and overlong prompts before the network", () => {
    expectKhalaReason(() => prepareUserTurn([], "  "), "Prompt cannot be empty")
    expectKhalaReason(() => prepareUserTurn([], "x".repeat(KHALA_CHAT_MAX_MESSAGE_CHARS + 1)), "at most")
  })

  test("drops oldest turns to keep message count inside the public contract", () => {
    const history = Array.from({ length: KHALA_CHAT_MAX_MESSAGES }, (_, index): KhalaChatMessage => ({
      role: index % 2 === 0 ? "user" : "assistant",
      content: `message ${index}`,
    }))

    const prepared = prepareUserTurn(history, "new prompt")

    expect(prepared).toHaveLength(KHALA_CHAT_MAX_MESSAGES)
    expect(prepared[0]?.content).toBe("message 1")
    expect(prepared.at(-1)).toEqual({ role: "user", content: "new prompt" })
  })

  test("drops oldest turns to keep total characters inside the public contract", () => {
    const history = Array.from({ length: 6 }, (_, index): KhalaChatMessage => ({
      role: index % 2 === 0 ? "user" : "assistant",
      content: "x".repeat(5_000),
    }))

    const prepared = prepareUserTurn(history, "new prompt")

    expect(totalMessageChars(prepared)).toBeLessThanOrEqual(KHALA_CHAT_MAX_TOTAL_CHARS)
    expect(prepared.at(-1)).toEqual({ role: "user", content: "new prompt" })
  })
})

function expectKhalaReason(fn: () => unknown, reasonSubstring: string): void {
  try {
    fn()
  } catch (error) {
    expect(String((error as { readonly reason?: unknown }).reason)).toContain(reasonSubstring)
    return
  }
  throw new Error("Expected KhalaCliError")
}
