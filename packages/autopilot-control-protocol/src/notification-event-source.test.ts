import { describe, expect, test } from "bun:test"

import { notificationsFromSessions } from "./notification-event-source.js"

describe("notification event source", () => {
  test("returns no notifications and preserves seen refs for no sessions", () => {
    expect(notificationsFromSessions([], ["local.alpha:completed"])).toEqual({
      new: [],
      seenRefs: ["local.alpha:completed"],
    })
  })

  test("emits a completed session notification once", () => {
    expect(notificationsFromSessions([
      {
        sessionRef: "local.alpha",
        state: "completed",
        latestActivity: "Finished verification",
      },
    ], [])).toEqual({
      new: [{
        sessionRef: "local.alpha",
        title: "Autopilot completed",
        body: "local.alpha: Finished verification",
        priority: "normal",
      }],
      seenRefs: ["local.alpha:completed"],
    })
  })

  test("emits failed and needs_decision sessions as high priority", () => {
    expect(notificationsFromSessions([
      {
        sessionRef: "cloud.beta",
        state: "failed",
        latestActivity: "Smoke test failed",
      },
      {
        sessionRef: "bridge.gamma",
        state: "needs_decision",
        latestActivity: "Choose a deployment target",
      },
    ], [])).toEqual({
      new: [
        {
          sessionRef: "cloud.beta",
          title: "Autopilot failed",
          body: "cloud.beta: Smoke test failed",
          priority: "high",
        },
        {
          sessionRef: "bridge.gamma",
          title: "Autopilot needs decision",
          body: "bridge.gamma: Choose a deployment target",
          priority: "high",
        },
      ],
      seenRefs: ["cloud.beta:failed", "bridge.gamma:needs_decision"],
    })
  })

  test("suppresses non-notify-worthy states without marking them seen", () => {
    expect(notificationsFromSessions([
      {
        sessionRef: "local.delta",
        state: "running",
        latestActivity: "Still working",
      },
      {
        sessionRef: "local.echo",
        state: "needs_approval",
        latestActivity: "Approve publish step",
      },
    ], [])).toEqual({
      new: [],
      seenRefs: [],
    })
  })

  test("does not re-emit a session state already in seen refs", () => {
    expect(notificationsFromSessions([
      {
        sessionRef: "local.foxtrot",
        state: "completed",
        latestActivity: "Finished again",
      },
    ], ["local.foxtrot:completed"])).toEqual({
      new: [],
      seenRefs: ["local.foxtrot:completed"],
    })
  })

  test("emits when the same session enters a different notify-worthy state", () => {
    expect(notificationsFromSessions([
      {
        sessionRef: "local.golf",
        state: "failed",
        latestActivity: "Regression failed",
      },
    ], ["local.golf:completed"])).toEqual({
      new: [{
        sessionRef: "local.golf",
        title: "Autopilot failed",
        body: "local.golf: Regression failed",
        priority: "high",
      }],
      seenRefs: ["local.golf:completed", "local.golf:failed"],
    })
  })

  test("uses a state fallback when latest activity is missing", () => {
    expect(notificationsFromSessions([
      {
        sessionRef: "local.hotel",
        state: "completed",
      },
    ], [])).toEqual({
      new: [{
        sessionRef: "local.hotel",
        title: "Autopilot completed",
        body: "local.hotel: Session completed",
        priority: "normal",
      }],
      seenRefs: ["local.hotel:completed"],
    })
  })

  test("does not mutate sessions or seen refs", () => {
    const sessions = [{
      sessionRef: "local.india",
      state: "needs_decision",
      latestActivity: "Pick a branch",
    }]
    const seenRefs = ["local.juliet:failed"]
    const beforeSessions = structuredClone(sessions)
    const beforeSeenRefs = [...seenRefs]

    notificationsFromSessions(sessions, seenRefs)

    expect(sessions).toEqual(beforeSessions)
    expect(seenRefs).toEqual(beforeSeenRefs)
  })
})
