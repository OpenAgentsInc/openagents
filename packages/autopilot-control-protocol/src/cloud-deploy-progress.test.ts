import { describe, expect, test } from "bun:test"

import { projectDeployProgress } from "./cloud-deploy-progress.js"

describe("cloud deploy progress projection", () => {
  test("returns idle progress for no events", () => {
    expect(projectDeployProgress([])).toEqual({
      percent: 0,
      phase: "",
      done: false,
      failed: false,
    })
  })

  test("maps queued events to 10 percent", () => {
    expect(projectDeployProgress([
      { state: "queued", at: "2026-06-13T12:00:00.000Z" },
    ])).toEqual({
      percent: 10,
      phase: "queued",
      done: false,
      failed: false,
    })
  })

  test("maps building events to 60 percent", () => {
    expect(projectDeployProgress([
      { state: "building", at: "2026-06-13T12:01:00.000Z" },
    ])).toEqual({
      percent: 60,
      phase: "building",
      done: false,
      failed: false,
    })
  })

  test("maps deployed events to done progress", () => {
    expect(projectDeployProgress([
      { state: "queued", at: "2026-06-13T12:00:00.000Z" },
      { state: "building", at: "2026-06-13T12:01:00.000Z" },
      { state: "deployed", at: "2026-06-13T12:02:00.000Z" },
    ])).toEqual({
      percent: 100,
      phase: "deployed",
      done: true,
      failed: false,
    })
  })

  test("maps failed events to failed progress", () => {
    expect(projectDeployProgress([
      { state: "queued", at: "2026-06-13T12:00:00.000Z" },
      { state: "failed", at: "2026-06-13T12:02:00.000Z" },
    ])).toEqual({
      percent: 100,
      phase: "failed",
      done: false,
      failed: true,
    })
  })

  test("ignores unknown states and keeps the latest known progress", () => {
    expect(projectDeployProgress([
      { state: "queued", at: "2026-06-13T12:00:00.000Z" },
      { state: "waiting", at: "2026-06-13T12:01:00.000Z" },
      { state: "BUILDING", at: "2026-06-13T12:02:00.000Z" },
      { state: "unknown", at: "2026-06-13T12:03:00.000Z" },
    ])).toEqual({
      percent: 60,
      phase: "building",
      done: false,
      failed: false,
    })
  })
})
