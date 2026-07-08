import { describe, expect, test } from "bun:test"

import {
  codexDeviceAuthStateFromAccount,
  codexDeviceAuthStateFromAttempt,
  codexQuotaLabel,
  codexReadinessForAccount,
  codexReadinessLabel,
} from "../src/sync/khala-mobile-codex-accounts-core"

describe("contract khala_mobile.cx2.codex_connect_state_machine.v1", () => {
  test("device auth reaches connected, denied, revoked, and expired states", () => {
    expect(codexDeviceAuthStateFromAttempt("connected")).toBe("connected")
    expect(codexDeviceAuthStateFromAttempt("denied")).toBe("denied")
    expect(codexDeviceAuthStateFromAttempt("expired")).toBe("expired")
    expect(codexDeviceAuthStateFromAccount("disconnected")).toBe("revoked")
  })

  test("account readiness and quota labels cover ready, exhausted, rate-limited, and unavailable", () => {
    const ready = codexReadinessForAccount({ health: "healthy", quotaState: "available", status: "connected" })
    const exhausted = codexReadinessForAccount({
      failure: "account_exhausted",
      health: "healthy",
      quotaState: "exhausted",
      status: "connected",
    })
    const rateLimited = codexReadinessForAccount({
      failure: "account_rate_limited",
      health: "healthy",
      quotaState: "rate_limited",
      status: "connected",
    })
    const unavailable = codexReadinessForAccount({ health: "unhealthy", status: "connected" })

    expect([ready, exhausted, rateLimited, unavailable]).toEqual([
      "ready",
      "account_exhausted",
      "account_rate_limited",
      "unavailable",
    ])
    expect(codexReadinessLabel(exhausted)).toBe("Exhausted")
    expect(codexReadinessLabel(rateLimited)).toBe("Rate limited")
    expect(codexQuotaLabel("exhausted")).toBe("Quota exhausted")
    expect(codexQuotaLabel("rate_limited")).toBe("Cooling down")
  })
})
