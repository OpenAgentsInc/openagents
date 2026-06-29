import { describe, expect, test } from "bun:test"

import { buildCloudPanel } from "./cloud-panel-view-model.js"

describe("cloud panel view model", () => {
  test("reports unavailable without fabricating quota or cost", () => {
    expect(buildCloudPanel({ provider: "openai" })).toEqual({
      available: false,
      line: "cloud metering unavailable",
      quota: null,
      costSats: null,
    })
  })

  test("reports malformed metering as unavailable", () => {
    expect(buildCloudPanel({
      usage: "42",
      observedAt: new Date().toISOString(),
      capSats: 100,
    })).toEqual({
      available: false,
      line: "cloud metering unavailable",
      quota: null,
      costSats: null,
    })
  })

  test("reports stale metering as unavailable", () => {
    expect(buildCloudPanel({
      usage: 42,
      observedAt: "2026-06-13T12:00:00.000Z",
      capSats: 100,
    })).toEqual({
      available: false,
      line: "cloud metering unavailable",
      quota: null,
      costSats: null,
    })
  })

  test("builds a quota and cost line from a fresh root payload", () => {
    const panel = buildCloudPanel({
      usedSats: 42,
      capSats: 100,
      observedAt: new Date().toISOString(),
      tokensIn: 2_000_000,
      tokensOut: 500_000,
      ratePerMTokIn: 3,
      ratePerMTokOut: 12,
      failoverState: "primary",
    })

    expect(panel).toEqual({
      available: true,
      line: "cloud metering available; quota: 42/100 sats (42% used, 58 sats remaining); est. cost: 12 sats",
      quota: {
        usedSats: 42,
        capSats: 100,
        remainingSats: 58,
        percentUsed: 42,
        failoverState: "primary",
        blockers: [],
      },
      costSats: 12,
    })
  })

  test("reads nested quota and cost payloads", () => {
    const panel = buildCloudPanel({
      cloud_metering: {
        used_sats: 1200,
        observed_at: new Date().toISOString(),
      },
      quota: {
        used_msats: 1_200_000,
        cap_msats: 2_000_000,
        failover_state: "fallback",
      },
      cost: {
        tokens_in: 333_333,
        tokens_out: 333_333,
        rate_per_mtok_in: 3,
        rate_per_mtok_out: 6,
      },
    })

    expect(panel.available).toBe(true)
    expect(panel.costSats).toBe(3)
    expect(panel.quota).toEqual({
      usedSats: 1200,
      capSats: 2000,
      remainingSats: 800,
      percentUsed: 60,
      failoverState: "failover",
      blockers: [],
    })
    expect(panel.line).toBe(
      "cloud metering available; quota: 1200/2000 sats (60% used, 800 sats remaining); est. cost: 3 sats",
    )
  })

  test("keeps cost unknown when estimate fields are absent", () => {
    const panel = buildCloudPanel({
      usageSats: 12,
      observedAt: new Date().toISOString(),
      capSats: 100,
      failoverState: "ok",
    })

    expect(panel.available).toBe(true)
    expect(panel.costSats).toBe(null)
    expect(panel.line).toBe(
      "cloud metering available; quota: 12/100 sats (12% used, 88 sats remaining); est. cost: unknown",
    )
  })

  test("keeps quota unknown when cap is missing", () => {
    const panel = buildCloudPanel({
      usageSats: 12,
      observedAt: new Date().toISOString(),
      tokensIn: 1_000_000,
      tokensOut: 1_000_000,
      ratePerMTokIn: 4,
      ratePerMTokOut: 8,
    })

    expect(panel.available).toBe(true)
    expect(panel.costSats).toBe(12)
    expect(panel.quota?.capSats).toBe(null)
    expect(panel.quota?.blockers).toContain("cap_sats_unknown")
    expect(panel.line).toBe(
      "cloud metering available; quota: unknown; est. cost: 12 sats",
    )
  })
})
