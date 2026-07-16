import { randomUUID } from "node:crypto"

import type { FullAutoProfile, FullAutoRecord, FullAutoRegistry } from "./full-auto-registry.ts"

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

export type FullAutoDispatchResult = Readonly<{ ok: boolean; reason?: string }>
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
  /** FA-H5: a dispatch failed (thrown or `ok: false`) -- always invoked. */
  onDispatchFailed?: (threadRef: string, failure: Readonly<{
    reason: string
    consecutiveFailures: number
    disabled: boolean
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
    // FA-H3: the lease and the dispatched turn share ONE identity. Claim it
    // durably before dispatch; a concurrent pass that lost the claim skips.
    const turnRef = `turn.full-auto.${randomUUID()}`
    if (!input.registry.claimPending(threadRef, turnRef)) continue
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
    try {
      const result = await input.dispatch({
        threadRef,
        turnRef,
        message: FULL_AUTO_CONTINUE_MESSAGE,
        ...(record.profile === undefined ? {} : { profile: record.profile }),
      })
      if (result.ok) {
        input.registry.incrementContinuation(threadRef)
        input.registry.recordSuccess(threadRef)
        dispatched.push(threadRef)
      } else {
        failThread(result.reason ?? "dispatch_failed")
      }
    } catch (error) {
      failThread(error instanceof Error ? `${error.name}: ${error.message}` : "dispatch_threw")
    }
  }
  return dispatched
}
