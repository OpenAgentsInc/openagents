import { randomUUID } from "node:crypto"

import type {
  FullAutoProfile,
  FullAutoRecord,
  FullAutoRegistry,
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
    // FA-H5: respect the bounded failure backoff window.
    const failures = record.consecutiveFailures ?? 0
    if (failures > 0 && record.lastFailureAt !== undefined) {
      const sinceFailureMs = now().getTime() - Date.parse(record.lastFailureAt)
      if (sinceFailureMs < fullAutoFailureBackoffMs(failures)) continue
    }
    // Cap check BEFORE dispatch: counts increment only on successful dispatch
    // (FA-H5), so a record at the cap disables here without minting a turn.
    if (record.continuationCount >= FULL_AUTO_MAX_CONTINUATIONS) {
      input.registry.set(threadRef, false, {
        blockedReason: "continuation_cap_reached",
        disabledBy: "continuation_cap",
      })
      input.onCapReached?.(threadRef)
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
    const failThread = (reason: string): void => {
      const consecutiveFailures = input.registry.recordFailure(threadRef, reason)
      const disabled = consecutiveFailures >= FULL_AUTO_MAX_CONSECUTIVE_FAILURES
      if (disabled) {
        input.registry.set(threadRef, false, {
          blockedReason: reason,
          disabledBy: "dispatch_failure_limit",
        })
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
          input.registry.incrementContinuation(threadRef)
          input.registry.recordSuccess(threadRef)
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
