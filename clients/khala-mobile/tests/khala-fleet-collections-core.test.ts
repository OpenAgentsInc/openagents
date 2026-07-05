import { describe, expect, test } from "bun:test"

import {
  formatAccountRefHash,
  sortAccountsByReadinessThenRef,
  sortWorkersByIdAsc
} from "../src/sync/khala-fleet-collections-core"

describe("khala fleet collections core", () => {
  test("formatAccountRefHash shortens the hex digest", () => {
    expect(formatAccountRefHash("account.pylon.codex.6be7b6501be36164f9c6ecda")).toBe(
      "pylon.codex.6be7b650…"
    )
  })

  test("formatAccountRefHash leaves a malformed ref untouched", () => {
    expect(formatAccountRefHash("not-a-ref")).toBe("not-a-ref")
  })

  test("sortWorkersByIdAsc orders by workerId", () => {
    const workers = [
      { accountRefHash: undefined, assignmentRef: undefined, lastProgressAt: undefined, phase: "idle" as const, updatedAt: "2026-01-01T00:00:00Z", workerId: "w2" },
      { accountRefHash: undefined, assignmentRef: undefined, lastProgressAt: undefined, phase: "idle" as const, updatedAt: "2026-01-01T00:00:00Z", workerId: "w1" }
    ]
    expect(sortWorkersByIdAsc(workers).map(w => w.workerId)).toEqual(["w1", "w2"])
  })

  test("sortAccountsByReadinessThenRef ranks ready first, then cooldown/unavailable/unknown", () => {
    const accounts = [
      { accountRefHash: "account.pylon.codex.bbbbbbbb", readiness: "unavailable" as const, updatedAt: "" },
      { accountRefHash: "account.pylon.codex.aaaaaaaa", readiness: "ready" as const, updatedAt: "" },
      { accountRefHash: "account.pylon.codex.cccccccc", readiness: "cooldown" as const, updatedAt: "" }
    ]
    expect(sortAccountsByReadinessThenRef(accounts).map(a => a.readiness)).toEqual([
      "ready",
      "cooldown",
      "unavailable"
    ])
  })
})
