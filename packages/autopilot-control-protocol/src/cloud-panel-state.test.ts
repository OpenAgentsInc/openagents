import { describe, expect, test } from "bun:test"

import { cloudPanelState } from "./cloud-panel-state.js"

describe("cloud panel state", () => {
  test("reports no feed honestly without fabricated quota", () => {
    expect(cloudPanelState({ provider: "local" })).toEqual({
      state: "unavailable",
      line: "Cloud metering not available on this node",
      quota: null,
    })
  })

  test("reports non-object input as unavailable", () => {
    expect(cloudPanelState(null)).toEqual({
      state: "unavailable",
      line: "Cloud metering not available on this node",
      quota: null,
    })
  })

  test("projects a fresh root metering feed as available", () => {
    const observedAt = new Date().toISOString()
    const raw = {
      usage: 42,
      observedAt,
      capSats: 100,
    }

    expect(cloudPanelState(raw)).toEqual({
      state: "available",
      line: `Cloud metering available; observed ${observedAt}`,
      quota: raw,
    })
  })

  test("uses nested quota object when available", () => {
    const observedAt = new Date().toISOString()
    const quota = {
      used_sats: 1200,
      observed_at: observedAt,
      cap_sats: 5000,
    }

    expect(cloudPanelState({ cloud_quota: quota })).toEqual({
      state: "available",
      line: `Cloud metering available; observed ${observedAt}`,
      quota,
    })
  })

  test("does not surface quota for malformed metering", () => {
    expect(cloudPanelState({
      quota: {
        usage: "42",
        observedAt: new Date().toISOString(),
      },
    })).toEqual({
      state: "unavailable",
      line: "Cloud metering unavailable: malformed feed",
      quota: null,
    })
  })

  test("does not surface quota for stale metering", () => {
    const observedAt = "2026-06-13T12:00:00.000Z"

    expect(cloudPanelState({
      usage: 42,
      observedAt,
    })).toEqual({
      state: "unavailable",
      line: `Cloud metering unavailable: stale feed observed ${observedAt}`,
      quota: null,
    })
  })
})
