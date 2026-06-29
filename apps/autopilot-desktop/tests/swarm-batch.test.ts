// #5469 (EPIC #5461): swarm batch launch + account-failover/routing visibility
// + sub-agent nesting tree.
//
// These tests drive the PURE swarm-batch.ts module (parsing, bounded-concurrency
// queue, failover-routing derivation, sub-agent tree) AND the pure reducer
// (update.ts) without a DOM or a runtime — the same idiom as swarm.test.ts. They
// prove (1) a batch never exceeds the visible concurrency cap and drains its
// queue as spawns settle, (2) routing reasons are derived refs-only from the
// event tail, and (3) parentRef nesting becomes a real depth-annotated tree.

import { describe, expect, test } from "bun:test"

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import {
  advanceSwarmBatch,
  buildSwarmTree,
  clampSwarmBatchConcurrency,
  parseSwarmBatchObjectives,
  startSwarmBatch,
  swarmBatchRunning,
  swarmBatchStatusLine,
  swarmFailoverRouting,
  swarmRoutingReasonLabel,
  SWARM_BATCH_MAX_CONCURRENCY,
  SWARM_BATCH_MAX_OBJECTIVES,
  type SwarmBatchState,
} from "../src/ui/swarm-batch"
import { initialModel, Model } from "../src/ui/model"
import {
  ChangedSwarmBatchConcurrency,
  ChangedSwarmBatchObjectives,
  ClickedSwarmBatchLaunch,
  FailedSwarmBatchSpawn,
  SucceededSwarmBatchSpawn,
} from "../src/ui/message"
import { update } from "../src/ui/update"

const session = (over: Partial<SessionSummary> = {}): SessionSummary => ({
  sessionRef: over.sessionRef ?? "session.pylon.codex.a",
  adapter: over.adapter ?? "codex",
  state: over.state ?? "running",
  accountRefHash: over.accountRefHash ?? null,
  updatedAt: over.updatedAt ?? "2026-06-19T12:00:00.000Z",
  ...over,
})

describe("swarm batch parsing + concurrency clamp (#5469)", () => {
  test("parseSwarmBatchObjectives: trims, drops blanks, de-dupes, caps", () => {
    expect(parseSwarmBatchObjectives("")).toEqual([])
    expect(parseSwarmBatchObjectives("  \n \n\t")).toEqual([])
    expect(parseSwarmBatchObjectives("a\n  b  \n\na")).toEqual(["a", "b"])
    const many = Array.from({ length: SWARM_BATCH_MAX_OBJECTIVES + 5 }, (_, i) => `o${i}`).join(
      "\n",
    )
    expect(parseSwarmBatchObjectives(many).length).toBe(SWARM_BATCH_MAX_OBJECTIVES)
  })

  test("clampSwarmBatchConcurrency: clamps into [1, MAX], NaN → default", () => {
    expect(clampSwarmBatchConcurrency(0)).toBe(1)
    expect(clampSwarmBatchConcurrency(-3)).toBe(1)
    expect(clampSwarmBatchConcurrency(2.9)).toBe(2)
    expect(clampSwarmBatchConcurrency(999)).toBe(SWARM_BATCH_MAX_CONCURRENCY)
    expect(clampSwarmBatchConcurrency(Number.NaN)).toBe(3)
  })
})

describe("swarm batch bounded-concurrency queue (#5469)", () => {
  test("startSwarmBatch: dispatches up to the cap, queues the rest", () => {
    const { state, toDispatch } = startSwarmBatch(["a", "b", "c", "d", "e"], 2)
    expect(toDispatch).toEqual(["a", "b"])
    expect(state.active).toBe(2)
    expect(state.queue).toEqual(["c", "d", "e"])
    expect(state.concurrency).toBe(2)
    expect(state.total).toBe(5)
    expect(swarmBatchRunning(state)).toBe(true)
  })

  test("startSwarmBatch: cap larger than the set dispatches everything", () => {
    const { state, toDispatch } = startSwarmBatch(["a", "b"], 8)
    expect(toDispatch).toEqual(["a", "b"])
    expect(state.queue).toEqual([])
    expect(state.active).toBe(2)
  })

  test("advanceSwarmBatch: a settle pulls the next queued objective (never over cap)", () => {
    let { state } = startSwarmBatch(["a", "b", "c", "d", "e"], 2)
    // a settles → active back to 2 (b still in flight + c pulled), c dispatched.
    const step1 = advanceSwarmBatch(state, "launched")
    expect(step1.next).toBe("c")
    expect(step1.state.active).toBe(2)
    expect(step1.state.launched).toBe(1)
    expect(step1.state.queue).toEqual(["d", "e"])
    state = step1.state
    const step2 = advanceSwarmBatch(state, "failed")
    expect(step2.next).toBe("d")
    expect(step2.state.failed).toBe(1)
    expect(step2.state.active).toBe(2)
    expect(step2.state.queue).toEqual(["e"])
  })

  test("advanceSwarmBatch: drains to empty without overshooting active", () => {
    let { state } = startSwarmBatch(["a", "b", "c"], 2)
    // Settle all three; active must never go negative and queue ends empty.
    let next: string | null
    let pulls: string[] = []
    for (let i = 0; i < 3; i++) {
      const r = advanceSwarmBatch(state, "launched")
      state = r.state
      next = r.next
      if (next) pulls.push(next)
      expect(state.active).toBeLessThanOrEqual(2)
      expect(state.active).toBeGreaterThanOrEqual(0)
    }
    expect(pulls).toEqual(["c"]) // only one was queued behind the first wave
    expect(state.queue).toEqual([])
    expect(state.active).toBe(0)
    expect(state.launched).toBe(3)
    expect(swarmBatchRunning(state)).toBe(false)
  })

  test("swarmBatchStatusLine: honest progress", () => {
    expect(swarmBatchStatusLine({ queue: [], active: 0, concurrency: 2, launched: 0, failed: 0, total: 0 })).toBe(
      "",
    )
    expect(
      swarmBatchStatusLine({ queue: ["c"], active: 2, concurrency: 2, launched: 1, failed: 1, total: 4 }),
    ).toBe("2/4 launched · 1 failed · 1 queued · 2/2 in flight")
    expect(
      swarmBatchStatusLine({ queue: [], active: 0, concurrency: 2, launched: 3, failed: 0, total: 3 }),
    ).toBe("3/3 launched · done")
  })
})

describe("swarm failover / routing visibility (#5469)", () => {
  test("swarmFailoverRouting: no events → no reason", () => {
    expect(swarmFailoverRouting(undefined)).toEqual({ reason: null, failovers: 0 })
    expect(swarmFailoverRouting([])).toEqual({ reason: null, failovers: 0 })
  })

  test("swarmFailoverRouting: latest recognized reason wins; failovers counted", () => {
    // quota_block then a succeeded route = one failover, current reason succeeded.
    const r = swarmFailoverRouting([
      { phase: "progress", detail: "account quota_block on primary" },
      { phase: "progress", detail: "routing succeeded on fallback" },
    ])
    expect(r.reason).toBe("succeeded")
    expect(r.failovers).toBe(1)
  })

  test("swarmFailoverRouting: matches spaced variants, ignores unrelated text", () => {
    expect(
      swarmFailoverRouting([{ phase: "progress", detail: "skipped unavailable account" }]).reason,
    ).toBe("skipped_unavailable")
    expect(
      swarmFailoverRouting([{ phase: "progress", detail: "edited file foo.ts (+3 -1)" }]).reason,
    ).toBe(null)
  })

  test("swarmRoutingReasonLabel: each reason maps to a public-safe label + tone", () => {
    expect(swarmRoutingReasonLabel("quota_block")).toEqual({
      text: "quota block",
      toneClass: "swarm-route-block",
    })
    expect(swarmRoutingReasonLabel("failed")).toEqual({
      text: "route failed",
      toneClass: "swarm-route-failed",
    })
    expect(swarmRoutingReasonLabel("succeeded")).toEqual({
      text: "routed",
      toneClass: "swarm-route-ok",
    })
  })
})

describe("swarm sub-agent tree (#5469)", () => {
  test("buildSwarmTree: roots are depth 0, direct children depth 1 with counts", () => {
    const tree = buildSwarmTree([
      session({ sessionRef: "parent" }),
      session({ sessionRef: "child-a", parentRef: "parent" }),
      session({ sessionRef: "child-b", parentRef: "parent" }),
    ])
    const byRef = new Map(tree.map((n) => [n.session.sessionRef, n]))
    expect(byRef.get("parent")?.depth).toBe(0)
    expect(byRef.get("parent")?.childCount).toBe(2)
    expect(byRef.get("child-a")?.depth).toBe(1)
    expect(byRef.get("child-a")?.childCount).toBe(0)
  })

  test("buildSwarmTree: grandchild nests at depth 2 (arbitrary nesting)", () => {
    const tree = buildSwarmTree([
      session({ sessionRef: "root" }),
      session({ sessionRef: "mid", parentRef: "root" }),
      session({ sessionRef: "leaf", parentRef: "mid" }),
    ])
    const byRef = new Map(tree.map((n) => [n.session.sessionRef, n]))
    expect(byRef.get("leaf")?.depth).toBe(2)
    expect(byRef.get("mid")?.childCount).toBe(1)
  })

  test("buildSwarmTree: orphan (parent not in set) is a root; cycle is bounded", () => {
    const orphan = buildSwarmTree([session({ sessionRef: "x", parentRef: "missing" })])
    expect(orphan[0].depth).toBe(0)
    // A self-parent does not loop forever.
    const selfish = buildSwarmTree([session({ sessionRef: "y", parentRef: "y" })])
    expect(selfish[0].depth).toBe(0)
  })
})

describe("swarm batch reducer (#5469)", () => {
  const withBatch = (over: Partial<Model> = {}): Model =>
    Model.make({ ...initialModel, ...over })

  test("ClickedSwarmBatchLaunch: dispatches a bounded first wave + queues the rest", () => {
    const start = withBatch({
      pane: "swarm",
      spawnAdapter: "codex",
      spawnLane: "auto",
      swarmBatchObjectives: "task one\ntask two\ntask three\ntask four",
      swarmBatchConcurrency: "2",
    })
    const [model, commands] = update(start, ClickedSwarmBatchLaunch())
    // Only the cap is dispatched immediately; the rest is queued.
    expect(commands).toHaveLength(2)
    expect(model.swarmBatchActive).toBe(2)
    expect(model.swarmBatchTotal).toBe(4)
    expect(model.swarmBatchQueue).toEqual(["task three", "task four"])
  })

  test("ClickedSwarmBatchLaunch: empty objective set is a no-op", () => {
    const start = withBatch({ pane: "swarm", swarmBatchObjectives: "   \n " })
    const [model, commands] = update(start, ClickedSwarmBatchLaunch())
    expect(commands).toHaveLength(0)
    expect(model.swarmBatchActive).toBe(0)
  })

  test("ClickedSwarmBatchLaunch: refuses to start while a batch is in flight", () => {
    const start = withBatch({
      pane: "swarm",
      swarmBatchObjectives: "a\nb",
      swarmBatchActive: 1,
    })
    const [model, commands] = update(start, ClickedSwarmBatchLaunch())
    expect(commands).toHaveLength(0)
    expect(model.swarmBatchActive).toBe(1)
  })

  test("apple_fm adapter falls back to claude_agent for the batch", () => {
    const start = withBatch({
      pane: "swarm",
      spawnAdapter: "apple_fm",
      swarmBatchObjectives: "only one",
      swarmBatchConcurrency: "3",
    })
    const [model, commands] = update(start, ClickedSwarmBatchLaunch())
    expect(commands).toHaveLength(1)
    // The dispatched SpawnBatchSession command carries claude_agent, not apple_fm.
    const cmd = commands[0] as unknown as { args?: { adapter?: string } }
    expect(cmd.args?.adapter).toBe("claude_agent")
    expect(model.swarmBatchActive).toBe(1)
  })

  test("SucceededSwarmBatchSpawn pulls the next queued objective", () => {
    // Mid-batch: 1 in flight, one queued.
    const start = withBatch({
      pane: "swarm",
      spawnAdapter: "codex",
      swarmBatchObjectives: "a\nb\nc",
      swarmBatchConcurrency: "1",
      swarmBatchQueue: ["c"],
      swarmBatchActive: 1,
      swarmBatchLaunched: 1,
      swarmBatchTotal: 3,
    })
    const [model, commands] = update(
      start,
      SucceededSwarmBatchSpawn({ sessionRef: "session.pylon.codex.b" }),
    )
    expect(model.swarmBatchLaunched).toBe(2)
    expect(model.swarmBatchActive).toBe(1) // pulled "c"
    expect(model.swarmBatchQueue).toEqual([])
    expect(commands).toHaveLength(1)
  })

  test("FailedSwarmBatchSpawn still drains the queue (one failure does not stall)", () => {
    const start = withBatch({
      pane: "swarm",
      spawnAdapter: "codex",
      swarmBatchObjectives: "a\nb",
      swarmBatchConcurrency: "1",
      swarmBatchQueue: ["b"],
      swarmBatchActive: 1,
      swarmBatchTotal: 2,
    })
    const [model, commands] = update(start, FailedSwarmBatchSpawn({ error: "quota" }))
    expect(model.swarmBatchFailed).toBe(1)
    expect(commands).toHaveLength(1) // pulled "b"
    expect(model.swarmBatchActive).toBe(1)
  })

  test("the last settle ends the batch with no further commands", () => {
    const start = withBatch({
      pane: "swarm",
      swarmBatchObjectives: "a",
      swarmBatchConcurrency: "1",
      swarmBatchQueue: [],
      swarmBatchActive: 1,
      swarmBatchLaunched: 0,
      swarmBatchTotal: 1,
    })
    const [model, commands] = update(
      start,
      SucceededSwarmBatchSpawn({ sessionRef: "session.pylon.codex.a" }),
    )
    expect(commands).toHaveLength(0)
    expect(model.swarmBatchActive).toBe(0)
    expect(model.swarmBatchLaunched).toBe(1)
  })

  test("ChangedSwarmBatchObjectives / Concurrency update model only", () => {
    const [m1] = update(initialModel, ChangedSwarmBatchObjectives({ value: "x\ny" }))
    expect(m1.swarmBatchObjectives).toBe("x\ny")
    const [m2] = update(initialModel, ChangedSwarmBatchConcurrency({ value: "5" }))
    expect(m2.swarmBatchConcurrency).toBe("5")
  })
})
