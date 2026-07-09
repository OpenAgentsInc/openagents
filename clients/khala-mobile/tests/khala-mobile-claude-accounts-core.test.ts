import { describe, expect, test } from "bun:test"

import {
  claudeQuotaLabel,
  claudeReadinessForAccount,
  claudeReadinessLabel,
  isVisibleClaudeAccount,
  type KhalaMobileClaudeAccountView,
  visibleClaudeAccounts,
} from "../src/sync/khala-mobile-claude-accounts-core"

describe("contract khala_mobile.cx5.claude_connect_readiness.v1", () => {
  test("account readiness and quota labels cover ready, exhausted, rate-limited, and unavailable", () => {
    const ready = claudeReadinessForAccount({ health: "healthy", quotaState: "available", status: "connected" })
    const exhausted = claudeReadinessForAccount({
      failure: "account_exhausted",
      health: "healthy",
      quotaState: "exhausted",
      status: "connected",
    })
    const rateLimited = claudeReadinessForAccount({
      failure: "account_rate_limited",
      health: "healthy",
      quotaState: "rate_limited",
      status: "connected",
    })
    const unavailable = claudeReadinessForAccount({ health: "unhealthy", status: "connected" })

    expect([ready, exhausted, rateLimited, unavailable]).toEqual([
      "ready",
      "account_exhausted",
      "account_rate_limited",
      "unavailable",
    ])
    expect(claudeReadinessLabel(exhausted)).toBe("Exhausted")
    expect(claudeReadinessLabel(rateLimited)).toBe("Rate limited")
    expect(claudeQuotaLabel("exhausted")).toBe("Quota exhausted")
    expect(claudeQuotaLabel("rate_limited")).toBe("Cooling down")
  })
})

describe("contract khala_mobile.cx5.claude_disconnect_hides_stale.v1", () => {
  const view = (
    providerAccountRef: string,
    status: KhalaMobileClaudeAccountView["status"],
  ): KhalaMobileClaudeAccountView => ({
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
    expect(isVisibleClaudeAccount(view("a", "connected"))).toBe(true)
    expect(isVisibleClaudeAccount(view("b", "pending"))).toBe(true)
    for (const dead of ["disconnected", "denied", "expired", "unhealthy"] as const) {
      expect(isVisibleClaudeAccount(view(`dead_${dead}`, dead))).toBe(false)
    }
  })

  test("visibleClaudeAccounts drops stale/dead residue", () => {
    const accounts = [
      view("live", "connected"),
      view("stale-disconnected", "disconnected"),
      view("stale-expired", "expired"),
      view("in-progress", "pending"),
    ]
    expect(visibleClaudeAccounts(accounts).map(account => account.providerAccountRef)).toEqual([
      "live",
      "in-progress",
    ])
  })
})
