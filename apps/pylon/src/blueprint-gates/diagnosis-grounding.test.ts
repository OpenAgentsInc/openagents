import { describe, expect, test } from "bun:test"

import {
  DIAGNOSIS_GROUNDING_EVIDENCE,
  diagnosisClaimsRateLimit,
  evaluateDiagnosisGrounding,
  type DiagnosisGroundingInputs,
} from "./diagnosis-grounding.js"

const grounded: DiagnosisGroundingInputs = {
  claimedRootCause: "provider returned 429 rate-limited responses",
  quotaLedgerSnapshot: { accounts: [{ accountRef: "codex", remaining: 0 }] },
  supervisorDispatchLog: [{ attempt: 1, outcome: "provider_429" }],
  accountRateLimitHeaders: {
    statusCode: 429,
    retryAfter: "60",
    xRateLimitReset: null,
  },
}

describe("diagnosis-grounding", () => {
  test("a rate-limit diagnosis with ledger, dispatch log, and provider headers reaches GROUNDED", () => {
    const result = evaluateDiagnosisGrounding(grounded)
    expect(result.state).toBe("GROUNDED")
    expect(result.canProposeRemediation).toBe(true)
    expect(result.satisfiedEvidence).toEqual([
      DIAGNOSIS_GROUNDING_EVIDENCE.quotaLedgerRead,
      DIAGNOSIS_GROUNDING_EVIDENCE.supervisorDispatchLog,
      DIAGNOSIS_GROUNDING_EVIDENCE.providerRateLimitHeaders,
    ])
  })

  test("a rate-limit diagnosis without provider 429 headers locks before remediation", () => {
    const result = evaluateDiagnosisGrounding({
      ...grounded,
      accountRateLimitHeaders: null,
    })
    expect(result.state).toBe("DISPATCH_LOG_EXAMINED")
    expect(result.canProposeRemediation).toBe(false)
    expect(result.lockedAt).toBe("PROVIDER_VERIFIED")
    expect(result.missingEvidence).toEqual([
      DIAGNOSIS_GROUNDING_EVIDENCE.providerRateLimitHeaders,
    ])
  })

  test("missing quota ledger locks at LEDGER_READ", () => {
    const result = evaluateDiagnosisGrounding({
      ...grounded,
      quotaLedgerSnapshot: null,
    })
    expect(result.state).toBe("UNGROUNDED")
    expect(result.lockedAt).toBe("LEDGER_READ")
    expect(result.canProposeRemediation).toBe(false)
  })

  test("missing dispatch log locks at DISPATCH_LOG_EXAMINED", () => {
    const result = evaluateDiagnosisGrounding({
      ...grounded,
      supervisorDispatchLog: [],
    })
    expect(result.state).toBe("LEDGER_READ")
    expect(result.lockedAt).toBe("DISPATCH_LOG_EXAMINED")
  })

  test("non-rate-limit claims still require provider verification stage but not 429 headers", () => {
    const result = evaluateDiagnosisGrounding({
      ...grounded,
      claimedRootCause: "supervisor dispatch loop is wedged",
      accountRateLimitHeaders: null,
    })
    expect(result.state).toBe("GROUNDED")
    expect(result.canProposeRemediation).toBe(true)
  })

  test("detects rate-limit claims", () => {
    expect(diagnosisClaimsRateLimit("429 from provider")).toBe(true)
    expect(diagnosisClaimsRateLimit("rate limited")).toBe(true)
    expect(diagnosisClaimsRateLimit("stale heartbeat")).toBe(false)
  })
})
