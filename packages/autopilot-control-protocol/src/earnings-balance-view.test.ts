import { describe, expect, test } from "bun:test"

import { projectBalance } from "./earnings-balance-view.js"

describe("earnings balance view projection", () => {
  test("projects a read-only balance from earnings fields", () => {
    expect(projectBalance({
      balanceSats: 1234,
      pendingSats: 56,
      lifetimeSats: 7890,
    })).toEqual({
      balanceSats: 1234,
      pendingSats: 56,
      lifetimeSats: 7890,
      currency: "sats",
      readOnly: true,
    })
  })

  test("reuses nested earnings aliases", () => {
    expect(projectBalance({
      earnings: {
        available_sats: "100",
        pending_payout_sats: "25",
        total_earned_sats: "500",
      },
    })).toEqual({
      balanceSats: 100,
      pendingSats: 25,
      lifetimeSats: 500,
      currency: "sats",
      readOnly: true,
    })
  })

  test("accepts msat aliases through the earnings projection", () => {
    expect(projectBalance({
      wallet: {
        balance_msats: "1234000",
        held_msats: 56000,
        earned_msats: 7890000,
      },
    })).toEqual({
      balanceSats: 1234,
      pendingSats: 56,
      lifetimeSats: 7890,
      currency: "sats",
      readOnly: true,
    })
  })

  test("coerces unavailable payloads to zero sats", () => {
    expect(projectBalance(null)).toEqual({
      balanceSats: 0,
      pendingSats: 0,
      lifetimeSats: 0,
      currency: "sats",
      readOnly: true,
    })
    expect(projectBalance(["not", "an", "object"])).toEqual({
      balanceSats: 0,
      pendingSats: 0,
      lifetimeSats: 0,
      currency: "sats",
      readOnly: true,
    })
  })

  test("coerces negative NaN and invalid amounts to zero", () => {
    expect(projectBalance({
      balanceSats: -1,
      pendingSats: NaN,
      lifetimeSats: "not-a-number",
    })).toEqual({
      balanceSats: 0,
      pendingSats: 0,
      lifetimeSats: 0,
      currency: "sats",
      readOnly: true,
    })
  })

  test("always returns a read-only data projection without spend authority", () => {
    const view = projectBalance({
      balanceSats: 10,
      pendingSats: 2,
      lifetimeSats: 50,
      readOnly: false,
      send: () => "blocked",
      spend: () => "blocked",
      withdraw: () => "blocked",
      transfer: () => "blocked",
      spendAuthority: true,
    })

    expect(view.readOnly).toBe(true)
    expect(Object.hasOwn(view, "send")).toBe(false)
    expect(Object.hasOwn(view, "spend")).toBe(false)
    expect(Object.hasOwn(view, "withdraw")).toBe(false)
    expect(Object.hasOwn(view, "transfer")).toBe(false)
    expect(Object.hasOwn(view, "spendAuthority")).toBe(false)
  })
})
