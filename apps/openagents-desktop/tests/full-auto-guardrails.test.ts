import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  FULL_AUTO_BLOCKED_REASON_LIMIT,
  FULL_AUTO_DECISION_HISTORY_LIMIT,
  FULL_AUTO_REGISTRY_SCHEMA,
  openFullAutoRegistry,
  projectFullAutoDecisionHistory,
  type FullAutoRegistry,
} from "../src/full-auto-registry.ts"
import {
  detectFullAutoNoProgress,
  FULL_AUTO_MAX_CONSECUTIVE_FAILURES,
  FULL_AUTO_MAX_CONTINUATIONS,
  FULL_AUTO_NO_PROGRESS_TURN_THRESHOLD,
  FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS,
  fullAutoFailureBackoffMs,
  reconcileFullAutoThreads,
  resumeFullAuto,
  type FullAutoGuardrailViolation,
} from "../src/full-auto-reconcile.ts"
import { validateFullAutoRoutingPolicy } from "../src/full-auto-routing.ts"
import { openFullAutoRunRegistry } from "../src/full-auto-run-registry.ts"
import { openFullAutoRunReportStore } from "../src/full-auto-run-report.ts"

/**
 * FA-GD-01 (#8991): guardrails, budgets, and confidence-gated continuation.
 * Follows the exact "Runtime A seeds durable state, Runtime B re-opens the
 * same files" shape as full-auto-restart.e2e.test.ts.
 */

const GRANTED_WORKSPACE = "/granted/workspace-a"

const reconcile = (
  registry: FullAutoRegistry,
  overrides: Partial<Parameters<typeof reconcileFullAutoThreads>[0]> &
    Pick<Parameters<typeof reconcileFullAutoThreads>[0], "dispatch">,
) => reconcileFullAutoThreads({
  registry,
  nonterminalThreadRefs: () => new Set<string>(),
  resolveWorkspaceRef: () => GRANTED_WORKSPACE,
  journalHasNonterminalTurn: () => false,
  ...overrides,
})

describe("FA-GD-01 guardrail schema on the durable record", () => {
  test("a rev-11-era registry file WITHOUT any guardrail/pause/decision fields still decodes and behaves exactly as before (legacy fixture)", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-legacy-"))
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      mkdirSync(registryDir, { recursive: true })
      // The exact pre-#8991 on-disk shape (rev 11: routing fields present).
      writeFileSync(registryFile, JSON.stringify({
        schema: FULL_AUTO_REGISTRY_SCHEMA,
        records: [{
          threadRef: "thread-legacy",
          enabled: true,
          continuationCount: 3,
          updatedAt: "2026-07-17T00:00:00.000Z",
          workspaceRef: GRANTED_WORKSPACE,
          profile: { lane: "codex-local" },
          routingPolicy: [{ lane: "codex-local" }],
          rotationHistory: [{
            fromLane: "codex-local",
            toLane: "claude-local",
            reason: "rate_limited",
            at: "2026-07-16T00:00:00.000Z",
          }],
        }],
      }), "utf8")

      const registry = openFullAutoRegistry(registryFile)
      expect(readdirSync(registryDir).filter(name => name.includes("quarantined"))).toEqual([])
      const record = registry.record("thread-legacy")
      expect(record?.enabled).toBe(true)
      expect(record?.guardrails).toBeUndefined()
      expect(record?.enabledAt).toBeUndefined()
      expect(record?.pausedReason).toBeUndefined()
      expect(record?.decisionHistory).toBeUndefined()
      expect(projectFullAutoDecisionHistory(record!)).toEqual([])
      // Mutating through existing paths never invents the new fields.
      registry.recordFailure("thread-legacy", "transient")
      registry.recordSuccess("thread-legacy")
      const touched = openFullAutoRegistry(registryFile).record("thread-legacy")
      expect(touched?.guardrails).toBeUndefined()
      expect(touched?.pausedReason).toBeUndefined()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("bindGuardrails persists durably, survives enable/disable transitions like routingPolicy, null clears, and invalid shapes fail closed", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-bind-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-g", true, { workspaceRef: GRANTED_WORKSPACE })
      const guardrails = {
        maxWallClockMs: 60_000,
        maxTurns: 5,
        maxPerTurnFailures: 2,
        tokenBudgetRef: "budget.overnight-run",
      }
      expect(registry.bindGuardrails("thread-g", guardrails)?.guardrails).toEqual(guardrails)
      // Durable across a fresh open.
      expect(openFullAutoRegistry(registryFile).record("thread-g")?.guardrails).toEqual(guardrails)
      // Survives disable/enable (a durable grant, like routingPolicy).
      registry.set("thread-g", false, { disabledBy: "ui_toggle" })
      registry.set("thread-g", true, { workspaceRef: GRANTED_WORKSPACE })
      expect(registry.record("thread-g")?.guardrails).toEqual(guardrails)
      // An enable-time option rebinds; null clears.
      registry.set("thread-g", true, { workspaceRef: GRANTED_WORKSPACE, guardrails: { maxTurns: 9 } })
      expect(registry.record("thread-g")?.guardrails).toEqual({ maxTurns: 9 })
      expect(registry.bindGuardrails("thread-g", null)?.guardrails).toBeUndefined()
      // Missing record is a null no-op; non-positive limits fail closed.
      expect(registry.bindGuardrails("thread-missing", guardrails)).toBe(null)
      expect(() => registry.bindGuardrails("thread-g", { maxTurns: 0 })).toThrow()
      expect(() => registry.bindGuardrails("thread-g", { maxWallClockMs: -5 })).toThrow()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("enabledAt stamps on a fresh grant, is preserved by a redundant re-enable, and re-stamps on off-then-on", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-enabledat-"))
    try {
      let tick = 0
      const registry = openFullAutoRegistry(
        path.join(root, "full-auto", "registry.json"),
        () => new Date(Date.UTC(2026, 6, 17, 0, 0, tick++)),
      )
      const first = registry.set("thread-anchor", true, { workspaceRef: GRANTED_WORKSPACE })
      expect(first.enabledAt).toBeDefined()
      // Redundant re-enable keeps the anchor (the wall clock must not reset).
      expect(registry.set("thread-anchor", true).enabledAt).toBe(first.enabledAt)
      // Off-then-on is a fresh grant with a fresh anchor.
      registry.set("thread-anchor", false, { disabledBy: "ui_toggle" })
      const regranted = registry.set("thread-anchor", true, { workspaceRef: GRANTED_WORKSPACE })
      expect(regranted.enabledAt).not.toBe(first.enabledAt)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("FA-GD-01 continuation decision records", () => {
  test(`recordDecision appends typed facts, truncates the reason, caps at ${FULL_AUTO_DECISION_HISTORY_LIMIT} oldest-evicted, survives a fresh open AND disable/enable, and projects public-safe`, () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-decisions-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-d", true, { workspaceRef: GRANTED_WORKSPACE })

      const first = registry.recordDecision("thread-d", {
        decision: "continue",
        reason: "dispatch_succeeded",
        budgetRemaining: 19,
        goalRef: "run.goal-ref",
      })
      expect(first?.decisionHistory).toEqual([{
        at: expect.any(String),
        decision: "continue",
        reason: "dispatch_succeeded",
        budgetRemaining: 19,
        goalRef: "run.goal-ref",
      }])
      // Over-long reasons truncate write-side rather than throwing.
      const truncated = registry.recordDecision("thread-d", {
        decision: "stop_guardrail",
        reason: "x".repeat(FULL_AUTO_BLOCKED_REASON_LIMIT + 50),
      })
      expect(truncated?.decisionHistory?.at(-1)?.reason).toHaveLength(FULL_AUTO_BLOCKED_REASON_LIMIT)

      for (let index = 0; index < FULL_AUTO_DECISION_HISTORY_LIMIT + 4; index += 1) {
        registry.recordDecision("thread-d", { decision: "rotate", reason: `rotation-${index}` })
      }
      const history = registry.record("thread-d")?.decisionHistory ?? []
      expect(history).toHaveLength(FULL_AUTO_DECISION_HISTORY_LIMIT)
      // The oldest entries evicted; the most recent entry is last.
      expect(history.every(entry => entry.decision === "rotate")).toBe(true)
      expect(history.at(-1)?.reason).toBe(`rotation-${FULL_AUTO_DECISION_HISTORY_LIMIT + 3}`)

      // Durable across a fresh open AND across disable/enable (evidence,
      // like rotationHistory).
      const reopened = openFullAutoRegistry(registryFile)
      expect(reopened.record("thread-d")?.decisionHistory).toEqual(history)
      reopened.set("thread-d", false, { disabledBy: "ui_toggle" })
      reopened.set("thread-d", true, { workspaceRef: GRANTED_WORKSPACE })
      expect(reopened.record("thread-d")?.decisionHistory).toEqual(history)

      // The projection is bounded and returns exactly the typed fields.
      const projected = projectFullAutoDecisionHistory(reopened.record("thread-d")!)
      expect(projected).toHaveLength(FULL_AUTO_DECISION_HISTORY_LIMIT)
      for (const entry of projected) {
        expect(Object.keys(entry).sort()).toEqual(["at", "decision", "reason"])
      }
      // Missing record is a null no-op.
      expect(registry.recordDecision("thread-missing", { decision: "continue", reason: "x" })).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a successful continuation records a `continue` decision with the remaining turn budget; a rotation records a `rotate` decision", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-decision-flow-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-flow", true, {
        workspaceRef: GRANTED_WORKSPACE,
        profile: { lane: "codex-local" },
        routingPolicy: [{ lane: "codex-local" }, { lane: "claude-local" }],
        guardrails: { maxTurns: 5 },
      })
      await reconcile(registry, {
        dispatch: async input =>
          input.profile?.lane === "codex-local"
            ? { ok: false, reason: "usage limit", failureClass: "account_exhausted" }
            : { ok: true },
      })
      const decisions = registry.record("thread-flow")?.decisionHistory ?? []
      expect(decisions.map(entry => entry.decision)).toEqual(["rotate", "continue"])
      expect(decisions[0]?.reason).toBe("codex-local>claude-local:account_exhausted")
      expect(decisions[1]).toMatchObject({ reason: "dispatch_succeeded", budgetRemaining: 4 })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("FA-GD-01 guardrail enforcement halts a synthetic run with its typed reason", () => {
  test("max_wall_clock: an expired run terminates before dispatch with guardrail_max_wall_clock, disabledBy guardrail, a stop_guardrail decision, and the typed violation callback; within the window it dispatches", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-wallclock-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const enabledAtMs = Date.UTC(2026, 6, 17, 0, 0, 0)
      const registry = openFullAutoRegistry(registryFile, () => new Date(enabledAtMs))
      registry.set("thread-wc", true, {
        workspaceRef: GRANTED_WORKSPACE,
        guardrails: { maxWallClockMs: 60_000 },
      })

      // Within the window: dispatches normally.
      let dispatchCount = 0
      expect(await reconcile(registry, {
        now: () => new Date(enabledAtMs + 59_999),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })).toEqual(["thread-wc"])
      expect(dispatchCount).toBe(1)

      // At/after the window: terminates typed, without dispatching.
      const violations: Array<FullAutoGuardrailViolation> = []
      expect(await reconcile(registry, {
        now: () => new Date(enabledAtMs + 60_001),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onGuardrailStopped: (_threadRef, violation) => { violations.push(violation) },
      })).toEqual([])
      expect(dispatchCount).toBe(1)
      const record = registry.record("thread-wc")
      expect(record?.enabled).toBe(false)
      expect(record?.blockedReason).toBe("guardrail_max_wall_clock")
      expect(record?.disabledBy).toBe("guardrail")
      expect(record?.decisionHistory?.at(-1)).toMatchObject({
        decision: "stop_guardrail",
        reason: "guardrail_max_wall_clock",
        budgetRemaining: 0,
      })
      expect(violations).toEqual([{
        guardrail: "max_wall_clock",
        limit: 60_000,
        observed: 60_001,
        reason: "guardrail_max_wall_clock",
      }])
      // Durable: a fresh open (restart) still shows the typed stop.
      expect(openFullAutoRegistry(registryFile).record("thread-wc")?.blockedReason)
        .toBe("guardrail_max_wall_clock")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("max_wall_clock fails CLOSED on a guardrail-bearing record with no enabledAt anchor (hand-edited file): terminated, never run unbounded", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-wallclock-unanchored-"))
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      mkdirSync(registryDir, { recursive: true })
      writeFileSync(registryFile, JSON.stringify({
        schema: FULL_AUTO_REGISTRY_SCHEMA,
        records: [{
          threadRef: "thread-unanchored",
          enabled: true,
          continuationCount: 0,
          updatedAt: "2026-07-17T00:00:00.000Z",
          workspaceRef: GRANTED_WORKSPACE,
          guardrails: { maxWallClockMs: 999_999_999 },
        }],
      }), "utf8")
      const registry = openFullAutoRegistry(registryFile)
      let dispatchCount = 0
      expect(await reconcile(registry, {
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(registry.record("thread-unanchored")?.blockedReason).toBe("guardrail_max_wall_clock")
      expect(registry.record("thread-unanchored")?.disabledBy).toBe("guardrail")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("max_turns generalizes the cap: at the configured bound the run stops with guardrail_max_turns/guardrail attribution, and the legacy 20-cap semantics are untouched when the guardrail is absent", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-maxturns-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-turns", true, {
        workspaceRef: GRANTED_WORKSPACE,
        guardrails: { maxTurns: 2 },
      })
      // Two successful continuations consume the whole configured budget.
      expect(await reconcile(registry, { dispatch: async () => ({ ok: true }) })).toEqual(["thread-turns"])
      expect(await reconcile(registry, { dispatch: async () => ({ ok: true }) })).toEqual(["thread-turns"])
      const violations: Array<FullAutoGuardrailViolation> = []
      let capCallback = 0
      expect(await reconcile(registry, {
        dispatch: async () => ({ ok: true }),
        onCapReached: () => { capCallback += 1 },
        onGuardrailStopped: (_threadRef, violation) => { violations.push(violation) },
      })).toEqual([])
      const record = registry.record("thread-turns")
      expect(record?.enabled).toBe(false)
      expect(record?.blockedReason).toBe("guardrail_max_turns")
      expect(record?.disabledBy).toBe("guardrail")
      expect(record?.decisionHistory?.at(-1)).toMatchObject({
        decision: "stop_guardrail",
        reason: "guardrail_max_turns",
      })
      // The guardrail stop is NOT the legacy cap callback.
      expect(capCallback).toBe(0)
      expect(violations).toEqual([{
        guardrail: "max_turns",
        limit: 2,
        observed: 2,
        reason: "guardrail_max_turns",
      }])

      // Absent guardrail: existing cap semantics byte-for-byte (reason,
      // attribution, onCapReached), plus the additive stop_guardrail
      // decision fact.
      registry.set("thread-legacy-cap", true, { workspaceRef: GRANTED_WORKSPACE })
      for (let index = 0; index < FULL_AUTO_MAX_CONTINUATIONS; index += 1) {
        registry.incrementContinuation("thread-legacy-cap")
      }
      let legacyCapFor: string | null = null
      await reconcile(registry, {
        dispatch: async () => ({ ok: true }),
        onCapReached: threadRef => { legacyCapFor = threadRef },
        onGuardrailStopped: (_threadRef, violation) => { violations.push(violation) },
      })
      expect(legacyCapFor).toBe("thread-legacy-cap")
      expect(violations).toHaveLength(1)
      expect(registry.record("thread-legacy-cap")?.blockedReason).toBe("continuation_cap_reached")
      expect(registry.record("thread-legacy-cap")?.disabledBy).toBe("continuation_cap")
      expect(registry.record("thread-legacy-cap")?.decisionHistory?.at(-1)).toMatchObject({
        decision: "stop_guardrail",
        reason: "continuation_cap_reached",
      })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test(`max_per_turn_failures generalizes the failure budget: 2 configured failures disable (vs the built-in ${FULL_AUTO_MAX_CONSECUTIVE_FAILURES}) with dispatch_failure_limit attribution plus the typed guardrail violation`, async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-failures-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-fail", true, {
        workspaceRef: GRANTED_WORKSPACE,
        guardrails: { maxPerTurnFailures: 2 },
      })
      const violations: Array<FullAutoGuardrailViolation> = []
      const outcomes: Array<{ consecutiveFailures: number; disabled: boolean }> = []
      for (let attempt = 0; attempt < 2; attempt += 1) {
        const failedAt = registry.record("thread-fail")?.lastFailureAt
        await reconcile(registry, {
          now: () => new Date(
            (failedAt === undefined ? Date.now() : Date.parse(failedAt)) +
            fullAutoFailureBackoffMs(attempt) + 1,
          ),
          dispatch: async () => ({ ok: false, reason: "runtime_unavailable" }),
          onDispatchFailed: (_threadRef, failure) => {
            outcomes.push({ consecutiveFailures: failure.consecutiveFailures, disabled: failure.disabled })
          },
          onGuardrailStopped: (_threadRef, violation) => { violations.push(violation) },
        })
      }
      expect(outcomes).toEqual([
        { consecutiveFailures: 1, disabled: false },
        { consecutiveFailures: 2, disabled: true },
      ])
      const record = registry.record("thread-fail")
      expect(record?.enabled).toBe(false)
      // The failure class keeps its existing attribution...
      expect(record?.disabledBy).toBe("dispatch_failure_limit")
      expect(record?.blockedReason).toBe("runtime_unavailable")
      // ...and additionally reports as a typed guardrail stop.
      expect(violations).toEqual([{
        guardrail: "max_per_turn_failures",
        limit: 2,
        observed: 2,
        reason: "runtime_unavailable",
      }])
      expect(record?.decisionHistory?.at(-1)).toMatchObject({
        decision: "stop_guardrail",
        reason: "guardrail_max_per_turn_failures",
      })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("tokenBudgetRef is carried durably as an owner-visible ref and never fabricated into enforcement (honest gap: no local token source)", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-tokenref-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-token", true, {
        workspaceRef: GRANTED_WORKSPACE,
        guardrails: { tokenBudgetRef: "budget.acct-42" },
      })
      // Dispatch proceeds normally -- the ref alone never blocks.
      expect(await reconcile(registry, { dispatch: async () => ({ ok: true }) })).toEqual(["thread-token"])
      expect(openFullAutoRegistry(registryFile).record("thread-token")?.guardrails?.tokenBudgetRef)
        .toBe("budget.acct-42")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("FA-GD-01 confidence-gated continuation (pause_low_confidence)", () => {
  const failedTurns = (count: number, startIso: string): Array<{ disposition: string; updatedAt: string }> =>
    Array.from({ length: count }, (_, index) => ({
      disposition: "failed",
      updatedAt: new Date(Date.parse(startIso) + (index + 1) * 1000).toISOString(),
    }))

  test(`${FULL_AUTO_NO_PROGRESS_TURN_THRESHOLD} consecutive failed/interrupted settled turns pause the run durably with a typed reason instead of continuing`, async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-pause-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile, () => new Date("2026-07-17T00:00:00.000Z"))
      registry.set("thread-p", true, { workspaceRef: GRANTED_WORKSPACE })

      const pauses: Array<{ reason: string; consecutiveNoProgressTurns: number }> = []
      let dispatchCount = 0
      expect(await reconcile(registry, {
        turnEvidence: () => failedTurns(FULL_AUTO_NO_PROGRESS_TURN_THRESHOLD, "2026-07-17T00:00:00.000Z"),
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onPausedLowConfidence: (_threadRef, pause) => { pauses.push({ ...pause }) },
      })).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(pauses).toEqual([{
        reason: "no_progress:3_consecutive_unproductive_turns",
        consecutiveNoProgressTurns: 3,
      }])
      const record = registry.record("thread-p")
      // Paused, NOT disabled -- the owner's grant stands.
      expect(record?.enabled).toBe(true)
      expect(record?.pausedReason).toBe("no_progress:3_consecutive_unproductive_turns")
      expect(record?.pausedAt).toBeDefined()
      expect(record?.decisionHistory?.at(-1)).toMatchObject({ decision: "pause_low_confidence" })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a completed turn breaks the streak: 2 failed then completed dispatches normally; owner_interrupted never counts toward no-progress", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-progress-"))
    try {
      const registry = openFullAutoRegistry(
        path.join(root, "full-auto", "registry.json"),
        () => new Date("2026-07-17T00:00:00.000Z"),
      )
      registry.set("thread-ok", true, { workspaceRef: GRANTED_WORKSPACE })
      const evidence = [
        { disposition: "failed", updatedAt: "2026-07-17T00:01:00.000Z" },
        { disposition: "failed", updatedAt: "2026-07-17T00:02:00.000Z" },
        { disposition: "completed", updatedAt: "2026-07-17T00:03:00.000Z" },
        { disposition: "owner_interrupted", updatedAt: "2026-07-17T00:04:00.000Z" },
        { disposition: "owner_interrupted", updatedAt: "2026-07-17T00:05:00.000Z" },
        { disposition: "owner_interrupted", updatedAt: "2026-07-17T00:06:00.000Z" },
      ]
      expect(await reconcile(registry, {
        turnEvidence: () => evidence,
        dispatch: async () => ({ ok: true }),
      })).toEqual(["thread-ok"])
      expect(registry.record("thread-ok")?.pausedReason).toBeUndefined()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("the pause is durable across restart: Runtime B never dispatches a paused record; resumeFullAuto is the explicit path back, and pre-resume evidence can never immediately re-pause", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-pause-restart-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      // Runtime A: pause on no-progress, then quit.
      const registryA = openFullAutoRegistry(registryFile, () => new Date("2026-07-17T00:00:00.000Z"))
      registryA.set("thread-r", true, { workspaceRef: GRANTED_WORKSPACE })
      const staleEvidence = failedTurns(FULL_AUTO_NO_PROGRESS_TURN_THRESHOLD, "2026-07-17T00:00:00.000Z")
      await reconcile(registryA, {
        turnEvidence: () => staleEvidence,
        dispatch: async () => ({ ok: true }),
      })
      expect(registryA.record("thread-r")?.pausedReason).toBeDefined()

      // Runtime B: fresh process, same durable file -- still paused, and the
      // startup pass never dispatches it (even with clearStaleLeases).
      const registryB = openFullAutoRegistry(registryFile, () => new Date("2026-07-17T06:00:00.000Z"))
      expect(registryB.record("thread-r")?.pausedReason)
        .toBe("no_progress:3_consecutive_unproductive_turns")
      let dispatchCount = 0
      expect(await reconcile(registryB, {
        clearStaleLeases: true,
        turnEvidence: () => staleEvidence,
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })).toEqual([])
      expect(dispatchCount).toBe(0)

      // resumeFullAuto: explicit, attributed, schedules reconciliation.
      let scheduled = 0
      const resumed = resumeFullAuto({
        registry: registryB,
        threadRef: "thread-r",
        actor: "control_api",
        scheduleReconciliation: () => { scheduled += 1 },
      })
      expect(scheduled).toBe(1)
      expect(resumed?.pausedReason).toBeUndefined()
      expect(resumed?.resumedBy).toBe("control_api")
      expect(resumed?.lastResumedAt).toBeDefined()
      expect(resumed?.enabled).toBe(true)
      expect(resumed?.decisionHistory?.at(-1)).toMatchObject({
        decision: "continue",
        reason: "resumed_by_control_api",
      })

      // The SAME stale evidence (settled before lastResumedAt) can no longer
      // trip the detector -- the resumed loop dispatches.
      expect(await reconcile(registryB, {
        turnEvidence: () => staleEvidence,
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
      })).toEqual(["thread-r"])
      expect(dispatchCount).toBe(1)

      // Resuming a non-paused (or missing) record is a null no-op that never
      // schedules anything.
      expect(resumeFullAuto({
        registry: registryB,
        threadRef: "thread-r",
        actor: "cli",
        scheduleReconciliation: () => { scheduled += 1 },
      })).toBe(null)
      expect(resumeFullAuto({
        registry: registryB,
        threadRef: "thread-missing",
        actor: "cli",
        scheduleReconciliation: () => { scheduled += 1 },
      })).toBe(null)
      expect(scheduled).toBe(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("detectFullAutoNoProgress is deterministic: anchor filtering, trailing-run counting, and the exact disposition set", () => {
    const failed = (at: string) => ({ disposition: "failed", updatedAt: at })
    expect(detectFullAutoNoProgress({
      evidence: [failed("2026-01-01T00:01:00Z"), failed("2026-01-01T00:02:00Z"), failed("2026-01-01T00:03:00Z")],
      anchorAt: null,
    })).toEqual({ noProgress: true, consecutive: 3 })
    // Interrupted-by-restart counts; in-flight (null disposition) rows do not
    // settle anything and are ignored.
    expect(detectFullAutoNoProgress({
      evidence: [
        { disposition: "interrupted_by_restart", updatedAt: "2026-01-01T00:01:00Z" },
        failed("2026-01-01T00:02:00Z"),
        { disposition: null, updatedAt: "2026-01-01T00:04:00Z" },
        failed("2026-01-01T00:03:00Z"),
      ],
      anchorAt: null,
    })).toEqual({ noProgress: true, consecutive: 3 })
    // Anchor excludes older evidence.
    expect(detectFullAutoNoProgress({
      evidence: [failed("2026-01-01T00:01:00Z"), failed("2026-01-01T00:02:00Z"), failed("2026-01-01T00:03:00Z")],
      anchorAt: "2026-01-01T00:02:30.000Z",
    })).toEqual({ noProgress: false, consecutive: 1 })
    // A completed turn resets the trailing run.
    expect(detectFullAutoNoProgress({
      evidence: [
        failed("2026-01-01T00:01:00Z"),
        failed("2026-01-01T00:02:00Z"),
        { disposition: "completed", updatedAt: "2026-01-01T00:03:00Z" },
        failed("2026-01-01T00:04:00Z"),
      ],
      anchorAt: null,
    })).toEqual({ noProgress: false, consecutive: 1 })
  })
})

describe("FA-GD-01 non-overridable core guardrails are immune to config and env", () => {
  test("the non-overridable set is a frozen, documented constant", () => {
    expect(FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS).toEqual([
      "workspace_binding",
      "own_capacity_only",
      "no_rate_limit_reset_triggering",
    ])
    expect(Object.isFrozen(FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS)).toBe(true)
  })

  test("no env var and no guardrails-object key can relax workspace binding, own-capacity admission, or the rate-limit backoff window", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-immunity-"))
    const adversarialEnv = {
      OPENAGENTS_DESKTOP_FULL_AUTO_DISABLE_WORKSPACE_GUARD: "1",
      OPENAGENTS_DESKTOP_FULL_AUTO_ALLOW_FOREIGN_LANES: "1",
      OPENAGENTS_DESKTOP_FULL_AUTO_SKIP_BACKOFF: "1",
      OPENAGENTS_DESKTOP_FULL_AUTO_RETRY_ON_RATE_LIMIT_RESET: "1",
      FULL_AUTO_GUARDRAILS: JSON.stringify({ workspaceBinding: false }),
    } as const
    const previous = new Map<string, string | undefined>()
    for (const [key, value] of Object.entries(adversarialEnv)) {
      previous.set(key, process.env[key])
      process.env[key] = value
    }
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      mkdirSync(registryDir, { recursive: true })
      // A hand-edited durable file that TRIES to write the non-overridable
      // set down as config keys on the guardrails object.
      writeFileSync(registryFile, JSON.stringify({
        schema: FULL_AUTO_REGISTRY_SCHEMA,
        records: [{
          threadRef: "thread-adversarial",
          enabled: true,
          continuationCount: 0,
          updatedAt: "2026-07-17T00:00:00.000Z",
          enabledAt: "2026-07-17T00:00:00.000Z",
          workspaceRef: "/repo/granted-elsewhere",
          guardrails: {
            maxTurns: 500,
            workspaceBinding: false,
            ownCapacityOnly: false,
            noRateLimitResetTriggering: false,
            skipBackoff: true,
          },
        }],
      }), "utf8")

      const registry = openFullAutoRegistry(registryFile)
      // Unknown guardrail keys never survive decode -- there is nothing for
      // enforcement to even read.
      const decoded = registry.record("thread-adversarial")
      expect(decoded?.guardrails).toEqual({ maxTurns: 500 })

      // (1) workspace_binding: the mismatch still fails closed.
      let dispatchCount = 0
      const blocks: Array<string> = []
      expect(await reconcile(registry, {
        resolveWorkspaceRef: () => "/repo/actual",
        dispatch: async () => { dispatchCount += 1; return { ok: true } },
        onWorkspaceBlocked: (_threadRef, block) => { blocks.push(block.reason) },
      })).toEqual([])
      expect(dispatchCount).toBe(0)
      expect(blocks).toEqual(["workspace_mismatch"])
      expect(registry.record("thread-adversarial")?.disabledBy).toBe("workspace_guard")

      // (2) own_capacity_only: routing admission still refuses unknown and
      // unadmitted lanes fail-closed regardless of env.
      expect(validateFullAutoRoutingPolicy(
        [{ lane: "some-foreign-fleet" }],
        () => null,
      )).toEqual({ ok: false, reason: "lane_unknown", lane: "some-foreign-fleet" })
      expect(validateFullAutoRoutingPolicy(
        [{ lane: "codex-local" }],
        () => ({ admitted: false, fullAuto: true }),
      )).toEqual({ ok: false, reason: "lane_not_admitted", lane: "codex-local" })

      // (3) no_rate_limit_reset_triggering: after a rate-limited failure the
      // full bounded backoff window still holds -- no env flag shrinks it.
      const registry2 = openFullAutoRegistry(path.join(root, "full-auto", "registry2.json"))
      registry2.set("thread-rl", true, { workspaceRef: GRANTED_WORKSPACE })
      await reconcile(registry2, {
        dispatch: async () => ({ ok: false, reason: "rate limited", failureClass: "rate_limited" }),
      })
      const failedAt = Date.parse(registry2.record("thread-rl")!.lastFailureAt!)
      let retried = 0
      await reconcile(registry2, {
        now: () => new Date(failedAt + fullAutoFailureBackoffMs(1) - 1),
        dispatch: async () => { retried += 1; return { ok: true } },
      })
      expect(retried).toBe(0)
    } finally {
      for (const [key, value] of previous) {
        if (value === undefined) delete process.env[key]
        else process.env[key] = value
      }
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("FA-GD-01 guardrail stops flow into the run report (#8988 derivation)", () => {
  test("a guardrail-terminated thread record surfaces in threadFailureHistory with disabledBy guardrail and the typed blockedReason", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa-gd-report-"))
    try {
      const now = () => new Date("2026-07-17T01:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), now)
      const reportStore = openFullAutoRunReportStore(path.join(root, "reports.json"), now)
      const registry = openFullAutoRegistry(path.join(root, "registry.json"), now)

      const started = runRegistry.startNew({
        title: "Guardrail stop pickup",
        objective: "prove the guardrail reason reaches the report",
        doneCondition: "threadFailureHistory carries disabledBy guardrail",
        objectiveSource: "control_caller",
        workspaceRef: GRANTED_WORKSPACE,
        threadRef: "thread-report",
        actor: "control_api",
        reason: "test bootstrap",
      })
      expect(started.ok).toBe(true)
      if (!started.ok) return

      registry.set("thread-report", true, {
        workspaceRef: GRANTED_WORKSPACE,
        guardrails: { maxTurns: 1 },
      })
      registry.recordDecision("thread-report", { decision: "stop_guardrail", reason: "guardrail_max_turns" })
      registry.set("thread-report", false, {
        blockedReason: "guardrail_max_turns",
        disabledBy: "guardrail",
      })

      const report = reportStore.sync({
        run: runRegistry.get(started.run.runRef)!,
        turns: [],
        handoffs: [],
        threadRecord: registry.record("thread-report"),
      })
      expect(report.threadFailureHistory).toMatchObject({
        blockedReason: "guardrail_max_turns",
        disabledBy: "guardrail",
      })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
