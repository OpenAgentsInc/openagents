import { describe, expect, test } from "bun:test"

import { applySteerVerb } from "./steer-verbs.js"

describe("steer verb state machine", () => {
  test("pauses a running session", () => {
    expect(applySteerVerb("running", "pause")).toEqual({
      state: "paused",
      accepted: true,
      reason: "paused",
    })
  })

  test("rejects pause when already paused", () => {
    expect(applySteerVerb("paused", "pause")).toEqual({
      state: "paused",
      accepted: false,
      reason: "pause_requires_running",
    })
  })

  test("resumes a paused session", () => {
    expect(applySteerVerb("paused", "resume")).toEqual({
      state: "running",
      accepted: true,
      reason: "resumed",
    })
  })

  test("rejects resume while running", () => {
    expect(applySteerVerb("running", "resume")).toEqual({
      state: "running",
      accepted: false,
      reason: "resume_requires_paused",
    })
  })

  test("accepts interrupt from running as steer injection", () => {
    expect(applySteerVerb("running", "interrupt")).toEqual({
      state: "running",
      accepted: true,
      reason: "interrupt_injected",
    })
  })

  test("accepts interrupt from paused and returns to running", () => {
    expect(applySteerVerb("paused", "interrupt")).toEqual({
      state: "running",
      accepted: true,
      reason: "interrupt_injected",
    })
  })

  test("cancels from running or paused", () => {
    expect(applySteerVerb("running", "cancel")).toEqual({
      state: "cancelled",
      accepted: true,
      reason: "cancelled",
    })
    expect(applySteerVerb("paused", "cancel")).toEqual({
      state: "cancelled",
      accepted: true,
      reason: "cancelled",
    })
  })

  test("rejects all verbs after cancellation", () => {
    expect(applySteerVerb("cancelled", "pause")).toEqual({
      state: "cancelled",
      accepted: false,
      reason: "session_cancelled",
    })
    expect(applySteerVerb("cancelled", "resume")).toEqual({
      state: "cancelled",
      accepted: false,
      reason: "session_cancelled",
    })
    expect(applySteerVerb("cancelled", "interrupt")).toEqual({
      state: "cancelled",
      accepted: false,
      reason: "session_cancelled",
    })
    expect(applySteerVerb("cancelled", "cancel")).toEqual({
      state: "cancelled",
      accepted: false,
      reason: "session_cancelled",
    })
  })
})
