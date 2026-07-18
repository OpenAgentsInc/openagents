import type { FullAutoRunMobileProjection, FullAutoRunProjectionResult } from "./full-auto-run-projection"
import type { FullAutoRunProjectionSource } from "./full-auto-run-projection-source"

/**
 * Public-safe fixture data and a fixture `FullAutoRunProjectionSource`,
 * matching the real landed `full_auto_run.mobile_projection.v1` run shape
 * (openagents #8981/#8982), for unit tests and honestly-labeled manual
 * verification without needing a live Desktop Full Auto run.
 */
export const fullAutoRunFixtureRunning: FullAutoRunMobileProjection = {
  runRef: "run.full-auto.fixture-0001",
  threadRef: "thread.full-auto.fixture.0001",
  objective: "Ship the mobile Full Auto live thread fast-follow end to end.",
  doneCondition: "Mobile home screen shows the live run thread and state header.",
  lifecycleState: "running",
  workspaceLabel: "openagents (fixture)",
  startedAt: "2026-07-17T00:00:00.000Z",
  updatedAt: "2026-07-17T00:00:00.000Z",
  lastTransition: { actor: "owner_ui", at: "2026-07-17T00:00:00.000Z" },
  laneRef: "codex-local",
  accountRef: null,
  turnCap: 20,
  successfulAttempts: 6,
  failedAttempts: 0,
  rotationCount: 0,
  receiptSummary: null,
}

export const fullAutoRunFixturePaused: FullAutoRunMobileProjection = {
  ...fullAutoRunFixtureRunning,
  lifecycleState: "paused",
  updatedAt: "2026-07-17T00:05:00.000Z",
  lastTransition: { actor: "owner_ui", at: "2026-07-17T00:05:00.000Z" },
}

export const fullAutoRunFixtureStalled: FullAutoRunMobileProjection = {
  ...fullAutoRunFixtureRunning,
  lifecycleState: "stalled",
  updatedAt: "2026-07-17T00:10:00.000Z",
  lastTransition: { actor: "liveness_monitor", at: "2026-07-17T00:10:00.000Z" },
}

/** MOB-FA-02 (#8994): a terminal fixture carrying a bounded run-report
 * summary, for rendering the "Report" surface once a run ends. */
export const fullAutoRunFixtureCompleted: FullAutoRunMobileProjection = {
  ...fullAutoRunFixtureRunning,
  lifecycleState: "completed",
  updatedAt: "2026-07-17T00:20:00.000Z",
  lastTransition: { actor: "owner_ui", at: "2026-07-17T00:20:00.000Z" },
  successfulAttempts: 9,
  rotationCount: 1,
  receiptSummary: {
    schema: "full_auto_run.mobile_receipt.v1",
    runRef: "run.full-auto.fixture-0001",
    threadRef: "thread.full-auto.fixture.0001",
    objectiveDigest: "a".repeat(64),
    doneConditionDigest: "b".repeat(64),
    workspaceRefDigest: "c".repeat(64),
    state: "completed",
    startedAt: "2026-07-17T00:00:00.000Z",
    endedAt: "2026-07-17T00:20:00.000Z",
    turnCap: 20,
    successfulAttempts: 9,
    failedAttempts: 1,
    providerIdentities: ["codex-local"],
    providerTransitionCount: 1,
    providerTransitionDispositions: ["complete_within_bounds"],
    livenessGapCount: 0,
    recoveryActionsUsed: [],
    verifiedRefCount: 3,
    claimedRefCount: 1,
    progressDisposition: "unknown",
    usageKnown: false,
    reportRevision: 4,
    createdAt: "2026-07-17T00:20:00.000Z",
  },
}

/**
 * A source that cycles through a fixed sequence of results, one call per
 * step, repeating the final entry once exhausted. This is what lets a manual
 * verification run genuinely observe a live lifecycle-state transition (the
 * poll loop in `app.tsx` calls the source repeatedly) without needing a real
 * Desktop run.
 */
export const makeSequencedFullAutoRunProjectionSource = (
  sequence: ReadonlyArray<FullAutoRunProjectionResult>,
): FullAutoRunProjectionSource => {
  let step = 0
  return async () => {
    const result = sequence[Math.min(step, sequence.length - 1)] ?? { state: "none" }
    step += 1
    return result
  }
}

/** Fixed fixture source: always reports the same active running fixture. */
export const fixedActiveFullAutoRunProjectionSource: FullAutoRunProjectionSource = async () => ({
  state: "active",
  projection: fullAutoRunFixtureRunning,
})

/** Fixed fixture source reporting no active run — the default/off state. */
export const noActiveFullAutoRunProjectionSource: FullAutoRunProjectionSource = async () => ({
  state: "none",
})
