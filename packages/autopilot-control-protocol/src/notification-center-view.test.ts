import { describe, expect, test } from "bun:test"

import {
  buildNotificationCenter,
  type NotificationCenterInputItem,
} from "./notification-center-view.js"

const item = (
  sessionRef: string,
  priority: NotificationCenterInputItem["priority"],
  at: string,
): NotificationCenterInputItem => ({
  sessionRef,
  title: `Title ${sessionRef}`,
  body: `Body ${sessionRef}`,
  priority,
  at,
})

describe("notification center view", () => {
  test("returns an empty center for an empty feed", () => {
    expect(buildNotificationCenter([])).toEqual({
      items: [],
      unread: 0,
      hasHigh: false,
    })
  })

  test("sorts center items by priority before recency", () => {
    const lowNewest = item("low-newest", "low", "2026-06-13T12:00:00.000Z")
    const normalMiddle = item("normal-middle", "normal", "2026-06-13T11:00:00.000Z")
    const highOldest = item("high-oldest", "high", "2026-06-13T10:00:00.000Z")

    expect(buildNotificationCenter([
      lowNewest,
      normalMiddle,
      highOldest,
    ]).items).toEqual([
      highOldest,
      normalMiddle,
      lowNewest,
    ])
  })

  test("sorts items with the same priority by newest timestamp first", () => {
    const old = item("old", "normal", "2026-06-13T09:00:00.000Z")
    const newest = item("newest", "normal", "2026-06-13T12:00:00.000Z")
    const middle = item("middle", "normal", "2026-06-13T10:00:00.000Z")

    expect(buildNotificationCenter([old, newest, middle]).items).toEqual([
      newest,
      middle,
      old,
    ])
  })

  test("counts all supplied notifications as unread", () => {
    expect(buildNotificationCenter([
      item("first", "low", "2026-06-13T09:00:00.000Z"),
      item("second", "normal", "2026-06-13T10:00:00.000Z"),
      item("third", "high", "2026-06-13T11:00:00.000Z"),
    ])).toMatchObject({
      unread: 3,
    })
  })

  test("flags when any notification is high priority", () => {
    expect(buildNotificationCenter([
      item("low", "low", "2026-06-13T09:00:00.000Z"),
      item("high", "high", "2026-06-13T08:00:00.000Z"),
    ])).toMatchObject({
      hasHigh: true,
    })
  })

  test("does not flag high priority for low and normal notifications", () => {
    expect(buildNotificationCenter([
      item("low", "low", "2026-06-13T09:00:00.000Z"),
      item("normal", "normal", "2026-06-13T10:00:00.000Z"),
    ])).toMatchObject({
      hasHigh: false,
    })
  })

  test("does not mutate input and preserves original item objects", () => {
    const old = item("old", "normal", "2026-06-13T09:00:00.000Z")
    const newest = item("newest", "normal", "2026-06-13T10:00:00.000Z")
    const input = [old, newest]

    const center = buildNotificationCenter(input)

    expect(center.items).toEqual([newest, old])
    expect(center.items[0]).toBe(newest)
    expect(center.items[1]).toBe(old)
    expect(input).toEqual([old, newest])
  })
})
