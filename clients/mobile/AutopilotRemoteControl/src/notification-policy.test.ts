import { describe, expect, test } from "bun:test"
import { DEFAULT_QUIET_HOURS, selectNotificationsToFire } from "./notification-policy"

const items = [
  { priority: "high", title: "decision" },
  { priority: "normal", title: "completed" },
  { priority: "low", title: "summary" },
]

describe("quiet-hours notification policy (#5003)", () => {
  test("outside quiet hours: everything fires", () => {
    // window 22→7; nowHour 12 is outside.
    const fired = selectNotificationsToFire(items, DEFAULT_QUIET_HOURS, 12)
    expect(fired.map((n) => n.title)).toEqual(["decision", "completed", "summary"])
  })

  test("inside quiet hours: only high-priority fires", () => {
    // nowHour 23 is inside 22→7.
    const fired = selectNotificationsToFire(items, DEFAULT_QUIET_HOURS, 23)
    expect(fired.map((n) => n.title)).toEqual(["decision"])
    // early morning (06:00) is also inside the overnight window.
    expect(selectNotificationsToFire(items, DEFAULT_QUIET_HOURS, 6).map((n) => n.title)).toEqual(["decision"])
  })

  test("null window disables quiet hours", () => {
    expect(selectNotificationsToFire(items, null, 23).length).toBe(3)
  })
})
