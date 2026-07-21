/**
 * RLM-06 (#9142) — hermetic synthetic long-run tests for the Full Auto RLM
 * recall consumer.
 *
 * Everything here is deterministic and spend-free: the "model" is a scripted
 * root/leaf plan emitting fixed RlmProgram JSON and fixed token counts (the
 * same fixture style as the RLM-05 suite). The suite proves:
 *
 * - run-scope isolation: a run reads ONLY its registry-bound thread, never an
 *   unrelated thread or owner, even when a foreign scope names the other
 *   thread;
 * - the framed continuation references only bounded validated citations, never
 *   raw corpus or the recursive transcript;
 * - deterministic-only, admitted-semantic, refusal, partial, missing-usage,
 *   provider-failure, timeout/interruption, budget-exhaustion, and unavailable
 *   cases all preserve run progress and never fabricate success;
 * - recall usage and result refs replay idempotently.
 */

import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect, Fiber } from "effect"
import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  makeInMemoryEventLogStore,
  type HarnessEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract"
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema"
import { RlmError } from "@openagentsinc/rlm"
import { describe, expect, test } from "vite-plus/test"

import type { HistoryRecallHostSources } from "./history-recall-host.ts"
import type {
  DesktopRlmCompleteFn,
  DesktopSemanticRecallAdmission,
} from "./history-recall-semantic.ts"
import {
  openFullAutoRunRegistry,
  type FullAutoRunRegistry,
} from "./full-auto-run-registry.ts"
import {
  applyFullAutoRecallToContinuation,
  fullAutoRecallContinuationFragment,
  fullAutoRecallHonesty,
  fullAutoRunRecallScope,
  fullAutoRunThreadMembership,
  makeFullAutoRecallLedger,
  makeFullAutoRunRecallSources,
  runFullAutoRecall,
  type FullAutoRecallInput,
} from "./full-auto-recall.ts"

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" }
const RUN_THREAD = "thread.run"
const OTHER_THREAD = "thread.other-owner"
const PLANTED = "DECISION: adopt long-run recall"
const PLANTED_TURN = "turn.run.8"
const SECRET = "SECRET: unrelated owner private note"
const ADMISSION: DesktopSemanticRecallAdmission = {
  admitted: true,
  basis: "user_explicit",
  grantRef: "grant.test.rlm06.1",
}

// ---------------------------------------------------------------------------
// Multi-thread fixture: the run thread (planted decision) and an unrelated
// owner's thread (planted secret) share one event log.
// ---------------------------------------------------------------------------

const turnEvents = (
  threadId: string,
  turnId: string,
  words: ReadonlyArray<string>,
): Array<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = []
  let seq = 0
  events.push(
    buildTurnStarted({
      turnId,
      threadId,
      sequence: seq++,
      source: SOURCE,
      observedAt: "2026-07-21T09:00:00.000Z",
    }),
  )
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId,
        sequence: seq++,
        source: SOURCE,
        observedAt: "2026-07-21T09:00:01.000Z",
        messageId: `msg.${turnId}`,
        text: word,
      }),
    )
  }
  events.push(
    buildTurnFinished({
      turnId,
      threadId,
      sequence: seq,
      source: SOURCE,
      observedAt: "2026-07-21T09:00:02.000Z",
      finishReason: "stop",
    }),
  )
  return events
}

interface Fixture {
  readonly base: HistoryRecallHostSources
  readonly runTurnCount: number
}

const makeFixture = async (runTurnCount: number): Promise<Fixture> => {
  const eventLog: HarnessEventLogStore = makeInMemoryEventLogStore()
  const runTurnIds: Array<string> = []
  const otherTurnIds: Array<string> = []
  await Effect.runPromise(
    Effect.gen(function* () {
      for (let i = 1; i <= runTurnCount; i++) {
        const turnId = `turn.run.${i}`
        runTurnIds.push(turnId)
        const words =
          turnId === PLANTED_TURN
            ? [PLANTED, "confirmed by the owner"]
            : [`routine update ${i}`, `nothing decided in step ${i}`]
        for (const event of turnEvents(RUN_THREAD, turnId, words)) {
          yield* eventLog.append(event)
        }
      }
      // The unrelated owner's thread carries a secret the run must NEVER read.
      for (let i = 1; i <= 12; i++) {
        const turnId = `turn.other.${i}`
        otherTurnIds.push(turnId)
        const words = i === 6 ? [SECRET, "do not disclose"] : [`other chat ${i}`]
        for (const event of turnEvents(OTHER_THREAD, turnId, words)) {
          yield* eventLog.append(event)
        }
      }
    }),
  )
  return {
    base: {
      eventLog,
      turnIdsForThread: (threadId) =>
        threadId === RUN_THREAD
          ? runTurnIds
          : threadId === OTHER_THREAD
            ? otherTurnIds
            : [],
      builtAt: () => "2026-07-21T12:00:00.000Z",
      source: SOURCE,
    },
    runTurnCount,
  }
}

// ---------------------------------------------------------------------------
// Registry helper: one running run bound to RUN_THREAD.
// ---------------------------------------------------------------------------

const openRunRegistry = (): { registry: FullAutoRunRegistry; runRef: string } => {
  const root = mkdtempSync(path.join(tmpdir(), "full-auto-recall-"))
  const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
  const started = registry.startNew({
    title: "Long recall run",
    objective: "Burn down the backlog; recall prior decisions when useful.",
    doneCondition: "The named packets are complete with explicit final reasons.",
    objectiveSource: "user",
    workspaceRef: "/workspace",
    profile: { lane: "codex-local", accountRef: "codex-account" },
    threadRef: RUN_THREAD,
    actor: "owner_ui",
    reason: "test start",
  })
  if (!started.ok) throw new Error(`failed to start run: ${started.reason}`)
  return { registry, runRef: started.run.runRef }
}

// ---------------------------------------------------------------------------
// Scripted model plans (no spend, fixed tokens).
// ---------------------------------------------------------------------------

const grepCommitRoot = (options?: {
  readonly pattern?: string
  readonly tokens?: { readonly input: number; readonly output: number } | null
}): DesktopRlmCompleteFn => {
  const pattern = options?.pattern ?? "DECISION:"
  const tokens = options?.tokens === undefined ? { input: 100, output: 20 } : options.tokens
  return () =>
    Effect.succeed({
      text: JSON.stringify({
        schemaId: "openagents.ai.rlm_program.v1",
        programRef: "program.grep-commit",
        nodes: [
          {
            _tag: "CorpusOp",
            nodeRef: "n1",
            operator: "Grep",
            params: { pattern },
            inputValueRefs: [],
            outputValueRef: "v1",
          },
          { _tag: "Commit", nodeRef: "n2", valueRef: "v1", citationValueRefs: [] },
        ],
      }),
      ...(tokens === null ? {} : { inputTokens: tokens.input, outputTokens: tokens.output }),
    })
}

/** Root that commits an uncited value (InspectMetadata carries no citations). */
const uncitedCommitRoot = (): DesktopRlmCompleteFn => () =>
  Effect.succeed({
    text: JSON.stringify({
      schemaId: "openagents.ai.rlm_program.v1",
      programRef: "program.uncited-commit",
      nodes: [
        {
          _tag: "CorpusOp",
          nodeRef: "n1",
          operator: "InspectMetadata",
          params: {},
          inputValueRefs: [],
          outputValueRef: "v1",
        },
        { _tag: "Commit", nodeRef: "n2", valueRef: "v1", citationValueRefs: [] },
      ],
    }),
    inputTokens: 40,
    outputTokens: 10,
  })

/** Root that fails with a typed provider error before emitting a program. */
const providerFailureRoot = (): DesktopRlmCompleteFn => () =>
  new RlmError({ reason: "model_unavailable", retryable: false, detailSafe: "outage" })

const baseInput = (
  fixture: Fixture,
  registry: FullAutoRunRegistry,
  runRef: string,
  overrides: Partial<FullAutoRecallInput> & Pick<FullAutoRecallInput, "recallRef" | "ledger">,
): FullAutoRecallInput => ({
  runRef,
  registry,
  base: fixture.base,
  deterministicQuestion: { _tag: "Grep", pattern: "DECISION:" },
  caps: { maxSpans: 5 },
  ...overrides,
})

// ---------------------------------------------------------------------------
// Run-scope isolation — the security core.
// ---------------------------------------------------------------------------

describe("run-scope isolation (RLM-06)", () => {
  test("membership resolves only the registry-bound thread", () => {
    const { registry, runRef } = openRunRegistry()
    expect(fullAutoRunThreadMembership(registry, runRef)).toEqual([RUN_THREAD])
    const scope = fullAutoRunRecallScope(registry, runRef)
    expect(scope).toEqual({ _tag: "Run", runRef, threadIds: [RUN_THREAD] })
    // An unknown run resolves to zero threads (recall becomes unavailable).
    expect(fullAutoRunThreadMembership(registry, "run.does-not-exist")).toEqual([])
    expect(fullAutoRunRecallScope(registry, "run.does-not-exist")).toBeNull()
  })

  test("the wrapped sources authorize only this run's thread and only this run", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const sources = makeFullAutoRunRecallSources({ base: fixture.base, registry, runRef })
    expect(await sources.authorizeThread!(RUN_THREAD)).toBe(true)
    // A different owner's thread is refused fail-closed.
    expect(await sources.authorizeThread!(OTHER_THREAD)).toBe(false)
    // Run membership resolves only for THIS run ref, only to its bound thread.
    expect(await sources.threadIdsForRun!(runRef)).toEqual([RUN_THREAD])
    expect(await sources.threadIdsForRun!("run.other")).toEqual([])
  })

  test("a run cannot recall an unrelated thread's secret even via a semantic run", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    // Ask (structurally and semantically) for the secret planted in the OTHER
    // owner's thread. The run scope resolves only RUN_THREAD, so the secret is
    // unreachable and uncitable.
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.continuation.1",
          ledger,
          deterministicQuestion: { _tag: "Grep", pattern: "SECRET:" },
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: { question: "What secret did the other owner note?", completeRoot: grepCommitRoot({ pattern: "SECRET:" }) },
        }),
      ),
    )
    // The run reads only its own thread; no secret span is ever cited.
    const allSpans = [...outcome.deterministicCitedSpans, ...outcome.citedSpans]
    expect(allSpans.some((span) => span.turnId.startsWith("turn.other"))).toBe(false)
    expect(allSpans.some((span) => span.excerpt.includes(SECRET))).toBe(false)
    const fragment = fullAutoRecallContinuationFragment(outcome)
    if (fragment !== null) {
      expect(fragment.text.includes(SECRET)).toBe(false)
      expect(fragment.text.includes(OTHER_THREAD)).toBe(false)
    }
  })
})

// ---------------------------------------------------------------------------
// Deterministic-first structural recall.
// ---------------------------------------------------------------------------

describe("deterministic-first structural recall (RLM-06)", () => {
  for (const runTurnCount of [100, 400, 1000]) {
    test(`${runTurnCount}-turn run: deterministic recall cites the planted decision and frames it`, async () => {
      const fixture = await makeFixture(runTurnCount)
      const { registry, runRef } = openRunRegistry()
      const ledger = makeFullAutoRecallLedger()
      const outcome = await Effect.runPromise(
        runFullAutoRecall(
          baseInput(fixture, registry, runRef, {
            recallRef: "turn.continuation.det",
            ledger,
          }),
        ),
      )
      expect(outcome.status).toBe("deterministic_only")
      expect(outcome.tier).toBe("deterministic")
      expect(outcome.scopeResolved).toBe(true)
      expect(outcome.consumedBudget).toBe(true)
      expect(
        outcome.citedSpans.some((span) => span.turnId === PLANTED_TURN),
      ).toBe(true)
      // No model calls on the deterministic path.
      expect(outcome.usage.modelCalls).toBe(0)
      expect(outcome.usageRows).toEqual([])
      // The framed continuation carries the bounded cited candidate.
      const fragment = fullAutoRecallContinuationFragment(outcome)
      expect(fragment).not.toBeNull()
      expect(fragment!.text).toContain("cited candidate")
      expect(fragment!.text).toContain("NOT verified")
      expect(fragment!.text).toContain(PLANTED_TURN)
      // Budget consumed exactly once.
      expect(ledger.remaining(runRef)).toBe(ledger.budgetPerRun - 1)
    }, 30_000)
  }

  test("the framed continuation references only bounded validated citations, not raw corpus", async () => {
    const fixture = await makeFixture(200)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, { recallRef: "turn.c.frame", ledger }),
      ),
    )
    const fragment = fullAutoRecallContinuationFragment(outcome)
    expect(fragment).not.toBeNull()
    // Bounded span count.
    expect(fragment!.citedSpans.length).toBeLessThanOrEqual(6)
    // Every framed span is one of the outcome's validated cited spans.
    for (const span of fragment!.citedSpans) {
      expect(outcome.citedSpans).toContainEqual(span)
    }
    // Splicing prepends the fragment; a null fragment leaves base untouched.
    const spliced = applyFullAutoRecallToContinuation("CONTINUE: do the next thing.", fragment)
    expect(spliced.startsWith("RECALL")).toBe(true)
    expect(spliced).toContain("CONTINUE: do the next thing.")
    expect(applyFullAutoRecallToContinuation("BASE", null)).toBe("BASE")
  })
})

// ---------------------------------------------------------------------------
// Admitted semantic recall through the #9141 policy.
// ---------------------------------------------------------------------------

describe("admitted semantic recall (RLM-06)", () => {
  test("explicit admitted semantic completes, cites, and records exact idempotent usage", async () => {
    const fixture = await makeFixture(300)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.continuation.sem",
          ledger,
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: {
            question: "What decision was made about long-run recall?",
            completeRoot: grepCommitRoot(),
          },
        }),
      ),
    )
    expect(outcome.status).toBe("completed")
    expect(outcome.tier).toBe("semantic")
    expect(outcome.summary?.state).toBe("completed")
    expect(outcome.summary?.usageCompleteness).toBe("complete")
    expect(outcome.usage.totalTokens).toBe(120)
    // Exact usage row keyed by the distinct recall run ref (no cross-recall key
    // collision within the Full Auto run).
    expect(outcome.usageRows.map((row) => row.key)).toEqual([
      `rlm:${runRef}::recall::turn.continuation.sem:root.1`,
    ])
    // Result refs record the SDK strategy pin and the exact corpus digest.
    expect(outcome.resultRefs.strategyRef).toBe("openagents.desktop.rlm.history.v1")
    expect(outcome.resultRefs.contentDigest).not.toBeNull()
    expect(outcome.citedSpans.some((span) => span.turnId === PLANTED_TURN)).toBe(true)
    // Honesty surface never asserts verification.
    const honesty = fullAutoRecallHonesty(outcome)
    expect(honesty.verified).toBe(false)
    expect(honesty.tier).toBe("semantic")
    expect(honesty.totalTokens).toBe(120)
    // Ledger projects exact usage into the session usage ledger.
    const inputs = ledger.usageLedgerInputs({
      provider: "codex",
      accountRef: "codex",
      requestedModel: "gpt-5.5",
    })
    expect(inputs).toHaveLength(1)
    expect(inputs[0]?.usage?.totalTokens).toBe(120)
  }, 30_000)
})

// ---------------------------------------------------------------------------
// Refusal, partial, missing usage, provider failure — always continue.
// ---------------------------------------------------------------------------

describe("recall failures preserve run progress (RLM-06)", () => {
  test("a semantic request without admission stays deterministic and continues", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.c.refused",
          ledger,
          tierRequest: { requestedTier: "semantic" },
          // No admission — the model cannot self-authorize semantic recall.
          semantic: { question: "planted?", completeRoot: grepCommitRoot() },
        }),
      ),
    )
    expect(outcome.tier).toBe("deterministic")
    expect(outcome.status).toBe("deterministic_only")
    expect(outcome.reason).toBe("semantic recall not admitted")
    // Still cited the planted decision deterministically, no model spend.
    expect(outcome.usageRows).toEqual([])
    expect(outcome.citedSpans.some((span) => span.turnId === PLANTED_TURN)).toBe(true)
  })

  test("an uncited semantic answer is Partial (never completed) and continues", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.c.partial",
          ledger,
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: { question: "planted?", completeRoot: uncitedCommitRoot() },
        }),
      ),
    )
    expect(outcome.status).toBe("partial")
    expect(outcome.reason).toBe("invalid_citations")
    // A partial with no valid citations frames nothing but does not stall.
    expect(fullAutoRecallContinuationFragment(outcome)).toBeNull()
    const honesty = fullAutoRecallHonesty(outcome)
    expect(honesty.verified).toBe(false)
  })

  test("missing exact usage fails typed — unavailable, never zero — and continues", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.c.no-usage",
          ledger,
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: { question: "planted?", completeRoot: grepCommitRoot({ tokens: null }) },
        }),
      ),
    )
    expect(outcome.status).toBe("failed")
    expect(outcome.reason).toBe("usage_required_but_unavailable")
    expect(outcome.usageRows).toEqual([])
    // No fabricated success; run continues; nothing to frame.
    expect(fullAutoRecallContinuationFragment(outcome)).toBeNull()
  })

  test("a typed provider failure surfaces as failed and continues", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.c.provider-fail",
          ledger,
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: { question: "planted?", completeRoot: providerFailureRoot() },
        }),
      ),
    )
    expect(outcome.status).toBe("failed")
    expect(outcome.reason).toBe("model_unavailable")
    expect(fullAutoRecallContinuationFragment(outcome)).toBeNull()
  })

  test("an unbound run yields an unavailable recall and continues", async () => {
    const fixture = await makeFixture(20)
    const root = mkdtempSync(path.join(tmpdir(), "full-auto-recall-unbound-"))
    const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
    // A draft run has no bound thread.
    const draft = registry.createDraft({
      title: "Unbound",
      objective: "objective",
      doneCondition: "done",
      objectiveSource: "user",
    })
    const ledger = makeFullAutoRecallLedger()
    const outcome = await Effect.runPromise(
      runFullAutoRecall(
        baseInput(fixture, registry, draft.runRef, { recallRef: "turn.c.unbound", ledger }),
      ),
    )
    expect(outcome.status).toBe("unavailable")
    expect(outcome.scopeResolved).toBe(false)
    expect(outcome.consumedBudget).toBe(false)
    // Unavailable does not consume budget.
    expect(ledger.remaining(draft.runRef)).toBe(ledger.budgetPerRun)
    expect(fullAutoRecallContinuationFragment(outcome)).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Finite per-run recall budget.
// ---------------------------------------------------------------------------

describe("finite per-run recall budget (RLM-06)", () => {
  test("budget exhaustion refuses further recall without stalling the run", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger({ budgetPerRun: 1 })
    const first = await Effect.runPromise(
      runFullAutoRecall(baseInput(fixture, registry, runRef, { recallRef: "turn.c.b1", ledger })),
    )
    expect(first.status).toBe("deterministic_only")
    expect(first.consumedBudget).toBe(true)
    expect(ledger.remaining(runRef)).toBe(0)
    const second = await Effect.runPromise(
      runFullAutoRecall(baseInput(fixture, registry, runRef, { recallRef: "turn.c.b2", ledger })),
    )
    expect(second.status).toBe("budget_exhausted")
    expect(second.consumedBudget).toBe(false)
    expect(second.usageRows).toEqual([])
    expect(fullAutoRecallContinuationFragment(second)).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Idempotent replay.
// ---------------------------------------------------------------------------

describe("idempotent recall replay (RLM-06)", () => {
  test("replaying the same recall returns identical refs/usage and consumes budget once", async () => {
    const fixture = await makeFixture(120)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    const input = baseInput(fixture, registry, runRef, {
      recallRef: "turn.c.replay",
      ledger,
      tierRequest: { requestedTier: "semantic" },
      admission: ADMISSION,
      semantic: {
        question: "What decision was made?",
        completeRoot: grepCommitRoot(),
      },
    })
    const first = await Effect.runPromise(runFullAutoRecall(input))
    const remainingAfterFirst = ledger.remaining(runRef)
    const second = await Effect.runPromise(runFullAutoRecall(input))
    // Identical bounded outcome (same object identity from the ledger).
    expect(second).toEqual(first)
    expect(second.usageRows).toEqual(first.usageRows)
    expect(second.resultRefs).toEqual(first.resultRefs)
    // Budget consumed exactly once; usage never double-counts.
    expect(ledger.remaining(runRef)).toBe(remainingAfterFirst)
    const inputs = ledger.usageLedgerInputs({
      provider: "codex",
      accountRef: "codex",
      requestedModel: "gpt-5.5",
    })
    expect(inputs).toHaveLength(1)
  }, 30_000)
})

// ---------------------------------------------------------------------------
// Interruption — teardown must not fabricate success or leak a ledger entry.
// ---------------------------------------------------------------------------

describe("interruption safety (RLM-06)", () => {
  test("interrupting an in-flight recall records nothing and consumes no budget", async () => {
    const fixture = await makeFixture(40)
    const { registry, runRef } = openRunRegistry()
    const ledger = makeFullAutoRecallLedger()
    let started = false
    let resolveStarted: () => void = () => {}
    const startedSignal = new Promise<void>((resolve) => {
      resolveStarted = resolve
    })
    // A root completion that signals it began, then hangs on an interruptible
    // point — only interruption (stop/reconcile/teardown) ends it.
    const hangingRoot: DesktopRlmCompleteFn = () =>
      Effect.sync(() => {
        started = true
        resolveStarted()
      }).pipe(Effect.andThen(() => Effect.never))
    const fiber = Effect.runFork(
      runFullAutoRecall(
        baseInput(fixture, registry, runRef, {
          recallRef: "turn.c.interrupt",
          ledger,
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: { question: "planted?", completeRoot: hangingRoot },
        }),
      ),
    )
    await startedSignal
    expect(started).toBe(true)
    await Effect.runPromise(Fiber.interrupt(fiber))
    // No fabricated terminal: nothing recorded, no budget consumed.
    expect(ledger.get(runRef, "turn.c.interrupt")).toBeNull()
    expect(ledger.entries(runRef)).toEqual([])
    expect(ledger.remaining(runRef)).toBe(ledger.budgetPerRun)
  }, 30_000)
})
