// FA-RT-02 (fleet contention rotation): the reconcile loop must use the WHOLE
// admitted fleet under contention instead of waiting on -- or deadlocking two
// runs onto -- one busy lane. These tests compose the REAL exported
// `reconcileFullAutoThreads` against a real `openFullAutoRegistry` and an
// injected `laneReady` / `dispatch` (never a reimplementation of the loop), so
// a false-green oracle cannot pass.
//
// The two owner-hit incidents this covers:
//  1. Two Full Auto runs pinned to the same lane no longer deadlock -- the
//     second run rotates onto a ready sibling lane and BOTH progress.
//  2. A run whose current lane is busy/overdue/unavailable rotates to the next
//     READY admitted lane rather than stalling on `dispatch_overdue`.
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import { reconcileFullAutoThreads } from "./full-auto-reconcile.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"

const withRegistry = async (
  run: (registry: ReturnType<typeof openFullAutoRegistry>) => Promise<void>,
): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "full-auto-contention-"))
  try {
    await run(openFullAutoRegistry(path.join(root, "registry.json")))
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const baseInput = (
  registry: ReturnType<typeof openFullAutoRegistry>,
): Pick<
  Parameters<typeof reconcileFullAutoThreads>[0],
  "registry" | "nonterminalThreadRefs" | "resolveWorkspaceRef" | "journalHasNonterminalTurn"
> => ({
  registry,
  nonterminalThreadRefs: () => new Set(),
  resolveWorkspaceRef: () => "/workspace",
  journalHasNonterminalTurn: () => false,
})

describe("FA-RT-02 fleet contention rotation", () => {
  test("a run whose bound lane is BUSY rotates to the ready sibling lane in the same pass, records a typed lane_busy rotation, and consumes NO failure budget", async () => {
    await withRegistry(async registry => {
      registry.set("thread.busy", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      })
      registry.bindRoutingPolicy("thread.busy", [{ lane: "codex-local" }, { lane: "claude-local" }])

      const dispatched: Array<string | undefined> = []
      const rotations: Array<{ fromLane: string; toLane: string; reason: string }> = []
      const dispatchedThreads = await reconcileFullAutoThreads({
        ...baseInput(registry),
        // codex-local is saturated; claude-local is free.
        laneReady: ({ lane }) => lane !== "codex-local",
        dispatch: async ({ profile }) => {
          dispatched.push(profile?.lane)
          return { ok: profile?.lane === "claude-local" }
        },
        onRotated: (_thread, rotation) => rotations.push(rotation),
      })

      expect(dispatchedThreads).toEqual(["thread.busy"])
      // It never dispatched onto the busy codex lane; it went straight to claude.
      expect(dispatched).toEqual(["claude-local"])
      expect(rotations).toEqual([
        { fromLane: "codex-local", toLane: "claude-local", reason: "lane_busy", at: expect.any(String) },
      ])
      const record = registry.record("thread.busy")
      expect(record?.enabled).toBe(true)
      // A pure fleet-spread never touched the FA-H5 failure budget.
      expect(record?.consecutiveFailures ?? 0).toBe(0)
      // The next continuation now starts on the lane that actually worked.
      expect(record?.profile?.lane).toBe("claude-local")
    })
  })

  test("TWO runs pinned to the same lane no longer deadlock: one takes the lane, the other rotates to the ready sibling, and BOTH progress in one pass", async () => {
    await withRegistry(async registry => {
      for (const threadRef of ["thread.a", "thread.b"]) {
        registry.set(threadRef, true, { workspaceRef: "/workspace", profile: { lane: "codex-local" } })
        registry.bindRoutingPolicy(threadRef, [{ lane: "codex-local" }, { lane: "claude-local" }])
      }

      // Each lane runs exactly one owner-local turn at a time: a lane is busy
      // for a thread when a DIFFERENT thread already holds it. `dispatch`
      // reserves the lane synchronously (before its first await), exactly as
      // the live path stamps turn_running before the provider call, so the
      // second worker observes the reservation.
      const heldBy = new Map<string, string>()
      const landed = new Map<string, string>()
      const dispatchedThreads = await reconcileFullAutoThreads({
        ...baseInput(registry),
        laneReady: ({ threadRef, lane }) => {
          const holder = heldBy.get(lane ?? "")
          return holder === undefined || holder === threadRef
        },
        dispatch: async ({ threadRef, profile }) => {
          heldBy.set(profile!.lane!, threadRef)
          landed.set(threadRef, profile!.lane!)
          await Promise.resolve()
          return { ok: true }
        },
      })

      // Both runs progressed (no deadlock, no dispatch_overdue stall).
      expect([...dispatchedThreads].sort()).toEqual(["thread.a", "thread.b"])
      // They SPREAD across the fleet -- one per lane, never both on codex.
      expect(new Set(landed.values())).toEqual(new Set(["codex-local", "claude-local"]))
    })
  })

  test("when EVERY admitted candidate is busy the pass WAITS (no dispatch, no failure budget, no disable) instead of piling a doomed second turn", async () => {
    await withRegistry(async registry => {
      registry.set("thread.wait", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      })
      registry.bindRoutingPolicy("thread.wait", [{ lane: "codex-local" }, { lane: "claude-local" }])

      let dispatchCalls = 0
      let failures = 0
      const dispatchedThreads = await reconcileFullAutoThreads({
        ...baseInput(registry),
        laneReady: () => false, // the whole fleet is saturated
        dispatch: async () => {
          dispatchCalls += 1
          return { ok: true }
        },
        onDispatchFailed: () => {
          failures += 1
        },
      })

      expect(dispatchedThreads).toEqual([])
      expect(dispatchCalls).toBe(0)
      expect(failures).toBe(0)
      const record = registry.record("thread.wait")
      expect(record?.enabled).toBe(true)
      expect(record?.consecutiveFailures ?? 0).toBe(0)
    })
  })

  test("a single pinned lane that is busy WAITS rather than hanging on a doomed turn (no routing policy, no budget consumed)", async () => {
    await withRegistry(async registry => {
      registry.set("thread.pinned", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      })

      let dispatchCalls = 0
      const dispatchedThreads = await reconcileFullAutoThreads({
        ...baseInput(registry),
        laneReady: () => false,
        dispatch: async () => {
          dispatchCalls += 1
          return { ok: true }
        },
      })

      expect(dispatchedThreads).toEqual([])
      expect(dispatchCalls).toBe(0)
      expect(registry.record("thread.pinned")?.enabled).toBe(true)
    })
  })

  test("a lane that LOST admission (full_auto_lane_not_eligible) rotates to its next admitted candidate WITHOUT a pre-check and without consuming failure budget", async () => {
    await withRegistry(async registry => {
      registry.set("thread.lost", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      })
      registry.bindRoutingPolicy("thread.lost", [{ lane: "codex-local" }, { lane: "claude-local" }])

      const dispatched: Array<string | undefined> = []
      const rotations: Array<{ reason: string }> = []
      await reconcileFullAutoThreads({
        ...baseInput(registry),
        // No laneReady gate at all -- the rotation must come from classifying
        // the per-dispatch lane gate's typed refusal reason.
        dispatch: async ({ profile }) => {
          dispatched.push(profile?.lane)
          return profile?.lane === "codex-local"
            ? { ok: false, reason: "full_auto_lane_not_eligible:codex-local" }
            : { ok: true }
        },
        onRotated: (_thread, rotation) => rotations.push({ reason: rotation.reason }),
      })

      expect(dispatched).toEqual(["codex-local", "claude-local"])
      expect(rotations).toEqual([{ reason: "lane_unavailable" }])
      expect(registry.record("thread.lost")?.consecutiveFailures ?? 0).toBe(0)
    })
  })

  test("a SINGLE pinned lane that lost admission still fails CLOSED (consumes FA-H5 budget) -- lane_unavailable is not treated as transient with no alternative", async () => {
    await withRegistry(async registry => {
      registry.set("thread.dead", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      })

      let failures = 0
      await reconcileFullAutoThreads({
        ...baseInput(registry),
        dispatch: async () => ({ ok: false, reason: "full_auto_lane_not_eligible:codex-local" }),
        onDispatchFailed: (_thread, failure) => {
          failures = failure.consecutiveFailures
        },
      })

      expect(failures).toBe(1)
      expect(registry.record("thread.dead")?.consecutiveFailures).toBe(1)
    })
  })

  test("with NO laneReady gate the loop behaves byte-for-byte as before: it dispatches straight onto the bound lane", async () => {
    await withRegistry(async registry => {
      registry.set("thread.plain", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      })

      const dispatched: Array<string | undefined> = []
      await reconcileFullAutoThreads({
        ...baseInput(registry),
        dispatch: async ({ profile }) => {
          dispatched.push(profile?.lane)
          return { ok: true }
        },
      })

      expect(dispatched).toEqual(["codex-local"])
      expect(registry.record("thread.plain")?.continuationCount).toBe(1)
    })
  })
})
