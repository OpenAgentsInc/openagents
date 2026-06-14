// CL-55 unit tests for the pure stateBreakdown helper in panes/sessions.ts.
// No DOM required — this is a pure function over plain objects.

import { describe, expect, test } from "bun:test"
import { stateBreakdown } from "../src/ui/panes/sessions"

describe("stateBreakdown", () => {
  test("returns empty string for an empty array", () => {
    expect(stateBreakdown([])).toBe("")
  })

  test("single session", () => {
    expect(stateBreakdown([{ state: "running" }])).toBe("1 running")
  })

  test("multiple sessions with the same state", () => {
    expect(stateBreakdown([{ state: "running" }, { state: "running" }, { state: "running" }])).toBe("3 running")
  })

  test("multiple distinct states produce a dot-separated line", () => {
    const sessions = [
      { state: "running" },
      { state: "running" },
      { state: "running" },
      { state: "failed" },
    ]
    const result = stateBreakdown(sessions)
    expect(result).toContain("3 running")
    expect(result).toContain("1 failed")
    expect(result).toContain("·")
  })

  test("does not include states with zero sessions", () => {
    const result = stateBreakdown([{ state: "completed" }, { state: "completed" }])
    expect(result).toBe("2 completed")
    expect(result).not.toContain("running")
    expect(result).not.toContain("failed")
  })

  test("handles all canonical states together", () => {
    const sessions = [
      { state: "running" },
      { state: "queued" },
      { state: "completed" },
      { state: "failed" },
      { state: "cancelled" },
    ]
    const result = stateBreakdown(sessions)
    expect(result).toContain("1 running")
    expect(result).toContain("1 queued")
    expect(result).toContain("1 completed")
    expect(result).toContain("1 failed")
    expect(result).toContain("1 cancelled")
  })

  test("handles unknown/custom states gracefully", () => {
    const result = stateBreakdown([{ state: "paused" }, { state: "paused" }])
    expect(result).toBe("2 paused")
  })
})
