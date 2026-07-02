import { describe, expect, test } from "bun:test"

import { formatCompactThreadTimestamp } from "../src/ui/thread-time"

describe("thread sidebar compact timestamps", () => {
  const nowMs = Date.UTC(2026, 6, 1, 16, 34, 57)

  test("formats recent thread times like the Codex sidebar", () => {
    expect(formatCompactThreadTimestamp(nowMs / 1000 - 43 * 60, nowMs)).toBe("43m")
    expect(formatCompactThreadTimestamp(nowMs / 1000 - 4 * 60 * 60, nowMs)).toBe("4h")
    expect(formatCompactThreadTimestamp(nowMs / 1000 - 3 * 24 * 60 * 60, nowMs)).toBe("3d")
    expect(formatCompactThreadTimestamp(nowMs / 1000 - 360 * 24 * 60 * 60, nowMs)).toBe("12mo")
    expect(formatCompactThreadTimestamp(nowMs / 1000 - 370 * 24 * 60 * 60, nowMs)).toBe("1y")
  })

  test("falls back cleanly for empty or current timestamps", () => {
    expect(formatCompactThreadTimestamp(null, nowMs)).toBe("")
    expect(formatCompactThreadTimestamp(nowMs / 1000, nowMs)).toBe("now")
  })

  test("defensively formats millisecond timestamps", () => {
    expect(formatCompactThreadTimestamp(nowMs - 2 * 60 * 60 * 1000, nowMs)).toBe("2h")
  })
})
