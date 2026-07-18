// Oracle for FA-RUN-04 (#8972): the bounded, durable, private
// `FullAutoRunReport` aggregator and its derived public-safe
// `FullAutoRunReceipt` projection. Covers aggregation correctness across a
// real multi-event run (lifecycle + stall + handoff + turns) reproducing the
// issue's own overnight-incident fixture shape ("first packet succeeded,
// continuation dispatch failed, liveness gap, recovery action, and final
// state as separate facts"), adversarial receipt redaction, and store
// retention/eviction.
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { FullAutoLivenessProjection } from "./full-auto-liveness.ts"
import { openFullAutoRunRegistry, type FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import type { ProviderHandoffTransitionRecord } from "./full-auto-provider-handoff.ts"
import { LOCAL_TURN_RECORD_SCHEMA, type LocalTurnRecord } from "./local-turn-journal.ts"
import { FULL_AUTO_MAX_CONSECUTIVE_FAILURES } from "./full-auto-reconcile.ts"
import type { FullAutoRecord } from "./full-auto-registry.ts"
import {
  FULL_AUTO_METRICS_ENV_FLAG,
  FULL_AUTO_RUN_REPORT_LIMIT,
  FULL_AUTO_RUN_REPORT_ROTATION_REASON_LIMIT,
  deriveFullAutoRunLivenessSpans,
  deriveFullAutoRunReceipt,
  isFullAutoMetricsEnabled,
  openFullAutoRunReportStore,
  sha256HexDigest,
  type FullAutoRunReportStore,
} from "./full-auto-run-report.ts"

const GRANTED_WORKSPACE = "/Users/secret-owner/private-workspace/mission-control"

const makeTurn = (
  input: Readonly<{
    threadRef: string
    turnRef: string
    lane?: string
    phase: LocalTurnRecord["phase"]
    disposition: LocalTurnRecord["disposition"]
    createdAt: string
    updatedAt: string
    assistantText?: string
  }>,
): LocalTurnRecord => ({
  schema: LOCAL_TURN_RECORD_SCHEMA,
  threadRef: input.threadRef,
  turnRef: input.turnRef,
  lane: input.lane ?? "codex-local",
  userMessageKey: `${input.turnRef}.user`,
  assistantMessageKey: `${input.turnRef}.assistant`,
  accountRef: "codex-primary",
  providerSessionRef: "sess.private-provider-session-id",
  model: "gpt-codex",
  phase: input.phase,
  persistedCursor: 0,
  assistantText: input.assistantText ?? "SECRET_TRANSCRIPT_TEXT_MUST_NEVER_LEAK",
  assistantSegments: [],
  recoveryGeneration: 0,
  disposition: input.disposition,
  createdAt: input.createdAt,
  updatedAt: input.updatedAt,
})

type Harness = Readonly<{
  root: string
  runRegistry: FullAutoRunRegistry
  reportStore: FullAutoRunReportStore
  now: () => Date
  advance: (deltaMs: number) => void
  dispose: () => void
}>

const startHarness = (): Harness => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-run-report-"))
  let clockMs = Date.parse("2026-07-17T00:00:00.000Z")
  const now = () => new Date(clockMs)
  const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), now)
  const reportStore = openFullAutoRunReportStore(path.join(root, "reports.json"), now)
  return {
    root,
    runRegistry,
    reportStore,
    now,
    advance: (deltaMs) => {
      clockMs += deltaMs
    },
    dispose: () => rmSync(root, { recursive: true, force: true }),
  }
}

const START = {
  title: "Overnight incident replay",
  objective: "SECRET_OBJECTIVE_do not leak this raw text anywhere in a public receipt",
  doneCondition: "SECRET_DONE_CONDITION_also must never leak",
  objectiveSource: "control_caller" as const,
  workspaceRef: GRANTED_WORKSPACE,
  threadRef: "thread.overnight-incident",
  actor: "control_api" as const,
  reason: "test bootstrap",
}

describe("full-auto-run-report aggregation (FA-RUN-04 #8972)", () => {
  test("aggregates lifecycle + liveness + handoff + turns into one bounded private report, reproducing the overnight-incident fixture shape", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const run = started.run

      // Fact 1: the first packet succeeded (a completed turn).
      const turnOne = makeTurn({
        threadRef: run.threadRef!,
        turnRef: "turn.full-auto.1",
        phase: "completed",
        disposition: "completed",
        createdAt: harness.now().toISOString(),
        updatedAt: harness.now().toISOString(),
      })
      harness.runRegistry.recordAttempt(run.runRef, "success", { turnRef: turnOne.turnRef })

      // Fact 2: liveness observes a healthy Running state right after.
      // Literal projections (rather than driving the real classifier) keep
      // this test focused on the REPORT aggregator's own logic -- the
      // classifier's correctness is FA-RUN-03's own dedicated test surface.
      const projectionRunning: FullAutoLivenessProjection = {
        runRef: run.runRef,
        projectedState: "running",
        cause: null,
        nextRetryAt: null,
        recoveryAction: "none",
        sinceLastProgressMs: 0,
      }
      let report = harness.reportStore.sync({
        run: harness.runRegistry.get(run.runRef)!,
        turns: [turnOne],
        handoffs: [],
        livenessProjection: projectionRunning,
      })
      expect(report.turns).toHaveLength(1)
      expect(report.turns[0]!.turnRef).toBe("turn.full-auto.1")
      expect(report.turns[0]!.disposition).toBe("completed")
      // Never the raw transcript text -- only identity/phase/disposition/outcomeSummary.
      expect(JSON.stringify(report)).not.toContain("SECRET_TRANSCRIPT_TEXT_MUST_NEVER_LEAK")

      // Fact 3: continuation dispatch fails, then the liveness SLO elapses --
      // a genuine Running -> Stalled transition, attributed to liveness_monitor.
      harness.advance(6 * 60_000)
      const stalledResult = harness.runRegistry.transition(run.runRef, {
        to: "stalled",
        actor: "liveness_monitor",
        reason:
          "liveness monitor: no continuation was accepted within the liveness SLO window (cause: dispatch_overdue)",
      })
      expect(stalledResult.ok).toBe(true)
      const stalledRun = stalledResult.ok ? stalledResult.run : run
      const projectionStalled: FullAutoLivenessProjection = {
        runRef: run.runRef,
        projectedState: "stalled",
        cause: "dispatch_overdue",
        nextRetryAt: null,
        recoveryAction: "retry_now",
        sinceLastProgressMs: 360_000,
      }
      report = harness.reportStore.sync({
        run: stalledRun,
        turns: [turnOne],
        handoffs: [],
        livenessProjection: projectionStalled,
      })
      expect(report.livenessObservations.length).toBeGreaterThanOrEqual(2)
      expect(report.livenessGaps).toHaveLength(1)
      expect(report.livenessGaps[0]!.exitedAt).toBeNull() // still ongoing
      expect(report.livenessGaps[0]!.cause).toBe("dispatch_overdue")

      // Fact 4: recovery action -- retry now, back to Retrying.
      harness.advance(30_000)
      const retryResult = harness.runRegistry.transition(run.runRef, {
        to: "retrying",
        actor: "control_api",
        reason: "retry now requested (cause was: dispatch_overdue)",
      })
      expect(retryResult.ok).toBe(true)
      const retryingRun = retryResult.ok ? retryResult.run : stalledRun

      // Fact 5: a provider handoff happens while investigating.
      const handoff: ProviderHandoffTransitionRecord = {
        handoffRef: "handoff.provider.1",
        runRef: run.runRef,
        threadRef: run.threadRef!,
        from: "codex-local",
        to: "claude-local",
        actor: "control_api",
        at: harness.now().toISOString(),
        reason: "SECRET_HANDOFF_REASON_do_not_leak_this_either",
        disposition: "complete_within_bounds",
        truncated: false,
      }
      const projectionRetrying: FullAutoLivenessProjection = {
        runRef: run.runRef,
        projectedState: "retrying",
        cause: "dispatch_overdue",
        nextRetryAt: new Date(harness.now().getTime() + 120_000).toISOString(),
        recoveryAction: "none",
        sinceLastProgressMs: 30_000,
      }
      report = harness.reportStore.sync({
        run: retryingRun,
        turns: [turnOne],
        handoffs: [handoff],
        livenessProjection: projectionRetrying,
      })
      expect(report.providerTransitions).toHaveLength(1)
      expect(report.providerTransitions[0]!.handoffRef).toBe("handoff.provider.1")

      // Recovery resolves; the run goes back to a healthy Running state,
      // closing the gap, then finally reaches a terminal state.
      harness.advance(10_000)
      const recoveredResult = harness.runRegistry.transition(run.runRef, {
        to: "running",
        actor: "liveness_monitor",
        reason: "liveness monitor: continuation dispatch resumed normally",
      })
      expect(recoveredResult.ok).toBe(true)
      const recoveredRun = recoveredResult.ok ? recoveredResult.run : retryingRun
      const projectionRecovered: FullAutoLivenessProjection = {
        runRef: run.runRef,
        projectedState: "running",
        cause: null,
        nextRetryAt: null,
        recoveryAction: "none",
        sinceLastProgressMs: 0,
      }
      report = harness.reportStore.sync({
        run: recoveredRun,
        turns: [turnOne],
        handoffs: [handoff],
        livenessProjection: projectionRecovered,
      })
      // Fact 6: final state -- the gap closed with a real duration.
      expect(report.livenessGaps).toHaveLength(1)
      expect(report.livenessGaps[0]!.exitedAt).not.toBeNull()
      expect(report.livenessGaps[0]!.durationMs).toBeGreaterThan(0)
      expect(report.uninterruptedIntervals.length).toBeGreaterThanOrEqual(2)

      harness.advance(1_000)
      const stoppedResult = harness.runRegistry.transition(run.runRef, {
        to: "stopped",
        actor: "control_api",
        reason: "Stop requested via the local control API.",
      })
      expect(stoppedResult.ok).toBe(true)
      const stoppedRun = stoppedResult.ok ? stoppedResult.run : recoveredRun
      report = harness.reportStore.sync({ run: stoppedRun, turns: [turnOne], handoffs: [handoff] })
      expect(report.state).toBe("stopped")
      expect(report.endedAt).toBeDefined()
      expect(report.lifecycleTransitions.length).toBeGreaterThanOrEqual(4)
      // Every fact stayed a SEPARATE record -- never collapsed/coerced.
      const transitionsToStates = report.lifecycleTransitions.map((transition) => transition.to)
      expect(transitionsToStates).toContain("stalled")
      expect(transitionsToStates).toContain("retrying")
      expect(transitionsToStates).toContain("running")
      expect(transitionsToStates).toContain("stopped")
    } finally {
      harness.dispose()
    }
  })

  test("restart continuity: a second sync with a shrunken upstream turn/handoff read never drops previously captured facts or duplicates a turn", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const run = started.run
      const turnA = makeTurn({
        threadRef: run.threadRef!,
        turnRef: "turn.full-auto.a",
        phase: "completed",
        disposition: "completed",
        createdAt: harness.now().toISOString(),
        updatedAt: harness.now().toISOString(),
      })
      const handoffA: ProviderHandoffTransitionRecord = {
        handoffRef: "handoff.a",
        runRef: run.runRef,
        from: "codex-local",
        to: "claude-local",
        actor: "control_api",
        at: harness.now().toISOString(),
        reason: "first handoff",
        disposition: "complete_within_bounds",
        truncated: false,
      }
      const firstSync = harness.reportStore.sync({ run, turns: [turnA], handoffs: [handoffA] })
      expect(firstSync.turns).toHaveLength(1)
      expect(firstSync.providerTransitions).toHaveLength(1)

      // Simulate the shared-limit upstream stores evicting turnA/handoffA
      // because unrelated activity pushed them out -- the caller's fresh
      // read for THIS run no longer includes them. The report must still
      // remember them (merge, never replace).
      harness.advance(1_000)
      const turnB = makeTurn({
        threadRef: run.threadRef!,
        turnRef: "turn.full-auto.b",
        phase: "completed",
        disposition: "completed",
        createdAt: harness.now().toISOString(),
        updatedAt: harness.now().toISOString(),
      })
      const secondSync = harness.reportStore.sync({ run, turns: [turnB], handoffs: [] })
      expect(secondSync.turns.map((turn) => turn.turnRef).toSorted()).toEqual([
        "turn.full-auto.a",
        "turn.full-auto.b",
      ])
      expect(secondSync.providerTransitions.map((handoff) => handoff.handoffRef)).toEqual([
        "handoff.a",
      ])

      // Re-syncing the SAME turn again (e.g. after an app restart re-reads
      // the journal) must never duplicate it.
      const thirdSync = harness.reportStore.sync({
        run,
        turns: [turnA, turnB],
        handoffs: [handoffA],
      })
      expect(thirdSync.turns).toHaveLength(2)
      expect(thirdSync.providerTransitions).toHaveLength(1)
      expect(thirdSync.reportRevision).toBe(3)
    } finally {
      harness.dispose()
    }
  })

  test("deriveFullAutoRunLivenessSpans: an ongoing gap with no closing observation stays explicitly open (null exitedAt/durationMs), never fabricated", () => {
    const spans = deriveFullAutoRunLivenessSpans({
      observations: [
        {
          at: "2026-07-17T00:00:00.000Z",
          projectedState: "stalled",
          cause: "dispatch_overdue",
          recoveryAction: "retry_now",
          sinceLastProgressMs: 400_000,
        },
      ],
      anchorAt: "2026-07-16T23:50:00.000Z",
      currentState: "stalled",
    })
    expect(spans.gaps).toHaveLength(1)
    expect(spans.gaps[0]!.exitedAt).toBeNull()
    expect(spans.gaps[0]!.durationMs).toBeNull()
    expect(spans.intervals).toHaveLength(1)
    expect(spans.intervals[0]!.startedAt).toBe("2026-07-16T23:50:00.000Z")
    expect(spans.intervals[0]!.endedAt).toBe("2026-07-17T00:00:00.000Z")
    expect(spans.intervals[0]!.durationMs).toBe(600_000)
  })
})

describe("full-auto-run-report retention/eviction", () => {
  test("the store never exceeds its bound and protects active-run reports from eviction", () => {
    const harness = startHarness()
    try {
      // One protected active run.
      const active = harness.runRegistry.startNew({ ...START, threadRef: "thread.active" })
      expect(active.ok).toBe(true)
      if (!active.ok) return
      harness.reportStore.sync({ run: active.run, turns: [], handoffs: [] })

      // Stop it isn't allowed to make room without a real second run since
      // v1 concurrency permits only one active run; stop the active run
      // first so subsequent runs can start, then fill past the bound with
      // terminal runs.
      const stopped = harness.runRegistry.transition(active.run.runRef, {
        to: "stopped",
        actor: "control_api",
        reason: "test setup",
      })
      expect(stopped.ok).toBe(true)
      if (!stopped.ok) return
      harness.reportStore.sync({ run: stopped.run, turns: [], handoffs: [] })

      for (let index = 0; index < FULL_AUTO_RUN_REPORT_LIMIT + 20; index += 1) {
        harness.advance(1_000)
        const result = harness.runRegistry.startNew({
          ...START,
          threadRef: `thread.fill.${index}`,
        })
        expect(result.ok).toBe(true)
        if (!result.ok) continue
        const stop = harness.runRegistry.transition(result.run.runRef, {
          to: "stopped",
          actor: "control_api",
          reason: "test fill",
        })
        expect(stop.ok).toBe(true)
        if (stop.ok) harness.reportStore.sync({ run: stop.run, turns: [], handoffs: [] })
      }

      expect(harness.reportStore.list().length).toBeLessThanOrEqual(FULL_AUTO_RUN_REPORT_LIMIT)

      // Now start (and keep active) one more run -- it must always be
      // present even though the store is at its bound.
      harness.advance(1_000)
      const finalActive = harness.runRegistry.startNew({
        ...START,
        threadRef: "thread.final-active",
      })
      expect(finalActive.ok).toBe(true)
      if (!finalActive.ok) return
      const finalReport = harness.reportStore.sync({
        run: finalActive.run,
        turns: [],
        handoffs: [],
      })
      expect(harness.reportStore.get(finalReport.runRef)).not.toBeNull()
      expect(harness.reportStore.list().length).toBeLessThanOrEqual(FULL_AUTO_RUN_REPORT_LIMIT)
    } finally {
      harness.dispose()
    }
  })
})

describe("full-auto-run receipt redaction (public-safe projection)", () => {
  test("ADVERSARIAL: no combination of secret-bearing report fields (reasons, objective, doneCondition, workspace path, title, account/session refs, assistant text) ever appears in the derived receipt", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const run = started.run

      // Deliberately inject secret-shaped content into every free-text
      // surface the private report legitimately carries.
      const secrets = [
        "SECRET_OBJECTIVE",
        "SECRET_DONE_CONDITION",
        GRANTED_WORKSPACE,
        "secret-owner",
        "SECRET_HANDOFF_REASON_do_not_leak_this_either",
        "SECRET_TRANSCRIPT_TEXT_MUST_NEVER_LEAK",
        "sess.private-provider-session-id",
        "codex-primary",
        run.title,
        "sk-live-fake-api-key-should-never-appear-1234567890",
        "/etc/passwd",
        "Bearer super-secret-token-value",
      ]

      const turn = makeTurn({
        threadRef: run.threadRef!,
        turnRef: "turn.full-auto.adversarial",
        phase: "failed",
        disposition: "failed",
        createdAt: harness.now().toISOString(),
        updatedAt: harness.now().toISOString(),
        assistantText:
          "sk-live-fake-api-key-should-never-appear-1234567890 Bearer super-secret-token-value /etc/passwd",
      })
      const handoff: ProviderHandoffTransitionRecord = {
        handoffRef: "handoff.adversarial",
        runRef: run.runRef,
        threadRef: run.threadRef!,
        from: "codex-local",
        to: "claude-local",
        actor: "control_api",
        at: harness.now().toISOString(),
        reason: "SECRET_HANDOFF_REASON_do_not_leak_this_either",
        disposition: "truncated_with_confirmation",
        truncated: true,
      }
      const projection: FullAutoLivenessProjection = {
        runRef: run.runRef,
        projectedState: "stalled",
        cause: "dispatch_overdue",
        nextRetryAt: null,
        recoveryAction: "retry_now",
        sinceLastProgressMs: 400_000,
      }
      const report = harness.reportStore.sync({
        run,
        turns: [turn],
        handoffs: [handoff],
        livenessProjection: projection,
      })
      // Sanity: the PRIVATE report legitimately does carry some of this
      // (workspaceRef, title) -- prove the fixture is meaningful.
      expect(JSON.stringify(report)).toContain(GRANTED_WORKSPACE)

      const receipt = deriveFullAutoRunReceipt(report, harness.now)
      const receiptJson = JSON.stringify(receipt)
      for (const secret of secrets) {
        expect(receiptJson, `receipt must never contain: ${secret}`).not.toContain(secret)
      }
      // The receipt is provably structural: every string value is either a
      // known-safe enum member, an opaque system-minted ref, an ISO
      // timestamp, or a 64-character hex digest.
      const digestPattern = /^[0-9a-f]{64}$/
      const refPattern = /^[\w.:/-]{1,180}$/
      const isoPattern = /^\d{4}-\d{2}-\d{2}T/
      const knownEnumValues = new Set([
        "complete_within_bounds",
        "truncated_with_confirmation",
        "refused",
        "retry_now",
        "stop_only",
        "none",
        "unknown",
        "draft",
        "running",
        "pausing",
        "paused",
        "retrying",
        "stalled",
        "completed",
        "failed",
        "stopped",
        "cap_reached",
        "codex-local",
        "claude-local",
      ])
      const visit = (value: unknown): void => {
        if (typeof value === "string") {
          expect(
            digestPattern.test(value) ||
              refPattern.test(value) ||
              isoPattern.test(value) ||
              knownEnumValues.has(value),
            `unexpected free-form string in receipt: ${JSON.stringify(value)}`,
          ).toBe(true)
          return
        }
        if (Array.isArray(value)) {
          for (const entry of value) visit(entry)
          return
        }
        if (value !== null && typeof value === "object") {
          for (const entry of Object.values(value)) visit(entry)
        }
      }
      visit(receipt)

      // Field-level redaction assertions.
      expect((receipt as Record<string, unknown>).title).toBeUndefined()
      expect((receipt as Record<string, unknown>).objective).toBeUndefined()
      expect((receipt as Record<string, unknown>).doneCondition).toBeUndefined()
      expect((receipt as Record<string, unknown>).workspaceRef).toBeUndefined()
      expect((receipt as Record<string, unknown>).reason).toBeUndefined()
      expect(receipt.workspaceRefDigest).toBe(sha256HexDigest(GRANTED_WORKSPACE))
      expect(receipt.objectiveDigest).toBe(sha256HexDigest(run.objective))
    } finally {
      harness.dispose()
    }
  })

  test("the receipt schema itself has no field capable of holding unbounded free text (structural redaction proof)", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const report = harness.reportStore.sync({ run: started.run, turns: [], handoffs: [] })
      const receipt = deriveFullAutoRunReceipt(report, harness.now)
      // The receipt object's own key set is small, fixed, and reviewed --
      // pin it so a future field addition is a deliberate, visible diff.
      expect(Object.keys(receipt).toSorted()).toEqual(
        [
          "claimedRefCount",
          "createdAt",
          "doneConditionDigest",
          "livenessGapCount",
          "objectiveDigest",
          "progressDisposition",
          "providerIdentities",
          "providerTransitionCount",
          "providerTransitionDispositions",
          "recoveryActionsUsed",
          "reportRevision",
          "runRef",
          "schema",
          "startedAt",
          "state",
          "successfulAttempts",
          "threadRef",
          "turnCap",
          "usageKnown",
          "verifiedRefCount",
          "workspaceRefDigest",
          "failedAttempts",
        ].toSorted(),
      )
    } finally {
      harness.dispose()
    }
  })
})

// ---------------------------------------------------------------------------
// FA-RPT-01 (#8988): report extensions -- terminal-class fixtures, typed
// failure history + disabledBy attribution, rotation passthrough, claimed
// commit-SHA evidence, and the default-on local-only metrics gate.
// ---------------------------------------------------------------------------

const makeThreadRecord = (
  overrides?: Partial<FullAutoRecord> & Readonly<{ rotationHistory?: unknown }>,
): FullAutoRecord & Readonly<{ rotationHistory?: unknown }> => ({
  threadRef: START.threadRef,
  enabled: true,
  continuationCount: 3,
  updatedAt: "2026-07-17T00:00:00.000Z",
  workspaceRef: GRANTED_WORKSPACE,
  profile: { lane: "codex-local", accountRef: "codex-primary" },
  ...overrides,
})

describe("FA-RPT-01 terminal classes (#8988 acceptance 1)", () => {
  const terminalFixtures = [
    { to: "completed", actor: "control_api", expectedStopAttribution: "control_api" },
    { to: "cap_reached", actor: "continuation_cap", expectedStopAttribution: "continuation_cap" },
    { to: "failed", actor: "dispatch_failure_limit", expectedStopAttribution: "dispatch_failure_limit" },
    { to: "stopped", actor: "control_api", expectedStopAttribution: "control_api" },
  ] as const

  for (const fixture of terminalFixtures) {
    test(`${fixture.to}: every section renders with exact bounded values and typed stop attribution`, () => {
      const harness = startHarness()
      try {
        const started = harness.runRegistry.startNew(START)
        expect(started.ok).toBe(true)
        if (!started.ok) return
        const run = started.run
        const turn = makeTurn({
          threadRef: run.threadRef!,
          turnRef: "turn.full-auto.1",
          phase: "completed",
          disposition: "completed",
          createdAt: harness.now().toISOString(),
          updatedAt: harness.now().toISOString(),
        })
        harness.runRegistry.recordAttempt(run.runRef, "success", { turnRef: turn.turnRef })
        harness.advance(1_000)
        const result = harness.runRegistry.transition(run.runRef, {
          to: fixture.to,
          actor: fixture.actor,
          reason: `test fixture: ${fixture.to}`,
        })
        expect(result.ok).toBe(true)
        if (!result.ok) return
        const report = harness.reportStore.sync({
          run: result.run,
          turns: [turn],
          handoffs: [],
          threadRecord: makeThreadRecord({
            enabled: false,
            disabledBy: fixture.to === "cap_reached"
              ? "continuation_cap"
              : fixture.to === "failed"
                ? "dispatch_failure_limit"
                : "control_api",
            disabledAt: harness.now().toISOString(),
          }),
          metricsEnabled: true,
        })
        expect(report.state).toBe(fixture.to)
        expect(report.endedAt).toBeDefined()
        expect(report.stopAttribution).toBe(fixture.expectedStopAttribution)
        expect(report.turns).toHaveLength(1)
        expect(report.turns[0]!.lane).toBe("codex-local")
        expect(report.turns[0]!.accountRef).toBe("codex-primary")
        expect(report.threadFailureHistory).toBeDefined()
        expect(report.threadFailureHistory!.failureLimit).toBe(FULL_AUTO_MAX_CONSECUTIVE_FAILURES)
        expect(report.metricsEnabled).toBe(true)
        expect(report.metrics).toBeDefined()
        expect(report.metrics!.stopAttributed).toBe(true)
        expect(report.metrics!.continuationsDispatched).toBe(1)
        // Never raw transcript/objective text, in ANY terminal class.
        const raw = JSON.stringify(report)
        expect(raw).not.toContain("SECRET_TRANSCRIPT_TEXT_MUST_NEVER_LEAK")
        expect(raw).not.toContain("SECRET_OBJECTIVE")
        expect(raw).not.toContain("SECRET_DONE_CONDITION")
      } finally {
        harness.dispose()
      }
    })
  }

  test("a non-terminal run has no stopAttribution and metrics.stopAttributed is false", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const report = harness.reportStore.sync({
        run: started.run,
        turns: [],
        handoffs: [],
        metricsEnabled: true,
      })
      expect(report.stopAttribution).toBeUndefined()
      expect(report.metrics!.stopAttributed).toBe(false)
    } finally {
      harness.dispose()
    }
  })
})

describe("FA-RPT-01 thread failure history + rotation passthrough", () => {
  test("the thread record's typed failure history (counters, disabledBy attribution) lands on the report", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const report = harness.reportStore.sync({
        run: started.run,
        turns: [],
        handoffs: [],
        threadRecord: makeThreadRecord({
          enabled: false,
          consecutiveFailures: 5,
          lastFailureAt: "2026-07-17T00:10:00.000Z",
          blockedReason: "dispatch failed: provider unavailable",
          disabledBy: "dispatch_failure_limit",
          disabledAt: "2026-07-17T00:11:00.000Z",
        }),
      })
      expect(report.threadFailureHistory).toEqual({
        consecutiveFailures: 5,
        failureLimit: FULL_AUTO_MAX_CONSECUTIVE_FAILURES,
        lastFailureAt: "2026-07-17T00:10:00.000Z",
        blockedReason: "dispatch failed: provider unavailable",
        disabledBy: "dispatch_failure_limit",
        disabledAt: "2026-07-17T00:11:00.000Z",
      })

      // A later sync WITHOUT a thread record never regresses the captured
      // section (merge, never replace).
      const again = harness.reportStore.sync({ run: harness.runRegistry.get(started.run.runRef)!, turns: [], handoffs: [] })
      expect(again.threadFailureHistory).toEqual(report.threadFailureHistory)
    } finally {
      harness.dispose()
    }
  })

  test("rotationHistory passthrough: absent today, re-validated/bounded when a future record carries one, invalid entries skipped", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      // Every current record has no rotationHistory -- the section is absent.
      const plain = harness.reportStore.sync({
        run: started.run,
        turns: [],
        handoffs: [],
        threadRecord: makeThreadRecord(),
      })
      expect(plain.rotationHistory).toBeUndefined()

      const report = harness.reportStore.sync({
        run: harness.runRegistry.get(started.run.runRef)!,
        turns: [],
        handoffs: [],
        threadRecord: makeThreadRecord({
          rotationHistory: [
            { fromLane: "codex-local", toLane: "fable-local", reason: "x".repeat(500), at: "2026-07-17T00:05:00.000Z" },
            { fromLane: "fable-local", toLane: "codex-local", reason: "rate limited", at: "2026-07-17T00:20:00.000Z" },
            { garbage: true }, // invalid entries are skipped, never guessed into shape
          ],
        }),
      })
      expect(report.rotationHistory).toHaveLength(2)
      expect(report.rotationHistory![0]!.reason).toHaveLength(FULL_AUTO_RUN_REPORT_ROTATION_REASON_LIMIT)
      expect(report.rotationHistory![1]).toEqual({
        fromLane: "fable-local",
        toLane: "codex-local",
        reason: "rate limited",
        at: "2026-07-17T00:20:00.000Z",
      })
    } finally {
      harness.dispose()
    }
  })
})

describe("FA-RPT-01 claimed commit-SHA evidence", () => {
  test("full 40-hex SHAs from the journal become claimed commit refs -- deduplicated, turn-attributed, never verified, never leaking surrounding text", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const sha = "0123456789abcdef0123456789abcdef01234567"
      const otherSha = "fedcba9876543210fedcba9876543210fedcba98"
      const turnOne = makeTurn({
        threadRef: started.run.threadRef!,
        turnRef: "turn.full-auto.1",
        phase: "completed",
        disposition: "completed",
        createdAt: harness.now().toISOString(),
        updatedAt: harness.now().toISOString(),
        assistantText: `SECRET_PROSE committed ${sha}; short abc1234 must not count; again ${sha}.`,
      })
      const report = harness.reportStore.sync({ run: started.run, turns: [turnOne], handoffs: [] })
      expect(report.verifiedRefs).toEqual([
        { ref: sha, kind: "commit", verification: "claimed", turnRef: "turn.full-auto.1" },
      ])
      expect(JSON.stringify(report)).not.toContain("SECRET_PROSE")

      // A later sync with a SHRUNKEN journal read never drops the captured
      // ref, and new SHAs merge in.
      harness.advance(1_000)
      const turnTwo = makeTurn({
        threadRef: started.run.threadRef!,
        turnRef: "turn.full-auto.2",
        phase: "completed",
        disposition: "completed",
        createdAt: harness.now().toISOString(),
        updatedAt: harness.now().toISOString(),
        assistantText: `pushed ${otherSha}`,
      })
      const again = harness.reportStore.sync({
        run: harness.runRegistry.get(started.run.runRef)!,
        turns: [turnTwo],
        handoffs: [],
      })
      expect(again.verifiedRefs.map((ref) => ref.ref).toSorted()).toEqual([sha, otherSha].toSorted())
      expect(again.verifiedRefs.every((ref) => ref.verification === "claimed")).toBe(true)

      // The public-safe receipt reflects the claimed count without carrying
      // the refs' surrounding context (it never could -- counts only).
      const receipt = deriveFullAutoRunReceipt(again, harness.now)
      expect(receipt.claimedRefCount).toBe(2)
      expect(receipt.verifiedRefCount).toBe(0)
    } finally {
      harness.dispose()
    }
  })
})

describe("FA-RPT-01 metrics default-on (#8988 acceptance 3)", () => {
  test("the env gate is ON by default and disabled only by the explicit owner override", () => {
    expect(isFullAutoMetricsEnabled({})).toBe(true)
    expect(isFullAutoMetricsEnabled({ [FULL_AUTO_METRICS_ENV_FLAG]: undefined })).toBe(true)
    expect(isFullAutoMetricsEnabled({ [FULL_AUTO_METRICS_ENV_FLAG]: "" })).toBe(true)
    expect(isFullAutoMetricsEnabled({ [FULL_AUTO_METRICS_ENV_FLAG]: "1" })).toBe(true)
    expect(isFullAutoMetricsEnabled({ [FULL_AUTO_METRICS_ENV_FLAG]: "0" })).toBe(false)
    expect(isFullAutoMetricsEnabled({ [FULL_AUTO_METRICS_ENV_FLAG]: "false" })).toBe(false)
    expect(isFullAutoMetricsEnabled({ [FULL_AUTO_METRICS_ENV_FLAG]: "off" })).toBe(false)
  })

  test("a fresh sync with no explicit gate input follows the env default (ON in this test environment)", () => {
    const harness = startHarness()
    try {
      expect(process.env[FULL_AUTO_METRICS_ENV_FLAG]).toBeUndefined()
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const report = harness.reportStore.sync({ run: started.run, turns: [], handoffs: [] })
      expect(report.metricsEnabled).toBe(true)
      expect(report.metrics).toBeDefined()
    } finally {
      harness.dispose()
    }
  })

  test("enabled metrics carry exact public-safe counters over the merged turn history", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const sha = "0123456789abcdef0123456789abcdef01234567"
      const at = (seconds: number) => new Date(Date.UTC(2026, 6, 17, 0, 0, seconds)).toISOString()
      const turns = [
        makeTurn({ threadRef: started.run.threadRef!, turnRef: "turn.full-auto.1", phase: "completed", disposition: "completed", createdAt: at(1), updatedAt: at(1), assistantText: `did ${sha}` }),
        makeTurn({ threadRef: started.run.threadRef!, turnRef: "turn.full-auto.2", phase: "completed", disposition: "completed", createdAt: at(2), updatedAt: at(2) }),
        makeTurn({ threadRef: started.run.threadRef!, turnRef: "turn.full-auto.3", phase: "failed", disposition: "failed", createdAt: at(3), updatedAt: at(3) }),
        makeTurn({ threadRef: started.run.threadRef!, turnRef: "turn.full-auto.4", phase: "completed", disposition: "completed", createdAt: at(4), updatedAt: at(4) }),
        makeTurn({ threadRef: started.run.threadRef!, turnRef: "turn.full-auto.5", phase: "interrupted", disposition: "owner_interrupted", createdAt: at(5), updatedAt: at(5) }),
      ]
      harness.runRegistry.recordAttempt(started.run.runRef, "success", { turnRef: "turn.full-auto.1" })
      harness.runRegistry.recordAttempt(started.run.runRef, "success", { turnRef: "turn.full-auto.2" })
      harness.runRegistry.recordAttempt(started.run.runRef, "failure", { turnRef: "turn.full-auto.3" })
      const report = harness.reportStore.sync({
        run: harness.runRegistry.get(started.run.runRef)!,
        turns,
        handoffs: [],
        metricsEnabled: true,
      })
      expect(report.metrics).toEqual({
        turnsObserved: 5,
        turnsCompleted: 3,
        turnsFailed: 1,
        turnsInterrupted: 1,
        longestCompletedStreak: 2,
        continuationsDispatched: 2,
        dispatchFailures: 1,
        repoGroundedTurns: 1,
        evidenceRefCount: 1,
        stopAttributed: false,
      })
    } finally {
      harness.dispose()
    }
  })

  test("a disabled gate is an honest absence -- metricsEnabled false and NO metrics row -- and everything else still derives", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const report = harness.reportStore.sync({
        run: started.run,
        turns: [],
        handoffs: [],
        metricsEnabled: false,
      })
      expect(report.metricsEnabled).toBe(false)
      expect(report.metrics).toBeUndefined()
      expect(report.state).toBe("running")
    } finally {
      harness.dispose()
    }
  })
})

describe("FA-RPT-01 back-compat with pre-#8988 report files", () => {
  test("a persisted report file written before the #8988 fields existed still decodes (no quarantine) and upgrades on the next sync", () => {
    const harness = startHarness()
    try {
      const started = harness.runRegistry.startNew(START)
      expect(started.ok).toBe(true)
      if (!started.ok) return
      harness.reportStore.sync({ run: started.run, turns: [], handoffs: [], metricsEnabled: true })

      // Strip every #8988-added field from the durable file, reproducing a
      // file written by the original FA-RUN-04 code.
      const filePath = path.join(harness.root, "reports.json")
      const file = JSON.parse(readFileSync(filePath, "utf8")) as {
        schema: string
        reports: Array<Record<string, unknown>>
      }
      for (const report of file.reports) {
        delete report.threadFailureHistory
        delete report.rotationHistory
        delete report.stopAttribution
        delete report.metricsEnabled
        delete report.metrics
      }
      writeFileSync(filePath, `${JSON.stringify(file)}\n`, "utf8")

      // Reopen: decodes cleanly (a quarantine would start empty).
      const reopened = openFullAutoRunReportStore(filePath, harness.now)
      const stored = reopened.get(started.run.runRef)
      expect(stored).not.toBeNull()
      expect(stored!.metricsEnabled).toBeUndefined()

      // The next sync upgrades the row in place with the new sections.
      const upgraded = reopened.sync({
        run: harness.runRegistry.get(started.run.runRef)!,
        turns: [],
        handoffs: [],
        metricsEnabled: true,
      })
      expect(upgraded.metricsEnabled).toBe(true)
      expect(upgraded.metrics).toBeDefined()
    } finally {
      harness.dispose()
    }
  })
})
