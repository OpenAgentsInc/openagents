import { describe, expect, test } from "bun:test"

import { applySteer } from "../src/coordinator/steer-contract"

describe("steer contract", () => {
  test("accepts steer while running without changing state", () => {
    expect(applySteer("running", "steer")).toEqual({
      next: "running",
      accepted: true,
      reason: "Steer accepted while session is running.",
    })
  })

  test("accepts steer while paused without changing state", () => {
    expect(applySteer("paused", "steer")).toEqual({
      next: "paused",
      accepted: true,
      reason: "Steer accepted while session is paused.",
    })
  })

  test("accepts pause only from running", () => {
    expect(applySteer("running", "pause")).toEqual({
      next: "paused",
      accepted: true,
      reason: "Pause accepted from running.",
    })
  })

  test("accepts resume only from paused", () => {
    expect(applySteer("paused", "resume")).toEqual({
      next: "running",
      accepted: true,
      reason: "Resume accepted from paused.",
    })
  })

  test("accepts interrupt from running", () => {
    expect(applySteer("running", "interrupt")).toEqual({
      next: "interrupted",
      accepted: true,
      reason: "Interrupt accepted from running.",
    })
  })

  test("accepts interrupt from paused", () => {
    expect(applySteer("paused", "interrupt")).toEqual({
      next: "interrupted",
      accepted: true,
      reason: "Interrupt accepted from paused.",
    })
  })

  test("rejects pause outside running", () => {
    expect(applySteer("paused", "pause")).toEqual({
      next: "paused",
      accepted: false,
      reason:
        "Illegal steer transition: pause from paused; pause is only accepted from running.",
    })
  })

  test("rejects resume outside paused", () => {
    expect(applySteer("running", "resume")).toEqual({
      next: "running",
      accepted: false,
      reason:
        "Illegal steer transition: resume from running; resume is only accepted from paused.",
    })
  })

  test("rejects steer after terminal states", () => {
    expect(applySteer("completed", "steer")).toEqual({
      next: "completed",
      accepted: false,
      reason:
        "Illegal steer transition: steer from completed; steer is only accepted while running or paused.",
    })
  })

  test("rejects interrupt after terminal states", () => {
    expect(applySteer("completed", "interrupt")).toEqual({
      next: "completed",
      accepted: false,
      reason:
        "Illegal steer transition: interrupt from completed; interrupt is only accepted from running or paused.",
    })
  })
})
