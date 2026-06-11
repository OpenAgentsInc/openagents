import { beforeEach, describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  activeRoute,
  balanceHistory,
  maxBalanceHistory,
  recordBalancePoint,
  resetSurfaceState,
  setActiveRoute,
  setAssignmentRows,
  assignmentRows,
} from "../src/tui/store"
import {
  loadComposerState,
  maxComposerHistory,
  pushHistory,
  saveComposerState,
} from "../src/node/composer-store"

describe("routes and surfaces", () => {
  beforeEach(() => {
    resetSurfaceState()
  })

  test("route signal defaults to dashboard and switches", () => {
    expect(activeRoute()).toBe("dashboard")
    setActiveRoute("assignments")
    expect(activeRoute()).toBe("assignments")
    setActiveRoute("wallet")
    expect(activeRoute()).toBe("wallet")
  })

  test("assignment rows are keyed by lease ref", () => {
    setAssignmentRows([
      { assignmentRef: "a1", leaseRef: "lease-1", goal: "do work", paymentMode: "no-spend", expiresAt: "2026-06-11T00:00:00Z" },
    ])
    expect(assignmentRows()[0]?.leaseRef).toBe("lease-1")
  })

  test("balance history dedupes consecutive identical balances and caps", () => {
    recordBalancePoint("t1", 100)
    recordBalancePoint("t2", 100)
    recordBalancePoint("t3", 150)
    expect(balanceHistory.length).toBe(2)
    for (let i = 0; i < maxBalanceHistory + 10; i += 1) {
      recordBalancePoint(`x${i}`, i)
    }
    expect(balanceHistory.length).toBe(maxBalanceHistory)
  })
})

describe("composer persistence", () => {
  test("history and stash survive a save/load round trip", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-composer-"))
    await saveComposerState(dir, { history: ["first prompt", "second prompt"], stash: "a draft that is long enough to stash" })
    const restored = await loadComposerState(dir)
    expect(restored.history).toEqual(["first prompt", "second prompt"])
    expect(restored.stash).toContain("long enough")
  })

  test("short drafts are not stashed", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-composer-"))
    await saveComposerState(dir, { history: [], stash: "short" })
    const restored = await loadComposerState(dir)
    expect(restored.stash).toBe("")
  })

  test("corrupt files load as empty state", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-composer-"))
    await Bun.write(join(dir, "composer-history.json"), "{nope")
    const restored = await loadComposerState(dir)
    expect(restored).toEqual({ history: [], stash: "" })
  })

  test("pushHistory dedupes and bounds", () => {
    let history: string[] = []
    history = pushHistory(history, "one")
    history = pushHistory(history, "two")
    history = pushHistory(history, "one")
    expect(history).toEqual(["two", "one"])
    for (let i = 0; i < maxComposerHistory + 5; i += 1) {
      history = pushHistory(history, `p${i}`)
    }
    expect(history.length).toBe(maxComposerHistory)
  })
})
