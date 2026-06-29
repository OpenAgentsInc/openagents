import { describe, expect, test } from "bun:test"

import { buildNotificationFeed } from "./notification-render-model.js"

describe("notification render model", () => {
  test("returns an empty feed for no events", () => {
    expect(buildNotificationFeed({
      events: [],
      hour: 12,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [],
      suppressed: 0,
    })
  })

  test("renders a completed event outside quiet hours", () => {
    expect(buildNotificationFeed({
      events: [{
        phase: "completed",
        sessionRef: "local.alpha",
        messageText: "Finished verification",
      }],
      hour: 12,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [{
        sessionRef: "local.alpha",
        title: "Autopilot completed",
        body: "local.alpha: Finished verification",
        priority: "normal",
      }],
      suppressed: 0,
    })
  })

  test("drops dispatch-suppressed routine events", () => {
    expect(buildNotificationFeed({
      events: [
        {
          phase: "agent_message",
          sessionRef: "local.alpha",
          messageText: "Routine progress",
        },
        {
          phase: "completed",
          sessionRef: "local.beta",
          messageText: "Done",
        },
      ],
      hour: 12,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [{
        sessionRef: "local.beta",
        title: "Autopilot completed",
        body: "local.beta: Done",
        priority: "normal",
      }],
      suppressed: 1,
    })
  })

  test("filters normal priority notifications during quiet hours", () => {
    expect(buildNotificationFeed({
      events: [
        {
          phase: "completed",
          sessionRef: "local.alpha",
          messageText: "Finished verification",
        },
        {
          phase: "needs_approval",
          sessionRef: "local.beta",
          messageText: "Approve deploy",
        },
      ],
      hour: 23,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [{
        sessionRef: "local.beta",
        title: "Autopilot needs approval",
        body: "local.beta: Approve deploy",
        priority: "high",
      }],
      suppressed: 1,
    })
  })

  test("coalesces multiple visible events by session", () => {
    expect(buildNotificationFeed({
      events: [
        {
          phase: "completed",
          sessionRef: "local.alpha",
          messageText: "Finished first step",
        },
        {
          phase: "completed",
          sessionRef: "local.alpha",
          messageText: "Finished second step",
        },
        {
          phase: "completed",
          sessionRef: "local.beta",
          messageText: "Finished beta",
        },
      ],
      hour: 12,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [
        {
          sessionRef: "local.alpha",
          title: "Autopilot completed",
          body: "local.alpha: Finished first step",
          priority: "normal",
        },
        {
          sessionRef: "local.beta",
          title: "Autopilot completed",
          body: "local.beta: Finished beta",
          priority: "normal",
        },
      ],
      suppressed: 0,
    })
  })

  test("coalesced sessions keep highest priority title and body", () => {
    expect(buildNotificationFeed({
      events: [
        {
          phase: "completed",
          sessionRef: "local.alpha",
          messageText: "Finished verification",
        },
        {
          phase: "failed",
          sessionRef: "local.alpha",
          messageText: "Smoke test failed",
        },
      ],
      hour: 12,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [{
        sessionRef: "local.alpha",
        title: "Autopilot failed",
        body: "local.alpha: Smoke test failed",
        priority: "high",
      }],
      suppressed: 0,
    })
  })

  test("handles wrapped quiet hours and dispatch suppression together", () => {
    expect(buildNotificationFeed({
      events: [
        {
          phase: "reasoning",
          sessionRef: "local.alpha",
          messageText: "Thinking",
        },
        {
          phase: "completed",
          sessionRef: "local.beta",
          messageText: "Done",
        },
        {
          phase: "needs_decision",
          sessionRef: "local.gamma",
          messageText: "Pick target",
        },
      ],
      hour: 2,
      quietStart: 22,
      quietEnd: 7,
    })).toEqual({
      visible: [{
        sessionRef: "local.gamma",
        title: "Autopilot needs decision",
        body: "local.gamma: Pick target",
        priority: "high",
      }],
      suppressed: 2,
    })
  })

  test("does not mutate input events", () => {
    const input = {
      events: [{
        phase: "completed",
        sessionRef: " local.alpha ",
        messageText: "Finished\n checks",
      }],
      hour: 12,
      quietStart: 22,
      quietEnd: 7,
    }
    const before = structuredClone(input)

    buildNotificationFeed(input)

    expect(input).toEqual(before)
  })
})
