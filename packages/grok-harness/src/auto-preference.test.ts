import { describe, expect, test } from "bun:test"

import { buildFreeWindowAutoPreference } from "./auto-preference.ts"

describe("buildFreeWindowAutoPreference", () => {
  test("free window prefers grok first and derates measured concurrency", () => {
    const r = buildFreeWindowAutoPreference({
      freeWindowActive: true,
      grokMeasuredFullSuccessConcurrency: 48,
      derate: 0.5,
    })
    expect(r.preferenceOrder[0]).toBe("grok")
    expect(r.marginalCostClassForGrok).toBe("free")
    expect(r.maxConcurrentGrokWorkers).toBe(24)
  })

  test("paid window prefers codex and marks grok api_metered", () => {
    const r = buildFreeWindowAutoPreference({
      freeWindowActive: false,
      grokMeasuredFullSuccessConcurrency: 48,
    })
    expect(r.preferenceOrder[0]).toBe("codex")
    expect(r.marginalCostClassForGrok).toBe("api_metered")
  })
})
