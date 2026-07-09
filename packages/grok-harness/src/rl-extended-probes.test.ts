import { describe, expect, test } from "bun:test"

import {
  buildRlExtendedMatrixReceipt,
  evaluateFreeWindowDeath,
  planRl3MultiAccountProbe,
  recordRl5CalendarQuotaObservation,
} from "./rl-extended-probes.ts"

describe("MH-4 extended RL probes (#8590)", () => {
  test("RL-3 skips without two accounts", () => {
    const plan = planRl3MultiAccountProbe({ accountIds: ["only-one"] })
    expect(plan.runnable).toBe(false)
    expect(plan.skipReason).toContain("≥2")
  })

  test("RL-3 runnable with two accounts", () => {
    const plan = planRl3MultiAccountProbe({
      accountIds: ["a", "b"],
      sessionsPerAccount: 3,
    })
    expect(plan.runnable).toBe(true)
    expect(plan.totalWorkers).toBe(6)
  })

  test("RL-5 records honest non-observation", () => {
    const obs = recordRl5CalendarQuotaObservation({})
    expect(obs.calendarCapObserved).toBe(false)
    expect(obs.dailyCap).toBeNull()
    expect(obs.notes.some((n) => n.includes("not invent"))).toBe(true)
  })

  test("RL-6 flips free → api_metered when free ends", () => {
    const death = evaluateFreeWindowDeath({
      wasFree: true,
      freeWindowActive: false,
      freeEndedAt: "2026-07-09T12:00:00.000Z",
    })
    expect(death.flip).toBe(true)
    expect(death.marginalCostClass).toBe("api_metered")
    expect(death.alert).toBe("grok_free_window_ended")
  })

  test("RL-6 soft-death on deprioritization", () => {
    const death = evaluateFreeWindowDeath({
      wasFree: true,
      freeWindowActive: true,
      observedDeprioritization: true,
    })
    expect(death.flip).toBe(true)
    expect(death.alert).toBe("grok_free_window_deprioritized")
  })

  test("RL-6 no flip while free holds", () => {
    const death = evaluateFreeWindowDeath({
      wasFree: true,
      freeWindowActive: true,
    })
    expect(death.flip).toBe(false)
    expect(death.marginalCostClass).toBe("free")
    expect(death.alert).toBeNull()
  })

  test("extended matrix receipt encodes MH-4 exit", () => {
    const receipt = buildRlExtendedMatrixReceipt({
      rl1MaxFullSuccessConcurrency: 48,
      rl4MaxFullSuccessConcurrency: 4,
    })
    expect(receipt.exit.executorFixtureGreen).toBe(true)
    expect(receipt.exit.rl1Rl2ReceiptsSetCeiling).toBe(true)
    expect(receipt.rl1Rl2Rl4.rl2Metering).toBe("not_measured")
    expect(receipt.rl3.runnable).toBe(false)
  })
})
