import { describe, expect, test } from "bun:test"

import {
  codexDeviceAuthStateFromAccount,
  codexDeviceAuthStateFromAttempt,
  codexQuotaLabel,
  codexReadinessForAccount,
  codexReadinessLabel,
  isVisibleCodexAccount,
  type KhalaMobileCodexAccountView,
  visibleCodexAccounts,
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

// Oracle for khala_mobile.settings.disconnect_removes_account_and_hides_stale.v1
describe("contract khala_mobile.settings.disconnect_removes_account_and_hides_stale.v1", () => {
  const view = (
    providerAccountRef: string,
    status: KhalaMobileCodexAccountView["status"],
  ): KhalaMobileCodexAccountView => ({
    accountLabel: null,
    health: status === "connected" ? "healthy" : "requires_reauth",
    lastStatusAt: "2026-07-09T00:00:00.000Z",
    planType: null,
    providerAccountRef,
    quotaState: "unknown",
    readiness: status === "connected" ? "ready" : "unavailable",
    status,
  })

  test("only connected and in-progress pending accounts are visible", () => {
    expect(isVisibleCodexAccount(view("a", "connected"))).toBe(true)
    expect(isVisibleCodexAccount(view("b", "pending"))).toBe(true)
    for (const dead of ["disconnected", "denied", "expired", "unhealthy"] as const) {
      expect(isVisibleCodexAccount(view(`dead_${dead}`, dead))).toBe(false)
    }
  })

  test("visibleCodexAccounts drops stale/dead residue so a disconnected row never lingers", () => {
    const accounts = [
      view("live", "connected"),
      view("stale-disconnected", "disconnected"),
      view("stale-expired", "expired"),
      view("in-progress", "pending"),
    ]
    expect(visibleCodexAccounts(accounts).map(account => account.providerAccountRef)).toEqual([
      "live",
      "in-progress",
    ])
  })
})
