import { describe, expect, test } from "bun:test"

import { classifyQuotaSignal } from "../src/account-quota"

const codexUsageLimitMessage =
  "You've hit your usage limit. Visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at Jun 14th, 2026 9:58 PM."

describe("classifyQuotaSignal", () => {
  test("classifies Codex usage limit reset messages", () => {
    const signal = classifyQuotaSignal(codexUsageLimitMessage, "codex")

    expect(signal.exhausted).toBe(true)
    expect(signal.retryAtRaw).toBe("Jun 14th, 2026 9:58 PM")
    expect(signal.retryAtIso).toEqual(expect.any(String))
  })

  test("parses provider retry-after seconds into a concrete reset time", () => {
    const signal = classifyQuotaSignal(
      "HTTP 429 rate limit exceeded\nretry-after: 120",
      "codex",
      { now: new Date("2026-06-28T21:41:00.000Z") },
    )

    expect(signal.exhausted).toBe(true)
    expect(signal.retryAtRaw).toBe("120")
    expect(signal.retryAtIso).toBe("2026-06-28T21:43:00.000Z")
  })

  test("parses relative provider cooldown prose into a concrete reset time", () => {
    const signal = classifyQuotaSignal(
      "rate limit exceeded, try again in 17 minutes.",
      "claude_agent",
      { now: new Date("2026-06-28T21:41:00.000Z") },
    )

    expect(signal.exhausted).toBe(true)
    expect(signal.retryAtRaw).toBe("17 minutes")
    expect(signal.retryAtIso).toBe("2026-06-28T21:58:00.000Z")
  })

  test("classifies Claude-style rate limits without reset timestamps", () => {
    const signal = classifyQuotaSignal(
      "rate limit exceeded, try again later",
      "claude_agent",
    )

    expect(signal.exhausted).toBe(true)
    expect(signal.retryAtRaw).toBe(null)
  })

  test("does not classify plain compile errors as quota exhaustion", () => {
    const signal = classifyQuotaSignal("compile error: missing semicolon", "codex")

    expect(signal.exhausted).toBe(false)
  })

  test("does not expose raw provider output in the projection", () => {
    const signal = classifyQuotaSignal(codexUsageLimitMessage, "codex")

    expect(JSON.stringify(signal)).not.toContain(codexUsageLimitMessage)
  })
})
