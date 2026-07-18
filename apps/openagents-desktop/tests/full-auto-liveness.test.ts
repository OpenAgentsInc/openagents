// Oracle for FA-RUN-03 (#8971): "Detect, explain, recover, and notify on
// stalled Full Auto runs" -- ProductSpec rev 10 FA-AC-42/47/48 and the
// `openagents_desktop.full_auto_play_pause_stop_lifecycle.v1` behavior
// contract's Retrying/Stalled states.
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { fullAutoFailureBackoffMs, reconcileFullAutoThreads } from "../src/full-auto-reconcile.ts"
import { openFullAutoRegistry, type FullAutoRecord } from "../src/full-auto-registry.ts"
import {
  FULL_AUTO_LIVENESS_APP_OFFLINE_GAP_MS,
  FULL_AUTO_LIVENESS_DISPATCH_SLO_MS,
  FULL_AUTO_LIVENESS_RETRY_GRACE_MS,
  classifyFullAutoDispatchFailureReason,
  classifyFullAutoRunLiveness,
  decideFullAutoLivenessNotification,
  recoveryActionForCause,
  retryFullAutoRunNow,
  settleFullAutoRunLiveness,
} from "../src/full-auto-liveness.ts"
import {
  openFullAutoRunRegistry,
  type FullAutoRun,
  type FullAutoRunRegistry,
  type FullAutoRunThreadSnapshot,
} from "../src/full-auto-run-registry.ts"

const withTempDir = <A>(prefix: string, fn: (root: string) => A): A => {
  const root = mkdtempSync(path.join(tmpdir(), prefix))
  try {
    return fn(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const draftInput = (overrides?: Partial<Parameters<FullAutoRunRegistry["createDraft"]>[0]>) => ({
  title: "Fix the flaky test",
  objective: "Make tests/flaky.test.ts stop flaking under repeated runs -- SECRET internal detail.",
  doneCondition: "The test passes 20 consecutive local runs and CI is green.",
  objectiveSource: "user" as const,
  workspaceRef: "/granted/workspace",
  threadRef: "thread-liveness",
  ...overrides,
})

const enabledRecord = (overrides?: Partial<FullAutoRecord>): FullAutoRecord => ({
  threadRef: "thread-liveness",
  enabled: true,
  continuationCount: 1,
  updatedAt: "2026-07-17T00:00:00.000Z",
  ...overrides,
})

/** A mutable clock so tests can advance "now" across backoff/SLO windows
 * without depending on real wall-clock time. */
const makeClock = (startIso: string) => {
  let ms = Date.parse(startIso)
  return {
    now: () => new Date(ms),
    advance: (deltaMs: number) => {
      ms += deltaMs
    },
    iso: () => new Date(ms).toISOString(),
  }
}

describe("classifyFullAutoDispatchFailureReason (bounded reason -> stall-cause lookup)", () => {
  test("maps the exact FA-RUN-02 (#8970) incident string to host_thread_missing", () => {
    expect(classifyFullAutoDispatchFailureReason("That conversation no longer exists.")).toBe(
      "host_thread_missing",
    )
  })
  test("maps the FA-H3 in-flight guard to stale_lease", () => {
    expect(classifyFullAutoDispatchFailureReason("turn_already_in_flight")).toBe("stale_lease")
  })
  test("maps the lane-eligibility prefix to auth_admission_failure", () => {
    expect(classifyFullAutoDispatchFailureReason("full_auto_lane_not_eligible:codex-local")).toBe(
      "auth_admission_failure",
    )
  })
  test("maps workspace block reasons to workspace_mismatch", () => {
    expect(classifyFullAutoDispatchFailureReason("workspace_mismatch")).toBe("workspace_mismatch")
    expect(classifyFullAutoDispatchFailureReason("workspace_unbound")).toBe("workspace_mismatch")
  })
  test("retains typed provider terminal-failure causes", () => {
    expect(classifyFullAutoDispatchFailureReason("account_exhausted")).toBe("account_exhausted")
    expect(classifyFullAutoDispatchFailureReason("rate_limited")).toBe("rate_limited")
    expect(classifyFullAutoDispatchFailureReason("provider_error")).toBe("provider_error")
  })
  test("an unrecognized or absent reason is honestly unknown_error, never a guess", () => {
    expect(classifyFullAutoDispatchFailureReason("some brand new provider error string")).toBe("unknown_error")
    expect(classifyFullAutoDispatchFailureReason(undefined)).toBe("unknown_error")
    expect(classifyFullAutoDispatchFailureReason(null)).toBe("unknown_error")
  })
})

describe("recoveryActionForCause (AC-48 fail-closed vs retryable)", () => {
  test("host_thread_missing, workspace_mismatch, and auth_admission_failure fail closed to stop_only", () => {
    expect(recoveryActionForCause("host_thread_missing")).toBe("stop_only")
    expect(recoveryActionForCause("workspace_mismatch")).toBe("stop_only")
    expect(recoveryActionForCause("auth_admission_failure")).toBe("stop_only")
  })
  test("provider failures and other recoverable causes offer retry_now", () => {
    expect(recoveryActionForCause("provider_session_missing")).toBe("retry_now")
    expect(recoveryActionForCause("account_exhausted")).toBe("retry_now")
    expect(recoveryActionForCause("rate_limited")).toBe("retry_now")
    expect(recoveryActionForCause("provider_error")).toBe("retry_now")
    expect(recoveryActionForCause("stale_lease")).toBe("retry_now")
    expect(recoveryActionForCause("app_offline")).toBe("retry_now")
    expect(recoveryActionForCause("dispatch_overdue")).toBe("retry_now")
    expect(recoveryActionForCause("unknown_error")).toBe("retry_now")
  })
  test("no cause (healthy) is no recovery action", () => {
    expect(recoveryActionForCause(null)).toBe("none")
  })
})

describe("classifyFullAutoRunLiveness (FA-AC-47: liveness distinct from a healthy long turn)", () => {
  const startRun = (root: string, clock: ReturnType<typeof makeClock>, overrides?: Partial<Parameters<FullAutoRunRegistry["startNew"]>[0]>) => {
    const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
    const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start", ...overrides })
    if (!started.ok) throw new Error("expected start to succeed")
    return { runRegistry, run: started.run }
  }

  test("a run mid-turn for a long time (20+ minutes) is still Running, never Stalled -- liveness is not turn duration", () => {
    withTempDir("oa-liveness-long-turn-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { run } = startRun(root, clock)
      clock.advance(25 * 60_000)
      const projection = classifyFullAutoRunLiveness({
        run,
        snapshot: { threadRecord: enabledRecord(), turnRunning: true },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("running")
      expect(projection.cause).toBeNull()
    })
  })

  test("within the SLO window with no turn in flight and no failure is still Running", () => {
    withTempDir("oa-liveness-within-slo-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { run } = startRun(root, clock)
      clock.advance(FULL_AUTO_LIVENESS_DISPATCH_SLO_MS - 1_000)
      const projection = classifyFullAutoRunLiveness({
        run,
        snapshot: { threadRecord: enabledRecord(), turnRunning: false },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("running")
    })
  })

  test(
    "past the SLO window with no failure and no turn is Stalled with cause dispatch_overdue -- the exact 2026-07-17 incident shape (enabled, no recorded failure, nothing dispatched)",
    () => {
      withTempDir("oa-liveness-overdue-", root => {
        const clock = makeClock("2026-07-17T00:00:00.000Z")
        const { run } = startRun(root, clock)
        clock.advance(FULL_AUTO_LIVENESS_DISPATCH_SLO_MS + 1_000)
        const projection = classifyFullAutoRunLiveness({
          run,
          snapshot: { threadRecord: enabledRecord(), turnRunning: false },
          now: clock.now(),
        })
        expect(projection.projectedState).toBe("stalled")
        expect(projection.cause).toBe("dispatch_overdue")
        expect(projection.recoveryAction).toBe("retry_now")
        expect(projection.nextRetryAt).toBeNull()
      })
    },
  )

  test("a very stale lastLivenessCheckAt reclassifies the same overdue gap as app_offline instead of dispatch_overdue", () => {
    withTempDir("oa-liveness-app-offline-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { runRegistry, run } = startRun(root, clock)
      const touched = runRegistry.touchLiveness(run.runRef, clock.iso())
      expect(touched).not.toBeNull()
      clock.advance(FULL_AUTO_LIVENESS_APP_OFFLINE_GAP_MS + 60_000)
      const projection = classifyFullAutoRunLiveness({
        run: touched!,
        snapshot: { threadRecord: enabledRecord(), turnRunning: false },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("stalled")
      expect(projection.cause).toBe("app_offline")
    })
  })

  test("a recorded failure inside its backoff window is Retrying with an exact next-retry ETA and typed cause, never generic Running or a terminal failure banner", () => {
    withTempDir("oa-liveness-retrying-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { run } = startRun(root, clock)
      const lastFailureAt = clock.iso()
      const projection = classifyFullAutoRunLiveness({
        run,
        snapshot: {
          threadRecord: enabledRecord({
            consecutiveFailures: 2,
            lastFailureAt,
            blockedReason: "That conversation no longer exists.",
          }),
          turnRunning: false,
        },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("retrying")
      expect(projection.cause).toBe("host_thread_missing")
      expect(projection.nextRetryAt).toBe(new Date(Date.parse(lastFailureAt) + fullAutoFailureBackoffMs(2)).toISOString())
      expect(projection.recoveryAction).toBe("none")
    })
  })

  test("a scheduled retry that never happened (backoff + grace elapsed, no fresh attempt) becomes Stalled with the same cause and a retry_now affordance", () => {
    withTempDir("oa-liveness-retry-overdue-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { run } = startRun(root, clock)
      const lastFailureAt = clock.iso()
      const record = enabledRecord({
        consecutiveFailures: 1,
        lastFailureAt,
        blockedReason: "That conversation no longer exists.",
      })
      clock.advance(fullAutoFailureBackoffMs(1) + FULL_AUTO_LIVENESS_RETRY_GRACE_MS + 1_000)
      const projection = classifyFullAutoRunLiveness({
        run,
        snapshot: { threadRecord: record, turnRunning: false },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("stalled")
      expect(projection.cause).toBe("host_thread_missing")
      expect(projection.recoveryAction).toBe("stop_only")
    })
  })

  test("a bound thread whose registry record is missing entirely is Stalled with host_thread_missing (fail closed, never a silent reattachment)", () => {
    withTempDir("oa-liveness-missing-thread-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { run } = startRun(root, clock)
      const projection = classifyFullAutoRunLiveness({
        run,
        snapshot: { threadRecord: null, turnRunning: false },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("stalled")
      expect(projection.cause).toBe("host_thread_missing")
      expect(projection.recoveryAction).toBe("stop_only")
    })
  })

  test("Draft, Paused, Pausing, and every terminal state pass through unchanged -- this classifier never overrides FA-RUN-01's own edges", () => {
    withTempDir("oa-liveness-passthrough-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { runRegistry, run } = startRun(root, clock)
      const paused = runRegistry.transition(run.runRef, { to: "paused", actor: "owner_ui", reason: "pause" })
      expect(paused.ok).toBe(true)
      if (!paused.ok) return
      clock.advance(FULL_AUTO_LIVENESS_DISPATCH_SLO_MS * 10)
      const projection = classifyFullAutoRunLiveness({
        run: paused.run,
        snapshot: { threadRecord: null, turnRunning: false },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("paused")
      expect(projection.cause).toBeNull()
      expect(projection.recoveryAction).toBe("none")
    })
  })

  test("a Stalled run is sticky: it never automatically re-classifies to Running even if the snapshot looks healthy again -- only an explicit retry-now/Stop can move it", () => {
    withTempDir("oa-liveness-sticky-stalled-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { runRegistry, run } = startRun(root, clock)
      const stalled = runRegistry.transition(run.runRef, {
        to: "stalled",
        actor: "liveness_monitor",
        reason: "test setup",
      })
      expect(stalled.ok).toBe(true)
      if (!stalled.ok) return
      // Snapshot now looks perfectly healthy (an unrelated trigger dispatched
      // successfully) -- the classifier must still say Stalled.
      const projection = classifyFullAutoRunLiveness({
        run: stalled.run,
        snapshot: { threadRecord: enabledRecord({ consecutiveFailures: 0 }), turnRunning: false },
        now: clock.now(),
      })
      expect(projection.projectedState).toBe("stalled")
    })
  })
})

describe("settleFullAutoRunLiveness (the single mutating entry point)", () => {
  test("Running with a stale run transitions to Stalled through the real registry, stamps lastLivenessCheckAt, and attributes the transition to liveness_monitor", () => {
    withTempDir("oa-liveness-settle-stall-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      clock.advance(FULL_AUTO_LIVENESS_DISPATCH_SLO_MS + 1_000)
      const snapshot: FullAutoRunThreadSnapshot = { threadRecord: enabledRecord(), turnRunning: false }
      const { run: settled, projection } = settleFullAutoRunLiveness(runRegistry, started.run, snapshot, clock.now)
      expect(settled.state).toBe("stalled")
      expect(projection.projectedState).toBe("stalled")
      expect(projection.cause).toBe("dispatch_overdue")
      expect(settled.transitions.at(-1)?.actor).toBe("liveness_monitor")
      expect(settled.lastLivenessCheckAt).toBe(clock.iso())
      // The persisted record agrees with a fresh read.
      expect(runRegistry.get(started.run.runRef)?.state).toBe("stalled")
    })
  })

  test("Running well within the SLO window with a healthy snapshot stays Running and does not append a transition record", () => {
    withTempDir("oa-liveness-settle-healthy-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const transitionsBefore = started.run.transitions.length
      const { run: settled, projection } = settleFullAutoRunLiveness(
        runRegistry,
        started.run,
        { threadRecord: enabledRecord(), turnRunning: false },
        clock.now,
      )
      expect(settled.state).toBe("running")
      expect(projection.projectedState).toBe("running")
      expect(settled.transitions.length).toBe(transitionsBefore)
      expect(settled.lastLivenessCheckAt).toBe(clock.iso())
    })
  })

  test("a failure in backoff settles Running -> Retrying, and once backoff+grace elapses with no fresh attempt settles Retrying -> Stalled", () => {
    withTempDir("oa-liveness-settle-retry-then-stall-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const lastFailureAt = clock.iso()
      const record = enabledRecord({ consecutiveFailures: 1, lastFailureAt, blockedReason: "That conversation no longer exists." })

      const first = settleFullAutoRunLiveness(runRegistry, started.run, { threadRecord: record, turnRunning: false }, clock.now)
      expect(first.run.state).toBe("retrying")
      expect(first.projection.nextRetryAt).not.toBeNull()

      clock.advance(fullAutoFailureBackoffMs(1) + FULL_AUTO_LIVENESS_RETRY_GRACE_MS + 1_000)
      const second = settleFullAutoRunLiveness(runRegistry, first.run, { threadRecord: record, turnRunning: false }, clock.now)
      expect(second.run.state).toBe("stalled")
      expect(second.projection.cause).toBe("host_thread_missing")
    })
  })

  test("delegates missing-thread fail-closed sync to the existing FA-RUN-01 path first (settleFullAutoRunFromThreadState), so the two settle functions never disagree", () => {
    withTempDir("oa-liveness-settle-missing-thread-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const { run: settled } = settleFullAutoRunLiveness(runRegistry, started.run, { threadRecord: null, turnRunning: false }, clock.now)
      expect(settled.state).toBe("stalled")
      expect(settled.transitions.at(-1)?.actor).toBe("thread_state_sync")
    })
  })

  test("a terminal run is left completely untouched (no lastLivenessCheckAt stamp, no transition attempt)", () => {
    withTempDir("oa-liveness-settle-terminal-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const stopped = runRegistry.transition(started.run.runRef, { to: "stopped", actor: "owner_ui", reason: "done" })
      expect(stopped.ok).toBe(true)
      if (!stopped.ok) return
      const { run: settled } = settleFullAutoRunLiveness(runRegistry, stopped.run, { threadRecord: null, turnRunning: false }, clock.now)
      expect(settled).toEqual(stopped.run)
    })
  })
})

describe("retryFullAutoRunNow (AC-48 owner-actionable recovery affordance)", () => {
  const stalledRun = (
    root: string,
    clock: ReturnType<typeof makeClock>,
  ): Readonly<{ runRegistry: FullAutoRunRegistry; run: FullAutoRun }> => {
    const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
    const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
    if (!started.ok) throw new Error("expected start")
    const stalled = runRegistry.transition(started.run.runRef, { to: "stalled", actor: "liveness_monitor", reason: "test" })
    if (!stalled.ok) throw new Error("expected stall")
    return { runRegistry, run: stalled.run }
  }

  test("refuses when the run is not Stalled", () => {
    withTempDir("oa-retry-now-not-stalled-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      const started = runRegistry.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      if (!started.ok) throw new Error("expected start")
      const result = retryFullAutoRunNow(
        runRegistry,
        started.run,
        { threadRecord: enabledRecord(), turnRunning: false },
        { actor: "control_api" },
        clock.now,
      )
      expect(result.ok).toBe(false)
      if (result.ok) return
      expect(result.reason).toBe("not_stalled")
    })
  })

  test("refuses a nonrecoverable cause (host_thread_missing) with not_recoverable, presenting Stop as the only safe action", () => {
    withTempDir("oa-retry-now-nonrecoverable-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { runRegistry, run } = stalledRun(root, clock)
      const result = retryFullAutoRunNow(runRegistry, run, { threadRecord: null, turnRunning: false }, { actor: "control_api" }, clock.now)
      expect(result.ok).toBe(false)
      if (result.ok || result.reason !== "not_recoverable") throw new Error("expected not_recoverable")
      expect(result.reason).toBe("not_recoverable")
      expect(result.cause).toBe("host_thread_missing")
      expect(runRegistry.get(run.runRef)?.state).toBe("stalled")
    })
  })

  test("a recoverable cause transitions Stalled -> Retrying, attributed to the requesting actor", () => {
    withTempDir("oa-retry-now-recoverable-", root => {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const { runRegistry, run } = stalledRun(root, clock)
      const record = enabledRecord({ consecutiveFailures: 0 })
      const result = retryFullAutoRunNow(runRegistry, run, { threadRecord: record, turnRunning: false }, { actor: "control_api" }, clock.now)
      expect(result.ok).toBe(true)
      if (!result.ok) return
      expect(result.run.state).toBe("retrying")
      expect(result.run.transitions.at(-1)?.actor).toBe("control_api")
    })
  })
})

describe("decideFullAutoLivenessNotification (attention signals: dedup, permission, redaction)", () => {
  test("Running/Paused/other non-notifiable states never notify", () => {
    for (const state of ["running", "paused", "pausing", "draft", "completed", "failed", "stopped", "cap_reached"] as const) {
      expect(decideFullAutoLivenessNotification({
        runRef: "run.x",
        runTitle: "Fix the flaky test",
        projectedState: state,
        cause: null,
        previousDedupKey: null,
        permissionGranted: true,
      })).toBeNull()
    }
  })

  test("a fresh Stalled classification notifies (when permitted) with a title/body naming only the run title and state -- never objective/doneCondition text", () => {
    const decision = decideFullAutoLivenessNotification({
      runRef: "run.x",
      runTitle: "Fix the flaky test",
      projectedState: "stalled",
      cause: "dispatch_overdue",
      previousDedupKey: null,
      permissionGranted: true,
    })
    expect(decision).not.toBeNull()
    expect(decision?.notify).toBe(true)
    expect(decision?.title).toContain("stalled")
    expect(decision?.body).toContain("Fix the flaky test")
    expect(decision?.body).not.toContain("SECRET")
    expect(decision?.body).not.toContain("flaky.test.ts stop flaking")
  })

  test("dedup: the identical (run, state, cause) triple does not notify again", () => {
    const first = decideFullAutoLivenessNotification({
      runRef: "run.x",
      runTitle: "Fix the flaky test",
      projectedState: "stalled",
      cause: "dispatch_overdue",
      previousDedupKey: null,
      permissionGranted: true,
    })
    expect(first).not.toBeNull()
    const second = decideFullAutoLivenessNotification({
      runRef: "run.x",
      runTitle: "Fix the flaky test",
      projectedState: "stalled",
      cause: "dispatch_overdue",
      previousDedupKey: first!.dedupKey,
      permissionGranted: true,
    })
    expect(second).toBeNull()
    // A genuinely different cause on the same run/state DOES re-notify.
    const third = decideFullAutoLivenessNotification({
      runRef: "run.x",
      runTitle: "Fix the flaky test",
      projectedState: "stalled",
      cause: "host_thread_missing",
      previousDedupKey: first!.dedupKey,
      permissionGranted: true,
    })
    expect(third).not.toBeNull()
  })

  test("permission denial: a genuinely new notifiable state returns notify:false (never calls the OS layer) but still yields a dedup key so the caller does not re-decide every tick", () => {
    const decision = decideFullAutoLivenessNotification({
      runRef: "run.x",
      runTitle: "Fix the flaky test",
      projectedState: "retrying",
      cause: "provider_session_missing",
      previousDedupKey: null,
      permissionGranted: false,
    })
    expect(decision).not.toBeNull()
    expect(decision?.notify).toBe(false)
    expect(decision?.dedupKey).toBeTruthy()
  })
})

/**
 * AC: "The overnight incident fixture produces an actionable stall
 * classification instead of only `That conversation no longer exists.`"
 * Composes the REAL `reconcileFullAutoThreads` dispatch-failure path
 * (FA-H5) with the durable `FullAutoRun` registry and the liveness
 * classifier -- the exact fail-closed string production dispatchTurn
 * returns (pinned by the FA-RUN-02 #8970 regression in
 * tests/full-auto-restart.e2e.test.ts) all the way through to a bounded,
 * owner-actionable `host_thread_missing` classification with a real retry
 * ETA and then, once the schedule exhausts its window, a fail-closed Stalled
 * disposition with a `stop_only` recovery affordance.
 */
describe("End-to-end: the FA-RUN-02 (#8970) incident's exact failure string produces an actionable stall (not just the raw string)", () => {
  test("repeated 'That conversation no longer exists.' dispatch failures classify as Retrying-with-ETA, then Stalled with host_thread_missing and a stop_only affordance once the schedule is exhausted", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-liveness-incident-"))
    try {
      const clock = makeClock("2026-07-17T00:00:00.000Z")
      const threadRef = "thread-incident"
      const registry = openFullAutoRegistry(path.join(root, "registry.json"), clock.now)
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), clock.now)
      registry.set(threadRef, true, { workspaceRef: "/granted/workspace", profile: { lane: "codex-local" } })
      const started = runRegistry.startNew({
        ...draftInput({ threadRef }),
        actor: "control_api",
        reason: "start",
      })
      expect(started.ok).toBe(true)
      if (!started.ok) return

      const dispatch = async () => ({ ok: false, reason: "That conversation no longer exists." }) as const

      // First dispatch attempt: the exact incident failure.
      const dispatched = await reconcileFullAutoThreads({
        registry,
        nonterminalThreadRefs: () => new Set(),
        resolveWorkspaceRef: () => "/granted/workspace",
        journalHasNonterminalTurn: () => false,
        dispatch,
        now: clock.now,
      })
      expect(dispatched).toEqual([])
      const record = registry.record(threadRef)
      expect(record?.blockedReason).toBe("That conversation no longer exists.")
      expect(record?.consecutiveFailures).toBe(1)

      const afterFirstFailure = settleFullAutoRunLiveness(
        runRegistry,
        started.run,
        { threadRecord: registry.record(threadRef), turnRunning: false },
        clock.now,
      )
      // Actionable: a real typed cause and a real retry ETA -- not just the
      // raw provider string, and not a generic failed-turn banner.
      expect(afterFirstFailure.run.state).toBe("retrying")
      expect(afterFirstFailure.projection.cause).toBe("host_thread_missing")
      expect(afterFirstFailure.projection.nextRetryAt).not.toBeNull()

      // Advance past the FIRST backoff window plus grace with no further
      // dispatch attempt (nothing else triggers reconciliation) -- exactly
      // the "scheduled retry silently never happened" shape.
      clock.advance(fullAutoFailureBackoffMs(1) + FULL_AUTO_LIVENESS_RETRY_GRACE_MS + 1_000)
      const overdue = settleFullAutoRunLiveness(
        runRegistry,
        afterFirstFailure.run,
        { threadRecord: registry.record(threadRef), turnRunning: false },
        clock.now,
      )
      expect(overdue.run.state).toBe("stalled")
      expect(overdue.projection.cause).toBe("host_thread_missing")
      expect(overdue.projection.recoveryAction).toBe("stop_only")
      // The last transition names WHY, distinctly from the low-level
      // "That conversation no longer exists." string.
      expect(overdue.run.transitions.at(-1)?.reason).toContain("host_thread_missing")
      expect(overdue.run.transitions.at(-1)?.actor).toBe("liveness_monitor")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
