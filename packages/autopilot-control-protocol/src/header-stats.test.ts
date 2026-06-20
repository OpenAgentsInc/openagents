import { describe, expect, test } from "bun:test"

import { headerStats } from "./header-stats.js"

describe("header stats", () => {
  test("summarizes an empty header row", () => {
    expect(headerStats({
      sessions: [],
      pendingDecisions: 0,
      unreadNotifs: 0,
    })).toEqual({
      running: 0,
      total: 0,
      badges: [
        { label: "sessions: 0/0 running", tone: "idle" },
      ],
    })
  })

  test("counts running sessions out of total sessions", () => {
    expect(headerStats({
      sessions: [
        { state: "running" },
        { state: "completed" },
        { state: "failed" },
        { state: "running" },
      ],
      pendingDecisions: 0,
      unreadNotifs: 0,
    })).toEqual({
      running: 2,
      total: 4,
      badges: [
        { label: "sessions: 2/4 running", tone: "running" },
      ],
    })
  })

  test("normalizes running session casing and whitespace", () => {
    expect(headerStats({
      sessions: [
        { state: " RUNNING " },
        { state: "Running" },
        { state: "queued" },
      ],
      pendingDecisions: 0,
      unreadNotifs: 0,
    })).toEqual({
      running: 2,
      total: 3,
      badges: [
        { label: "sessions: 2/3 running", tone: "running" },
      ],
    })
  })

  test("adds a singular pending decision badge", () => {
    expect(headerStats({
      sessions: [{ state: "completed" }],
      pendingDecisions: 1,
      unreadNotifs: 0,
    })).toEqual({
      running: 0,
      total: 1,
      badges: [
        { label: "sessions: 0/1 running", tone: "idle" },
        { label: "decisions: 1 pending", tone: "warn" },
      ],
    })
  })

  test("adds plural pending decision and unread notification badges", () => {
    expect(headerStats({
      sessions: [{ state: "running" }],
      pendingDecisions: 3,
      unreadNotifs: 2,
    })).toEqual({
      running: 1,
      total: 1,
      badges: [
        { label: "sessions: 1/1 running", tone: "running" },
        { label: "decisions: 3 pending", tone: "warn" },
        { label: "notifications: 2 unread", tone: "ok" },
      ],
    })
  })

  test("omits zero actionable badges", () => {
    expect(headerStats({
      sessions: [
        { state: "completed" },
        { state: "cancelled" },
      ],
      pendingDecisions: 0,
      unreadNotifs: 0,
    })).toEqual({
      running: 0,
      total: 2,
      badges: [
        { label: "sessions: 0/2 running", tone: "idle" },
      ],
    })
  })

  test("floors positive counts and ignores invalid counts", () => {
    expect(headerStats({
      sessions: [{ state: "running" }],
      pendingDecisions: 2.9,
      unreadNotifs: Number.NaN,
    })).toEqual({
      running: 1,
      total: 1,
      badges: [
        { label: "sessions: 1/1 running", tone: "running" },
        { label: "decisions: 2 pending", tone: "warn" },
      ],
    })
  })
})
