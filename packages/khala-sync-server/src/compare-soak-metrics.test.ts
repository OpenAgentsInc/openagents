import { describe, expect, test } from "vite-plus/test"

import {
  makeCompareSoakMetrics,
  noopCompareSoakMetrics,
} from "./compare-soak-metrics.js"

describe("makeCompareSoakMetrics", () => {
  test("forwards a bounded sample to the injected sink", () => {
    const written: Array<unknown> = []
    const write = (sample: unknown) => written.push(sample)
    const metrics = makeCompareSoakMetrics({ write })
    const sample = {
      domain: "artanis",
      readKind: "ledger:readById",
      outcome: "match" as const,
    }

    metrics.record(sample)

    expect(written).toEqual([sample])
  })

  test("is a no-op without a sink", () => {
    expect(() =>
      makeCompareSoakMetrics(undefined).record({
        domain: "supervision",
        readKind: "lease:read",
        outcome: "mismatch",
      }),
    ).not.toThrow()
  })

  test("swallows sink failures", () => {
    const metrics = makeCompareSoakMetrics({
      write: () => {
        throw new Error("telemetry unavailable")
      },
    })

    expect(() =>
      metrics.record({
        domain: "entitlements",
        readKind: "quota:read",
        outcome: "error",
      }),
    ).not.toThrow()
  })

  test("the exported no-op recorder never throws", () => {
    expect(() =>
      noopCompareSoakMetrics.record({
        domain: "d",
        readKind: "k",
        outcome: "match",
      }),
    ).not.toThrow()
  })
})
