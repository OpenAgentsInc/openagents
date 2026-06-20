import { describe, expect, test } from "bun:test"

import { sessionActions } from "./session-action-availability.js"

describe("session action availability", () => {
  test("enables running session actions supported by the node", () => {
    expect(sessionActions({
      state: "running",
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
    })).toEqual({
      cancel: true,
      pause: true,
      resume: false,
      interrupt: true,
      reasons: {
        resume: "only paused sessions can be resumed",
      },
    })
  })

  test("enables paused session actions supported by the node", () => {
    expect(sessionActions({
      state: "paused",
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
    })).toEqual({
      cancel: true,
      pause: false,
      resume: true,
      interrupt: true,
      reasons: {
        pause: "only running sessions can be paused",
      },
    })
  })

  test("explains missing node support before state eligibility", () => {
    expect(sessionActions({
      state: "running",
      nodeSupports: ["pause"],
    })).toEqual({
      cancel: false,
      pause: true,
      resume: false,
      interrupt: false,
      reasons: {
        cancel: "node does not support cancel yet",
        resume: "node does not support resume yet",
        interrupt: "node does not support interrupt yet",
      },
    })
  })

  test("filters unsupported and unknown node capabilities", () => {
    expect(sessionActions({
      state: "paused",
      nodeSupports: ["resume", "unknown"],
    })).toEqual({
      cancel: false,
      pause: false,
      resume: true,
      interrupt: false,
      reasons: {
        cancel: "node does not support cancel yet",
        pause: "node does not support pause yet",
        interrupt: "node does not support interrupt yet",
      },
    })
  })

  test("disables all actions for completed sessions", () => {
    expect(sessionActions({
      state: "completed",
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
    })).toEqual({
      cancel: false,
      pause: false,
      resume: false,
      interrupt: false,
      reasons: {
        cancel: "only running or paused sessions can be cancelled",
        pause: "only running sessions can be paused",
        resume: "only paused sessions can be resumed",
        interrupt: "only running or paused sessions can be interrupted",
      },
    })
  })

  test("disables all actions for cancelled sessions", () => {
    expect(sessionActions({
      state: "cancelled",
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
    })).toEqual({
      cancel: false,
      pause: false,
      resume: false,
      interrupt: false,
      reasons: {
        cancel: "only running or paused sessions can be cancelled",
        pause: "only running sessions can be paused",
        resume: "only paused sessions can be resumed",
        interrupt: "only running or paused sessions can be interrupted",
      },
    })
  })

  test("treats unknown session states as state-disabled when supported", () => {
    expect(sessionActions({
      state: "queued",
      nodeSupports: ["pause", "resume", "interrupt", "cancel"],
    })).toEqual({
      cancel: false,
      pause: false,
      resume: false,
      interrupt: false,
      reasons: {
        cancel: "only running or paused sessions can be cancelled",
        pause: "only running sessions can be paused",
        resume: "only paused sessions can be resumed",
        interrupt: "only running or paused sessions can be interrupted",
      },
    })
  })
})
