import { describe, expect, test } from "bun:test"

import { renderCloudCard } from "./cloud-card-render.js"

describe("cloud card render", () => {
  test("stays visible and honest when metering is absent", () => {
    expect(renderCloudCard({ provider: "openai" })).toEqual({
      visible: true,
      title: "Cloud metering",
      body: "Cloud metering is not reported by this node.",
    })
  })

  test("does not fabricate values for malformed metering", () => {
    expect(renderCloudCard({
      usage: "42",
      observedAt: new Date().toISOString(),
      capSats: 100,
    })).toEqual({
      visible: true,
      title: "Cloud metering",
      body: "Cloud metering is not reported by this node.",
    })
  })

  test("does not render stale metering as available", () => {
    expect(renderCloudCard({
      usage: 42,
      observedAt: "2026-06-13T12:00:00.000Z",
      capSats: 100,
    })).toEqual({
      visible: true,
      title: "Cloud metering",
      body: "Cloud metering is not reported by this node.",
    })
  })

  test("summarizes quota and estimated cost when metering is available", () => {
    expect(renderCloudCard({
      usedSats: 42,
      capSats: 100,
      observedAt: new Date().toISOString(),
      tokensIn: 2_000_000,
      tokensOut: 500_000,
      ratePerMTokIn: 3,
      ratePerMTokOut: 12,
      failoverState: "primary",
    })).toEqual({
      visible: true,
      title: "Cloud metering",
      body: "cloud metering available; quota: 42/100 sats (42% used, 58 sats remaining); est. cost: 12 sats",
    })
  })

  test("keeps cost unknown when cost inputs are not reported", () => {
    expect(renderCloudCard({
      usageSats: 12,
      observedAt: new Date().toISOString(),
      capSats: 100,
      failoverState: "ok",
    }).body).toBe(
      "cloud metering available; quota: 12/100 sats (12% used, 88 sats remaining); est. cost: unknown",
    )
  })

  test("keeps quota unknown when quota cap is not reported", () => {
    expect(renderCloudCard({
      usageSats: 12,
      observedAt: new Date().toISOString(),
      tokensIn: 1_000_000,
      tokensOut: 1_000_000,
      ratePerMTokIn: 4,
      ratePerMTokOut: 8,
    }).body).toBe(
      "cloud metering available; quota: unknown; est. cost: 12 sats",
    )
  })
})
