import { describe, expect, test } from "bun:test"

import { filterByQuietHours, inQuietHours } from "./notification-quiet-hours.js"

describe("notification quiet hours", () => {
  test("matches hours inside a same-day window", () => {
    expect(inQuietHours({ hour: 13, startHour: 9, endHour: 17 })).toBe(true)
  })

  test("excludes the end hour for a same-day window", () => {
    expect(inQuietHours({ hour: 17, startHour: 9, endHour: 17 })).toBe(false)
  })

  test("matches late hours when quiet hours wrap midnight", () => {
    expect(inQuietHours({ hour: 23, startHour: 22, endHour: 7 })).toBe(true)
  })

  test("matches early hours when quiet hours wrap midnight", () => {
    expect(inQuietHours({ hour: 2, startHour: 22, endHour: 7 })).toBe(true)
  })

  test("excludes daytime hours when quiet hours wrap midnight", () => {
    expect(inQuietHours({ hour: 12, startHour: 22, endHour: 7 })).toBe(false)
  })

  test("clamps hours into the supported day range", () => {
    expect(inQuietHours({ hour: 30, startHour: 20, endHour: 2 })).toBe(true)
    expect(inQuietHours({ hour: -3, startHour: 22, endHour: 6 })).toBe(true)
  })

  test("returns all items outside quiet hours without mutating the source", () => {
    const items = [
      { priority: "low" as const },
      { priority: "normal" as const },
      { priority: "high" as const },
    ]

    const filtered = filterByQuietHours(items, false)

    expect(filtered).toEqual(items)
    expect(filtered).not.toBe(items)
  })

  test("suppresses all but high priority during quiet hours", () => {
    const items = [
      { priority: "low" as const },
      { priority: "normal" as const },
      { priority: "high" as const },
    ]

    expect(filterByQuietHours(items, true)).toEqual([{ priority: "high" }])
  })
})
