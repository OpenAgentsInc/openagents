import { describe, expect, test } from "bun:test"

import { availableSteerVerbs } from "./steer-availability.js"

describe("steer availability", () => {
  test("returns running verbs that the node supports", () => {
    expect(availableSteerVerbs({
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
      state: "running",
    })).toEqual({
      verbs: ["pause", "interrupt", "cancel"],
      reason: "state:running;nodeSupports:pause,resume,interrupt,cancel",
    })
  })

  test("returns paused verbs that the node supports", () => {
    expect(availableSteerVerbs({
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
      state: "paused",
    })).toEqual({
      verbs: ["resume", "interrupt", "cancel"],
      reason: "state:paused;nodeSupports:pause,resume,interrupt,cancel",
    })
  })

  test("only includes verbs listed by nodeSupports", () => {
    expect(availableSteerVerbs({
      nodeSupports: ["pause", "cancel"],
      state: "running",
    }).verbs).toEqual(["pause", "cancel"])
  })

  test("filters out unsupported and invalid verbs while preserving protocol order", () => {
    expect(availableSteerVerbs({
      nodeSupports: ["cancel", "unknown", "interrupt", "pause"],
      state: "running",
    }).verbs).toEqual(["pause", "interrupt", "cancel"])
  })

  test("resume is only available while paused", () => {
    expect(availableSteerVerbs({
      nodeSupports: ["resume"],
      state: "running",
    }).verbs).toEqual([])

    expect(availableSteerVerbs({
      nodeSupports: ["resume"],
      state: "paused",
    }).verbs).toEqual(["resume"])
  })

  test("terminal states expose no steer verbs", () => {
    for (const state of ["completed", "failed", "cancelled"] as const) {
      expect(availableSteerVerbs({
        nodeSupports: ["pause", "resume", "interrupt", "cancel"],
        state,
      }).verbs).toEqual([])
    }
  })

  test("returns no verbs when the node lists none", () => {
    expect(availableSteerVerbs({
      nodeSupports: [],
      state: "paused",
    })).toEqual({
      verbs: [],
      reason: "state:paused;nodeSupports:",
    })
  })
})
