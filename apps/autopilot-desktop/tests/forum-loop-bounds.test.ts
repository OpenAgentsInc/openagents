import { describe, expect, it } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  FORUM_LOOP_MAX_WRITES_PER_DAY,
  FORUM_LOOP_MAX_WRITES_PER_TICK,
  canAttemptForumWrite,
  classifyForumWriteStatus,
  forumWriteBudgetRemaining,
  loadForumLoopLedger,
  recordForumWriteAttempt,
  utcDayKey,
} from "../src/bun/forum-loop-bounds"

const home = () => mkdtempSync(join(tmpdir(), "flb-"))

describe("forum-loop bounds caps (AF-5)", () => {
  it("mirrors the Artanis responder caps", () => {
    expect(FORUM_LOOP_MAX_WRITES_PER_DAY).toBe(20)
    expect(FORUM_LOOP_MAX_WRITES_PER_TICK).toBe(3)
  })

  it("starts with a full daily budget on a fresh home", () => {
    const h = home()
    try {
      expect(forumWriteBudgetRemaining(h)).toBe(FORUM_LOOP_MAX_WRITES_PER_DAY)
      expect(canAttemptForumWrite(h)).toBe(true)
      expect(loadForumLoopLedger(h).writes).toBe(0)
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("records attempts and exhausts the daily budget at the cap", () => {
    const h = home()
    try {
      for (let i = 0; i < FORUM_LOOP_MAX_WRITES_PER_DAY; i++) {
        expect(canAttemptForumWrite(h)).toBe(true)
        recordForumWriteAttempt(h)
      }
      expect(forumWriteBudgetRemaining(h)).toBe(0)
      expect(canAttemptForumWrite(h)).toBe(false)
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("rolls the budget over at UTC midnight (a new day resets the count)", () => {
    const h = home()
    try {
      const day1 = new Date("2026-06-21T12:00:00.000Z")
      const day2 = new Date("2026-06-22T00:30:00.000Z")
      for (let i = 0; i < FORUM_LOOP_MAX_WRITES_PER_DAY; i++) {
        recordForumWriteAttempt(h, undefined, undefined, day1)
      }
      expect(canAttemptForumWrite(h, undefined, day1)).toBe(false)
      // The next UTC day reads a fresh, full budget.
      expect(canAttemptForumWrite(h, undefined, day2)).toBe(true)
      expect(loadForumLoopLedger(h, undefined, day2).writes).toBe(0)
      expect(forumWriteBudgetRemaining(h, undefined, day2)).toBe(
        FORUM_LOOP_MAX_WRITES_PER_DAY,
      )
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("formats the UTC day key as YYYY-MM-DD", () => {
    expect(utcDayKey(new Date("2026-06-21T23:59:59.000Z"))).toBe("2026-06-21")
  })
})

describe("classifyForumWriteStatus (AF-5)", () => {
  it("maps HTTP statuses to typed dispositions", () => {
    expect(classifyForumWriteStatus(200)).toBe("ok")
    expect(classifyForumWriteStatus(201)).toBe("ok")
    expect(classifyForumWriteStatus(402)).toBe("payment_required")
    expect(classifyForumWriteStatus(409)).toBe("conflict")
    expect(classifyForumWriteStatus(429)).toBe("rate_limited")
    expect(classifyForumWriteStatus(500)).toBe("error")
    expect(classifyForumWriteStatus(404)).toBe("error")
  })
})
