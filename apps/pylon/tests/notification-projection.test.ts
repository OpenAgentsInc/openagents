import { describe, expect, test } from "bun:test"
import {
  projectNotification,
  shouldNotify,
} from "../src/node/notification-projection"

describe("node notification projection", () => {
  test("projects decision requests as high-priority decision notifications", () => {
    const payload = projectNotification({
      type: "decision_required",
      observedAt: "2026-06-13T14:00:00.000Z",
      sessionRef: "session.fixture.decision",
      decisionRef: "decision.fixture.approve",
    })

    expect(payload).toEqual({
      kind: "decision_required",
      title: "Decision required",
      body: "decision.fixture.approve",
      sessionRef: "session.fixture.decision",
      observedAt: "2026-06-13T14:00:00.000Z",
      priority: "high",
    })
    expect(shouldNotify({
      type: "decision_required",
      observedAt: "2026-06-13T14:00:00.000Z",
    })).toBe(true)
  })

  test("projects failed sessions as high-priority failure notifications", () => {
    const payload = projectNotification({
      type: "session.failed",
      observedAt: "2026-06-13T14:01:00.000Z",
      sessionRef: "session.fixture.failed",
      reason: "agent exited with code 1",
    })

    expect(payload).toEqual({
      kind: "session_failed",
      title: "Session failed",
      body: "agent exited with code 1",
      sessionRef: "session.fixture.failed",
      observedAt: "2026-06-13T14:01:00.000Z",
      priority: "high",
    })
  })

  test("projects completed sessions as normal-priority completion notifications", () => {
    const payload = projectNotification({
      type: "completed",
      observedAt: "2026-06-13T14:02:00.000Z",
      sessionRef: "session.fixture.completed",
    })

    expect(payload).toEqual({
      kind: "session_completed",
      title: "Session completed",
      body: "The session completed successfully.",
      sessionRef: "session.fixture.completed",
      observedAt: "2026-06-13T14:02:00.000Z",
      priority: "normal",
    })
  })

  test("projects generic attention events with caller-provided copy", () => {
    const payload = projectNotification({
      type: "attention.required",
      observedAt: "2026-06-13T14:03:00.000Z",
      title: "Quota needs review",
      body: "The node needs a quota decision before dispatching more work.",
    })

    expect(payload).toEqual({
      kind: "attention",
      title: "Quota needs review",
      body: "The node needs a quota decision before dispatching more work.",
      observedAt: "2026-06-13T14:03:00.000Z",
      priority: "normal",
    })
  })

  test("filters non-notification node events", () => {
    const event = {
      type: "wallet",
      observedAt: "2026-06-13T14:04:00.000Z",
      message: "wallet balance changed",
    }

    expect(shouldNotify(event)).toBe(false)
    expect(() => projectNotification(event)).toThrow("unsupported notification event: wallet")
  })

  test("passes observedAt through from input without reading the clock", () => {
    const payload = projectNotification({
      type: "session_completed",
      observedAt: "1999-12-31T23:59:59.000Z",
      sessionRef: "session.fixture.clock",
    })

    expect(payload.observedAt).toBe("1999-12-31T23:59:59.000Z")
  })
})
