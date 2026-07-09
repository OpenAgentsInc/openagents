import { describe, expect, test } from "bun:test"

import { classifySessionError } from "./session-error-class.js"

describe("session error classification", () => {
  test("surfaces Codex usage exhaustion as the actual account error", () => {
    expect(classifySessionError("You have hit your usage limit.").errorClass).toBe(
      "account_exhausted",
    )
  })

  test("keeps invalid Codex resume ids distinct from provider account exhaustion", () => {
    expect(
      classifySessionError(
        "invalid session id: expected an optional prefix of urn:uuid followed by [0-9a-fA-F-]",
      ).errorClass,
    ).toBe("invalid_codex_session_id")
  })
})
