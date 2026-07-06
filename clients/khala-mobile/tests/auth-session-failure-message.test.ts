import { describe, expect, test } from "bun:test"

import { describeAuthSessionFailure } from "../src/auth/auth-session-failure"

// Regression guard for #8467: a GitHub sign-in that ends any way OTHER than a
// clean success must report a real reason. The server completed the whole
// OAuth round-trip and issued a code, but the app never exchanged it and told
// the user nothing — it looked like "nothing happened". These cases must all
// produce a non-empty, human-readable message that names the outcome.
describe("describeAuthSessionFailure", () => {
  test("surfaces an OAuth error redirected back on an error result", () => {
    const message = describeAuthSessionFailure({
      type: "error",
      error: { description: "Cross-Site request verification failed." },
      params: { error: "state_mismatch" },
    })
    expect(message).toContain("Cross-Site request verification failed.")
  })

  test("falls back to the params error when there is no error object", () => {
    const message = describeAuthSessionFailure({
      type: "error",
      params: { error: "unauthorized_client" },
    })
    expect(message).toContain("unauthorized_client")
  })

  test("names a locked concurrent prompt", () => {
    const message = describeAuthSessionFailure({ type: "locked" })
    expect(message.toLowerCase()).toContain("already in progress")
  })

  test("always names the outcome type when there is no detail", () => {
    const message = describeAuthSessionFailure({ type: "dismiss" })
    expect(message).toContain("dismiss")
    expect(message.length).toBeGreaterThan(0)
  })
})
