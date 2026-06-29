import { describe, expect, test } from "bun:test"

import { projectFailover } from "./provider-failover-state.js"

describe("provider failover state projection", () => {
  test("keeps the primary active when it is ready and unlimited", () => {
    expect(projectFailover([
      { provider: "openai", ready: true },
      { provider: "anthropic", ready: true },
    ])).toEqual({
      active: "openai",
      standby: ["anthropic"],
      failedOver: false,
      reason: "primary_active",
    })
  })

  test("fails over to the first ready unlimited standby when primary is not ready", () => {
    expect(projectFailover([
      { provider: "openai", ready: false },
      { provider: "anthropic", ready: true },
      { provider: "local", ready: true },
    ])).toEqual({
      active: "anthropic",
      standby: ["local"],
      failedOver: true,
      reason: "primary_not_ready",
    })
  })

  test("fails over when the primary is limited", () => {
    expect(projectFailover([
      { provider: "openai", ready: true, limited: true },
      { provider: "anthropic", ready: true, limited: false },
    ])).toEqual({
      active: "anthropic",
      standby: [],
      failedOver: true,
      reason: "primary_limited",
    })
  })

  test("skips limited standby accounts", () => {
    expect(projectFailover([
      { provider: "openai", ready: false },
      { provider: "anthropic", ready: true, limited: true },
      { provider: "local", ready: true },
    ])).toEqual({
      active: "local",
      standby: [],
      failedOver: true,
      reason: "primary_not_ready",
    })
  })

  test("returns an empty state when no provider is ready and unlimited", () => {
    expect(projectFailover([
      { provider: "openai", ready: false },
      { provider: "anthropic", ready: true, limited: true },
    ])).toEqual({
      active: null,
      standby: [],
      failedOver: true,
      reason: "no_ready_unlimited_provider",
    })
  })

  test("defensively trims provider names and ignores blank providers", () => {
    expect(projectFailover([
      { provider: "  ", ready: true },
      { provider: " anthropic ", ready: true },
    ])).toEqual({
      active: "anthropic",
      standby: [],
      failedOver: true,
      reason: "primary_unavailable",
    })
  })

  test("defensively handles non-array input at runtime", () => {
    expect(projectFailover(null as unknown as Parameters<typeof projectFailover>[0])).toEqual({
      active: null,
      standby: [],
      failedOver: false,
      reason: "no_accounts",
    })
  })
})
