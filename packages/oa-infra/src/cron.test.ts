import { describe, expect, test } from "bun:test"
import { CronParseError, cronMatches, parseCron } from "./cron.ts"

/** UTC date helper: (y, m 1-12, d, hh, mm). */
const utc = (y: number, m: number, d: number, hh: number, mm: number) =>
  new Date(Date.UTC(y, m - 1, d, hh, mm))

describe("parseCron", () => {
  test("rejects wrong field counts", () => {
    expect(() => parseCron("* * * *")).toThrow(CronParseError)
    expect(() => parseCron("* * * * * *")).toThrow(CronParseError)
    expect(() => parseCron("")).toThrow(CronParseError)
  })

  test("rejects out-of-range and malformed values", () => {
    expect(() => parseCron("60 * * * *")).toThrow(CronParseError)
    expect(() => parseCron("* 24 * * *")).toThrow(CronParseError)
    expect(() => parseCron("* * 0 * *")).toThrow(CronParseError)
    expect(() => parseCron("* * * 13 *")).toThrow(CronParseError)
    expect(() => parseCron("* * * * 8")).toThrow(CronParseError)
    expect(() => parseCron("a * * * *")).toThrow(CronParseError)
    expect(() => parseCron("5-1 * * * *")).toThrow(CronParseError)
    expect(() => parseCron("*/0 * * * *")).toThrow(CronParseError)
    expect(() => parseCron("5/2 * * * *")).toThrow(CronParseError)
    expect(() => parseCron(",5 * * * *")).toThrow(CronParseError)
  })

  test("parses names, ranges, steps, and lists", () => {
    const schedule = parseCron("*/15 9-17 1,15 jan-mar mon-fri")
    expect([...schedule.minutes]).toEqual([0, 15, 30, 45])
    expect([...schedule.hours]).toEqual([9, 10, 11, 12, 13, 14, 15, 16, 17])
    expect([...schedule.daysOfMonth]).toEqual([1, 15])
    expect([...schedule.months]).toEqual([1, 2, 3])
    expect([...schedule.daysOfWeek]).toEqual([1, 2, 3, 4, 5])
  })

  test("day-of-week 7 normalizes to Sunday (0)", () => {
    const schedule = parseCron("0 0 * * 7")
    expect([...schedule.daysOfWeek]).toEqual([0])
  })
})

describe("cronMatches", () => {
  test("* * * * * matches every minute", () => {
    expect(cronMatches("* * * * *", utc(2026, 7, 6, 12, 34))).toBe(true)
    expect(cronMatches("* * * * *", utc(2026, 1, 1, 0, 0))).toBe(true)
  })

  test("exact minute/hour", () => {
    expect(cronMatches("30 4 * * *", utc(2026, 7, 6, 4, 30))).toBe(true)
    expect(cronMatches("30 4 * * *", utc(2026, 7, 6, 4, 31))).toBe(false)
    expect(cronMatches("30 4 * * *", utc(2026, 7, 6, 5, 30))).toBe(false)
  })

  test("steps: */10 fires on :00 :10 :20 ...", () => {
    expect(cronMatches("*/10 * * * *", utc(2026, 7, 6, 8, 20))).toBe(true)
    expect(cronMatches("*/10 * * * *", utc(2026, 7, 6, 8, 25))).toBe(false)
  })

  test("day-of-week: 2026-07-06 is a Monday", () => {
    expect(cronMatches("0 9 * * mon", utc(2026, 7, 6, 9, 0))).toBe(true)
    expect(cronMatches("0 9 * * tue", utc(2026, 7, 6, 9, 0))).toBe(false)
    expect(cronMatches("0 9 * * 1", utc(2026, 7, 6, 9, 0))).toBe(true)
  })

  test("month restriction", () => {
    expect(cronMatches("0 0 1 jul *", utc(2026, 7, 1, 0, 0))).toBe(true)
    expect(cronMatches("0 0 1 jun *", utc(2026, 7, 1, 0, 0))).toBe(false)
  })

  test("DOM/DOW both restricted -> OR (standard cron)", () => {
    // 2026-07-06 is a Monday, the 6th.
    // dom=6 matches even though dow=fri does not:
    expect(cronMatches("0 0 6 * fri", utc(2026, 7, 6, 0, 0))).toBe(true)
    // dow=mon matches even though dom=15 does not:
    expect(cronMatches("0 0 15 * mon", utc(2026, 7, 6, 0, 0))).toBe(true)
    // neither matches:
    expect(cronMatches("0 0 15 * fri", utc(2026, 7, 6, 0, 0))).toBe(false)
  })

  test("DOM restricted with DOW wildcard -> AND on DOM", () => {
    expect(cronMatches("0 0 6 * *", utc(2026, 7, 6, 0, 0))).toBe(true)
    expect(cronMatches("0 0 7 * *", utc(2026, 7, 6, 0, 0))).toBe(false)
  })

  test("evaluates in UTC, not local time", () => {
    // 23:59 UTC on the 6th — local timezones would disagree on the date.
    expect(cronMatches("59 23 6 * *", new Date(Date.UTC(2026, 6, 6, 23, 59)))).toBe(true)
  })
})
