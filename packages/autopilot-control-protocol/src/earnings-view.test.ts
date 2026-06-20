import { describe, expect, test } from "bun:test"

import { formatSats, projectEarnings } from "./earnings-view.js"

describe("earnings view projection", () => {
  test("projects a known balance without exposing spend authority", () => {
    const view = projectEarnings({
      balanceSats: 1234,
      pendingSats: 56,
      lifetimeSats: 7890,
      online: true,
      send: () => "do not expose",
      spendAuthority: true,
    })

    expect(view).toEqual({
      balanceSats: 1234,
      pendingSats: 56,
      lifetimeSats: 7890,
      online: true,
      blockers: [],
    })
    expect(Object.hasOwn(view, "send")).toBe(false)
    expect(Object.hasOwn(view, "spendAuthority")).toBe(false)
  })

  test("keeps unknown amounts nullable with blockers", () => {
    const view = projectEarnings({ online: false })

    expect(view.balanceSats).toBe(null)
    expect(view.pendingSats).toBe(null)
    expect(view.lifetimeSats).toBe(null)
    expect(view.online).toBe(false)
    expect(view.blockers).toEqual([
      "balance_sats_unknown",
      "pending_sats_unknown",
      "lifetime_sats_unknown",
    ])
  })

  test("formats sats with thousands separators", () => {
    expect(formatSats(1234)).toBe("1,234 sats")
    expect(formatSats(0)).toBe("0 sats")
  })

  test("returns a closed read-only projection for bad input", () => {
    expect(projectEarnings(null)).toEqual({
      balanceSats: null,
      pendingSats: null,
      lifetimeSats: null,
      online: false,
      blockers: ["earnings_payload_unavailable"],
    })
    expect(projectEarnings(["not", "an", "object"])).toEqual({
      balanceSats: null,
      pendingSats: null,
      lifetimeSats: null,
      online: false,
      blockers: ["earnings_payload_unavailable"],
    })
  })

  test("defensively rejects invalid numeric fields", () => {
    const view = projectEarnings({
      balanceSats: -1,
      pendingSats: 1.5,
      lifetimeSats: "not-a-number",
      status: "online",
    })

    expect(view).toEqual({
      balanceSats: null,
      pendingSats: null,
      lifetimeSats: null,
      online: true,
      blockers: [
        "balance_sats_invalid",
        "pending_sats_invalid",
        "lifetime_sats_invalid",
      ],
    })
  })

  test("accepts common nested and msat shapes", () => {
    expect(projectEarnings({
      earnings: {
        balance_msats: "1234000",
        pending_payout_sats: "12",
        total_earned_msats: 3456000,
        status: "connected",
        blockers: ["wallet_receipt_pending"],
      },
    })).toEqual({
      balanceSats: 1234,
      pendingSats: 12,
      lifetimeSats: 3456,
      online: true,
      blockers: ["wallet_receipt_pending"],
    })
  })
})
