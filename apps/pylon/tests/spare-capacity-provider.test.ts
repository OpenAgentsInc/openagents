import { describe, expect, test } from "bun:test"

import {
  evaluateProviderAvailability,
  type ProviderMode,
} from "../src/coordinator/spare-capacity-provider"

describe("spare capacity provider availability", () => {
  test("accepts when online, local queue is empty, and capacity is available", () => {
    expect(
      evaluateProviderAvailability({
        mode: "online",
        ownedQueueDepth: 0,
        maxConcurrent: 3,
        activeJobs: 1,
      }),
    ).toEqual({
      accepting: true,
      reason: "provider is online with spare capacity",
      freeSlots: 2,
    })
  })

  test("rejects when provider mode is offline", () => {
    expect(
      evaluateProviderAvailability({
        mode: "offline",
        ownedQueueDepth: 0,
        maxConcurrent: 3,
        activeJobs: 0,
      }),
    ).toEqual({
      accepting: false,
      reason: "provider is offline",
      freeSlots: 3,
    })
  })

  test("rejects online provider when owned work is queued", () => {
    expect(
      evaluateProviderAvailability({
        mode: "online",
        ownedQueueDepth: 1,
        maxConcurrent: 3,
        activeJobs: 0,
      }),
    ).toEqual({
      accepting: false,
      reason: "owned work is queued",
      freeSlots: 3,
    })
  })

  test("rejects online provider at max concurrency", () => {
    expect(
      evaluateProviderAvailability({
        mode: "online",
        ownedQueueDepth: 0,
        maxConcurrent: 2,
        activeJobs: 2,
      }),
    ).toEqual({
      accepting: false,
      reason: "provider has no free slots",
      freeSlots: 0,
    })
  })

  test("rejects online provider above max concurrency and clamps free slots", () => {
    expect(
      evaluateProviderAvailability({
        mode: "online",
        ownedQueueDepth: 0,
        maxConcurrent: 2,
        activeJobs: 4,
      }),
    ).toEqual({
      accepting: false,
      reason: "provider has no free slots",
      freeSlots: 0,
    })
  })

  test("keeps owned work ahead of spare capacity even with free slots", () => {
    const result = evaluateProviderAvailability({
      mode: "online",
      ownedQueueDepth: 5,
      maxConcurrent: 10,
      activeJobs: 2,
    })

    expect(result.accepting).toBe(false)
    expect(result.reason).toBe("owned work is queued")
    expect(result.freeSlots).toBe(8)
  })

  test("exports provider mode as offline or online", () => {
    const modes: ProviderMode[] = ["offline", "online"]

    expect(modes).toEqual(["offline", "online"])
  })
})
