import { describe, expect, test } from "bun:test"

import {
  sessionEventStreamFixture,
  sessionListFixture,
} from "@openagentsinc/autopilot-control-protocol/fixtures"

import {
  nodeStatusLineViewModel,
  sessionRowsViewModel,
  sessionTimelineRowsViewModel,
} from "./session-view-model"

describe("session view model", () => {
  test("maps shared session summaries to desktop-shaped row data", () => {
    expect(sessionRowsViewModel(sessionListFixture)).toEqual([
      {
        sessionRef: "session.pylon.codex_composer.fixture0001",
        adapter: "codex",
        state: "running",
        stateClassName: "state-running",
        lastProgressRef: "progress.fixture.0001",
      },
      {
        sessionRef: "session.pylon.claude_composer.fixture0002",
        adapter: "claude_agent",
        state: "completed",
        stateClassName: "state-completed",
        lastProgressRef: "none",
      },
    ])
  })

  test("mirrors the desktop node status line text", () => {
    expect(nodeStatusLineViewModel({ ok: true, sessions: sessionListFixture })).toEqual({
      status: "connected",
      sessionCount: 2,
      text: "connected · 2 sessions",
    })
    expect(nodeStatusLineViewModel({ ok: false, sessions: [sessionListFixture[0]!] }).text).toBe("offline · 1 session")
  })

  test("maps timeline events into ordered public-safe rows", () => {
    const rows = sessionTimelineRowsViewModel([...sessionEventStreamFixture].reverse())

    expect(rows.map((row) => row.sequence)).toEqual([1, 2, 3, 4, 5])
    expect(rows[2]).toEqual({
      eventId: "evt.0003",
      sessionRef: "session.pylon.codex_composer.fixture0001",
      sequence: 3,
      phase: "decision_requested",
      phaseLabel: "decision requested",
      projectionLevel: "public_safe",
      observedAt: "2026-06-13T12:00:10.000Z",
      detailRef: "decision.fixture.req01",
    })
    expect(rows[4]?.detailRef).toBe("none")
  })
})
