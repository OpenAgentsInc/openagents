import { describe, expect, test } from "bun:test"

import {
  DEFAULT_WEDGE_THRESHOLD_MS,
  evaluateFleetLiveness,
  FLEET_LIVENESS_EXIT,
  parseLastDispatchTime,
} from "./fleet-liveness.js"

describe("evaluateFleetLiveness (#6646 wedge detection)", () => {
  const now = 1_000_000_000_000

  test("alive but last dispatch older than threshold is WEDGED", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: now - (DEFAULT_WEDGE_THRESHOLD_MS + 60_000),
      now,
    })
    expect(r.status).toBe("wedged")
    expect(r.wedged).toBe(true)
    expect(r.healthy).toBe(false)
    expect(FLEET_LIVENESS_EXIT[r.status]).toBe(3)
  })

  test("alive with a fresh dispatch is HEALTHY", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: now - 30_000,
      now,
    })
    expect(r.status).toBe("healthy")
    expect(r.healthy).toBe(true)
    expect(r.wedged).toBe(false)
    expect(FLEET_LIVENESS_EXIT[r.status]).toBe(0)
  })

  test("dispatch age exactly at threshold is HEALTHY (boundary)", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: now - DEFAULT_WEDGE_THRESHOLD_MS,
      now,
    })
    expect(r.status).toBe("healthy")
  })

  test("just over threshold is WEDGED (boundary)", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: now - (DEFAULT_WEDGE_THRESHOLD_MS + 1),
      now,
    })
    expect(r.status).toBe("wedged")
  })

  test("a dead/stopped process is UNKNOWN, not wedged", () => {
    const r = evaluateFleetLiveness({
      pidAlive: false,
      lastDispatchTime: now - 10 * DEFAULT_WEDGE_THRESHOLD_MS,
      now,
    })
    expect(r.status).toBe("unknown")
    expect(r.wedged).toBe(false)
    expect(FLEET_LIVENESS_EXIT[r.status]).toBe(4)
  })

  test("no dispatch recorded yet is UNKNOWN", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: null,
      now,
    })
    expect(r.status).toBe("unknown")
    expect(r.wedged).toBe(false)
  })

  test("a custom wedge threshold is honored", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: now - 5_000,
      now,
      wedgeThresholdMs: 1_000,
    })
    expect(r.status).toBe("wedged")
    expect(r.wedgeThresholdMs).toBe(1_000)
  })

  test("a non-positive threshold override falls back to the default", () => {
    const r = evaluateFleetLiveness({
      pidAlive: true,
      lastDispatchTime: now - 30_000,
      now,
      wedgeThresholdMs: 0,
    })
    expect(r.wedgeThresholdMs).toBe(DEFAULT_WEDGE_THRESHOLD_MS)
    expect(r.status).toBe("healthy")
  })
})

describe("parseLastDispatchTime", () => {
  test("parses epoch ms", () => {
    expect(parseLastDispatchTime("1719500000000")).toBe(1719500000000)
  })

  test("parses ISO-8601", () => {
    const iso = "2026-06-27T00:00:00.000Z"
    expect(parseLastDispatchTime(iso)).toBe(Date.parse(iso))
  })

  test("empty / whitespace / garbage / null -> null", () => {
    expect(parseLastDispatchTime("")).toBeNull()
    expect(parseLastDispatchTime("   ")).toBeNull()
    expect(parseLastDispatchTime("not-a-time")).toBeNull()
    expect(parseLastDispatchTime(null)).toBeNull()
    expect(parseLastDispatchTime(undefined)).toBeNull()
  })
})
