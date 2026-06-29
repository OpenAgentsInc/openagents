import { describe, expect, test } from "bun:test"

import { steerDisabledReason } from "./steer-disabled-reason.js"

describe("steer disabled reason", () => {
  test("enables pause for a running session when the node supports it", () => {
    expect(steerDisabledReason("pause", {
      nodeSupports: ["pause"],
      state: "running",
    })).toEqual({
      enabled: true,
      reason: "available",
    })
  })

  test("explains missing node pause support", () => {
    expect(steerDisabledReason("pause", {
      nodeSupports: ["resume", "interrupt", "cancel"],
      state: "running",
    })).toEqual({
      enabled: false,
      reason: "node does not support pause yet",
    })
  })

  test("explains pause state requirements", () => {
    expect(steerDisabledReason("pause", {
      nodeSupports: ["pause"],
      state: "paused",
    })).toEqual({
      enabled: false,
      reason: "only running sessions can be paused",
    })
  })

  test("enables resume only while paused", () => {
    expect(steerDisabledReason("resume", {
      nodeSupports: ["resume"],
      state: "paused",
    })).toEqual({
      enabled: true,
      reason: "available",
    })
  })

  test("explains resume state requirements", () => {
    expect(steerDisabledReason("resume", {
      nodeSupports: ["resume"],
      state: "running",
    })).toEqual({
      enabled: false,
      reason: "only paused sessions can be resumed",
    })
  })

  test("enables interrupt while paused", () => {
    expect(steerDisabledReason("interrupt", {
      nodeSupports: ["interrupt"],
      state: "paused",
    })).toEqual({
      enabled: true,
      reason: "available",
    })
  })

  test("explains terminal cancel state requirements", () => {
    expect(steerDisabledReason("cancel", {
      nodeSupports: ["cancel"],
      state: "completed",
    })).toEqual({
      enabled: false,
      reason: "only running sessions can be cancelled",
    })
  })
})
