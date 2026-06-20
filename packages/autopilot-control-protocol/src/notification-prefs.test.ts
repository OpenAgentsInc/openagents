import { describe, expect, test } from "bun:test"

import {
  normalizeNotificationPrefs,
  shouldDeliver,
  type NotificationPrefs,
} from "./notification-prefs.js"

const defaults: NotificationPrefs = {
  enabled: true,
  minPriority: "normal",
  quietStart: 22,
  quietEnd: 7,
}

describe("notification prefs", () => {
  test("returns defensive defaults for missing input", () => {
    expect(normalizeNotificationPrefs(null)).toEqual(defaults)
    expect(normalizeNotificationPrefs("")).toEqual(defaults)
  })

  test("keeps valid preferences", () => {
    expect(normalizeNotificationPrefs({
      enabled: false,
      minPriority: "high",
      quietStart: 8,
      quietEnd: 20,
    })).toEqual({
      enabled: false,
      minPriority: "high",
      quietStart: 8,
      quietEnd: 20,
    })
  })

  test("defaults malformed fields independently", () => {
    expect(normalizeNotificationPrefs({
      enabled: "false",
      minPriority: "urgent",
      quietStart: "22",
      quietEnd: Number.NaN,
    })).toEqual(defaults)
  })

  test("normalizes quiet hours into whole day hours", () => {
    expect(normalizeNotificationPrefs({
      quietStart: 25.8,
      quietEnd: -2.1,
    })).toEqual({
      ...defaults,
      quietStart: 23,
      quietEnd: 0,
    })
  })

  test("does not deliver when notifications are disabled", () => {
    expect(shouldDeliver({
      ...defaults,
      enabled: false,
      minPriority: "low",
    }, { priority: "high" })).toBe(false)
  })

  test("delivers priorities at or above the minimum", () => {
    const prefs = {
      ...defaults,
      minPriority: "normal" as const,
    }

    expect(shouldDeliver(prefs, { priority: "low" })).toBe(false)
    expect(shouldDeliver(prefs, { priority: "normal" })).toBe(true)
    expect(shouldDeliver(prefs, { priority: "high" })).toBe(true)
  })

  test("supports low and high minimum priority thresholds", () => {
    expect(shouldDeliver({
      ...defaults,
      minPriority: "low",
    }, { priority: "low" })).toBe(true)

    expect(shouldDeliver({
      ...defaults,
      minPriority: "high",
    }, { priority: "normal" })).toBe(false)
  })

  test("does not deliver unknown item priorities", () => {
    expect(shouldDeliver(defaults, { priority: "urgent" })).toBe(false)
  })

  test("does not treat object prototype names as priorities", () => {
    expect(normalizeNotificationPrefs({
      minPriority: "toString",
    })).toEqual(defaults)
    expect(shouldDeliver(defaults, { priority: "toString" })).toBe(false)
  })
})
