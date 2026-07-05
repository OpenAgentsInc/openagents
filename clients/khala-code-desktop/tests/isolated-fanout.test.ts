import { describe, expect, test } from "bun:test"

import { settleFanout } from "../src/ui/isolated-fanout"
import {
  allTurnSteerOutcomesOk,
  steerEachTurn,
  summarizeTurnSteerOutcomes,
} from "../src/ui/turn-steer-outcomes"
import { resolveAttachments } from "../src/ui/attachment-resolution"

// Oracle for the Promise.all landmine class documented in
// docs/2026-07-05-promise-all-cron-landmine-audit.md: an independent-item
// fanout must never let one item's rejection erase a sibling's outcome.
describe("settleFanout", () => {
  test("preserves every sibling outcome when one item rejects", async () => {
    const outcomes = await settleFanout(["a", "b", "c"], async item => {
      if (item === "b") throw new Error("b exploded")
      return `${item}-ok`
    })

    expect(outcomes).toEqual([
      { item: "a", ok: true, value: "a-ok" },
      { item: "b", ok: false, error: "b exploded" },
      { item: "c", ok: true, value: "c-ok" },
    ])
  })

  test("a bare Promise.all over the same items would have discarded the successes", async () => {
    // This is the pre-fix shape: the same map, but run through a raw
    // Promise.all instead of settleFanout. It must reject and must NOT give
    // us the "a" and "c" successes back — proving settleFanout is the fix,
    // not a no-op.
    const run = async (item: string): Promise<string> => {
      if (item === "b") throw new Error("b exploded")
      return `${item}-ok`
    }
    await expect(Promise.all(["a", "b", "c"].map(run))).rejects.toThrow("b exploded")
  })
})

describe("steerEachTurn / summarizeTurnSteerOutcomes", () => {
  test("captures a per-turn outcome for every targeted turn, even when one rejects", async () => {
    const calls: Array<string | undefined> = []
    const outcomes = await steerEachTurn(["turn-a", "turn-b", "turn-c"], async turnId => {
      calls.push(turnId)
      if (turnId === "turn-b") throw new Error("network blip")
      return { ok: true }
    })

    expect(calls).toEqual(["turn-a", "turn-b", "turn-c"])
    expect(outcomes).toEqual([
      { turnId: "turn-a", ok: true },
      { turnId: "turn-b", ok: false, error: "network blip" },
      { turnId: "turn-c", ok: true },
    ])
    expect(allTurnSteerOutcomesOk(outcomes)).toBe(false)
  })

  test("captures a logical (non-throwing) per-turn failure alongside a sibling success", async () => {
    const outcomes = await steerEachTurn(["turn-a", "turn-b"], async turnId =>
      turnId === "turn-b" ? { ok: false, error: "turn already closed" } : { ok: true })

    expect(outcomes).toEqual([
      { turnId: "turn-a", ok: true },
      { turnId: "turn-b", ok: false, error: "turn already closed" },
    ])
  })

  test("reports partial failure by count instead of collapsing to one opaque error", async () => {
    const outcomes = await steerEachTurn(["turn-a", "turn-b", "turn-c"], async turnId =>
      turnId === "turn-b" ? { ok: false, error: "turn already closed" } : { ok: true })

    const message = summarizeTurnSteerOutcomes(outcomes, {
      success: "Follow-up steering succeeded.",
      failurePrefix: "Follow-up steering failed",
    })

    expect(message).toBe("Follow-up steering failed for 1 of 3 turns: turn already closed")
  })

  test("reports full success across every turn", async () => {
    const outcomes = await steerEachTurn(["turn-a", "turn-b"], async () => ({ ok: true }))

    expect(allTurnSteerOutcomesOk(outcomes)).toBe(true)
    expect(summarizeTurnSteerOutcomes(outcomes, {
      success: "Follow-up steering succeeded.",
      failurePrefix: "Follow-up steering failed",
    })).toBe("Follow-up steering succeeded.")
  })

  test("reports total failure across every turn distinctly from partial failure", async () => {
    const outcomes = await steerEachTurn(["turn-a", "turn-b"], async () => ({
      ok: false,
      error: "session expired",
    }))

    expect(summarizeTurnSteerOutcomes(outcomes, {
      success: "Follow-up steering succeeded.",
      failurePrefix: "Follow-up steering failed",
    })).toBe("Follow-up steering failed for all 2 turns: session expired")
  })

  test("reports a single-target failure without turn-count phrasing", async () => {
    const outcomes = await steerEachTurn([undefined], async () => ({
      ok: false,
      error: "no active turn",
    }))

    expect(summarizeTurnSteerOutcomes(outcomes, {
      success: "Follow-up steering succeeded.",
      failurePrefix: "Follow-up steering failed",
    })).toBe("Follow-up steering failed: no active turn")
  })
})

describe("resolveAttachments", () => {
  test("keeps a sibling attachment's resolved bytes when another attachment's read rejects", async () => {
    const result = await resolveAttachments(
      [
        { id: "good-1", name: "good-1.png" },
        { id: "stale", name: "stale.png" },
        { id: "good-2", name: "good-2.png" },
      ],
      async item => {
        if (item.id === "stale") throw new Error("revoked object URL")
        return `bytes-for-${item.id}`
      },
      item => item.name,
    )

    expect(result.resolved).toEqual(["bytes-for-good-1", "bytes-for-good-2"])
    expect(result.failures).toEqual([{ name: "stale.png", error: "revoked object URL" }])
  })

  test("returns everything resolved when nothing fails", async () => {
    const result = await resolveAttachments(
      [{ id: "a", name: "a.png" }, { id: "b", name: "b.png" }],
      async item => `bytes-for-${item.id}`,
      item => item.name,
    )

    expect(result.resolved).toEqual(["bytes-for-a", "bytes-for-b"])
    expect(result.failures).toEqual([])
  })

  test("skips items whose resolver returns null without treating them as failures", async () => {
    const result = await resolveAttachments(
      [{ id: "a", name: "a.png" }, { id: "skip", name: "skip.png" }],
      async item => item.id === "skip" ? null : `bytes-for-${item.id}`,
      item => item.name,
    )

    expect(result.resolved).toEqual(["bytes-for-a"])
    expect(result.failures).toEqual([])
  })
})
