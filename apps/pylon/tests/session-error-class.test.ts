import { describe, expect, test } from "bun:test"

import { classifySessionError } from "../src/session-error-class"

describe("classifySessionError", () => {
  test("classifies session errors by precedence", () => {
    expect(classifySessionError("session was cancelled").errorClass).toBe(
      "cancelled",
    )
    expect(classifySessionError("account home not found").errorClass).toBe(
      "account_selection",
    )
    expect(classifySessionError("worktree_path_missing").errorClass).toBe(
      "workspace_materialization",
    )
    expect(
      classifySessionError("dev check did not pass: blocked").errorClass,
    ).toBe("verification_failed")
    expect(
      classifySessionError("retained proof failed redaction scan").errorClass,
    ).toBe("redaction_gate")
    expect(classifySessionError("something else broke").errorClass).toBe(
      "execution_error",
    )
  })

  test("returns a pylon session error digest reference", () => {
    const result = classifySessionError("something else broke")

    expect(result.errorDigestRef).toStartWith("digest.pylon.session.error.")
    expect(result.errorDigestRef.length).toBeGreaterThan(
      "digest.pylon.session.error.".length,
    )
  })
})
