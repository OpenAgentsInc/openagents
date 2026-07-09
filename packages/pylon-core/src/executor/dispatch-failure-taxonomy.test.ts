import { describe, expect, test } from "bun:test"

import {
  activePylonDispatchBreakers,
  classifyPylonDispatchFailure,
  normalizePylonDispatchFailureLane,
  pylonDispatchBreakerForAccount,
  pylonDispatchBreakerIsActive,
  pylonDispatchBreakerScopeKey,
  type PylonDispatchBreakerSnapshot,
  PYLON_DISPATCH_BREAKER_SCHEMA,
} from "./dispatch-failure-taxonomy.ts"

describe("PY-1 dispatch-failure-taxonomy (#8578)", () => {
  test("classifies rate limit as transient with cooldown", () => {
    const c = classifyPylonDispatchFailure({ error: "HTTP 429 too many requests" })
    expect(c.reason).toBe("account_rate_limited")
    expect(c.failureKind).toBe("transient")
    expect(c.cooldownMs).toBe(30 * 60 * 1000)
    expect(c.blockerRef).toContain("account_rate_limited")
  })

  test("classifies revoked credentials as permanent", () => {
    const c = classifyPylonDispatchFailure({ error: "unauthorized: credentials revoked" })
    expect(c.reason).toBe("account_credentials_revoked")
    expect(c.failureKind).toBe("permanent")
    expect(c.cooldownMs).toBeNull()
  })

  test("classifies public safety block as permanent", () => {
    const c = classifyPylonDispatchFailure({
      blockerRefs: ["request_public_safety_blocked"],
    })
    expect(c.reason).toBe("lane_public_safety_blocked")
    expect(c.failureKind).toBe("permanent")
  })

  test("redacts secret-shaped text before digest", () => {
    const c = classifyPylonDispatchFailure({
      error: "Bearer sk-abcdefghijklmnopqrstuvwxyz failed",
    })
    expect(c.sourceDigestRef.startsWith("digest.pylon.dispatch_failure.")).toBe(
      true,
    )
  })

  test("normalize lane aliases", () => {
    expect(normalizePylonDispatchFailureLane("claude")).toBe("claude_agent")
    expect(normalizePylonDispatchFailureLane("codex")).toBe("codex")
    expect(normalizePylonDispatchFailureLane("nope")).toBe("unknown")
  })

  test("scope keys prefer account then context", () => {
    expect(
      pylonDispatchBreakerScopeKey({ lane: "codex", accountRefHash: "abc" }),
    ).toBe("dispatch-breaker.account-lane.codex.abc")
    expect(
      pylonDispatchBreakerScopeKey({ lane: "codex", contextId: "ctx-1" }),
    ).toBe("dispatch-breaker.context-lane.codex.ctx-1")
  })

  test("active breakers filter permanent and cooling-down transients", () => {
    const now = new Date("2026-07-09T12:00:00.000Z")
    const permanent: PylonDispatchBreakerSnapshot = {
      schema: PYLON_DISPATCH_BREAKER_SCHEMA,
      scopeKey: "k1",
      accountRefHash: "a1",
      contextId: null,
      lane: "codex",
      reason: "account_usage_limited",
      failureKind: "permanent",
      failureCount: 1,
      firstObservedAt: now.toISOString(),
      lastObservedAt: now.toISOString(),
      cooldownUntil: null,
      blockerRefs: [],
      sourceDigestRef: "digest.x",
    }
    const cooling: PylonDispatchBreakerSnapshot = {
      ...permanent,
      scopeKey: "k2",
      accountRefHash: "a2",
      failureKind: "transient",
      reason: "account_rate_limited",
      cooldownUntil: "2026-07-09T12:30:00.000Z",
    }
    const expired: PylonDispatchBreakerSnapshot = {
      ...cooling,
      scopeKey: "k3",
      accountRefHash: "a3",
      cooldownUntil: "2026-07-09T11:00:00.000Z",
    }
    expect(pylonDispatchBreakerIsActive(permanent, now)).toBe(true)
    expect(pylonDispatchBreakerIsActive(cooling, now)).toBe(true)
    expect(pylonDispatchBreakerIsActive(expired, now)).toBe(false)
    expect(activePylonDispatchBreakers([permanent, cooling, expired], now)).toHaveLength(
      2,
    )
    expect(
      pylonDispatchBreakerForAccount({
        accountRefHash: "a2",
        breakers: [permanent, cooling, expired],
        lane: "codex",
        now,
      })?.scopeKey,
    ).toBe("k2")
  })
})
