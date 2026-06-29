import { describe, expect, test } from "bun:test"

import { buildCloudSummary } from "./cloud-quota-summary.js"

describe("cloud quota summary", () => {
  test("summarizes primary active provider, standby, and estimated cost", () => {
    expect(buildCloudSummary({
      accounts: [
        { provider: "openai", ready: true },
        { provider: "anthropic", ready: true },
      ],
      usage: {
        tokensIn: 2_000_000,
        tokensOut: 500_000,
        ratePerMTokIn: 3,
        ratePerMTokOut: 12,
      },
    })).toEqual({
      active: "openai",
      standby: ["anthropic"],
      failedOver: false,
      costSats: 12,
      line: "active: openai; standby: anthropic; est. cost: 12 sats",
    })
  })

  test("marks the active provider as failover when primary is limited", () => {
    expect(buildCloudSummary({
      accounts: [
        { provider: "openai", ready: true, limited: true },
        { provider: "anthropic", ready: true },
      ],
      usage: {
        tokensIn: 1_000_000,
        tokensOut: 1_000_000,
        ratePerMTokIn: 4,
        ratePerMTokOut: 8,
      },
    })).toEqual({
      active: "anthropic",
      standby: [],
      failedOver: true,
      costSats: 12,
      line: "active: anthropic (failover); standby: none; est. cost: 12 sats",
    })
  })

  test("keeps only ready unlimited providers in standby", () => {
    expect(buildCloudSummary({
      accounts: [
        { provider: "openai", ready: true },
        { provider: "anthropic", ready: true, limited: true },
        { provider: "local", ready: false },
        { provider: "groq", ready: true },
      ],
      usage: {
        tokensIn: 333_333,
        tokensOut: 333_333,
        ratePerMTokIn: 3,
        ratePerMTokOut: 6,
      },
    })).toEqual({
      active: "openai",
      standby: ["groq"],
      failedOver: false,
      costSats: 3,
      line: "active: openai; standby: groq; est. cost: 3 sats",
    })
  })

  test("summarizes unavailable providers with no active account", () => {
    expect(buildCloudSummary({
      accounts: [
        { provider: "openai", ready: false },
        { provider: "anthropic", ready: true, limited: true },
      ],
      usage: {
        tokensIn: -1_000_000,
        tokensOut: 1_000_000,
        ratePerMTokIn: 100,
        ratePerMTokOut: -10,
      },
    })).toEqual({
      active: null,
      standby: [],
      failedOver: true,
      costSats: 0,
      line: "active: none; standby: none; est. cost: 0 sats",
    })
  })

  test("returns a stable empty summary for no accounts", () => {
    expect(buildCloudSummary({
      accounts: [],
      usage: {
        tokensIn: 0,
        tokensOut: 0,
        ratePerMTokIn: 10,
        ratePerMTokOut: 20,
      },
    })).toEqual({
      active: null,
      standby: [],
      failedOver: false,
      costSats: 0,
      line: "active: none; standby: none; est. cost: 0 sats",
    })
  })

  test("normalizes provider whitespace in the display line", () => {
    expect(buildCloudSummary({
      accounts: [
        { provider: " openai\nprimary ", ready: true },
        { provider: " anthropic\tstandby ", ready: true },
      ],
      usage: {
        tokensIn: Number.POSITIVE_INFINITY,
        tokensOut: 1_000_000,
        ratePerMTokIn: 10,
        ratePerMTokOut: Number.NEGATIVE_INFINITY,
      },
    })).toEqual({
      active: "openai\nprimary",
      standby: ["anthropic\tstandby"],
      failedOver: false,
      costSats: 0,
      line: "active: openai primary; standby: anthropic standby; est. cost: 0 sats",
    })
  })
})
