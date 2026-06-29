import { describe, expect, test } from "bun:test"

import { estimateCloudCost } from "./cloud-cost-estimate.js"

describe("cloud cost estimate", () => {
  test("estimates input and output token cost in sats", () => {
    expect(estimateCloudCost({
      tokensIn: 2_000_000,
      tokensOut: 500_000,
      ratePerMTokIn: 3,
      ratePerMTokOut: 12,
    })).toEqual({
      costSats: 12,
      breakdown: {
        in: 6,
        out: 6,
      },
    })
  })

  test("rounds each side to integer sats", () => {
    expect(estimateCloudCost({
      tokensIn: 333_333,
      tokensOut: 333_333,
      ratePerMTokIn: 3,
      ratePerMTokOut: 6,
    })).toEqual({
      costSats: 3,
      breakdown: {
        in: 1,
        out: 2,
      },
    })
  })

  test("returns zero for zero usage", () => {
    expect(estimateCloudCost({
      tokensIn: 0,
      tokensOut: 0,
      ratePerMTokIn: 10,
      ratePerMTokOut: 20,
    })).toEqual({
      costSats: 0,
      breakdown: {
        in: 0,
        out: 0,
      },
    })
  })

  test("treats negative values as zero", () => {
    expect(estimateCloudCost({
      tokensIn: -1_000_000,
      tokensOut: 1_000_000,
      ratePerMTokIn: 100,
      ratePerMTokOut: -10,
    })).toEqual({
      costSats: 0,
      breakdown: {
        in: 0,
        out: 0,
      },
    })
  })

  test("treats NaN values as zero", () => {
    expect(estimateCloudCost({
      tokensIn: Number.NaN,
      tokensOut: 1_000_000,
      ratePerMTokIn: 10,
      ratePerMTokOut: Number.NaN,
    })).toEqual({
      costSats: 0,
      breakdown: {
        in: 0,
        out: 0,
      },
    })
  })

  test("treats infinite values as zero", () => {
    expect(estimateCloudCost({
      tokensIn: Number.POSITIVE_INFINITY,
      tokensOut: 1_000_000,
      ratePerMTokIn: 10,
      ratePerMTokOut: Number.NEGATIVE_INFINITY,
    })).toEqual({
      costSats: 0,
      breakdown: {
        in: 0,
        out: 0,
      },
    })
  })
})
