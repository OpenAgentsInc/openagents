export type LaneCBurstInput = {
  optIn: boolean
  tier: "public" | "private"
  ownedCapacityOnline: boolean
  ownedFreeSlots: number
  pendingOrders: number
}

export type LaneCBurstDecision = {
  burst: boolean
  count: number
  reason: string
}

export function shouldBurstToMarket(input: LaneCBurstInput): LaneCBurstDecision {
  const ownedAvailableSlots = input.ownedCapacityOnline ? input.ownedFreeSlots : 0
  const count = Math.max(0, input.pendingOrders - ownedAvailableSlots)

  if (!input.optIn) {
    return { burst: false, count, reason: "burst is not opted in" }
  }

  if (input.tier !== "public") {
    return { burst: false, count, reason: "burst is public-tier only" }
  }

  if (input.ownedCapacityOnline === false) {
    return { burst: true, count, reason: "owned capacity is offline" }
  }

  if (input.ownedFreeSlots < input.pendingOrders) {
    return { burst: true, count, reason: "owned capacity is limited" }
  }

  return { burst: false, count, reason: "owned capacity covers pending orders" }
}
