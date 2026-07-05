import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER,
  redactKhalaCodeDesktopDiagnosticsContext,
  redactKhalaCodeDesktopDiagnosticsText,
  redactKhalaCodeDesktopDiagnosticsValue,
} from "../src/shared/diagnostics-redaction"

describe("redactKhalaCodeDesktopDiagnosticsText", () => {
  test("redacts OpenAI-style secret keys embedded in free text", () => {
    const input = "startup failed calling provider with key sk-abcdefghijklmnopqrstuvwx"
    const output = redactKhalaCodeDesktopDiagnosticsText(input)
    expect(output).not.toContain("sk-abcdefghijklmnopqrstuvwx")
    expect(output).toContain(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })

  test("redacts GitHub, Slack, Google, Stripe, and JWT-shaped tokens", () => {
    // Fixture secrets are split across concatenated literals so no committed
    // source line contains a contiguous, scanner-shaped credential string —
    // these are synthetic test fixtures, not real secrets, but GitHub's push
    // protection pattern-matches on shape alone.
    const samples = [
      ["token ", "ghp_", "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"].join(""),
      ["slack ", "xoxb-", "1234567890-abcdefghij"].join(""),
      ["google ", "AIzaSy", "ABCDEFGHIJKLMNOPQRSTUVWXYZ01234"].join(""),
      ["stripe ", "sk_live_", "ABCDEFGHIJ1234567890"].join(""),
      [
        "jwt ",
        "eyJhbGciOiJIUzI1NiJ9",
        ".",
        "eyJzdWIiOiIxMjM0NTY3ODkwIn0",
        ".",
        "dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PYb4tR",
      ].join(""),
      ["aws ", "AKIA", "ABCDEFGHIJKLMNOP"].join(""),
    ]
    for (const sample of samples) {
      const output = redactKhalaCodeDesktopDiagnosticsText(sample)
      expect(output).toContain(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
    }
  })

  test("redacts a Bearer authorization header appearing in free text", () => {
    const output = redactKhalaCodeDesktopDiagnosticsText(
      "request failed: Authorization: Bearer abcdef0123456789",
    )
    expect(output).not.toContain("abcdef0123456789")
  })

  test("leaves ordinary diagnostic text untouched", () => {
    const input = "codex app-server exited with code 1 after 4021ms"
    expect(redactKhalaCodeDesktopDiagnosticsText(input)).toBe(input)
  })
})

describe("redactKhalaCodeDesktopDiagnosticsValue", () => {
  test("drops known-sensitive keys regardless of nesting depth", () => {
    const value = {
      apiKey: "sk-abcdefghijklmnopqrstuvwx",
      nested: {
        authorization: "Bearer abc123",
        deep: {
          token: "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
        },
      },
      prompt: "do the thing the user actually asked for",
      safeField: "codex",
    }
    const redacted = redactKhalaCodeDesktopDiagnosticsValue(value) as Record<string, unknown>
    expect(redacted.apiKey).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
    expect(redacted.prompt).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
    expect(redacted.safeField).toBe("codex")
    const nested = redacted.nested as Record<string, unknown>
    expect(nested.authorization).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
    const deep = nested.deep as Record<string, unknown>
    expect(deep.token).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })

  test("redacts secret-shaped strings inside arrays", () => {
    const redacted = redactKhalaCodeDesktopDiagnosticsValue([
      "plain text",
      "leaked sk-abcdefghijklmnopqrstuvwx here",
    ]) as unknown[]
    expect(redacted[0]).toBe("plain text")
    expect(redacted[1]).toContain(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })

  test("replaces functions and other non-JSON values rather than passing them through", () => {
    const redacted = redactKhalaCodeDesktopDiagnosticsValue(() => "danger")
    expect(redacted).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })

  test("bounds recursion depth defensively against pathological input", () => {
    let value: Record<string, unknown> = { leaf: "sk-abcdefghijklmnopqrstuvwx" }
    for (let i = 0; i < 30; i += 1) value = { child: value }
    expect(() => redactKhalaCodeDesktopDiagnosticsValue(value)).not.toThrow()
  })
})

describe("redactKhalaCodeDesktopDiagnosticsContext", () => {
  test("returns undefined for undefined context", () => {
    expect(redactKhalaCodeDesktopDiagnosticsContext(undefined)).toBeUndefined()
  })

  test("redacts a context record's sensitive fields while preserving safe ones", () => {
    const redacted = redactKhalaCodeDesktopDiagnosticsContext({
      accountRef: "codex-2",
      token: "super-secret-value",
    })
    expect(redacted?.accountRef).toBe("codex-2")
    expect(redacted?.token).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })
})
