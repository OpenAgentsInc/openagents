import { describe, expect, test } from "bun:test"

import {
  type CompareSoakAnalyticsDataset,
  makeCompareSoakMetrics,
  noopCompareSoakMetrics,
} from "./compare-soak-metrics.js"

type WrittenEvent = Readonly<{
  indexes?: Array<string | null>
  blobs?: Array<string | null>
  doubles?: Array<number>
}>

const fakeDataset = (): CompareSoakAnalyticsDataset & { writes: WrittenEvent[] } => {
  const writes: WrittenEvent[] = []
  return {
    writes,
    writeDataPoint: event => {
      writes.push(event ?? {})
    },
  }
}

describe("makeCompareSoakMetrics", () => {
  test("records a match sample with the expected blob/double/index shape", () => {
    const dataset = fakeDataset()
    const metrics = makeCompareSoakMetrics(dataset)

    metrics.record({ domain: "supervision", readKind: "omni_public_proof_bundles:readById", outcome: "match" })

    expect(dataset.writes).toHaveLength(1)
    const [event] = dataset.writes
    expect(event.blobs).toEqual(["supervision", "omni_public_proof_bundles:readById", "match"])
    expect(event.doubles).toEqual([1, 1, 0, 0])
    expect(event.indexes).toEqual(["supervision"])
  })

  test("records a mismatch sample", () => {
    const dataset = fakeDataset()
    const metrics = makeCompareSoakMetrics(dataset)

    metrics.record({ domain: "entitlements_gate", readKind: "freeTierKeyExists", outcome: "mismatch" })

    const [event] = dataset.writes
    expect(event.doubles).toEqual([1, 0, 1, 0])
  })

  test("records an error sample", () => {
    const dataset = fakeDataset()
    const metrics = makeCompareSoakMetrics(dataset)

    metrics.record({ domain: "artanis", readKind: "dispatchLookup", outcome: "error" })

    const [event] = dataset.writes
    expect(event.doubles).toEqual([1, 0, 0, 1])
  })

  test("is a true no-op when no dataset binding is present", () => {
    const metrics = makeCompareSoakMetrics(undefined)
    // Must not throw regardless of how many samples are recorded.
    for (let i = 0; i < 5; i++) {
      metrics.record({ domain: "d", readKind: `k${i}`, outcome: "match" })
    }
    expect(true).toBe(true)
  })

  test("clips overlong domain/readKind strings instead of throwing", () => {
    const dataset = fakeDataset()
    const metrics = makeCompareSoakMetrics(dataset)
    const longDomain = "d".repeat(500)
    const longReadKind = "r".repeat(1000)

    metrics.record({ domain: longDomain, readKind: longReadKind, outcome: "match" })

    const [event] = dataset.writes
    expect(event.blobs?.[0]?.length).toBeLessThanOrEqual(96)
    expect(event.blobs?.[1]?.length).toBeLessThanOrEqual(512)
  })

  test("swallows a writeDataPoint failure without throwing", () => {
    const dataset: CompareSoakAnalyticsDataset = {
      writeDataPoint: () => {
        throw new Error("analytics engine unavailable")
      },
    }
    const metrics = makeCompareSoakMetrics(dataset)

    expect(() =>
      metrics.record({ domain: "billing", readKind: "readInvoice", outcome: "match" }),
    ).not.toThrow()
  })

  test("noopCompareSoakMetrics never throws", () => {
    expect(() =>
      noopCompareSoakMetrics.record({ domain: "d", readKind: "k", outcome: "match" }),
    ).not.toThrow()
  })
})
