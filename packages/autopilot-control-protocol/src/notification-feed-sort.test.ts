import { describe, expect, test } from "bun:test"

import { sortNotificationFeed, type NotificationFeedItem } from "./notification-feed-sort.js"

const notification = (
  sessionRef: string,
  priority: NotificationFeedItem["priority"],
  at: string,
): NotificationFeedItem => ({
  sessionRef,
  priority,
  at,
})

describe("notification feed sort", () => {
  test("orders high priority notifications before normal and low priority", () => {
    expect(sortNotificationFeed([
      notification("low-newest", "low", "2026-06-13T12:00:00.000Z"),
      notification("normal-newer", "normal", "2026-06-13T11:00:00.000Z"),
      notification("high-oldest", "high", "2026-06-13T10:00:00.000Z"),
    ]).map((item) => item.sessionRef)).toEqual([
      "high-oldest",
      "normal-newer",
      "low-newest",
    ])
  })

  test("orders notifications with the same priority by most recent timestamp first", () => {
    expect(sortNotificationFeed([
      notification("old", "normal", "2026-06-13T09:00:00.000Z"),
      notification("new", "normal", "2026-06-13T11:00:00.000Z"),
      notification("middle", "normal", "2026-06-13T10:00:00.000Z"),
    ]).map((item) => item.sessionRef)).toEqual([
      "new",
      "middle",
      "old",
    ])
  })

  test("applies recency within each priority group", () => {
    expect(sortNotificationFeed([
      notification("normal-new", "normal", "2026-06-13T11:00:00.000Z"),
      notification("high-old", "high", "2026-06-13T08:00:00.000Z"),
      notification("low-new", "low", "2026-06-13T12:00:00.000Z"),
      notification("high-new", "high", "2026-06-13T10:00:00.000Z"),
      notification("normal-old", "normal", "2026-06-13T09:00:00.000Z"),
    ]).map((item) => item.sessionRef)).toEqual([
      "high-new",
      "high-old",
      "normal-new",
      "normal-old",
      "low-new",
    ])
  })

  test("keeps priority and timestamp ties stable", () => {
    expect(sortNotificationFeed([
      notification("first", "high", "2026-06-13T08:00:00.000Z"),
      notification("second", "high", "2026-06-13T08:00:00.000Z"),
      notification("third", "high", "2026-06-13T08:00:00.000Z"),
    ]).map((item) => item.sessionRef)).toEqual([
      "first",
      "second",
      "third",
    ])
  })

  test("does not mutate the input and returns the original item objects", () => {
    const older = notification("older", "normal", "2026-06-13T09:00:00.000Z")
    const newer = notification("newer", "normal", "2026-06-13T10:00:00.000Z")
    const input = [older, newer]

    const sorted = sortNotificationFeed(input)

    expect(sorted).toEqual([newer, older])
    expect(sorted[0]).toBe(newer)
    expect(sorted[1]).toBe(older)
    expect(input).toEqual([older, newer])
  })

  test("preserves additional item fields through the generic return type", () => {
    const items = [
      {
        ...notification("first", "low", "2026-06-13T09:00:00.000Z"),
        title: "Autopilot update",
      },
      {
        ...notification("second", "high", "2026-06-13T08:00:00.000Z"),
        title: "Autopilot needs approval",
      },
    ]

    expect(sortNotificationFeed(items)).toEqual([items[1], items[0]])
  })
})
