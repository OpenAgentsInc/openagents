import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { decodeCodexLocalContinuationProfile } from "../src/codex-local-contract.ts"
import { openFullAutoRegistry, type FullAutoRegistry } from "../src/full-auto-registry.ts"
import {
  applyFullAutoComposerToggle,
  FULL_AUTO_MAX_CONSECUTIVE_FAILURES,
  FULL_AUTO_MAX_CONTINUATIONS,
  fullAutoFailureBackoffMs,
  makeSerialTaskQueue,
  reconcileFullAutoThreads,
} from "../src/full-auto-reconcile.ts"
import { makeThreadStore } from "../src/thread-store.ts"

/**
 * Full Auto (#8853) restart-survival proof, following the exact "Runtime A
 * seeds durable state to disk, Runtime B re-opens the same files and
 * reconciles" shape as local-turn-restart.e2e.test.ts. No Electron process is
 * spawned: the registry and the reconcile decision are plain modules, so
 * "process A" and "process B" are just two independent opens of the same
 * on-disk registry file, exactly like the existing turn-journal proof does
 * for interrupted-turn recovery.
 *
 * Wave 2 (FA-H2 #8875, FA-H3 #8876, FA-H5 #8878, FA-H6 #8879) adds workspace
 * binding, the exactly-once dispatch lease, the failure/backoff policy, and
 * profile continuity, including the audit's adversarial probes converted into
 * retained regression tests.
 */

const GRANTED_WORKSPACE = "/granted/workspace-a"

/** Reconcile with the wave-2 required capabilities defaulted for the common
 * "same workspace, empty journal" case; tests override per scenario. */
const reconcile = (
  registry: FullAutoRegistry,
  overrides: Partial<Parameters<typeof reconcileFullAutoThreads>[0]> & Pick<Parameters<typeof reconcileFullAutoThreads>[0], "dispatch">,
) => reconcileFullAutoThreads({
  registry,
  nonterminalThreadRefs: () => new Set<string>(),
  resolveWorkspaceRef: () => GRANTED_WORKSPACE,
  journalHasNonterminalTurn: () => false,
  ...overrides,
})

describe("Full Auto process restart", () => {
  test("enabling from the composer schedules the first autonomous turn immediately", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-toggle-go-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      let scheduled = 0
      applyFullAutoComposerToggle({
        registry,
        threadRef: "thread-new-session",
        enabled: true,
        workspaceRef: GRANTED_WORKSPACE,
        profile: { lane: "codex-local" },
        scheduleReconciliation: () => { scheduled += 1 },
      })

      expect(scheduled).toBe(1)
      expect(registry.get("thread-new-session")).toBe(true)
      expect(registry.record("thread-new-session")?.workspaceRef).toBe(GRANTED_WORKSPACE)

      const dispatched: string[] = []
      await reconcile(registry, {
        dispatch: async ({ message }) => {
          dispatched.push(message)
          return { ok: true }
        },
      })
      expect(dispatched).toEqual([expect.stringContaining("Continue Full Auto")])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("disabling from the composer does not schedule another turn", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-toggle-stop-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-stop", true, { workspaceRef: GRANTED_WORKSPACE })
      let scheduled = 0
      applyFullAutoComposerToggle({
        registry,
        threadRef: "thread-stop",
        enabled: false,
        workspaceRef: GRANTED_WORKSPACE,
        profile: { lane: "codex-local" },
        scheduleReconciliation: () => { scheduled += 1 },
      })
      expect(scheduled).toBe(0)
      expect(registry.get("thread-stop")).toBe(false)
      expect(registry.record("thread-stop")?.disabledBy).toBe("ui_toggle")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a thread left enabled by Runtime A resumes on Runtime B with no manual re-toggle or re-send", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-restart-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      // Runtime A: owner toggled Full Auto on for this thread (main bound the
      // granted workspace at enable time), sent one message, then quit the app
      // right after that turn completed cleanly (no turn left in flight).
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-restart", true, { workspaceRef: GRANTED_WORKSPACE })

      // Runtime B: a fresh process, re-opening the same durable file. Nothing
      // is in flight for this thread (Runtime A's own turn already
      // completed), so reconciliation must dispatch the next continuation on
      // its own -- this is the actual restart-survival behavior.
      const registryB = openFullAutoRegistry(registryFile)
      const dispatched: Array<{ threadRef: string; turnRef: string; message: string }> = []
      const dispatchedThreads = await reconcile(registryB, {
        dispatch: async input => {
          dispatched.push({ threadRef: input.threadRef, turnRef: input.turnRef, message: input.message })
          return { ok: true }
        },
      })
      expect(dispatchedThreads).toEqual(["thread-restart"])
      expect(dispatched).toEqual([{
        threadRef: "thread-restart",
        turnRef: expect.stringMatching(/^turn\.full-auto\./),
        message: expect.stringContaining("Continue Full Auto"),
      }])
      expect(registryB.get("thread-restart")).toBe(true)
      // The successful dispatch consumed exactly one cap slot and released
      // the lease (FA-H3/FA-H5).
      const record = registryB.record("thread-restart")
      expect(record?.continuationCount).toBe(1)
      expect(record?.pendingTurnRef ?? null).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a thread with a turn still in flight at restart is left alone until that turn resolves", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-restart-inflight-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-inflight", true, { workspaceRef: GRANTED_WORKSPACE })

      const registryB = openFullAutoRegistry(registryFile)
      let dispatchCount = 0
      const dispatchedThreads = await reconcile(registryB, {
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
      registryA.set("thread-stopped", true, { workspaceRef: GRANTED_WORKSPACE })
      registryA.set("thread-stopped", false, { disabledBy: "ui_toggle" })

      const registryB = openFullAutoRegistry(registryFile)
      let dispatchCount = 0
      const dispatchedThreads = await reconcile(registryB, {
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
      registry.set("thread-cap", true, { workspaceRef: GRANTED_WORKSPACE })
      for (let index = 0; index < FULL_AUTO_MAX_CONTINUATIONS; index += 1) registry.incrementContinuation("thread-cap")

      let capReachedFor: string | null = null
      let dispatchCount = 0
      const dispatchedThreads = await reconcile(registry, {
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onCapReached: threadRef => { capReachedFor = threadRef },
      })
      expect(dispatchedThreads).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(capReachedFor).toBe("thread-cap")
      expect(registry.get("thread-cap")).toBe(false)
      expect(registry.record("thread-cap")?.blockedReason).toBe("continuation_cap_reached")
      expect(registry.record("thread-cap")?.disabledBy).toBe("continuation_cap")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto workspace binding (FA-H2 #8875)", () => {
  test("enable on workspace A, resolve workspace B at reconcile -> no dispatch, record disabled with workspace_mismatch, block reported", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-workspace-mismatch-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-ws", true, { workspaceRef: "/repo/a" })

      let dispatchCount = 0
      const blocks: Array<{ threadRef: string; reason: string; grantedWorkspaceRef: string | null; resolvedWorkspaceRef: string }> = []
      const dispatchedThreads = await reconcile(registry, {
        resolveWorkspaceRef: () => "/repo/b",
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onWorkspaceBlocked: (threadRef, block) => { blocks.push({ threadRef, ...block }) },
      })
      expect(dispatchedThreads).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(registry.get("thread-ws")).toBe(false)
      expect(registry.record("thread-ws")?.blockedReason).toBe("workspace_mismatch")
      expect(registry.record("thread-ws")?.disabledBy).toBe("workspace_guard")
      expect(blocks).toEqual([{
        threadRef: "thread-ws",
        reason: "workspace_mismatch",
        grantedWorkspaceRef: "/repo/a",
        resolvedWorkspaceRef: "/repo/b",
      }])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("an enabled record with NO workspace binding (pre-upgrade v1 row) fails CLOSED: no dispatch, disabled with workspace_unbound", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-workspace-unbound-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      // Pre-upgrade shape: enabled without a workspaceRef (no options).
      registry.set("thread-unbound", true)
      expect(registry.record("thread-unbound")?.workspaceRef).toBeUndefined()

      let dispatchCount = 0
      const blocks: Array<{ reason: string; grantedWorkspaceRef: string | null }> = []
      const dispatchedThreads = await reconcile(registry, {
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onWorkspaceBlocked: (_threadRef, block) => {
          blocks.push({ reason: block.reason, grantedWorkspaceRef: block.grantedWorkspaceRef })
        },
      })
      expect(dispatchedThreads).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(registry.get("thread-unbound")).toBe(false)
      expect(registry.record("thread-unbound")?.blockedReason).toBe("workspace_unbound")
      expect(registry.record("thread-unbound")?.disabledBy).toBe("workspace_guard")
      expect(blocks).toEqual([{ reason: "workspace_unbound", grantedWorkspaceRef: null }])

      // The next successful ENABLE rebinds (main passes the resolved
      // workspace), after which the loop dispatches normally again.
      registry.set("thread-unbound", true, { workspaceRef: GRANTED_WORKSPACE })
      const resumed = await reconcile(registry, { dispatch: async () => ({ ok: true }) })
      expect(resumed).toEqual(["thread-unbound"])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto exactly-once dispatch (FA-H3 #8876)", () => {
  test("distinct enabled threads dispatch concurrently while each retains its own durable lease", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-concurrent-threads-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-codex", true, { workspaceRef: GRANTED_WORKSPACE, profile: { lane: "codex-local" } })
      registry.set("thread-claude", true, { workspaceRef: GRANTED_WORKSPACE, profile: { lane: "claude-local" } })

      const started: string[] = []
      let resolveBothStarted: (() => void) | null = null
      let release: (() => void) | null = null
      const bothStarted = new Promise<void>(resolve => { resolveBothStarted = resolve })
      const gate = new Promise<void>(resolve => { release = resolve })
      const reconciling = reconcile(registry, {
        dispatch: async ({ threadRef }) => {
          started.push(threadRef)
          if (started.length === 2) resolveBothStarted!()
          await gate
          return { ok: true }
        },
      })

      await Promise.race([
        bothStarted,
        new Promise<never>((_, reject) => setTimeout(() => reject(new Error("distinct threads did not overlap")), 1_000)),
      ])
      expect(started.toSorted()).toEqual(["thread-claude", "thread-codex"])
      expect(registry.record("thread-codex")?.pendingTurnRef).toMatch(/^turn\.full-auto\./)
      expect(registry.record("thread-claude")?.pendingTurnRef).toMatch(/^turn\.full-auto\./)

      release!()
      await expect(reconciling.then(threadRefs => threadRefs.toSorted())).resolves.toEqual(["thread-claude", "thread-codex"])
      expect(registry.record("thread-codex")?.continuationCount).toBe(1)
      expect(registry.record("thread-claude")?.continuationCount).toBe(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a staggered second start overlaps the first provider turn instead of waiting for that reconciliation pass", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-staggered-threads-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-first", true, { workspaceRef: GRANTED_WORKSPACE, profile: { lane: "codex-local" } })

      let resolveFirstStarted: (() => void) | null = null
      let resolveSecondStarted: (() => void) | null = null
      let release: (() => void) | null = null
      const firstStarted = new Promise<void>(resolve => { resolveFirstStarted = resolve })
      const secondStarted = new Promise<void>(resolve => { resolveSecondStarted = resolve })
      const gate = new Promise<void>(resolve => { release = resolve })
      const dispatch = async ({ threadRef }: { threadRef: string }) => {
        if (threadRef === "thread-first") resolveFirstStarted!()
        if (threadRef === "thread-second") resolveSecondStarted!()
        await gate
        return { ok: true }
      }

      const firstPass = reconcile(registry, { dispatch })
      await firstStarted
      registry.set("thread-second", true, { workspaceRef: GRANTED_WORKSPACE, profile: { lane: "claude-local" } })
      const secondPass = reconcile(registry, { dispatch })
      await Promise.race([
        secondStarted,
        new Promise<never>((_, reject) => setTimeout(() => reject(new Error("staggered thread waited behind first pass")), 1_000)),
      ])
      expect(registry.record("thread-first")?.pendingTurnRef).toMatch(/^turn\.full-auto\./)
      expect(registry.record("thread-second")?.pendingTurnRef).toMatch(/^turn\.full-auto\./)

      release!()
      const [firstResult, secondResult] = await Promise.all([firstPass, secondPass])
      expect(firstResult).toEqual(["thread-first"])
      expect(secondResult).toEqual(["thread-second"])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("audit probe (a): two overlapping reconcile passes against one enabled thread dispatch it exactly ONCE (durable lease), and continuationCount increments by exactly 1", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-overlap-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-race", true, { workspaceRef: GRANTED_WORKSPACE })

      let dispatchesBeforeRelease = 0
      let release: (() => void) | null = null
      const gate = new Promise<void>(resolve => { release = resolve })
      const dispatch = async () => {
        dispatchesBeforeRelease += 1
        await gate
        return { ok: true }
      }
      const first = reconcile(registry, { dispatch })
      const second = reconcile(registry, { dispatch })
      // Let both passes reach (or skip) the dispatch point before releasing.
      await new Promise(resolve => setTimeout(resolve, 10))
      expect(dispatchesBeforeRelease).toBe(1)
      release!()
      const [firstThreads, secondThreads] = await Promise.all([first, second])
      expect([...firstThreads, ...secondThreads]).toEqual(["thread-race"])
      expect(registry.record("thread-race")?.continuationCount).toBe(1)
      expect(registry.record("thread-race")?.pendingTurnRef ?? null).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("the serial task queue serializes overlapping reconciliation triggers: the second task starts only after the first resolves, and a rejection never blocks the chain", async () => {
    const queue = makeSerialTaskQueue()
    const order: string[] = []
    let releaseA: (() => void) | null = null
    const gateA = new Promise<void>(resolve => { releaseA = resolve })
    const a = queue(async () => { order.push("a-start"); await gateA; order.push("a-end") })
    const b = queue(async () => { order.push("b-start") })
    await new Promise(resolve => setTimeout(resolve, 5))
    expect(order).toEqual(["a-start"])
    releaseA!()
    await Promise.all([a, b])
    expect(order).toEqual(["a-start", "a-end", "b-start"])

    const failing = queue(async () => { throw new Error("boom") })
    await expect(failing).rejects.toThrow("boom")
    await expect(queue(async () => "after-failure")).resolves.toBe("after-failure")
  })

  test("a stale lease (crashed mid-dispatch: no journal row for its turn ref) is cleared ONLY by the startup pass, then the thread dispatches under a fresh identity", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-stale-lease-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      // Runtime A claimed a lease, then crashed before its turn was accepted
      // into the local-turn journal.
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-stale", true, { workspaceRef: GRANTED_WORKSPACE })
      expect(registryA.claimPending("thread-stale", "turn.full-auto.crashed")).toBe(true)

      const registryB = openFullAutoRegistry(registryFile)
      // A mid-session pass must treat the held lease as in-flight and skip.
      let midSessionDispatches = 0
      const midSession = await reconcile(registryB, {
        dispatch: async () => { midSessionDispatches += 1; return { ok: true } },
      })
      expect(midSession).toEqual([])
      expect(midSessionDispatches).toBe(0)
      expect(registryB.record("thread-stale")?.pendingTurnRef).toBe("turn.full-auto.crashed")

      // The startup pass clears the stale lease (the journal has no
      // nonterminal row for it) and dispatches under a fresh turn ref.
      const startupDispatched: string[] = []
      const startup = await reconcile(registryB, {
        clearStaleLeases: true,
        journalHasNonterminalTurn: turnRef => {
          expect(turnRef).toBe("turn.full-auto.crashed")
          return false
        },
        dispatch: async input => {
          startupDispatched.push(input.turnRef)
          return { ok: true }
        },
      })
      expect(startup).toEqual(["thread-stale"])
      expect(startupDispatched).toHaveLength(1)
      expect(startupDispatched[0]).not.toBe("turn.full-auto.crashed")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a lease whose turn IS still nonterminal in the journal is NOT cleared at startup (the thread is skipped as in flight)", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-live-lease-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-live", true, { workspaceRef: GRANTED_WORKSPACE })
      expect(registry.claimPending("thread-live", "turn.full-auto.live")).toBe(true)

      let dispatchCount = 0
      const dispatched = await reconcile(registry, {
        clearStaleLeases: true,
        // The crashed process's turn WAS accepted; recovery owns it.
        nonterminalThreadRefs: () => new Set(["thread-live"]),
        journalHasNonterminalTurn: () => true,
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })
      expect(dispatched).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(registry.record("thread-live")?.pendingTurnRef).toBe("turn.full-auto.live")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto dispatch-failure policy (FA-H5 #8878)", () => {
  test("audit probe (b): an { ok: false } dispatch is a typed, visible failure -- failure state persists, the callback fires, and the lease clears; the record is never silently dormant", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-okfalse-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-fail", true, { workspaceRef: GRANTED_WORKSPACE })

      const failures: Array<{ threadRef: string; reason: string; consecutiveFailures: number; disabled: boolean }> = []
      const dispatchedThreads = await reconcile(registry, {
        dispatch: async () => ({ ok: false, reason: "account_exhausted" }),
        onDispatchFailed: (threadRef, failure) => { failures.push({ threadRef, ...failure }) },
      })
      expect(dispatchedThreads).toEqual([])
      expect(failures).toEqual([{
        threadRef: "thread-fail",
        reason: "account_exhausted",
        consecutiveFailures: 1,
        disabled: false,
      }])
      const record = registry.record("thread-fail")
      expect(record?.enabled).toBe(true)
      expect(record?.consecutiveFailures).toBe(1)
      expect(record?.lastFailureAt).toBeDefined()
      expect(record?.blockedReason).toBe("account_exhausted")
      expect(record?.pendingTurnRef ?? null).toBe(null)
      // A failed dispatch never consumes a cap slot (documented decision).
      expect(record?.continuationCount).toBe(0)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a thrown dispatch is the same typed failure outcome as ok:false", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-throw-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-throw", true, { workspaceRef: GRANTED_WORKSPACE })

      const failures: Array<{ reason: string }> = []
      await reconcile(registry, {
        dispatch: async () => { throw new Error("socket closed") },
        onDispatchFailed: (_threadRef, failure) => { failures.push({ reason: failure.reason }) },
      })
      expect(failures).toEqual([{ reason: "Error: socket closed" }])
      expect(registry.record("thread-throw")?.consecutiveFailures).toBe(1)
      expect(registry.record("thread-throw")?.pendingTurnRef ?? null).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("the bounded backoff window skips dispatch after a failure, then allows it once the window has passed", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-backoff-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-backoff", true, { workspaceRef: GRANTED_WORKSPACE })

      await reconcile(registry, { dispatch: async () => ({ ok: false, reason: "transient" }) })
      const failedAt = Date.parse(registry.record("thread-backoff")!.lastFailureAt!)
      const windowMs = fullAutoFailureBackoffMs(1)

      // Inside the window: skipped, with no state change.
      let dispatchCount = 0
      const insideWindow = await reconcile(registry, {
        now: () => new Date(failedAt + windowMs - 1),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })
      expect(insideWindow).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(registry.record("thread-backoff")?.consecutiveFailures).toBe(1)

      // Past the window: retried; success clears the failure state.
      const pastWindow = await reconcile(registry, {
        now: () => new Date(failedAt + windowMs + 1),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })
      expect(pastWindow).toEqual(["thread-backoff"])
      expect(dispatchCount).toBe(1)
      const record = registry.record("thread-backoff")
      expect(record?.consecutiveFailures ?? 0).toBe(0)
      expect(record?.lastFailureAt).toBeUndefined()
      expect(record?.blockedReason ?? null).toBe(null)
      expect(record?.continuationCount).toBe(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test(`the ${FULL_AUTO_MAX_CONSECUTIVE_FAILURES}th consecutive failure disables the record with a blockedReason and reports disabled: true`, async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-disable-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-limit", true, { workspaceRef: GRANTED_WORKSPACE })

      const outcomes: Array<{ consecutiveFailures: number; disabled: boolean }> = []
      for (let attempt = 0; attempt < FULL_AUTO_MAX_CONSECUTIVE_FAILURES; attempt += 1) {
        const failedAt = registry.record("thread-limit")?.lastFailureAt
        await reconcile(registry, {
          // Step the clock past each backoff window so every attempt runs.
          now: () => new Date(
            (failedAt === undefined ? Date.now() : Date.parse(failedAt)) +
            fullAutoFailureBackoffMs(attempt) + 1,
          ),
          dispatch: async () => ({ ok: false, reason: "runtime_unavailable" }),
          onDispatchFailed: (_threadRef, failure) => {
            outcomes.push({ consecutiveFailures: failure.consecutiveFailures, disabled: failure.disabled })
          },
        })
      }
      expect(outcomes).toHaveLength(FULL_AUTO_MAX_CONSECUTIVE_FAILURES)
      expect(outcomes.at(-1)).toEqual({ consecutiveFailures: FULL_AUTO_MAX_CONSECUTIVE_FAILURES, disabled: true })
      expect(outcomes.slice(0, -1).every(outcome => !outcome.disabled)).toBe(true)
      expect(registry.get("thread-limit")).toBe(false)
      expect(registry.record("thread-limit")?.blockedReason).toBe("runtime_unavailable")
      expect(registry.record("thread-limit")?.disabledBy).toBe("dispatch_failure_limit")

      // Disabled means no further dispatch attempts at all.
      let dispatchCount = 0
      await reconcile(registry, { dispatch: async () => { dispatchCount += 1; return { ok: true } } })
      expect(dispatchCount).toBe(0)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("failed dispatches never consume cap slots: fail once then succeed -> continuationCount is exactly 1", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-cap-count-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-count", true, { workspaceRef: GRANTED_WORKSPACE })

      await reconcile(registry, { dispatch: async () => ({ ok: false, reason: "transient" }) })
      expect(registry.record("thread-count")?.continuationCount).toBe(0)

      const failedAt = Date.parse(registry.record("thread-count")!.lastFailureAt!)
      await reconcile(registry, {
        now: () => new Date(failedAt + fullAutoFailureBackoffMs(1) + 1),
        dispatch: async () => ({ ok: true }),
      })
      expect(registry.record("thread-count")?.continuationCount).toBe(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto execution-profile continuity (FA-H6 #8879)", () => {
  test("a Claude lane selection survives Runtime A -> Runtime B and reaches the shared dispatch seam", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-claude-restart-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-claude", true, {
        workspaceRef: GRANTED_WORKSPACE,
        profile: { lane: "claude-local", accountRef: "claude", model: "claude-sonnet-5" },
      })

      const registryB = openFullAutoRegistry(registryFile)
      const profiles: Array<unknown> = []
      expect(await reconcile(registryB, {
        dispatch: async input => { profiles.push(input.profile); return { ok: true } },
      })).toEqual(["thread-claude"])
      expect(profiles).toEqual([{
        lane: "claude-local",
        accountRef: "claude",
        model: "claude-sonnet-5",
      }])
      expect(registryB.record("thread-claude")?.continuationCount).toBe(1)
      expect(registryB.record("thread-claude")?.pendingTurnRef ?? null).toBeNull()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a continuation dispatch carries the profile bound by the initiating flagged turn (account, model, effort) -- including across a restart", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-profile-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      // Runtime A: enable + first flagged turn with an explicit target and
      // high effort; main binds that profile onto the record.
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-profile", true, { workspaceRef: GRANTED_WORKSPACE })
      registryA.bindProfile("thread-profile", {
        accountRef: "codex-2",
        model: "gpt-5.5",
        reasoningEffort: "high",
      })

      // Runtime B (fresh process): the continuation dispatch input carries
      // the exact bound profile.
      const registryB = openFullAutoRegistry(registryFile)
      const profiles: Array<unknown> = []
      const dispatched = await reconcile(registryB, {
        dispatch: async input => {
          profiles.push(input.profile)
          return { ok: true }
        },
      })
      expect(dispatched).toEqual(["thread-profile"])
      expect(profiles).toEqual([{ accountRef: "codex-2", model: "gpt-5.5", reasoningEffort: "high" }])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("decodeCodexLocalContinuationProfile revalidates stored strings against the live contract: valid fields narrow, stale/invalid fields drop to null (lane defaults)", () => {
    expect(decodeCodexLocalContinuationProfile({
      accountRef: "codex-2",
      model: "gpt-5.5",
      reasoningEffort: "high",
    })).toEqual({ accountRef: "codex-2", model: "gpt-5.5", reasoningEffort: "high" })
    // A model absent from the current installed catalog (or a Claude model on
    // the Codex lane) must not crash the loop -- it falls back to defaults.
    expect(decodeCodexLocalContinuationProfile({
      model: "gpt-4-retired",
      reasoningEffort: "impossible",
    }, ["gpt-5.6-sol", "gpt-5.5"])).toEqual({ accountRef: null, model: null, reasoningEffort: null })
    expect(decodeCodexLocalContinuationProfile({ model: "claude-fable-5" }).model).toBe(null)
    expect(decodeCodexLocalContinuationProfile(undefined)).toEqual({
      accountRef: null,
      model: null,
      reasoningEffort: null,
    })
  })
})

describe("Full Auto multi-lane never-halt rotation (FA-RT-01 #8987)", () => {
  const enableWithPolicy = (
    registry: FullAutoRegistry,
    threadRef: string,
    policy: ReadonlyArray<{ lane: string; accountRef?: string }>,
    profile: { lane?: string; accountRef?: string; model?: string } = { lane: policy[0]!.lane },
  ) => {
    registry.set(threadRef, true, { workspaceRef: GRANTED_WORKSPACE, profile, routingPolicy: policy })
  }

  test.each(["account_exhausted", "rate_limited", "provider_error"] as const)(
    "a typed %s failure on candidate 1 continues on candidate 2 in the SAME pass: rotation record persisted, NO failure budget consumed, profile rebound",
    async failureClass => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rotate-"))
      try {
        const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
        enableWithPolicy(registry, "thread-rotate", [
          { lane: "codex-local", accountRef: "codex-1" },
          { lane: "claude-local", accountRef: "claude-1" },
        ])

        const attempts: Array<{ turnRef: string; lane: string | undefined; accountRef: string | undefined }> = []
        const rotations: Array<{ fromLane: string; toLane: string; reason: string }> = []
        const failures: Array<string> = []
        const dispatched = await reconcile(registry, {
          dispatch: async input => {
            attempts.push({
              turnRef: input.turnRef,
              lane: input.profile?.lane,
              accountRef: input.profile?.accountRef,
            })
            return input.profile?.lane === "codex-local"
              ? { ok: false, reason: "lane refused", failureClass }
              : { ok: true }
          },
          onRotated: (_threadRef, rotation) => {
            rotations.push({ fromLane: rotation.fromLane, toLane: rotation.toLane, reason: rotation.reason })
          },
          onDispatchFailed: (_threadRef, failure) => { failures.push(failure.reason) },
        })

        // The run never halted: candidate 2 dispatched in the same pass.
        expect(dispatched).toEqual(["thread-rotate"])
        expect(attempts).toHaveLength(2)
        expect(attempts[0]).toMatchObject({ lane: "codex-local", accountRef: "codex-1" })
        expect(attempts[1]).toMatchObject({ lane: "claude-local", accountRef: "claude-1" })
        // Each attempt is its own leased turn identity.
        expect(attempts[0]!.turnRef).not.toBe(attempts[1]!.turnRef)
        // The rotation is a typed, persisted, owner-visible fact.
        expect(rotations).toEqual([{ fromLane: "codex-local", toLane: "claude-local", reason: failureClass }])
        const record = registry.record("thread-rotate")
        expect(record?.rotationHistory).toEqual([{
          fromLane: "codex-local",
          toLane: "claude-local",
          reason: failureClass,
          at: expect.any(String),
        }])
        // No failure budget consumed and no backoff entered (FA-H5 untouched
        // while an untried admitted candidate remained).
        expect(failures).toEqual([])
        expect(record?.consecutiveFailures ?? 0).toBe(0)
        expect(record?.lastFailureAt).toBeUndefined()
        expect(record?.blockedReason ?? null).toBe(null)
        expect(record?.enabled).toBe(true)
        expect(record?.continuationCount).toBe(1)
        expect(record?.pendingTurnRef ?? null).toBe(null)
        // The durable profile now points at the lane that actually worked,
        // so the NEXT continuation starts there.
        expect(record?.profile?.lane).toBe("claude-local")
        expect(record?.profile?.accountRef).toBe("claude-1")
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test("a FULL unsuccessful cycle consumes exactly ONE FA-H5 failure-budget step and records each intermediate rotation", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rotate-cycle-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      enableWithPolicy(registry, "thread-cycle", [
        { lane: "codex-local" },
        { lane: "claude-local" },
        { lane: "acp:grok-cli" },
      ])

      let attempts = 0
      const failures: Array<{ reason: string; consecutiveFailures: number; disabled: boolean }> = []
      const dispatched = await reconcile(registry, {
        dispatch: async () => {
          attempts += 1
          return { ok: false, reason: "account_exhausted", failureClass: "account_exhausted" }
        },
        onDispatchFailed: (_threadRef, failure) => { failures.push({ ...failure }) },
      })

      expect(dispatched).toEqual([])
      // Every candidate was tried exactly once in the one pass.
      expect(attempts).toBe(3)
      // ...but the whole cycle cost exactly ONE budget step.
      expect(failures).toEqual([{ reason: "account_exhausted", consecutiveFailures: 1, disabled: false }])
      const record = registry.record("thread-cycle")
      expect(record?.consecutiveFailures).toBe(1)
      expect(record?.enabled).toBe(true)
      expect(record?.lastFailureAt).toBeDefined()
      expect(record?.pendingTurnRef ?? null).toBe(null)
      // Two rotations happened (1->2, 2->3); the final failure is budget, not
      // a rotation.
      expect(record?.rotationHistory?.map(entry => `${entry.fromLane}>${entry.toLane}`)).toEqual([
        "codex-local>claude-local",
        "claude-local>acp:grok-cli",
      ])
      // A failed cycle never consumes a cap slot (FA-H5 pinned decision).
      expect(record?.continuationCount).toBe(0)

      // The NEXT pass respects the existing bounded backoff window exactly
      // as before: inside the window, no candidate is attempted at all.
      const failedAt = Date.parse(record!.lastFailureAt!)
      let insideWindowAttempts = 0
      await reconcile(registry, {
        now: () => new Date(failedAt + fullAutoFailureBackoffMs(1) - 1),
        dispatch: async () => { insideWindowAttempts += 1; return { ok: true } },
      })
      expect(insideWindowAttempts).toBe(0)

      // Past the window the cycle retries; repeated full cycles walk the
      // SAME disable-after-5 path as single-lane failures (regression:
      // existing cap/disable semantics unchanged).
      await reconcile(registry, {
        now: () => new Date(failedAt + fullAutoFailureBackoffMs(1) + 1),
        dispatch: async () => ({ ok: false, reason: "account_exhausted", failureClass: "account_exhausted" }),
      })
      expect(registry.record("thread-cycle")?.consecutiveFailures).toBe(2)
      expect(registry.record("thread-cycle")?.enabled).toBe(true)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("an UNTYPED failure (no failureClass) never rotates even with a policy bound -- it consumes budget immediately, exactly the pre-#8987 semantics", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rotate-untyped-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      enableWithPolicy(registry, "thread-untyped", [{ lane: "codex-local" }, { lane: "claude-local" }])

      let attempts = 0
      const failures: Array<string> = []
      await reconcile(registry, {
        dispatch: async () => { attempts += 1; return { ok: false, reason: "turn_already_in_flight" } },
        onDispatchFailed: (_threadRef, failure) => { failures.push(failure.reason) },
      })
      expect(attempts).toBe(1)
      expect(failures).toEqual(["turn_already_in_flight"])
      expect(registry.record("thread-untyped")?.consecutiveFailures).toBe(1)
      expect(registry.record("thread-untyped")?.rotationHistory).toBeUndefined()

      // A THROWN dispatch is equally untyped: no rotation.
      const thrownRegistry = openFullAutoRegistry(path.join(root, "full-auto", "registry2.json"))
      enableWithPolicy(thrownRegistry, "thread-thrown", [{ lane: "codex-local" }, { lane: "claude-local" }])
      let thrownAttempts = 0
      await reconcile(thrownRegistry, {
        dispatch: async () => { thrownAttempts += 1; throw new Error("socket closed") },
      })
      expect(thrownAttempts).toBe(1)
      expect(thrownRegistry.record("thread-thrown")?.consecutiveFailures).toBe(1)
      expect(thrownRegistry.record("thread-thrown")?.rotationHistory).toBeUndefined()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a record WITHOUT a routing policy keeps byte-for-byte legacy behavior even when the dispatch failure carries a typed class", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rotate-legacy-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-legacy", true, {
        workspaceRef: GRANTED_WORKSPACE,
        profile: { lane: "codex-local" },
      })

      let attempts = 0
      const failures: Array<string> = []
      await reconcile(registry, {
        dispatch: async () => {
          attempts += 1
          return { ok: false, reason: "account_exhausted", failureClass: "account_exhausted" }
        },
        onDispatchFailed: (_threadRef, failure) => { failures.push(failure.reason) },
      })
      // Exactly one attempt, budget consumed, no rotation invented.
      expect(attempts).toBe(1)
      expect(failures).toEqual(["account_exhausted"])
      expect(registry.record("thread-legacy")?.consecutiveFailures).toBe(1)
      expect(registry.record("thread-legacy")?.rotationHistory).toBeUndefined()
      expect(registry.record("thread-legacy")?.profile).toEqual({ lane: "codex-local" })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("the cycle starts at the currently bound lane, wraps through the ordered list, and a foreign-lane candidate never inherits the bound model string", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rotate-start-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      // Bound profile currently points at the SECOND candidate with a
      // Codex-family model; rotation must start there and wrap.
      enableWithPolicy(
        registry,
        "thread-start",
        [{ lane: "claude-local" }, { lane: "codex-local", accountRef: "codex-2" }, { lane: "acp:grok-cli" }],
        { lane: "codex-local", accountRef: "codex-2", model: "gpt-5.5" },
      )

      const attempts: Array<{ lane: string | undefined; model: string | undefined }> = []
      const dispatched = await reconcile(registry, {
        dispatch: async input => {
          attempts.push({ lane: input.profile?.lane, model: input.profile?.model })
          return attempts.length < 3
            ? { ok: false, reason: "rate limited", failureClass: "rate_limited" }
            : { ok: true }
        },
      })
      expect(dispatched).toEqual(["thread-start"])
      expect(attempts.map(attempt => attempt.lane)).toEqual(["codex-local", "acp:grok-cli", "claude-local"])
      // Same lane as the bound profile: model carries over. Foreign lanes:
      // model falls back to lane defaults (undefined), never another lane's
      // model string.
      expect(attempts[0]!.model).toBe("gpt-5.5")
      expect(attempts[1]!.model).toBeUndefined()
      expect(attempts[2]!.model).toBeUndefined()
      expect(registry.record("thread-start")?.rotationHistory?.map(entry => entry.toLane))
        .toEqual(["acp:grok-cli", "claude-local"])
      expect(registry.record("thread-start")?.profile?.lane).toBe("claude-local")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("rotation state survives a restart: Runtime B resumes on the rebound lane with the rotation history intact", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rotate-restart-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registryA = openFullAutoRegistry(registryFile)
      registryA.set("thread-rt-restart", true, {
        workspaceRef: GRANTED_WORKSPACE,
        profile: { lane: "codex-local" },
        routingPolicy: [{ lane: "codex-local" }, { lane: "claude-local" }],
      })
      await reconcile(registryA, {
        dispatch: async input =>
          input.profile?.lane === "codex-local"
            ? { ok: false, reason: "usage limit", failureClass: "account_exhausted" }
            : { ok: true },
      })

      const registryB = openFullAutoRegistry(registryFile)
      const record = registryB.record("thread-rt-restart")
      expect(record?.profile?.lane).toBe("claude-local")
      expect(record?.rotationHistory).toHaveLength(1)
      const lanes: Array<string | undefined> = []
      await reconcile(registryB, {
        dispatch: async input => { lanes.push(input.profile?.lane); return { ok: true } },
      })
      expect(lanes).toEqual(["claude-local"])
      expect(registryB.record("thread-rt-restart")?.continuationCount).toBe(2)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

/**
 * FA-RUN-02 (#8970): replays the 2026-07-17 overnight thread-eviction
 * incident against the REAL mutable thread-store cache (thread-store.ts),
 * not a registry-only fixture. Every other test in this file drives
 * `reconcileFullAutoThreads` against a plain in-memory `dispatch` stub, which
 * is exactly the shape the issue calls insufficient -- the actual defect
 * lived in the thread store's bounded-cache eviction policy, fixed by
 * `8cb900bbf9` (`compareDesktopThreadsByCreatedAt` -> a last-access compare),
 * not in the registry or the reconcile decision function.
 *
 * `makeIncidentDispatch` below mirrors the EXACT fail-closed check production
 * takes in provider-lane.ts's `dispatchTurn` (`store.open(threadRef) ===
 * null` -> the literal `"That conversation no longer exists."` string, which
 * flows unmodified into `FullAutoDispatchResult.reason` via main.ts's
 * `dispatch: async ({ threadRef, turnRef, message, profile }) => { ... return
 * result.ok ? { ok: true } : { ok: false, reason: result.error ?? ... } }`
 * wiring) -- so a regression here exercises the identical failure shape the
 * owner saw, not a synthetic stand-in for it.
 *
 * Composition proof (verified against this exact test file): reverting
 * thread-store.ts's `write()` to `8cb900bbf9`'s parent (sort by
 * `compareDesktopThreadsByCreatedAt` instead of by last access) makes both
 * tests below fail with the identical `"That conversation no longer
 * exists."` reason the incident produced, before the continuation that
 * should have succeeded. Restoring current `main`'s thread-store.ts makes
 * them pass again. See the FA-RUN-02 closeout comment on #8970 for the exact
 * revert/re-verify commands used to confirm this.
 */
describe("Full Auto composed multi-chat thread-store pressure (FA-RUN-02 #8970)", () => {
  /** Real Date.now() resolution is 1ms; a handful of synchronous store
   * writes in the same test can otherwise tie on `updatedAt`/`createdAt` and
   * fall back to a random UUID tiebreak, making the scenario's ordering
   * (and therefore the regression) flaky. A tiny real delay keeps every
   * mutation's timestamp strictly increasing. */
  const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 2))

  const makeIncidentDispatch = (store: ReturnType<typeof makeThreadStore>) =>
    async ({ threadRef, turnRef }: { threadRef: string; turnRef: string }) => {
      if (store.open(threadRef) === null) {
        return { ok: false, reason: "That conversation no longer exists." }
      }
      store.append(threadRef, {
        key: `${turnRef}-assistant`,
        role: "assistant",
        text: "Autonomous packet complete.",
        timestamp: "00:00",
      })
      return { ok: true }
    }

  test(
    "an older-created Full Auto thread survives six other top-level chats opened across three continuation cycles, and every continuation dispatches exactly once (this is the exact overnight composition: the thread is oldest-created, other chats are newer, and the cache is bounded to 5)",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-thread-pressure-"))
      try {
        const threadsFile = path.join(root, "threads.json")
        const registryFile = path.join(root, "full-auto", "registry.json")
        const store = makeThreadStore(threadsFile)
        const registry = openFullAutoRegistry(registryFile)
        const dispatch = makeIncidentDispatch(store)

        // The owner's overnight "Hello" thread: created first, so it is the
        // OLDEST-created thread in the mutable cache for the rest of the run
        // -- exactly the incident shape ("the older-created but still-active
        // Full Auto thread").
        const fullAutoThread = store.newThread("Hello")
        registry.set(fullAutoThread.id, true, { workspaceRef: GRANTED_WORKSPACE, profile: { lane: "codex-local" } })
        await tick()

        // Cycle 1: the first autonomous packet dispatches and completes,
        // exactly as it did at 12:12 AM in the incident.
        expect(await reconcile(registry, { dispatch })).toEqual([fullAutoThread.id])
        await tick()

        // Ordinary multi-chat pressure between continuations: two OTHER
        // top-level chats get created before the next continuation runs.
        store.newThread("Other chat 1")
        await tick()
        store.newThread("Other chat 2")
        await tick()

        // Cycle 2: the next continuation dispatches and completes, touching
        // (and re-freshening) the Full Auto thread again -- this re-touch is
        // exactly what the incident's stalled run never got to do.
        expect(await reconcile(registry, { dispatch })).toEqual([fullAutoThread.id])
        await tick()

        // Four more other chats before cycle 3 attempts -- six distinct
        // OTHER chats will exist by the time it dispatches, more than the
        // five-slot bounded cache can hold alongside the Full Auto thread.
        for (const label of ["Other chat 3", "Other chat 4", "Other chat 5", "Other chat 6"]) {
          store.newThread(label)
          await tick()
        }

        // The mutable cache is bounded to 5, and 7 threads now exist (Full
        // Auto + 6 others), so real eviction pressure has been exceeded --
        // not just simulated.
        expect(store.list().length).toBe(5)
        // The fix's contract: because the Full Auto thread was touched again
        // at the start of cycle 2, it still ranks inside the 5
        // most-recently-touched threads even after six total other chats
        // have been created since it was created.
        expect(store.open(fullAutoThread.id)).not.toBeNull()

        // Cycle 3: the continuation the incident lost is accepted exactly
        // once, with no "conversation no longer exists" failure and no
        // silently dormant enabled record.
        expect(await reconcile(registry, { dispatch })).toEqual([fullAutoThread.id])
        expect(registry.record(fullAutoThread.id)?.continuationCount).toBe(3)
        expect(registry.record(fullAutoThread.id)?.blockedReason ?? null).toBe(null)
        expect(registry.record(fullAutoThread.id)?.pendingTurnRef ?? null).toBe(null)
        expect(registry.get(fullAutoThread.id)).toBe(true)
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test(
    "restart across the pressure boundary: Runtime B reopens the same on-disk thread store and registry mid-pressure and resumes the run's thread exactly once, racing a turn-completion trigger against a startup trigger the way main.ts's two call sites can",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-thread-pressure-restart-"))
      try {
        const threadsFile = path.join(root, "threads.json")
        const registryFile = path.join(root, "full-auto", "registry.json")

        // Runtime A: the same shape as the single-process replay above,
        // through cycle 2 and four of the six other-chat touches, then it
        // quits -- the remaining pressure and the next continuation happen
        // after restart, on a fresh process reopening the same durable files.
        const storeA = makeThreadStore(threadsFile)
        const registryA = openFullAutoRegistry(registryFile)
        const dispatchA = makeIncidentDispatch(storeA)
        const fullAutoThread = storeA.newThread("Hello")
        registryA.set(fullAutoThread.id, true, { workspaceRef: GRANTED_WORKSPACE, profile: { lane: "codex-local" } })
        await tick()
        expect(await reconcile(registryA, { dispatch: dispatchA })).toEqual([fullAutoThread.id])
        await tick()
        storeA.newThread("Other chat 1")
        await tick()
        storeA.newThread("Other chat 2")
        await tick()
        expect(await reconcile(registryA, { dispatch: dispatchA })).toEqual([fullAutoThread.id])
        await tick()
        storeA.newThread("Other chat 3")
        await tick()
        storeA.newThread("Other chat 4")
        await tick()

        // Runtime B: a fresh process reopening the same on-disk thread store
        // and registry files. It applies the last two other-chat touches
        // (crossing the eviction boundary AFTER restart) and then races two
        // reconciliation triggers -- exactly like a turn-completion callback
        // and the startup pass could in production -- to prove the
        // restart-crossing continuation is still exactly-once.
        const storeB = makeThreadStore(threadsFile)
        const registryB = openFullAutoRegistry(registryFile)
        const dispatchB = makeIncidentDispatch(storeB)
        storeB.newThread("Other chat 5")
        await tick()
        storeB.newThread("Other chat 6")
        await tick()

        expect(storeB.list().length).toBe(5)
        expect(storeB.open(fullAutoThread.id)).not.toBeNull()

        const [first, second] = await Promise.all([
          reconcile(registryB, { dispatch: dispatchB, clearStaleLeases: true }),
          reconcile(registryB, { dispatch: dispatchB }),
        ])
        expect([...first, ...second]).toEqual([fullAutoThread.id])
        expect(registryB.record(fullAutoThread.id)?.continuationCount).toBe(3)
        expect(registryB.record(fullAutoThread.id)?.pendingTurnRef ?? null).toBe(null)
        expect(registryB.get(fullAutoThread.id)).toBe(true)
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )
})
