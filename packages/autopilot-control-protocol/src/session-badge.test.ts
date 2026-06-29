import { describe, expect, test } from "bun:test"

import { sessionBadge } from "./session-badge.js"

describe("session badge", () => {
  test("maps running sessions to the running tone", () => {
    expect(sessionBadge("running")).toEqual({
      label: "running",
      tone: "running",
    })
  })

  test("maps completed sessions to the ok tone", () => {
    expect(sessionBadge("completed")).toEqual({
      label: "completed",
      tone: "ok",
    })
  })

  test("maps failed sessions to the error tone", () => {
    expect(sessionBadge("failed")).toEqual({
      label: "failed",
      tone: "error",
    })
  })

  test("maps error sessions to the error tone", () => {
    expect(sessionBadge("error")).toEqual({
      label: "error",
      tone: "error",
    })
  })

  test("maps cancelled sessions to the warn tone", () => {
    expect(sessionBadge("cancelled")).toEqual({
      label: "cancelled",
      tone: "warn",
    })
  })

  test("maps unknown states to the idle tone", () => {
    expect(sessionBadge("queued")).toEqual({
      label: "queued",
      tone: "idle",
    })
  })

  test("maps blank states to the idle tone", () => {
    expect(sessionBadge("")).toEqual({
      label: "",
      tone: "idle",
    })
  })
})
