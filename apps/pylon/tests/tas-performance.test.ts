import { describe, expect, test } from "bun:test"

import { checkBudget, percentile, summarize } from "../src/tas/performance"

describe("tas performance aggregation", () => {
  test("calculates linear-interpolated percentiles without mutating samples", () => {
    const samples = [40, 10, 30, 20]

    expect(percentile(samples, 50)).toBe(25)
    expect(percentile(samples, 95)).toBe(38.5)
    expect(percentile(samples, 99)).toBeCloseTo(39.7)
    expect(percentile(samples, 0)).toBe(10)
    expect(percentile(samples, 100)).toBe(40)
    expect(samples).toEqual([40, 10, 30, 20])
  })

  test("summarizes samples and passes budget when p95 is within the max", () => {
    const summary = summarize([100, 200, 300, 400])

    expect(summary).toEqual({
      count: 4,
      p50: 250,
      p95: 385,
      p99: 397,
      max: 400,
    })
    expect(checkBudget(summary, { p95Max: 385 })).toEqual({
      ok: true,
      breaches: [],
    })
  })

  test("reports a budget breach when p95 exceeds the max", () => {
    expect(checkBudget(summarize([100, 200, 300, 400]), { p95Max: 300 })).toEqual(
      {
        ok: false,
        breaches: ["p95 385 exceeds budget 300"],
      },
    )
  })

  test("handles empty samples", () => {
    expect(percentile([], 95)).toBe(0)
    expect(summarize([])).toEqual({
      count: 0,
      p50: 0,
      p95: 0,
      p99: 0,
      max: 0,
    })
    expect(checkBudget(summarize([]), { p95Max: 1 })).toEqual({
      ok: true,
      breaches: [],
    })
  })
})
