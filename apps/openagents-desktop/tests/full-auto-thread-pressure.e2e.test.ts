import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { DesktopMessage } from "../src/chat-contract.ts"
import { classifyFullAutoDispatchFailureReason } from "../src/full-auto-liveness.ts"
import { reconcileFullAutoThreads } from "../src/full-auto-reconcile.ts"
import { openFullAutoRegistry } from "../src/full-auto-registry.ts"
import { openLocalTurnJournal } from "../src/local-turn-journal.ts"
import {
  makeProviderLaneDispatcher,
  type ProviderLane,
} from "../src/provider-lane.ts"
import { makeThreadStore } from "../src/thread-store.ts"

/**
 * FA-PRESS-01 (#8989): active-run thread retention under REAL cache pressure,
 * end to end through the actual production stack.
 *
 * The 2026-07-16 overnight failure evicted the active Full Auto thread from
 * the five-slot mutable thread cache (thread-store.ts `write()` sorted by
 * CREATED-at), so the next continuation hit provider-lane.ts's fail-closed
 * `store.open(threadRef) === null` check and died with the literal
 * `"That conversation no longer exists."`. The fix (8cb900bbf9) retains the
 * five most recently ACCESSED threads instead.
 *
 * FA-RUN-02 (#8970) already replays the incident against the real thread
 * store, but its dispatch is a hand-rolled stub that merely mirrors the
 * production check. This suite goes one layer deeper and drives the ACTUAL
 * shared dispatch engine: `reconcileFullAutoThreads` -> a dispatch wrapper
 * with main.ts's exact in-flight guard and result mapping ->
 * `makeProviderLaneDispatcher(...).dispatchTurn` over the real thread store
 * and the real local-turn journal, with a scripted stub `ProviderLane`
 * standing in for the provider child process (no live providers, no
 * Keychain). The `"That conversation no longer exists."` string this suite
 * guards against is therefore produced by the same provider-lane.ts line
 * production executes, not a test-local imitation.
 *
 * ORACLE BITE (verified in the FA-PRESS-01 worktree before landing):
 * reverting thread-store.ts `write()` to sort by
 * `compareDesktopThreadsByCreatedAt` (the pre-8cb900bbf9 hunk) makes the two
 * pressure tests below fail -- the active thread's openability probe trips
 * the moment a fifth newer-created thread lands, and the continuation that
 * follows surfaces the exact incident reason, classified
 * `provider_session_missing`. Restoring last-access retention turns both
 * green again. The typed-defect-class channel test passes under either
 * policy by design: it proves the failure SHAPE (typed, fail-closed, before
 * provider execution), not the retention policy.
 */

const GRANTED_WORKSPACE = "/granted/workspace-a"
/** thread-store.ts's bounded-cache size (`maxThreads`). Not exported -- the
 * final `store.list().length` assertions below fail if the bound drifts, so
 * this stays honest. */
const CACHE_BOUND = 5

/** Real Date.now() resolution is 1ms; consecutive synchronous store writes
 * can tie on `updatedAt` and fall back to the id tiebreak, making recency
 * ordering (and therefore the scenario) flaky. A 2ms real delay keeps every
 * mutation's timestamp strictly increasing (same device as FA-RUN-02). */
const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 2))

type FullAutoRuntime = ReturnType<typeof makeFullAutoRuntime>

/**
 * One "desktop process": the real thread store, the real local-turn journal,
 * the real shared provider-lane dispatch engine, and the real Full Auto
 * registry, all over durable files under `root`. Constructing a second
 * runtime over the same root is a process restart: every store re-reads from
 * disk (cold cache), exactly like the existing restart e2e proofs.
 */
const makeFullAutoRuntime = (root: string) => {
  const store = makeThreadStore(path.join(root, "threads.json"))
  const journal = openLocalTurnJournal(path.join(root, "turns.json"))
  const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))

  /** Provider execution stub: a scripted lane behind the REAL ProviderLane
   * SPI. Everything above it (content admission, thread existence, journal
   * accept/terminal, user-note persistence, history assembly) is the shared
   * production engine. */
  const providerTurnThreadRefs: string[] = []
  const lane: ProviderLane<null> = {
    laneRef: "pressure-stub",
    graphLaneRef: "pressure_stub",
    eventChannel: "openagents:pressure-stub:event",
    usageProvider: "pressure_stub_provider",
    capabilities: () => ({
      laneRef: "pressure-stub",
      provider: "pressure_stub_provider",
      models: ["pressure-model-1"],
      features: {
        skills: false,
        planOnly: false,
        reasoningEffort: false,
        images: false,
        fullAuto: true,
        interrupt: false,
        queueFollowup: false,
        steerTurn: false,
        steerChild: false,
        answerQuestion: false,
      },
      composer: {
        displayName: "Pressure stub",
        reasoningEfforts: [],
        permissionModes: ["owner_full"],
        approvals: "none",
        extensions: [],
      },
      policy: {
        source: "native-static-declaration",
        profileRef: "native:pressure-stub:v1",
        evidence: "conformant",
        allowedModels: ["pressure-model-1"],
        allowedFeatures: ["fullAuto"],
        allowedExtensions: [],
      },
      recovery: "interrupt_on_restart",
    }),
    admit: () => ({ ok: true, model: "pressure-model-1", context: null }),
    streamMeta: ctx => ({ lane: "pressure-stub", turnRef: ctx.request.turnRef }),
    modelNoteText: model => `Pressure stub · ${model}`,
    runTurn: async ({ request, emit }) => {
      providerTurnThreadRefs.push(request.threadRef)
      emit({ kind: "turn_started" })
      emit({ kind: "model_effective", model: "pressure-model-1" })
      emit({ kind: "text_delta", text: "Autonomous packet complete." })
      emit({ kind: "turn_completed", totalTokens: 7 })
      return { ok: true, text: "Autonomous packet complete.", totalTokens: 7 }
    },
    interrupt: () => false,
    finalMeta: ctx => ({ lane: "pressure-stub", turnRef: ctx.request.turnRef }),
    failureMessage: (reason, detail) => `Pressure stub turn failed (${reason} · ${detail}).`,
  }
  const dispatcher = makeProviderLaneDispatcher({
    threads: () => store,
    journal,
    liveAgentGraph: { beginTurn: () => {}, applyEvent: () => {} },
    usageLedger: { record: () => {} },
    captureTurnCheckpoint: async () => {},
    localTurnFlushers: new Set(),
    isQuitting: () => false,
  })

  /** Per-continuation facts recorded at the moment reconcile hands the
   * thread to dispatch: was the addressed thread openable right then? */
  const continuations: Array<{ threadRef: string; turnRef: string; openableAtDispatch: boolean }> = []
  /** Every typed dispatch failure, with its liveness classification -- a
   * retention regression must show up here as `host_thread_missing`,
   * never as a generic/unknown error. */
  const failures: Array<{ threadRef: string; reason: string; cause: string }> = []

  const reconcile = (options?: Readonly<{ startup?: boolean }>) =>
    reconcileFullAutoThreads({
      registry,
      nonterminalThreadRefs: () => new Set(journal.nonterminal().map(record => record.threadRef)),
      resolveWorkspaceRef: () => GRANTED_WORKSPACE,
      journalHasNonterminalTurn: turnRef => journal.nonterminal().some(record => record.turnRef === turnRef),
      ...(options?.startup === true ? { clearStaleLeases: true } : {}),
      // main.ts runFullAutoReconciliation's dispatch wiring, minus the
      // lane-eligibility lookup (the stub lane IS the eligible lane here):
      // the same journal in-flight guard, the same dispatchTurn call shape
      // (sender = null, fullAuto: true), and the same error->reason mapping
      // that carries provider-lane.ts's fail-closed string into FA-H5.
      dispatch: async ({ threadRef, turnRef, message }) => {
        if (journal.nonterminal().some(record => record.threadRef === threadRef)) {
          return { ok: false, reason: "turn_already_in_flight" }
        }
        continuations.push({ threadRef, turnRef, openableAtDispatch: store.open(threadRef) !== null })
        const result = await dispatcher.dispatchTurn(lane, { turnRef, threadRef, message, fullAuto: true }, null)
        return result.ok
          ? { ok: true }
          : {
              ok: false,
              reason: result.error ?? "dispatch_failed",
              ...(result.failureCause === undefined ? {} : { failureCause: result.failureCause }),
            }
      },
      onDispatchFailed: (threadRef, failure) => {
        failures.push({
          threadRef,
          reason: failure.reason,
          cause: classifyFullAutoDispatchFailureReason(failure.reason),
        })
      },
    })

  return { store, journal, registry, reconcile, continuations, failures, providerTurnThreadRefs }
}

/**
 * One pressure batch: create `count` NEW threads and interleave reads and
 * writes across the whole population -- each new thread gets a follow-up
 * message append (write), and every thread ever created (including already
 * evicted ones) gets an `open` probe plus a full `list` read between
 * mutations. This is eviction pressure by construction, not simulation: the
 * cache holds five, so every batch beyond the first forces real evictions.
 *
 * Batches stay at <= 4 distinct new threads between Full Auto touches
 * deliberately: a five-slot recency cache can only retain the active thread
 * alongside four fresher ones. That is the retention CONTRACT under test --
 * the run re-touches its thread every continuation, so it must survive any
 * number of TOTAL other threads as long as no more than four distinct others
 * are touched between its own touches. The broken created-at policy fails
 * this as soon as five newer-created threads exist at all.
 */
const pressureNote = (label: string): DesktopMessage => ({
  key: `pressure-${label}`,
  role: "user",
  text: `pressure follow-up ${label}`,
  timestamp: "00:00",
})
const applyPressureBatch = async (
  store: FullAutoRuntime["store"],
  labels: ReadonlyArray<string>,
  allPressureIds: string[],
): Promise<void> => {
  for (const label of labels) {
    const created = store.newThread(`Pressure ${label}`)
    allPressureIds.push(created.id)
    await tick()
    // Interleaved write: touch the just-created thread again.
    store.append(created.id, pressureNote(label))
    await tick()
    // Interleaved reads across every thread ever created -- evicted ids
    // honestly return null; the reads must never resurrect or reorder.
    for (const id of allPressureIds) void store.open(id)
    void store.list()
  }
}

describe("FA-PRESS-01: active-run thread retention under cache pressure (#8989)", () => {
  test(
    "in-session pressure: one Full Auto thread completes five continuation cycles through the real dispatch engine while 16 other threads (>3x the cache bound) are created and touched around it; the active thread stays openable throughout, every continuation addresses an openable thread, and no conversation-no-longer-exists class error occurs",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-fa-press-insession-"))
      try {
        const runtime = makeFullAutoRuntime(root)
        const fullAutoThread = runtime.store.newThread("Full Auto pressure run")
        runtime.registry.set(fullAutoThread.id, true, {
          workspaceRef: GRANTED_WORKSPACE,
          profile: { lane: "codex-local" },
        })
        await tick()

        const allPressureIds: string[] = []
        const batches: ReadonlyArray<ReadonlyArray<string>> = [
          ["p01", "p02", "p03", "p04"],
          ["p05", "p06", "p07", "p08"],
          ["p09", "p10", "p11", "p12"],
          ["p13", "p14", "p15", "p16"],
        ]
        for (const batch of batches) {
          // A continuation cycle completes (touching the active thread), then
          // a batch of other-thread creation/touch pressure lands before the
          // next cycle -- the exact overnight interleaving.
          expect(await runtime.reconcile()).toEqual([fullAutoThread.id])
          await tick()
          await applyPressureBatch(runtime.store, batch, allPressureIds)
          // The active-run thread survives every pressure batch. Under the
          // reverted created-at policy this trips on the SECOND batch (the
          // moment a fifth newer-created thread exists).
          expect(runtime.store.open(fullAutoThread.id)).not.toBeNull()
        }
        // The continuation cycle after ALL pressure has landed -- the one the
        // incident lost.
        expect(await runtime.reconcile()).toEqual([fullAutoThread.id])

        // 16 distinct other threads existed across the run: pressure well
        // beyond the five-slot bound was actually exceeded, not simulated.
        expect(allPressureIds).toHaveLength(16)
        expect(runtime.store.list()).toHaveLength(CACHE_BOUND)

        // Every continuation addressed an openable thread at dispatch time.
        expect(runtime.continuations).toHaveLength(5)
        expect(runtime.continuations.every(c => c.threadRef === fullAutoThread.id)).toBe(true)
        expect(runtime.continuations.every(c => c.openableAtDispatch)).toBe(true)

        // No dispatch failure of any class occurred -- in particular nothing
        // in the conversation-no-longer-exists (`provider_session_missing`)
        // class. Asserting on the classified list means a regression failure
        // here NAMES the typed defect class in the diff.
        expect(runtime.failures.map(failure => failure.cause)).toEqual([])

        // The provider stub really ran five turns for this thread through the
        // shared engine, and the transcript carries all five exchanges.
        expect(runtime.providerTurnThreadRefs).toEqual(Array(5).fill(fullAutoThread.id))
        const notes = runtime.store.open(fullAutoThread.id)?.notes ?? []
        expect(notes.filter(note => note.role === "user" && note.text.includes("Continue Full Auto"))).toHaveLength(5)
        expect(notes.filter(note => note.role === "assistant")).toHaveLength(5)

        // Durable state is clean: five successful continuations, no failure
        // residue, no held lease, no nonterminal journal rows, still enabled.
        const record = runtime.registry.record(fullAutoThread.id)
        expect(record?.continuationCount).toBe(5)
        expect(record?.consecutiveFailures ?? 0).toBe(0)
        expect(record?.blockedReason ?? null).toBeNull()
        expect(record?.pendingTurnRef ?? null).toBeNull()
        expect(runtime.registry.get(fullAutoThread.id)).toBe(true)
        expect(runtime.journal.nonterminal()).toEqual([])
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test(
    "restart under pressure: Runtime B reopens every durable file cold (thread store, journal, registry) mid-pressure, more pressure lands before the startup reconciliation, and the run still resumes on an openable thread -- then survives a further pressure batch and a mid-session continuation",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-fa-press-restart-"))
      try {
        // Runtime A: enable, complete two continuation cycles with pressure
        // interleaved, then quit with more pressure already on disk.
        const runtimeA = makeFullAutoRuntime(root)
        const fullAutoThread = runtimeA.store.newThread("Full Auto pressure run")
        runtimeA.registry.set(fullAutoThread.id, true, {
          workspaceRef: GRANTED_WORKSPACE,
          profile: { lane: "codex-local" },
        })
        await tick()
        const allPressureIds: string[] = []
        expect(await runtimeA.reconcile()).toEqual([fullAutoThread.id])
        await tick()
        await applyPressureBatch(runtimeA.store, ["a1", "a2", "a3"], allPressureIds)
        expect(await runtimeA.reconcile()).toEqual([fullAutoThread.id])
        await tick()
        await applyPressureBatch(runtimeA.store, ["a4", "a5"], allPressureIds)
        expect(runtimeA.continuations.every(c => c.openableAtDispatch)).toBe(true)
        expect(runtimeA.failures).toEqual([])

        // Runtime B: a fresh process. Every cache is cold -- the thread
        // store, the journal, and the registry are all re-read from disk.
        // MORE pressure lands before the startup pass runs (the app can
        // create threads before reconciliation fires), crossing further
        // eviction boundaries with the active thread still un-retouched
        // since Runtime A's second cycle.
        const runtimeB = makeFullAutoRuntime(root)
        await applyPressureBatch(runtimeB.store, ["b1", "b2"], allPressureIds)

        // Cold-cache openability: the active thread survived the restart AND
        // the four distinct other-thread touches since its last access.
        expect(runtimeB.store.list()).toHaveLength(CACHE_BOUND)
        expect(runtimeB.store.open(fullAutoThread.id)).not.toBeNull()

        // Startup reconciliation (the clearStaleLeases pass) resumes the run.
        expect(await runtimeB.reconcile({ startup: true })).toEqual([fullAutoThread.id])
        await tick()

        // And the run keeps surviving: another full batch, then a normal
        // mid-session continuation.
        await applyPressureBatch(runtimeB.store, ["b3", "b4", "b5", "b6"], allPressureIds)
        expect(runtimeB.store.open(fullAutoThread.id)).not.toBeNull()
        expect(await runtimeB.reconcile()).toEqual([fullAutoThread.id])

        // 11 distinct other threads total (>2x the bound) across the restart.
        expect(allPressureIds).toHaveLength(11)
        expect(runtimeB.continuations).toHaveLength(2)
        expect(runtimeB.continuations.every(c => c.threadRef === fullAutoThread.id)).toBe(true)
        expect(runtimeB.continuations.every(c => c.openableAtDispatch)).toBe(true)
        expect(runtimeB.failures.map(failure => failure.cause)).toEqual([])

        const record = runtimeB.registry.record(fullAutoThread.id)
        expect(record?.continuationCount).toBe(4)
        expect(record?.blockedReason ?? null).toBeNull()
        expect(record?.pendingTurnRef ?? null).toBeNull()
        expect(runtimeB.registry.get(fullAutoThread.id)).toBe(true)
        expect(runtimeB.journal.nonterminal()).toEqual([])
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )

  test(
    "typed defect class: a continuation that DOES address an unopenable thread is classified host_thread_missing (never provider_session_missing), fail-closed BEFORE provider execution, with FA-H5 failure state persisted and the lease released",
    async () => {
      const root = mkdtempSync(path.join(tmpdir(), "oa-fa-press-defect-class-"))
      try {
        const runtime = makeFullAutoRuntime(root)
        const fullAutoThread = runtime.store.newThread("Never re-touched")
        runtime.registry.set(fullAutoThread.id, true, {
          workspaceRef: GRANTED_WORKSPACE,
          profile: { lane: "codex-local" },
        })
        await tick()
        // Five distinct fresher threads with NO intervening Full Auto touch:
        // even the CORRECT last-access policy must evict the enabled thread
        // (it is sixth by recency). This is legitimate eviction, so the
        // contract under test is the failure SHAPE, not retention.
        const allPressureIds: string[] = []
        await applyPressureBatch(runtime.store, ["e1", "e2", "e3", "e4", "e5"], allPressureIds)
        expect(runtime.store.open(fullAutoThread.id)).toBeNull()

        expect(await runtime.reconcile()).toEqual([])

        // The failure surfaced as the typed defect class -- the exact
        // provider-lane.ts typed host cause, classified to the liveness
        // vocabulary's host_thread_missing bucket, not a provider error.
        expect(runtime.failures).toEqual([{
          threadRef: fullAutoThread.id,
          reason: "host_thread_missing",
          cause: "host_thread_missing",
        }])
        expect(runtime.failures[0]!.cause).not.toBe("unknown_error")
        // The continuation honestly addressed an unopenable thread...
        expect(runtime.continuations).toEqual([
          expect.objectContaining({ threadRef: fullAutoThread.id, openableAtDispatch: false }),
        ])
        // ...and failed closed before any provider execution or journal row.
        expect(runtime.providerTurnThreadRefs).toEqual([])
        expect(runtime.journal.nonterminal()).toEqual([])

        // FA-H5: the failure is durable and visible, never silently dormant.
        const record = runtime.registry.record(fullAutoThread.id)
        expect(record?.enabled).toBe(true)
        expect(record?.consecutiveFailures).toBe(1)
        expect(record?.blockedReason).toBe("host_thread_missing")
        expect(record?.pendingTurnRef ?? null).toBeNull()
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    },
  )
})
