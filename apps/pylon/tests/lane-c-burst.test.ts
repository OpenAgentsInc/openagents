import { describe, expect, test } from "bun:test"

import { shouldBurstToMarket } from "../src/coordinator/lane-c-burst"

describe("Lane C market burst decision", () => {
  test("bursts all pending public opt-in orders when owned capacity is offline", () => {
    expect(shouldBurstToMarket({
      optIn: true,
      tier: "public",
      ownedCapacityOnline: false,
      ownedFreeSlots: 10,
      pendingOrders: 4,
    })).toEqual({
      burst: true,
      count: 4,
      reason: "owned capacity is offline",
    })
  })

  test("bursts only the public opt-in overflow when owned capacity is limited", () => {
    expect(shouldBurstToMarket({
      optIn: true,
      tier: "public",
      ownedCapacityOnline: true,
      ownedFreeSlots: 3,
      pendingOrders: 8,
    })).toEqual({
      burst: true,
      count: 5,
      reason: "owned capacity is limited",
    })
  })

  test("does not burst when owned capacity covers all pending orders", () => {
    expect(shouldBurstToMarket({
      optIn: true,
      tier: "public",
      ownedCapacityOnline: true,
      ownedFreeSlots: 8,
      pendingOrders: 8,
    })).toEqual({
      burst: false,
      count: 0,
      reason: "owned capacity covers pending orders",
    })
  })

  test("does not burst without explicit opt-in", () => {
    expect(shouldBurstToMarket({
      optIn: false,
      tier: "public",
      ownedCapacityOnline: false,
      ownedFreeSlots: 0,
      pendingOrders: 6,
    })).toEqual({
      burst: false,
      count: 6,
      reason: "burst is not opted in",
    })
  })

  test("does not burst private-tier orders even when capacity is dark", () => {
    expect(shouldBurstToMarket({
      optIn: true,
      tier: "private",
      ownedCapacityOnline: false,
      ownedFreeSlots: 0,
      pendingOrders: 6,
    })).toEqual({
      burst: false,
      count: 6,
      reason: "burst is public-tier only",
    })
  })

  test("never returns a negative burst count", () => {
    expect(shouldBurstToMarket({
      optIn: true,
      tier: "public",
      ownedCapacityOnline: true,
      ownedFreeSlots: 12,
      pendingOrders: 2,
    })).toEqual({
      burst: false,
      count: 0,
      reason: "owned capacity covers pending orders",
    })
  })

  test("reports zero count for offline capacity when there are no pending orders", () => {
    expect(shouldBurstToMarket({
      optIn: true,
      tier: "public",
      ownedCapacityOnline: false,
      ownedFreeSlots: 0,
      pendingOrders: 0,
    })).toEqual({
      burst: true,
      count: 0,
      reason: "owned capacity is offline",
    })
  })
})
