import { describe, expect, test } from "bun:test"
import { reduceSessionState } from "./session-state-reducer.js"

describe("session state reducer", () => {
  test("returns idle with a blank last phase for no events", () => {
    expect(reduceSessionState([])).toEqual({ state: "idle", lastPhase: "" })
  })

  test("returns running for non-terminal activity", () => {
    expect(reduceSessionState([{ phase: "started" }])).toEqual({
      state: "running",
      lastPhase: "started",
    })
  })

  test("keeps running across multiple non-terminal phases", () => {
    expect(reduceSessionState([{ phase: "started" }, { phase: "thinking" }, { phase: "streaming" }])).toEqual({
      state: "running",
      lastPhase: "streaming",
    })
  })

  test("returns failed for a failed terminal phase", () => {
    expect(reduceSessionState([{ phase: "started" }, { phase: "failed" }])).toEqual({
      state: "failed",
      lastPhase: "failed",
    })
  })

  test("returns cancelled for a cancelled terminal phase", () => {
    expect(reduceSessionState([{ phase: "started" }, { phase: "cancelled" }])).toEqual({
      state: "cancelled",
      lastPhase: "cancelled",
    })
  })

  test("returns completed for a completed terminal phase", () => {
    expect(reduceSessionState([{ phase: "started" }, { phase: "completed" }])).toEqual({
      state: "completed",
      lastPhase: "completed",
    })
  })

  test("uses the last terminal phase as the final state", () => {
    expect(reduceSessionState([
      { phase: "started" },
      { phase: "completed" },
      { phase: "failed" },
      { phase: "cancelled" },
    ])).toEqual({
      state: "cancelled",
      lastPhase: "cancelled",
    })
  })

  test("preserves the actual last phase after a terminal phase", () => {
    expect(reduceSessionState([{ phase: "failed" }, { phase: "cleanup" }])).toEqual({
      state: "failed",
      lastPhase: "cleanup",
    })
  })
})
