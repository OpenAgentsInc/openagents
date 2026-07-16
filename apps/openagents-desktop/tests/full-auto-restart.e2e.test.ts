import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openFullAutoRegistry } from "../src/full-auto-registry.ts"
import { FULL_AUTO_MAX_CONTINUATIONS, reconcileFullAutoThreads } from "../src/full-auto-reconcile.ts"

/**
 * Full Auto (#8853) restart-survival proof, following the exact "Runtime A
 * seeds durable state to disk, Runtime B re-opens the same files and
 * reconciles" shape as local-turn-restart.e2e.test.ts. No Electron process is
 * spawned: the registry and the reconcile decision are plain modules, so
 * "process A" and "process B" are just two independent opens of the same
 * on-disk registry file, exactly like the existing turn-journal proof does
 * for interrupted-turn recovery.
 */
describe("Full Auto process restart", () => {
  test("a thread left enabled by Runtime A resumes on Runtime B with no manual re-toggle or re-send", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-restart-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      // Runtime A: owner toggled Full Auto on for this thread, sent one
      // message, then quit the app right after that turn completed cleanly
      // (no turn left in flight).
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-restart", true)

      // Runtime B: a fresh process, re-opening the same durable file. Nothing
      // is in flight for this thread (Runtime A's own turn already
      // completed), so reconciliation must dispatch the next continuation on
      // its own -- this is the actual restart-survival behavior.
      const registryB = openFullAutoRegistry(registryFile)
      const dispatched: Array<{ threadRef: string; message: string }> = []
      const dispatchedThreads = await reconcileFullAutoThreads({
        registry: registryB,
        nonterminalThreadRefs: () => new Set(),
        dispatch: async input => {
          dispatched.push(input)
          return { ok: true }
        },
      })
      expect(dispatchedThreads).toEqual(["thread-restart"])
      expect(dispatched).toEqual([{
        threadRef: "thread-restart",
        message: expect.stringContaining("Continue Full Auto"),
      }])
      expect(registryB.get("thread-restart")).toBe(true)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a thread with a turn still in flight at restart is left alone until that turn resolves", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-restart-inflight-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-inflight", true)

      const registryB = openFullAutoRegistry(registryFile)
      let dispatchCount = 0
      const dispatchedThreads = await reconcileFullAutoThreads({
        registry: registryB,
        // Existing turn-recovery (reconcileLocalTurns) is still resolving
        // this thread's interrupted turn; Full Auto must not race it.
        nonterminalThreadRefs: () => new Set(["thread-inflight"]),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })
      expect(dispatchedThreads).toEqual([])
      expect(dispatchCount).toBe(0)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("toggling off before restart durably stops it -- Runtime B never dispatches", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-restart-off-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-stopped", true)
      registryA.set("thread-stopped", false)

      const registryB = openFullAutoRegistry(registryFile)
      let dispatchCount = 0
      const dispatchedThreads = await reconcileFullAutoThreads({
        registry: registryB,
        nonterminalThreadRefs: () => new Set(),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })
      expect(dispatchedThreads).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(registryB.get("thread-stopped")).toBe(false)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a genuinely stuck loop self-disables at the continuation cap across restarts, rather than continuing unbounded", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-restart-cap-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-cap", true)
      for (let index = 0; index < FULL_AUTO_MAX_CONTINUATIONS; index += 1) registry.incrementContinuation("thread-cap")

      let capReachedFor: string | null = null
      let dispatchCount = 0
      const dispatchedThreads = await reconcileFullAutoThreads({
        registry,
        nonterminalThreadRefs: () => new Set(),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onCapReached: threadRef => { capReachedFor = threadRef },
      })
      expect(dispatchedThreads).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(capReachedFor).toBe("thread-cap")
      expect(registry.get("thread-cap")).toBe(false)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
