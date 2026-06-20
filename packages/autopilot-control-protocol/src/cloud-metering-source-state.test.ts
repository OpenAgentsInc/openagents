import { describe, expect, test } from "bun:test"

import { cloudMeteringState } from "./cloud-metering-source-state.js"

describe("cloud metering source state", () => {
  test("marks a fresh root usage feed available", () => {
    const observedAt = new Date().toISOString()

    expect(cloudMeteringState({
      usage: 42,
      observedAt,
    })).toEqual({
      available: true,
      reason: "ok",
      observedAt,
    })
  })

  test("marks a fresh nested cloud metering feed available", () => {
    const observedAt = new Date().toISOString()

    expect(cloudMeteringState({
      cloud_metering: {
        used_sats: 1200,
        observed_at: observedAt,
      },
    })).toEqual({
      available: true,
      reason: "ok",
      observedAt,
    })
  })

  test("reports no feed for absent metering data", () => {
    expect(cloudMeteringState(null)).toEqual({
      available: false,
      reason: "no_feed",
      observedAt: null,
    })
    expect(cloudMeteringState({ provider: "openai" })).toEqual({
      available: false,
      reason: "no_feed",
      observedAt: null,
    })
  })

  test("reports malformed when usage is present but not numeric", () => {
    expect(cloudMeteringState({
      usage: "42",
      observedAt: new Date().toISOString(),
    })).toEqual({
      available: false,
      reason: "malformed",
      observedAt: null,
    })
  })

  test("reports malformed when observedAt is missing or invalid", () => {
    expect(cloudMeteringState({ usage: 42 })).toEqual({
      available: false,
      reason: "malformed",
      observedAt: null,
    })
    expect(cloudMeteringState({
      usage: 42,
      observedAt: "not a date",
    })).toEqual({
      available: false,
      reason: "malformed",
      observedAt: null,
    })
  })

  test("reports stale for an old but otherwise valid feed", () => {
    const observedAt = "2026-06-13T12:00:00.000Z"

    expect(cloudMeteringState({
      usage: 42,
      observedAt,
    })).toEqual({
      available: false,
      reason: "stale",
      observedAt,
    })
  })

  test("rejects non-finite numeric usage defensively", () => {
    expect(cloudMeteringState({
      usage: Number.POSITIVE_INFINITY,
      observedAt: new Date().toISOString(),
    })).toEqual({
      available: false,
      reason: "malformed",
      observedAt: null,
    })
  })
})
