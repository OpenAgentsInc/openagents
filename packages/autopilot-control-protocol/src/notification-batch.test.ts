import { describe, expect, test } from "bun:test"

import { coalesceNotifications } from "./notification-batch.js"
import type { NotificationBatchItem } from "./notification-batch.js"

describe("notification batch coalescing", () => {
  test("returns an empty batch for no notifications", () => {
    expect(coalesceNotifications([])).toEqual({
      grouped: [],
      total: 0,
    })
  })

  test("keeps a single notification unchanged with count one", () => {
    expect(coalesceNotifications([{
      sessionRef: "local.alpha",
      title: "Autopilot completed",
      priority: "normal",
    }])).toEqual({
      grouped: [{
        sessionRef: "local.alpha",
        count: 1,
        title: "Autopilot completed",
        priority: "normal",
      }],
      total: 1,
    })
  })

  test("groups repeated session refs and increments count", () => {
    expect(coalesceNotifications([
      {
        sessionRef: "local.alpha",
        title: "Autopilot update",
        priority: "low",
      },
      {
        sessionRef: "local.alpha",
        title: "Autopilot update",
        priority: "low",
      },
      {
        sessionRef: "cloud.beta",
        title: "Autopilot completed",
        priority: "normal",
      },
    ])).toEqual({
      grouped: [
        {
          sessionRef: "local.alpha",
          count: 2,
          title: "Autopilot update",
          priority: "low",
        },
        {
          sessionRef: "cloud.beta",
          count: 1,
          title: "Autopilot completed",
          priority: "normal",
        },
      ],
      total: 3,
    })
  })

  test("keeps the highest priority and its title within a group", () => {
    expect(coalesceNotifications([
      {
        sessionRef: "local.alpha",
        title: "Autopilot update",
        priority: "low",
      },
      {
        sessionRef: "local.alpha",
        title: "Autopilot needs approval",
        priority: "high",
      },
      {
        sessionRef: "local.alpha",
        title: "Autopilot completed",
        priority: "normal",
      },
    ])).toEqual({
      grouped: [{
        sessionRef: "local.alpha",
        count: 3,
        title: "Autopilot needs approval",
        priority: "high",
      }],
      total: 3,
    })
  })

  test("preserves first-seen group order while priorities change", () => {
    expect(coalesceNotifications([
      {
        sessionRef: "local.alpha",
        title: "Autopilot update",
        priority: "low",
      },
      {
        sessionRef: "cloud.beta",
        title: "Autopilot completed",
        priority: "normal",
      },
      {
        sessionRef: "local.alpha",
        title: "Autopilot failed",
        priority: "high",
      },
    ])).toEqual({
      grouped: [
        {
          sessionRef: "local.alpha",
          count: 2,
          title: "Autopilot failed",
          priority: "high",
        },
        {
          sessionRef: "cloud.beta",
          count: 1,
          title: "Autopilot completed",
          priority: "normal",
        },
      ],
      total: 3,
    })
  })

  test("does not mutate source notifications", () => {
    const items: NotificationBatchItem[] = [
      {
        sessionRef: "local.alpha",
        title: "Autopilot update",
        priority: "low",
      },
      {
        sessionRef: "local.alpha",
        title: "Autopilot needs decision",
        priority: "high",
      },
    ]

    const before = structuredClone(items)

    coalesceNotifications(items)

    expect(items).toEqual(before)
  })
})
