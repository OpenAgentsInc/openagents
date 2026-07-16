import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { decodeCodexLocalContinuationProfile } from "../src/codex-local-contract.ts"
import { openFullAutoRegistry, type FullAutoRegistry } from "../src/full-auto-registry.ts"
import {
  FULL_AUTO_MAX_CONSECUTIVE_FAILURES,
  FULL_AUTO_MAX_CONTINUATIONS,
  fullAutoFailureBackoffMs,
  makeSerialTaskQueue,
  reconcileFullAutoThreads,
} from "../src/full-auto-reconcile.ts"

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
      registryA.set("thread-stopped", false)

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
    // A model literal removed from the contract enum (or a Claude model on
    // the Codex lane) must not crash the loop -- it falls back to defaults.
    expect(decodeCodexLocalContinuationProfile({
      model: "gpt-4-retired",
      reasoningEffort: "impossible",
    })).toEqual({ accountRef: null, model: null, reasoningEffort: null })
    expect(decodeCodexLocalContinuationProfile({ model: "claude-fable-5" }).model).toBe(null)
    expect(decodeCodexLocalContinuationProfile(undefined)).toEqual({
      accountRef: null,
      model: null,
      reasoningEffort: null,
    })
  })
})
