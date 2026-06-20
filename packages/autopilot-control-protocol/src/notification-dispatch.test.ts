import { describe, expect, test } from "bun:test"

import { buildNotification } from "./notification-dispatch.js"

describe("notification dispatch", () => {
  test("notifies needs_decision as high priority", () => {
    expect(buildNotification({
      phase: "needs_decision",
      sessionRef: "local.alpha",
      messageText: "Choose a deployment target",
    })).toEqual({
      shouldNotify: true,
      title: "Autopilot needs decision",
      body: "local.alpha: Choose a deployment target",
      priority: "high",
    })
  })

  test("notifies needs_approval as high priority", () => {
    expect(buildNotification({
      phase: "needs_approval",
      sessionRef: "cloud.beta",
      messageText: "Approve publish step",
    })).toEqual({
      shouldNotify: true,
      title: "Autopilot needs approval",
      body: "cloud.beta: Approve publish step",
      priority: "high",
    })
  })

  test("notifies failed as high priority", () => {
    expect(buildNotification({
      phase: "failed",
      sessionRef: "bridge.gamma",
      messageText: "Smoke test failed",
    })).toEqual({
      shouldNotify: true,
      title: "Autopilot failed",
      body: "bridge.gamma: Smoke test failed",
      priority: "high",
    })
  })

  test("notifies completed as normal priority", () => {
    expect(buildNotification({
      phase: "completed",
      sessionRef: "local.delta",
      messageText: "Finished verification",
    })).toEqual({
      shouldNotify: true,
      title: "Autopilot completed",
      body: "local.delta: Finished verification",
      priority: "normal",
    })
  })

  test("suppresses routine activity phases", () => {
    for (const phase of ["agent_message", "tool_use", "tool_result", "reasoning"]) {
      expect(buildNotification({
        phase,
        sessionRef: "local.echo",
        messageText: "Routine progress",
      })).toEqual({
        shouldNotify: false,
        title: "Autopilot update",
        body: "local.echo: Routine progress",
        priority: "low",
      })
    }
  })

  test("suppresses unknown phases conservatively", () => {
    expect(buildNotification({
      phase: "started",
      sessionRef: "local.foxtrot",
      messageText: "Session started",
    })).toEqual({
      shouldNotify: false,
      title: "Autopilot update",
      body: "local.foxtrot: Session started",
      priority: "low",
    })
  })

  test("normalizes multiline message text for notification bodies", () => {
    expect(buildNotification({
      phase: "completed",
      sessionRef: " local.golf ",
      messageText: "Finished\n  final\tchecks",
    })).toEqual({
      shouldNotify: true,
      title: "Autopilot completed",
      body: "local.golf: Finished final checks",
      priority: "normal",
    })
  })
})
