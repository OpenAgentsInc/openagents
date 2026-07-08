import { describe, expect, it } from "bun:test"
import {
  classifyCodexAccountFailure,
  codexAccountFailureBlockerRefs,
  PYLON_CODEX_CUSTODY_ASSIGNMENT_BLOCKER_REF,
} from "./index.js"

// Smoke coverage that the custody barrel re-exports the moved leaf modules
// and that their pure classifiers still behave after the extraction (#8578).
describe("custody barrel", () => {
  it("re-exports the moved custody symbols", () => {
    expect(typeof classifyCodexAccountFailure).toBe("function")
    expect(typeof codexAccountFailureBlockerRefs).toBe("function")
    expect(typeof PYLON_CODEX_CUSTODY_ASSIGNMENT_BLOCKER_REF).toBe("string")
  })

  it("classifies a rate-limit failure and redacts to a public-safe message", () => {
    const result = classifyCodexAccountFailure("HTTP 429 too many requests")
    expect(result.reason).toBe("rate_limited")
    expect(result.publicMessage.length).toBeGreaterThan(0)
    expect(result.sourceDigestRef.startsWith("digest.pylon.codex_account_failure.")).toBe(true)
  })

  it("classifies revoked credentials and emits the reauth blocker ref", () => {
    const result = classifyCodexAccountFailure("your credentials were revoked")
    expect(result.reason).toBe("credentials_revoked")
    const refs = codexAccountFailureBlockerRefs(result.reason)
    expect(refs).toContain("blocker.assignment.codex_account_credentials_revoked_needs_owner_reauth")
  })

  it("falls back to 'other' for unrecognized failures", () => {
    expect(classifyCodexAccountFailure("something unexpected happened").reason).toBe("other")
  })
})
