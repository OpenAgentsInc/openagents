import { describe, expect, test } from "bun:test"
import { sessionEventStreamFixture } from "@openagentsinc/autopilot-control-protocol/fixtures"
import {
  projectNotification,
  shouldDeliver,
  type NotificationPayload,
  type QuietHours,
} from "../src/notifications/notification-projection"

const quietHours: QuietHours = {
  enabled: true,
  startHour: 22,
  endHour: 7,
}

describe("notification projection", () => {
  test("projects notable session events to refs-only notification payloads", () => {
    const decisionRequested = projectNotification(sessionEventStreamFixture[2]!)
    const decisionResolved = projectNotification(sessionEventStreamFixture[3]!)
    const completed = projectNotification(sessionEventStreamFixture[4]!)
    const failed = projectNotification({
      ...sessionEventStreamFixture[4]!,
      eventId: "evt.failed",
      phase: "failed",
      detailRef: "failure.fixture.0001",
    })

    expect(decisionRequested).toEqual({
      kind: "decision_requested",
      title: "Decision requested",
      sessionRef: "session.pylon.codex_composer.fixture0001",
      detailRef: "decision.fixture.req01",
      decisionRef: "decision.fixture.req01",
    })
    expect(decisionResolved.kind).toBe("decision_resolved")
    expect(decisionResolved.decisionRef).toBe("decision.fixture.req01")
    expect(completed).toEqual({
      kind: "completed",
      title: "Session completed",
      sessionRef: "session.pylon.codex_composer.fixture0001",
    })
    expect(failed).toEqual({
      kind: "failed",
      title: "Session failed",
      sessionRef: "session.pylon.codex_composer.fixture0001",
      detailRef: "failure.fixture.0001",
    })
  })

  test("projects a decision request without carrying raw private content", () => {
    const rawPrompt = "delete the private branch at /Users/example/worktree"
    const payload = projectNotification({
      sessionRef: "session.pylon.codex.fixture0002",
      requestId: "decision.fixture.req02",
      actionRef: "action.fixture.approve_release",
      rawPrompt,
    } as Parameters<typeof projectNotification>[0])

    expect(payload).toEqual({
      kind: "decision_requested",
      title: "Decision requested",
      sessionRef: "session.pylon.codex.fixture0002",
      detailRef: "action.fixture.approve_release",
      decisionRef: "decision.fixture.req02",
    })
    expect(JSON.stringify(payload)).not.toContain(rawPrompt)
    expect(Object.keys(payload).sort()).toEqual([
      "decisionRef",
      "detailRef",
      "kind",
      "sessionRef",
      "title",
    ])
  })
})

describe("quiet-hours delivery", () => {
  test("suppresses non-urgent notifications during quiet hours", () => {
    const payload: NotificationPayload = {
      kind: "completed",
      title: "Session completed",
      sessionRef: "session.fixture",
    }

    expect(shouldDeliver(payload, new Date("2026-06-13T23:30:00"), quietHours)).toBe(false)
    expect(shouldDeliver(payload, new Date("2026-06-13T03:30:00"), quietHours)).toBe(false)
    expect(shouldDeliver(payload, new Date("2026-06-13T12:30:00"), quietHours)).toBe(true)
  })

  test("always delivers decision requests during quiet hours", () => {
    const payload: NotificationPayload = {
      kind: "decision_requested",
      title: "Decision requested",
      sessionRef: "session.fixture",
      decisionRef: "decision.fixture.req03",
    }

    expect(shouldDeliver(payload, new Date("2026-06-13T23:30:00"), quietHours)).toBe(true)
  })
})
