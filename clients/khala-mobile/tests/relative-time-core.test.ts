import { describe, expect, test } from "bun:test"

import { formatRelativeTime } from "../src/sync/relative-time-core"

const NOW = Date.parse("2026-01-01T12:00:00.000Z")

describe("formatRelativeTime", () => {
  test("under a minute reads as just now", () => {
    expect(formatRelativeTime("2026-01-01T11:59:31.000Z", NOW)).toBe("just now")
  })

  test("minutes", () => {
    expect(formatRelativeTime("2026-01-01T11:55:00.000Z", NOW)).toBe("5m")
  })

  test("hours", () => {
    expect(formatRelativeTime("2026-01-01T09:00:00.000Z", NOW)).toBe("3h")
  })

  test("days", () => {
    expect(formatRelativeTime("2025-12-30T12:00:00.000Z", NOW)).toBe("2d")
  })

  test("a future/invalid timestamp reads as just now rather than negative", () => {
    expect(formatRelativeTime("2026-01-01T12:05:00.000Z", NOW)).toBe("just now")
  })
})
