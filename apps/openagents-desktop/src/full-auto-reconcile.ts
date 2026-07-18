import { randomUUID } from "node:crypto"

import type {
  FullAutoProfile,
  FullAutoRecord,
  FullAutoRegistry,
  FullAutoResumeActor,
  FullAutoRotationReason,
  FullAutoRotationRecord,
  FullAutoRoutingCandidate,
} from "./full-auto-registry.ts"

/**
 * Full Auto (#8853): the single decision function called from two trigger
 * points in main.ts -- right after any Full-Auto-flagged turn completes, and
 * once at app startup after existing turn-recovery settles. Both call sites
 * share this exact logic so "should the next turn start" is decided in
 * exactly one durable place, not duplicated between a live-completion path
 * and a separate restart path.
 *
 * Wave 2 hardening:
 * - FA-H2 (#8875): a continuation only dispatches into the exact workspace
 *   granted at enable time; a mismatched or unbound workspace fails CLOSED
 *   (record disabled with a typed blockedReason, never silently redirected).
 * - FA-H3 (#8876): dispatch is exactly-once -- the continuation turn ref is
 *   claimed as a durable per-thread lease BEFORE dispatch, so two overlapping
 *   reconcile passes can never both dispatch the same thread. Startup clears
 *   a stale lease whose turn ref never reached the local-turn journal
 *   (crashed mid-dispatch) before the normal decision runs.
 * - FA-H5 (#8878): a failed dispatch (thrown OR `ok: false`) is a typed,
 *   visible outcome -- failure state persists on the record, retries respect
 *   bounded exponential backoff, and the record disables after
 *   FULL_AUTO_MAX_CONSECUTIVE_FAILURES. Failures do NOT consume cap slots:
 *   continuationCount increments only on successful dispatch.
 * - FA-H6 (#8879): the record's bound execution profile rides along on the
 *   dispatch input so continuations replay the initiating turn's
 *   account/model/effort.
 */
export const FULL_AUTO_CONTINUE_MESSAGE =
  "Continue Full Auto: look at this repository (README, docs, open issues) and do the next concrete useful thing."
export const FULL_AUTO_MAX_CONTINUATIONS = 20
/** FA-H5 (#8878): consecutive dispatch failures before the record disables. */
export const FULL_AUTO_MAX_CONSECUTIVE_FAILURES = 5
export const FULL_AUTO_FAILURE_BACKOFF_BASE_MS = 30_000
export const FULL_AUTO_FAILURE_BACKOFF_MAX_MS = 30 * 60_000

/** Bounded exponential backoff: min(2^failures * 30s, 30min). */
export const fullAutoFailureBackoffMs = (consecutiveFailures: number): number =>
  Math.min(2 ** consecutiveFailures * FULL_AUTO_FAILURE_BACKOFF_BASE_MS, FULL_AUTO_FAILURE_BACKOFF_MAX_MS)

/**
 * FA-GD-01 (#8991): the NON-OVERRIDABLE core guardrail set. These are
 * enforced in code, not configuration -- no `guardrails` field, registry
 * option, environment variable, or owner-conversation setting exists that
 * can relax them, and none of the modules that enforce them read
 * `process.env` at all (proven by the immunity test in
 * tests/full-auto-guardrails.test.ts):
 *
 * - `workspace_binding` (FA-H2 #8875): a continuation only ever dispatches
 *   into the exact workspace granted at enable time; mismatch or an unbound
 *   record disables the loop fail-closed in `reconcileFullAutoThreads`
 *   below. `FullAutoGuardrailsSchema` deliberately has no field touching
 *   this check, and unknown keys on a durable guardrails object are dropped
 *   at decode.
 * - `own_capacity_only` (FA-RT-01 #8987 admission): the loop can only run on
 *   lanes the owner's own accounts admit -- `validateFullAutoRoutingPolicy`
 *   refuses unknown/unadmitted/ineligible lanes fail-closed at bind time,
 *   and main's per-dispatch lane gate re-checks live admission. There is no
 *   configurable list of foreign capacity to rotate onto.
 * - `no_rate_limit_reset_triggering`: a rate_limited failure either rotates
 *   to a DIFFERENT owner-admitted lane (FA-RT-01) or consumes FA-H5 failure
 *   budget and waits out the full bounded backoff window. No guardrail field
 *   exists to shrink or skip the backoff window, and the loop never times a
 *   retry to a provider's rate-limit reset.
 */
export const FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS = Object.freeze([
  "workspace_binding",
  "own_capacity_only",
  "no_rate_limit_reset_triggering",
] as const)

/** FA-GD-01 (#8991): consecutive fully-settled unproductive turns before the
 * loop pauses itself instead of continuing blind. */
export const FULL_AUTO_NO_PROGRESS_TURN_THRESHOLD = 3

/**
 * FA-GD-01 (#8991): the turn dispositions that count as "no progress" for
 * the confidence gate. `failed` and `interrupted_by_restart` are machine
 * outcomes with no evidence of useful work; `owner_interrupted` is a human
 * steering action and `resumed_after_restart`/`completed` are progress-
 * bearing, so none of those ever count toward a pause.
 */
export const FULL_AUTO_NO_PROGRESS_DISPOSITIONS: ReadonlySet<string> = new Set([
  "failed",
  "interrupted_by_restart",
])

/** FA-GD-01 (#8991): one settled-turn evidence row for the no-progress
 * detector -- the caller projects these from the local turn journal
 * (disposition + updatedAt only; never transcript text). */
export type FullAutoTurnEvidence = Readonly<{
  disposition: string | null
  updatedAt: string
}>

/**
 * FA-GD-01 (#8991): the deterministic no-progress detector. Pure over
 * existing durable evidence: it counts the TRAILING run of consecutive
 * settled turns whose disposition is in FULL_AUTO_NO_PROGRESS_DISPOSITIONS,
 * considering only turns settled after `anchorAt` (the record's
 * lastResumedAt ?? enabledAt) so pre-grant or pre-resume history can never
 * pause a freshly (re)started loop. No inference, no heuristics beyond the
 * disposition set and the threshold.
 */
export const detectFullAutoNoProgress = (input: Readonly<{
  evidence: ReadonlyArray<FullAutoTurnEvidence>
  anchorAt: string | null
  threshold?: number
}>): Readonly<{ noProgress: boolean; consecutive: number }> => {
  const threshold = input.threshold ?? FULL_AUTO_NO_PROGRESS_TURN_THRESHOLD
  const settled = input.evidence
    .filter(entry => entry.disposition !== null)
    .filter(entry => input.anchorAt === null || entry.updatedAt > input.anchorAt)
    .toSorted((left, right) => left.updatedAt.localeCompare(right.updatedAt))
  let consecutive = 0
  for (let index = settled.length - 1; index >= 0; index -= 1) {
    if (!FULL_AUTO_NO_PROGRESS_DISPOSITIONS.has(settled[index]!.disposition!)) break
    consecutive += 1
  }
  return { noProgress: consecutive >= threshold, consecutive }
}

/**
 * FA-GD-01 (#8991): the explicit resume command for a low-confidence pause.
 * Exported as the registry-level API the control server (and, later, the
 * run-view UI) wires -- this issue deliberately adds NO control-server
 * route. Resuming a record that is not paused is a null no-op; a successful
 * resume records a typed `continue` decision and schedules the same
 * serialized reconciliation path every other trigger uses.
 */
export const resumeFullAuto = (input: Readonly<{
  /** FA-WIRE-01 (#8996): narrowed to exactly the two registry methods this
   * command uses so capability-scoped callers (the control server's widened
   * registry Pick) can wire it without holding the whole registry. Purely a
   * type-level narrowing -- every existing full-registry caller still
   * satisfies it and runtime behavior is unchanged. */
  registry: Pick<FullAutoRegistry, "resume" | "recordDecision">
  threadRef: string
  actor: FullAutoResumeActor
  scheduleReconciliation: () => void
}>): FullAutoRecord | null => {
  const resumed = input.registry.resume(input.threadRef, input.actor)
  if (resumed === null) return null
  const recorded = input.registry.recordDecision(input.threadRef, {
    decision: "continue",
    reason: `resumed_by_${input.actor}`,
  })
  input.scheduleReconciliation()
  return recorded ?? resumed
}

/**
 * Apply the composer toggle at the durable boundary. Enabling is also a
 * dispatch trigger: Full Auto means "go now", including on an empty new
 * session, rather than "wait for a separate Send click". The supplied
 * scheduler must enter the same serialized reconciliation path used by turn
 * completion and startup so this adds no parallel dispatch mechanism.
 */
export const applyFullAutoComposerToggle = (input: Readonly<{
  registry: FullAutoRegistry
  threadRef: string
  enabled: boolean
  workspaceRef: string
  profile: FullAutoProfile
  scheduleReconciliation: () => void
}>): FullAutoRecord => {
  const record = input.registry.set(
    input.threadRef,
    input.enabled,
    input.enabled
      ? { workspaceRef: input.workspaceRef, profile: input.profile }
      : { disabledBy: "ui_toggle" },
  )
  if (input.enabled) input.scheduleReconciliation()
  return record
}

/**
 * FA-H3 (#8876): a promise-chain mutex. Each queued task awaits every task
 * queued before it, so overlapping reconciliation triggers (turn completion +
 * startup + a future continue-now) can never interleave the snapshot/dispatch
 * sequence. A failed task never blocks the chain.
 */
export const makeSerialTaskQueue = (): (<A>(task: () => Promise<A>) => Promise<A>) => {
  let tail: Promise<unknown> = Promise.resolve()
  return task => {
    const run = tail.then(() => task())
    tail = run.then(() => undefined, () => undefined)
    return run
  }
}

export type FullAutoDispatchResult = Readonly<{
  ok: boolean
  reason?: string
  /**
   * FA-RT-01 (#8987): OPTIONAL typed failure class. When present (and the
   * record carries a routing policy with an untried admitted candidate),
   * reconciliation rotates to the next candidate in the SAME pass instead of
   * consuming FA-H5 failure budget. Absent = every failure keeps the
   * existing budget/backoff semantics exactly.
   */
  failureClass?: FullAutoRotationReason
}>
export type FullAutoDispatch = (input: Readonly<{
  threadRef: string
  /** The exact leased continuation turn ref -- the dispatched turn MUST use
   * this identity so the lease and the journal row agree (FA-H3). */
  turnRef: string
  message: string
  /** FA-H6: the record's bound execution profile, when one exists. */
  profile?: FullAutoProfile
}>) => Promise<FullAutoDispatchResult>

export type FullAutoWorkspaceBlockReason = "workspace_mismatch" | "workspace_unbound"

/**
 * FA-GD-01 (#8991): a typed guardrail violation. `reason` is the exact
 * blockedReason persisted on the disabled record, so the callback, the
 * durable record, and the run report's threadFailureHistory all agree.
 */
export type FullAutoGuardrailViolation = Readonly<{
  guardrail: "max_wall_clock" | "max_turns" | "max_per_turn_failures"
  limit: number
  observed: number
  reason: string
}>

/**
 * FA-RT-01 (#8987): map a lane's typed terminal failure reason (plus its
 * bounded public-safe detail) onto a rotation class, or null when the
 * failure must NOT rotate (owner interrupts, model substitution, workflow
 * incompatibility, and anything untyped). The detail markers mirror the
 * exact classification codex-app-server-turn.ts already applies
 * (`quotaExhausted` before `rateLimited`); this is deterministic
 * error-classification on an already-selected path, not intent routing.
 */
export const classifyFullAutoDispatchFailure = (
  reason: string | undefined,
  detail?: string,
): FullAutoRotationReason | null => {
  switch (reason) {
    case "budget_exceeded":
    case "no_claude_account":
    case "no_codex_account":
    case "account_reconnect_required":
      return "account_exhausted"
    case "timeout":
    case "sdk_unavailable":
    case "session_failed": {
      const lower = (detail ?? "").toLowerCase()
      if (lower.includes("usage limit") || lower.includes("quota") || lower.includes("purchase more credits")) {
        return "account_exhausted"
      }
      if (lower.includes("rate limit") || lower.includes("429") || lower.includes("too many requests")) {
        return "rate_limited"
      }
      return "provider_error"
    }
    default:
      return null
  }
}

export const reconcileFullAutoThreads = async (input: Readonly<{
  registry: FullAutoRegistry
  /** Thread refs with a nonterminal (in-flight or awaiting-recovery) turn right now. */
  nonterminalThreadRefs: () => ReadonlySet<string>
  /** FA-H2: the currently resolved workspace, from the SAME source of truth
   * main uses to run codex-local turns. */
  resolveWorkspaceRef: () => string
  /** FA-H3: whether the local-turn journal holds a nonterminal row for a turn
   * ref -- used to detect a stale (crashed mid-dispatch) lease at startup. */
  journalHasNonterminalTurn: (turnRef: string) => boolean
  /** FA-H3: only the STARTUP reconciliation clears stale leases; a mid-session
   * pass must treat a held lease as an in-flight dispatch and skip. */
  clearStaleLeases?: boolean
  now?: () => Date
  dispatch: FullAutoDispatch
  onCapReached?: (threadRef: string) => void
  /** FA-H2: the record was disabled because its workspace binding failed. */
  onWorkspaceBlocked?: (threadRef: string, block: Readonly<{
    reason: FullAutoWorkspaceBlockReason
    grantedWorkspaceRef: string | null
    resolvedWorkspaceRef: string
  }>) => void
  /** FA-H5: a dispatch failed (thrown or `ok: false`) -- always invoked.
   * FA-RT-01 (#8987): with a routing policy, invoked only for the ONE
   * budget-consuming failure that ends a full unsuccessful cycle (or a
   * non-rotatable failure); rotation-eligible intermediate failures invoke
   * `onRotated` instead. */
  onDispatchFailed?: (threadRef: string, failure: Readonly<{
    reason: string
    consecutiveFailures: number
    disabled: boolean
  }>) => void
  /** FA-RT-01 (#8987): a typed failure rotated the thread to its next
   * admitted candidate within the same pass (never consuming budget). */
  onRotated?: (threadRef: string, rotation: FullAutoRotationRecord) => void
  /**
   * FA-GD-01 (#8991): settled-turn evidence for the no-progress confidence
   * gate, projected by the caller from the local turn journal for one
   * thread (disposition + updatedAt only). Optional: callers without a
   * journal in scope simply run without the confidence gate, exactly the
   * pre-#8991 behavior.
   */
  turnEvidence?: (threadRef: string) => ReadonlyArray<FullAutoTurnEvidence>
  /** FA-GD-01 (#8991): a configured guardrail terminated the loop. */
  onGuardrailStopped?: (threadRef: string, violation: FullAutoGuardrailViolation) => void
  /** FA-GD-01 (#8991): the confidence gate paused the loop durably instead
   * of continuing blind; resume is an explicit command (resumeFullAuto). */
  onPausedLowConfidence?: (threadRef: string, pause: Readonly<{
    reason: string
    consecutiveNoProgressTurns: number
  }>) => void
}>): Promise<ReadonlyArray<string>> => {
  const now = input.now ?? (() => new Date())
  const dispatched: string[] = []
  const inFlight = input.nonterminalThreadRefs()
  for (const threadRef of input.registry.enabledThreads()) {
    if (inFlight.has(threadRef)) continue
    // Re-read fresh: an earlier iteration (or a durable write from the same
    // serialized pass) may have disabled the record since the snapshot.
    const record = input.registry.record(threadRef)
    if (record === null || !record.enabled) continue
    // FA-GD-01 (#8991): a durably paused record never dispatches. The pause
    // is not a disable -- the owner's grant stands -- but only an explicit
    // resume (resumeFullAuto) clears it. Checked before every other gate so
    // a paused loop can never be disabled underneath the owner by a
    // workspace change or wall-clock expiry while they decide.
    if (record.pausedReason !== undefined) continue
    // FA-H2: authority binding first. An enabled record whose granted
    // workspace cannot be matched exactly never dispatches -- disable it
    // visibly instead of silently redirecting high-trust work.
    const resolvedWorkspaceRef = input.resolveWorkspaceRef()
    if (record.workspaceRef === undefined) {
      input.registry.set(threadRef, false, {
        blockedReason: "workspace_unbound",
        disabledBy: "workspace_guard",
      })
      input.onWorkspaceBlocked?.(threadRef, {
        reason: "workspace_unbound",
        grantedWorkspaceRef: null,
        resolvedWorkspaceRef,
      })
      continue
    }
    if (record.workspaceRef !== resolvedWorkspaceRef) {
      input.registry.set(threadRef, false, {
        blockedReason: "workspace_mismatch",
        disabledBy: "workspace_guard",
      })
      input.onWorkspaceBlocked?.(threadRef, {
        reason: "workspace_mismatch",
        grantedWorkspaceRef: record.workspaceRef,
        resolvedWorkspaceRef,
      })
      continue
    }
    // FA-GD-01 (#8991): the wall-clock guardrail, checked before backoff so
    // an expired run terminates even while waiting out a failure window. The
    // anchor is the durable enabledAt; a guardrail-bearing record without an
    // anchor (only reachable by hand-editing the file) fails CLOSED rather
    // than running unbounded.
    const maxWallClockMs = record.guardrails?.maxWallClockMs
    if (maxWallClockMs !== undefined) {
      const elapsedMs = record.enabledAt === undefined
        ? Number.POSITIVE_INFINITY
        : now().getTime() - Date.parse(record.enabledAt)
      if (elapsedMs >= maxWallClockMs) {
        const reason = "guardrail_max_wall_clock"
        input.registry.recordDecision(threadRef, {
          decision: "stop_guardrail",
          reason,
          budgetRemaining: 0,
        })
        input.registry.set(threadRef, false, { blockedReason: reason, disabledBy: "guardrail" })
        input.onGuardrailStopped?.(threadRef, {
          guardrail: "max_wall_clock",
          limit: maxWallClockMs,
          observed: elapsedMs,
          reason,
        })
        continue
      }
    }
    // FA-H5: respect the bounded failure backoff window.
    const failures = record.consecutiveFailures ?? 0
    if (failures > 0 && record.lastFailureAt !== undefined) {
      const sinceFailureMs = now().getTime() - Date.parse(record.lastFailureAt)
      if (sinceFailureMs < fullAutoFailureBackoffMs(failures)) continue
    }
    // Cap check BEFORE dispatch: counts increment only on successful dispatch
    // (FA-H5), so a record at the cap disables here without minting a turn.
    // FA-GD-01 (#8991): guardrails.maxTurns generalizes the built-in cap;
    // absent, the existing FULL_AUTO_MAX_CONTINUATIONS semantics (reason,
    // attribution, callback) are preserved byte-for-byte.
    const effectiveTurnCap = record.guardrails?.maxTurns ?? FULL_AUTO_MAX_CONTINUATIONS
    if (record.continuationCount >= effectiveTurnCap) {
      const guardrailCap = record.guardrails?.maxTurns !== undefined
      const reason = guardrailCap ? "guardrail_max_turns" : "continuation_cap_reached"
      input.registry.recordDecision(threadRef, {
        decision: "stop_guardrail",
        reason,
        budgetRemaining: 0,
      })
      input.registry.set(threadRef, false, {
        blockedReason: reason,
        disabledBy: guardrailCap ? "guardrail" : "continuation_cap",
      })
      if (guardrailCap) {
        input.onGuardrailStopped?.(threadRef, {
          guardrail: "max_turns",
          limit: effectiveTurnCap,
          observed: record.continuationCount,
          reason,
        })
      } else {
        input.onCapReached?.(threadRef)
      }
      continue
    }
    // FA-H3: startup-only stale-lease recovery -- a lease whose turn ref never
    // produced a journal row belongs to a dispatch that crashed before the
    // turn was accepted; close that intent so the normal decision can run.
    if (
      input.clearStaleLeases === true &&
      typeof record.pendingTurnRef === "string" &&
      !input.journalHasNonterminalTurn(record.pendingTurnRef)
    ) {
      input.registry.clearPending(threadRef)
    }
    // FA-GD-01 (#8991): the no-progress confidence gate. Deterministic over
    // durable settled-turn evidence anchored at lastResumedAt ?? enabledAt;
    // when it trips, the loop pauses durably with a typed reason instead of
    // dispatching another blind continuation.
    if (input.turnEvidence !== undefined) {
      const progress = detectFullAutoNoProgress({
        evidence: input.turnEvidence(threadRef),
        anchorAt: record.lastResumedAt ?? record.enabledAt ?? null,
      })
      if (progress.noProgress) {
        const reason = `no_progress:${progress.consecutive}_consecutive_unproductive_turns`
        input.registry.pause(threadRef, reason)
        input.registry.recordDecision(threadRef, { decision: "pause_low_confidence", reason })
        input.onPausedLowConfidence?.(threadRef, {
          reason,
          consecutiveNoProgressTurns: progress.consecutive,
        })
        continue
      }
    }
    // FA-GD-01 (#8991): guardrails.maxPerTurnFailures generalizes the
    // built-in FA-H5 failure budget. The disable keeps its existing
    // dispatch_failure_limit attribution (same failure class) either way;
    // a guardrail-configured limit additionally reports as a typed
    // guardrail violation with a stop_guardrail decision.
    const effectiveFailureLimit =
      record.guardrails?.maxPerTurnFailures ?? FULL_AUTO_MAX_CONSECUTIVE_FAILURES
    const guardrailFailureLimit = record.guardrails?.maxPerTurnFailures !== undefined
    const failThread = (reason: string): void => {
      const consecutiveFailures = input.registry.recordFailure(threadRef, reason)
      const disabled = consecutiveFailures >= effectiveFailureLimit
      if (disabled) {
        input.registry.recordDecision(threadRef, {
          decision: "stop_guardrail",
          reason: guardrailFailureLimit ? "guardrail_max_per_turn_failures" : reason,
        })
        input.registry.set(threadRef, false, {
          blockedReason: reason,
          disabledBy: "dispatch_failure_limit",
        })
        if (guardrailFailureLimit) {
          input.onGuardrailStopped?.(threadRef, {
            guardrail: "max_per_turn_failures",
            limit: effectiveFailureLimit,
            observed: consecutiveFailures,
            reason,
          })
        }
      }
      input.onDispatchFailed?.(threadRef, { reason, consecutiveFailures, disabled })
    }
    // FA-RT-01 (#8987): resolve the ordered candidate cycle. Absent/empty
    // routingPolicy = the legacy single-candidate pass over the bound
    // profile, byte-for-byte the pre-#8987 behavior. With a policy, the
    // cycle starts at the currently bound lane (or the first candidate) and
    // tries each candidate AT MOST once in this pass.
    const policy: ReadonlyArray<FullAutoRoutingCandidate> = record.routingPolicy ?? []
    const useRouting = policy.length > 0
    const boundLaneIndex = policy.findIndex(candidate => candidate.lane === record.profile?.lane)
    const startIndex = boundLaneIndex === -1 ? 0 : boundLaneIndex
    const attempts = useRouting ? policy.length : 1
    for (let attempt = 0; attempt < attempts; attempt += 1) {
      const candidate = useRouting ? policy[(startIndex + attempt) % policy.length]! : null
      // The candidate's lane/account override the bound profile; model and
      // effort carry over only when the lane is unchanged (a foreign lane
      // must fall back to its own defaults, never replay another lane's
      // model string).
      const profile: FullAutoProfile | undefined = candidate === null
        ? record.profile
        : {
            ...(record.profile?.lane === candidate.lane ? record.profile : {}),
            lane: candidate.lane,
            ...(candidate.accountRef === undefined ? {} : { accountRef: candidate.accountRef }),
          }
      // FA-H3: the lease and the dispatched turn share ONE identity. Claim
      // it durably before dispatch; a concurrent pass that lost the claim
      // skips. Each rotation attempt is its own leased turn identity.
      const turnRef = `turn.full-auto.${randomUUID()}`
      if (!input.registry.claimPending(threadRef, turnRef)) break
      let failure: Readonly<{ reason: string; failureClass: FullAutoRotationReason | null }>
      try {
        const result = await input.dispatch({
          threadRef,
          turnRef,
          message: FULL_AUTO_CONTINUE_MESSAGE,
          ...(profile === undefined ? {} : { profile }),
        })
        if (result.ok) {
          const continuationCount = input.registry.incrementContinuation(threadRef)
          input.registry.recordSuccess(threadRef)
          // FA-GD-01 (#8991): every successful continuation is a typed,
          // durable decision fact with the remaining turn budget.
          input.registry.recordDecision(threadRef, {
            decision: "continue",
            reason: "dispatch_succeeded",
            budgetRemaining: Math.max(0, effectiveTurnCap - continuationCount),
          })
          // FA-RT-01: a rotation that succeeded on a different candidate
          // rebinds the durable profile so the NEXT continuation starts on
          // the lane that is actually working.
          if (
            candidate !== null && profile !== undefined &&
            (record.profile?.lane !== profile.lane || record.profile?.accountRef !== profile.accountRef)
          ) {
            input.registry.bindProfile(threadRef, profile)
          }
          dispatched.push(threadRef)
          break
        }
        failure = {
          reason: result.reason ?? "dispatch_failed",
          failureClass: result.failureClass ?? null,
        }
      } catch (error) {
        failure = {
          reason: error instanceof Error ? `${error.name}: ${error.message}` : "dispatch_threw",
          failureClass: null,
        }
      }
      const untriedRemain = attempt < attempts - 1
      if (useRouting && failure.failureClass !== null && untriedRemain) {
        // FA-RT-01: rotate WITHOUT consuming FA-H5 budget -- release the
        // lease, persist the typed rotation fact, and try the next admitted
        // candidate in this same pass.
        input.registry.clearPending(threadRef)
        const nextCandidate = policy[(startIndex + attempt + 1) % policy.length]!
        const rotated = input.registry.recordRotation(threadRef, {
          fromLane: candidate!.lane,
          toLane: nextCandidate.lane,
          reason: failure.failureClass,
        })
        const rotationRecord = rotated?.rotationHistory?.at(-1)
        // FA-GD-01 (#8991): a rotation is also a typed decision fact.
        input.registry.recordDecision(threadRef, {
          decision: "rotate",
          reason: `${candidate!.lane}>${nextCandidate.lane}:${failure.failureClass}`,
        })
        if (rotationRecord !== undefined) input.onRotated?.(threadRef, rotationRecord)
        continue
      }
      // A full unsuccessful cycle (or a non-rotatable failure) consumes
      // exactly ONE failure-budget step -- existing FA-H5 semantics.
      failThread(failure.reason)
      break
    }
  }
  return dispatched
}
