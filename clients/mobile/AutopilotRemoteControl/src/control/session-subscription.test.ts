import { describe, expect, test } from "bun:test"

import {
  sessionEventStreamFixture,
  sessionListFixture,
} from "@openagentsinc/autopilot-control-protocol/fixtures"

import { createSessionSubscription } from "./session-subscription"

describe("session subscription", () => {
  test("list populates sessions", () => {
    const subscription = createSessionSubscription()

    expect(subscription.applyList(sessionListFixture)).toEqual(sessionListFixture)
    expect(subscription.selectRows().sessions).toEqual([
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

  test("event batch appends in order and dedups a replayed event", () => {
    const subscription = createSessionSubscription()
    subscription.applyList(sessionListFixture)

    expect(subscription.applyEventBatch([...sessionEventStreamFixture].reverse())).toEqual(sessionEventStreamFixture)
    expect(subscription.applyEventBatch([sessionEventStreamFixture[2]!])).toEqual([])

    const timeline = subscription.selectRows().timelinesBySessionRef["session.pylon.codex_composer.fixture0001"]
    expect(timeline?.map((row) => row.eventId)).toEqual(["evt.0001", "evt.0002", "evt.0003", "evt.0004", "evt.0005"])
  })

  test("selectRows maps to view-model rows", () => {
    const subscription = createSessionSubscription()
    subscription.applyList(sessionListFixture)
    subscription.applyEventBatch(sessionEventStreamFixture)

    expect(subscription.selectRows()).toEqual({
      sessions: [
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
      ],
      timelinesBySessionRef: {
        "session.pylon.codex_composer.fixture0001": [
          {
            eventId: "evt.0001",
            sessionRef: "session.pylon.codex_composer.fixture0001",
            sequence: 1,
            phase: "started",
            phaseLabel: "started",
            projectionLevel: "public_safe",
            observedAt: "2026-06-13T12:00:00.000Z",
            detailRef: "none",
          },
          {
            eventId: "evt.0002",
            sessionRef: "session.pylon.codex_composer.fixture0001",
            sequence: 2,
            phase: "progress",
            phaseLabel: "progress",
            projectionLevel: "public_safe",
            observedAt: "2026-06-13T12:00:05.000Z",
            detailRef: "progress.fixture.0002",
          },
          {
            eventId: "evt.0003",
            sessionRef: "session.pylon.codex_composer.fixture0001",
            sequence: 3,
            phase: "decision_requested",
            phaseLabel: "decision requested",
            projectionLevel: "public_safe",
            observedAt: "2026-06-13T12:00:10.000Z",
            detailRef: "decision.fixture.req01",
          },
          {
            eventId: "evt.0004",
            sessionRef: "session.pylon.codex_composer.fixture0001",
            sequence: 4,
            phase: "decision_resolved",
            phaseLabel: "decision resolved",
            projectionLevel: "public_safe",
            observedAt: "2026-06-13T12:00:20.000Z",
            detailRef: "decision.fixture.req01",
          },
          {
            eventId: "evt.0005",
            sessionRef: "session.pylon.codex_composer.fixture0001",
            sequence: 5,
            phase: "completed",
            phaseLabel: "completed",
            projectionLevel: "public_safe",
            observedAt: "2026-06-13T12:00:30.000Z",
            detailRef: "none",
          },
        ],
        "session.pylon.claude_composer.fixture0002": [],
      },
    })
  })
})
