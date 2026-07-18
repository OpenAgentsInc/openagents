import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import { FULL_AUTO_MAX_CONTINUATIONS } from "./full-auto-reconcile.ts"
import { FullAutoProfileSchema, type FullAutoProfile, type FullAutoRecord } from "./full-auto-registry.ts"

/**
 * FA-RUN-01 (#8969): the durable `FullAutoRun` objective/lifecycle/control
 * model that ProductSpec rev 10 (FA-AC-38..51) commits to.
 *
 * This module is deliberately layered ON TOP of the existing per-thread
 * `full-auto-registry.ts` + `full-auto-reconcile.ts` machinery rather than
 * replacing it: every hardened FA-H2 (workspace binding), FA-H3
 * (exactly-once dispatch lease), FA-H5 (failure/backoff/disable), FA-H6
 * (profile continuity), and FA-H7 (continuation cap) mechanism stays exactly
 * as-is and is explicitly "retained unchanged" or "retained with stronger
 * proof" per the ProductSpec's Criterion Disposition Map. A `FullAutoRun`
 * record is the new durable authority for WHY a thread is being driven (its
 * objective, done condition, and owner-attributed lifecycle state); the
 * thread-level `enabled` boolean remains the low-level dispatch gate the
 * exactly-once reconcile loop already reads.
 *
 * The single source of truth for "is this transition legal" is
 * `applyFullAutoRunTransition` -- every mutating caller (control API, a
 * future CLI/MCP mirror, or a future launcher UI) is expected to route
 * through the registry's `transition`/`start`/`rerun` methods rather than
 * hand-writing state, per the ProductSpec's "one typed lifecycle, not
 * multiple booleans" requirement.
 */
export const FULL_AUTO_RUN_REGISTRY_SCHEMA = "openagents.desktop.full_auto_run_registry.v1" as const
export const FULL_AUTO_RUN_RECORD_LIMIT = 128
export const FULL_AUTO_RUN_REASON_LIMIT = 400
export const FULL_AUTO_RUN_TITLE_LIMIT = 120
export const FULL_AUTO_RUN_OBJECTIVE_LIMIT = 4000
export const FULL_AUTO_RUN_DONE_CONDITION_LIMIT = 2000
export const FULL_AUTO_RUN_TURN_CAP_DEFAULT = FULL_AUTO_MAX_CONTINUATIONS
export const FULL_AUTO_RUN_TRANSITION_HISTORY_LIMIT = 200
export const FULL_AUTO_RUN_OBJECTIVE_HISTORY_LIMIT = 50

/**
 * The exact lifecycle enumeration named by ProductSpec FA-AC-43 and the
 * `openagents_desktop.full_auto_play_pause_stop_lifecycle.v1` behavior
 * contract. `draft` is a run that has not Started yet (does not count
 * against the v1 one-active-run concurrency policy); the four listed
 * terminal states never transition again -- a rerun always mints a new
 * `runRef` (FA-AC-40).
 */
export const FullAutoRunStateSchema = Schema.Literals([
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
])
export type FullAutoRunState = typeof FullAutoRunStateSchema.Type

export const FULL_AUTO_RUN_TERMINAL_STATES: ReadonlySet<FullAutoRunState> = new Set([
  "completed",
  "failed",
  "stopped",
  "cap_reached",
])
/** The v1 concurrency-policy set: draft and terminal states are excluded. */
export const FULL_AUTO_RUN_ACTIVE_STATES: ReadonlySet<FullAutoRunState> = new Set([
  "running",
  "pausing",
  "paused",
  "retrying",
  "stalled",
])
export const isFullAutoRunTerminal = (state: FullAutoRunState): boolean => FULL_AUTO_RUN_TERMINAL_STATES.has(state)
export const isFullAutoRunActive = (state: FullAutoRunState): boolean => FULL_AUTO_RUN_ACTIVE_STATES.has(state)

/**
 * The exact legal transition graph. Every entry not listed here is illegal
 * and `applyFullAutoRunTransition` refuses it -- see FA-AC-43 ("An illegal
 * transition... is refused with a typed error and never silently coerced")
 * and the behavior contract's example ("Resume from a non-Paused state").
 *
 *  - draft -> running (Start) | stopped (cancel a draft before Start)
 *  - running -> pausing (Pause with an active turn) | paused (Pause, idle)
 *    | retrying (a transient dispatch failure enters backoff)
 *    | stalled (liveness SLO exceeded or the bound thread/session vanished --
 *      classification itself is FA-RUN-03 #8971's job, this module only
 *      carries the legal edge) | completed (self-reported) | failed
 *      (unrecoverable) | stopped (owner/API Stop) | cap_reached (turn cap)
 *  - pausing -> paused (the in-flight turn resolved) | failed (interrupt
 *    could not be confirmed) | stopped (Stop is legal from any non-terminal
 *    state per FA-AC-45, including while a Pause is settling)
 *  - paused -> running (Resume, FA-AC-44: legal ONLY from paused)
 *    | stopped
 *  - retrying -> running (retry succeeded) | stalled | failed | cap_reached
 *    | stopped
 *  - stalled -> retrying ("retry now" recovery affordance, FA-AC-48)
 *    | failed | stopped
 *  - completed / failed / stopped / cap_reached -> (terminal, no edges)
 */
export const FULL_AUTO_RUN_LEGAL_TRANSITIONS: ReadonlyMap<FullAutoRunState, ReadonlySet<FullAutoRunState>> = new Map<
  FullAutoRunState,
  ReadonlySet<FullAutoRunState>
>([
  ["draft", new Set<FullAutoRunState>(["running", "stopped"])],
  [
    "running",
    new Set<FullAutoRunState>([
      "pausing",
      "paused",
      "retrying",
      "stalled",
      "completed",
      "failed",
      "stopped",
      "cap_reached",
    ]),
  ],
  ["pausing", new Set<FullAutoRunState>(["paused", "failed", "stopped"])],
  ["paused", new Set<FullAutoRunState>(["running", "stopped"])],
  ["retrying", new Set<FullAutoRunState>(["running", "stalled", "failed", "cap_reached", "stopped"])],
  ["stalled", new Set<FullAutoRunState>(["retrying", "failed", "stopped"])],
  ["completed", new Set<FullAutoRunState>()],
  ["failed", new Set<FullAutoRunState>()],
  ["stopped", new Set<FullAutoRunState>()],
  ["cap_reached", new Set<FullAutoRunState>()],
])

export const isLegalFullAutoRunTransition = (from: FullAutoRunState, to: FullAutoRunState): boolean =>
  (FULL_AUTO_RUN_LEGAL_TRANSITIONS.get(from) ?? new Set()).has(to)

/**
 * Every transition's durable attribution vocabulary -- extends the existing
 * `FullAutoDisabledBySchema` pattern (ui_toggle/control_api/workspace_guard/
 * continuation_cap/dispatch_failure_limit) to the full run graph, per the
 * issue's requirement that "every renderer, loopback API, CLI, and MCP
 * mutation uses the same transition function and attribution vocabulary."
 */
export const FullAutoRunActorSchema = Schema.Literals([
  /** A human click in the (future, #8974) Desktop UI. */
  "owner_ui",
  /** The loopback control API (#8886). */
  "control_api",
  /** The `full-auto` CLI, itself a thin control-API client. */
  "cli",
  /** The stdio MCP server, itself a thin control-API client. */
  "mcp",
  /** FA-H2: workspace binding fail-closed. */
  "workspace_guard",
  /** FA-H7: the 20-continuation safety cap. */
  "continuation_cap",
  /** FA-H5: 5-consecutive-failure disable. */
  "dispatch_failure_limit",
  /** FA-GD-01 (#8991): a typed owner-configured guardrail (max wall clock,
   * max turns) terminated the underlying loop. */
  "guardrail",
  /** The in-flight turn a Pause was waiting on resolved. */
  "turn_resolution",
  /** Reconciliation observed the bound thread record changed underneath the
   * run (orphaned thread, or an unexpected external disable) and synced the
   * run's lifecycle to match -- see `settleFullAutoRunFromThreadState`. */
  "thread_state_sync",
  /** The one-time additive migration from the legacy per-thread registry. */
  "legacy_migration",
  /** Reserved for FA-RUN-03 (#8971)'s liveness/SLO classifier. */
  "liveness_monitor",
])
export type FullAutoRunActor = typeof FullAutoRunActorSchema.Type

/** FA-AC-38: objective provenance -- never invented, always attributed. */
export const FullAutoRunObjectiveSourceSchema = Schema.Literals(["user", "control_caller", "legacy_migration"])
export type FullAutoRunObjectiveSource = typeof FullAutoRunObjectiveSourceSchema.Type

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Cursor = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const Title = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_TITLE_LIMIT))
const Objective = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_OBJECTIVE_LIMIT))
const DoneCondition = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(FULL_AUTO_RUN_DONE_CONDITION_LIMIT),
)
const WorkspaceRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))
const Reason = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_REASON_LIMIT))
const TurnCap = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(1000),
)

/** FA-AC-38: revision history -- every objective/done-condition edit is
 * append-only and attributed; nothing overwrites silently. */
export const FullAutoRunObjectiveRevisionSchema = Schema.Struct({
  objective: Objective,
  doneCondition: DoneCondition,
  source: FullAutoRunObjectiveSourceSchema,
  actor: FullAutoRunActorSchema,
  at: Schema.String,
})
export type FullAutoRunObjectiveRevision = typeof FullAutoRunObjectiveRevisionSchema.Type

/** Every transition persists actor/time/reason/correlation per FA-AC-43. */
export const FullAutoRunTransitionRecordSchema = Schema.Struct({
  from: FullAutoRunStateSchema,
  to: FullAutoRunStateSchema,
  actor: FullAutoRunActorSchema,
  at: Schema.String,
  reason: Reason,
  correlationRef: Schema.optional(Ref),
})
export type FullAutoRunTransitionRecord = typeof FullAutoRunTransitionRecordSchema.Type

export const FullAutoRunSchema = Schema.Struct({
  runRef: Ref,
  /** FA-AC-38: independent of, and optional until bound to, a threadRef. */
  threadRef: Schema.optional(Ref),
  title: Title,
  objective: Objective,
  objectiveSource: FullAutoRunObjectiveSourceSchema,
  doneCondition: DoneCondition,
  objectiveHistory: Schema.Array(FullAutoRunObjectiveRevisionSchema),
  workspaceRef: Schema.optional(WorkspaceRef),
  profile: Schema.optional(FullAutoProfileSchema),
  turnCap: TurnCap,
  successfulAttempts: Cursor,
  failedAttempts: Cursor,
  state: FullAutoRunStateSchema,
  stateRevision: Cursor,
  /** Mirrors the thread-level FA-H3 lease for observability; the lease
   * itself is still enforced exactly-once by `full-auto-registry.ts`. */
  pendingTurnRef: Schema.optional(Schema.NullOr(Ref)),
  pendingStartedAt: Schema.optional(Schema.String),
  consecutiveFailures: Schema.optional(Cursor),
  lastFailureAt: Schema.optional(Schema.String),
  terminalReason: Schema.optional(Reason),
  /** FA-AC-40: rerun/new-generation linkage -- context only, never authority
   * or objective inheritance. */
  predecessorRunRef: Schema.optional(Ref),
  /** FA-AC-41: provenance for a migrated legacy row. */
  migratedFrom: Schema.optional(Schema.Literal("legacy_registry")),
  createdAt: Schema.String,
  startedAt: Schema.optional(Schema.String),
  lastProgressAt: Schema.optional(Schema.String),
  /** FA-RUN-03 (#8971): stamped every time the liveness/stall classifier
   * evaluates this run (whether or not anything changed). A large gap here
   * is direct durable evidence the app itself was not ticking recently
   * (sleep/quit/crash), not an inference from run content -- see
   * `full-auto-liveness.ts`'s `staleCheckCause`. */
  lastLivenessCheckAt: Schema.optional(Schema.String),
  pausedAt: Schema.optional(Schema.String),
  stoppedAt: Schema.optional(Schema.String),
  completedAt: Schema.optional(Schema.String),
  transitions: Schema.Array(FullAutoRunTransitionRecordSchema),
})
export type FullAutoRun = typeof FullAutoRunSchema.Type

const FullAutoRunRegistryFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_REGISTRY_SCHEMA),
  runs: Schema.Array(FullAutoRunSchema),
})

export class FullAutoRunRegistryError extends Error {
  readonly _tag = "FullAutoRunRegistryError"
  override readonly name = "FullAutoRunRegistryError"
  constructor(readonly reason: "storage_unavailable", message: string) {
    super(message)
  }
}

export class FullAutoRunTransitionError extends Error {
  readonly _tag = "FullAutoRunTransitionError"
  override readonly name = "FullAutoRunTransitionError"
  constructor(
    readonly reason: "illegal_transition",
    message: string,
    readonly from: FullAutoRunState,
    readonly to: FullAutoRunState,
  ) {
    super(message)
  }
}

const ensurePrivateParent = (filePath: string): void => {
  const parent = path.dirname(filePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
}

const writePrivateAtomic = (filePath: string, value: unknown): void => {
  ensurePrivateParent(filePath)
  const pending = `${filePath}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, `${JSON.stringify(value)}\n`, { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, filePath)
    if (process.platform !== "win32") chmodSync(filePath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw new FullAutoRunRegistryError(
      "storage_unavailable",
      error instanceof Error ? error.message : "full auto run registry unavailable",
    )
  }
}

/** FA-AC-41 migration note: corrupt-file quarantine extends to the run
 * store exactly like FA-H10 (#8883) already protects the legacy registry. */
const decodeFile = (filePath: string, now: () => Date): ReadonlyArray<FullAutoRun> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = Schema.decodeUnknownSync(FullAutoRunRegistryFileSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    )
    return decoded.runs
  } catch (error) {
    const quarantinePath = `${filePath}.quarantined-${now().toISOString()}`
    try {
      renameSync(filePath, quarantinePath)
      console.error(
        `full auto run registry failed validation; quarantined the corrupt file at ${quarantinePath} and starting with an empty registry`,
        error,
      )
    } catch {
      console.error(
        `full auto run registry failed validation and the corrupt file at ${filePath} could not be quarantined; starting with an empty registry`,
        error,
      )
    }
    return []
  }
}

const compactRunInput = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(Object.entries(value).filter(([, entry]) => entry !== undefined))

/**
 * Pure state-transition function -- the exhaustive-unit-test surface for
 * FA-AC-43/44/45/46/50. Every mutating caller MUST route through this (or
 * the registry's `transition`, which wraps it) rather than writing `state`
 * directly.
 */
export const applyFullAutoRunTransition = (
  run: FullAutoRun,
  input: Readonly<{
    to: FullAutoRunState
    actor: FullAutoRunActor
    reason: string
    correlationRef?: string
  }>,
  now: () => Date = () => new Date(),
): FullAutoRun => {
  if (!isLegalFullAutoRunTransition(run.state, input.to)) {
    throw new FullAutoRunTransitionError(
      "illegal_transition",
      `illegal Full Auto run transition ${run.state} -> ${input.to} for ${run.runRef}`,
      run.state,
      input.to,
    )
  }
  const timestamp = now().toISOString()
  const reason = input.reason.slice(0, FULL_AUTO_RUN_REASON_LIMIT)
  const transitionRecord: FullAutoRunTransitionRecord = {
    from: run.state,
    to: input.to,
    actor: input.actor,
    at: timestamp,
    reason,
    ...(input.correlationRef === undefined ? {} : { correlationRef: input.correlationRef }),
  }
  const transitions = [...run.transitions, transitionRecord].slice(-FULL_AUTO_RUN_TRANSITION_HISTORY_LIMIT)
  const timestampPatch: Record<string, string> = {}
  if (input.to === "running") {
    timestampPatch.lastProgressAt = timestamp
    if (run.startedAt === undefined) timestampPatch.startedAt = timestamp
  }
  if (input.to === "paused") timestampPatch.pausedAt = timestamp
  if (input.to === "stopped") timestampPatch.stoppedAt = timestamp
  if (input.to === "completed") timestampPatch.completedAt = timestamp
  return Schema.decodeUnknownSync(FullAutoRunSchema)(compactRunInput({
    ...run,
    ...timestampPatch,
    state: input.to,
    stateRevision: run.stateRevision + 1,
    terminalReason: isFullAutoRunTerminal(input.to) ? reason : run.terminalReason,
    transitions,
    // A settled Pause/Resume/Stop always clears the observational lease
    // mirror -- the underlying thread-level lease is the enforcement point.
    pendingTurnRef: input.to === "paused" || isFullAutoRunTerminal(input.to) ? undefined : run.pendingTurnRef,
    pendingStartedAt: input.to === "paused" || isFullAutoRunTerminal(input.to) ? undefined : run.pendingStartedAt,
  }))
}

export type FullAutoRunCreateInput = Readonly<{
  title: string
  objective: string
  doneCondition: string
  objectiveSource: FullAutoRunObjectiveSource
  workspaceRef?: string
  profile?: FullAutoProfile
  turnCap?: number
  threadRef?: string
  predecessorRunRef?: string
  migratedFrom?: "legacy_registry"
}>

export type FullAutoRunStartResult =
  | Readonly<{ ok: true; run: FullAutoRun }>
  | Readonly<{ ok: false; reason: "active_run_conflict"; activeRunRef: string }>
  | Readonly<{ ok: false; reason: "not_found" }>
  | Readonly<{ ok: false; reason: "illegal_transition"; from: FullAutoRunState }>

export type FullAutoRunTransitionResult =
  | Readonly<{ ok: true; run: FullAutoRun }>
  | Readonly<{ ok: false; reason: "illegal_transition"; from: FullAutoRunState; to: FullAutoRunState }>
  | Readonly<{ ok: false; reason: "not_found" }>

export type FullAutoRunRegistry = Readonly<{
  list: () => ReadonlyArray<FullAutoRun>
  get: (runRef: string) => FullAutoRun | null
  /** The single active (non-terminal, non-draft) run for this profile, or
   * null. v1 concurrency guarantees at most one ever exists. */
  activeRun: () => FullAutoRun | null
  findByThreadRef: (threadRef: string) => FullAutoRun | null
  /** Always succeeds; a draft never competes for the concurrency slot. */
  createDraft: (input: FullAutoRunCreateInput) => FullAutoRun
  /** draft -> running. Refuses with a typed conflict naming the existing
   * active runRef (FA-AC-39) rather than queuing or dispatching in parallel. */
  start: (
    runRef: string,
    options: Readonly<{ actor: FullAutoRunActor; reason: string; threadRef?: string; correlationRef?: string }>,
  ) => FullAutoRunStartResult
  /** createDraft + start in one call -- the control API's bootstrap path. */
  startNew: (
    input: FullAutoRunCreateInput & Readonly<{ actor: FullAutoRunActor; reason: string; correlationRef?: string }>,
  ) => FullAutoRunStartResult
  /** FA-AC-40: rerun a terminal run under a brand-new runRef; the terminal
   * record's fields/state are never mutated. */
  rerun: (
    fromRunRef: string,
    input: FullAutoRunCreateInput & Readonly<{ actor: FullAutoRunActor; reason: string }>,
  ) => FullAutoRunStartResult | Readonly<{ ok: false; reason: "predecessor_not_found" | "predecessor_not_terminal" }>
  transition: (
    runRef: string,
    input: Readonly<{ to: FullAutoRunState; actor: FullAutoRunActor; reason: string; correlationRef?: string }>,
  ) => FullAutoRunTransitionResult
  bindThread: (runRef: string, threadRef: string) => FullAutoRun | null
  /** FA-HO-01 (#8975): rebind the execution profile (provider lane) after a
   * caller-validated provider handoff. This function trusts the caller
   * already performed target admission/auth/capability re-validation and any
   * required state gating (the control-API handoff route requires
   * `paused`) -- it only performs the durable write, mirroring `bindThread`'s
   * "caller already checked" shape. Never invoked directly by a renderer or
   * CLI without going through that gated route. */
  rebindProfile: (runRef: string, profile: FullAutoProfile) => FullAutoRun | null
  /** FA-RUN-03 (#8971): stamp `lastLivenessCheckAt`. A no-op state change --
   * never appends a transition record -- so periodic liveness sweeps can
   * touch every active run cheaply without spamming transition history. */
  touchLiveness: (runRef: string, timestamp: string) => FullAutoRun | null
  recordAttempt: (
    runRef: string,
    outcome: "success" | "failure",
    options?: Readonly<{ turnRef?: string; reason?: string }>,
  ) => FullAutoRun | null
  reviseObjective: (
    runRef: string,
    input: Readonly<{
      objective: string
      doneCondition: string
      source: FullAutoRunObjectiveSource
      actor: FullAutoRunActor
    }>,
  ) => FullAutoRun | null
}>

export const openFullAutoRunRegistry = (
  file: string,
  now: () => Date = () => new Date(),
): FullAutoRunRegistry => {
  const filePath = path.resolve(file)
  let runs = [...decodeFile(filePath, now)]

  /**
   * FA-AC-50: eviction never drops a non-terminal (Running, Pausing,
   * Paused, Retrying, Stalled) run record -- extends FA-AC-12's
   * enabled-record protection to the full run state set. Draft runs are
   * eligible for eviction alongside terminal runs once the bound is
   * exceeded, most-recently-updated first.
   */
  const persist = (): void => {
    const sorted = [...runs].sort((left, right) => right.createdAt.localeCompare(left.createdAt))
    const protectedRuns = sorted.filter(run => isFullAutoRunActive(run.state))
    const evictable = sorted.filter(run => !isFullAutoRunActive(run.state))
    runs = [...protectedRuns, ...evictable.slice(0, Math.max(0, FULL_AUTO_RUN_RECORD_LIMIT - protectedRuns.length))]
    writePrivateAtomic(filePath, { schema: FULL_AUTO_RUN_REGISTRY_SCHEMA, runs })
  }
  const findIndex = (runRef: string): number => runs.findIndex(run => run.runRef === runRef)
  const mintRunRef = (): string => {
    const random = Math.random().toString(36).slice(2, 10)
    return `run.full-auto.${now().getTime().toString(36)}.${random}`
  }

  const list = (): ReadonlyArray<FullAutoRun> => [...runs]
  const get = (runRef: string): FullAutoRun | null => {
    const index = findIndex(runRef)
    return index === -1 ? null : runs[index]!
  }
  const activeRun = (): FullAutoRun | null => runs.find(run => isFullAutoRunActive(run.state)) ?? null
  const findByThreadRef = (threadRef: string): FullAutoRun | null =>
    runs.find(run => run.threadRef === threadRef) ?? null

  const createDraft = (input: FullAutoRunCreateInput): FullAutoRun => {
    const timestamp = now().toISOString()
    const run = Schema.decodeUnknownSync(FullAutoRunSchema)(compactRunInput({
      runRef: mintRunRef(),
      threadRef: input.threadRef,
      title: input.title,
      objective: input.objective,
      objectiveSource: input.objectiveSource,
      doneCondition: input.doneCondition,
      objectiveHistory: [{
        objective: input.objective,
        doneCondition: input.doneCondition,
        source: input.objectiveSource,
        actor: input.objectiveSource === "legacy_migration" ? "legacy_migration" : "control_api",
        at: timestamp,
      }],
      workspaceRef: input.workspaceRef,
      profile: input.profile,
      turnCap: input.turnCap ?? FULL_AUTO_RUN_TURN_CAP_DEFAULT,
      successfulAttempts: 0,
      failedAttempts: 0,
      state: "draft" as const,
      stateRevision: 0,
      predecessorRunRef: input.predecessorRunRef,
      migratedFrom: input.migratedFrom,
      createdAt: timestamp,
      transitions: [],
    }))
    runs.push(run)
    persist()
    return run
  }

  const transitionInternal = (
    runRef: string,
    input: Readonly<{ to: FullAutoRunState; actor: FullAutoRunActor; reason: string; correlationRef?: string }>,
  ): FullAutoRunTransitionResult => {
    const index = findIndex(runRef)
    if (index === -1) return { ok: false, reason: "not_found" }
    const current = runs[index]!
    try {
      const next = applyFullAutoRunTransition(current, input, now)
      runs[index] = next
      persist()
      return { ok: true, run: next }
    } catch (error) {
      if (error instanceof FullAutoRunTransitionError) {
        return { ok: false, reason: "illegal_transition", from: error.from, to: error.to }
      }
      throw error
    }
  }

  const start: FullAutoRunRegistry["start"] = (runRef, options) => {
    const existingActive = activeRun()
    if (existingActive !== null && existingActive.runRef !== runRef) {
      return { ok: false, reason: "active_run_conflict", activeRunRef: existingActive.runRef }
    }
    const index = findIndex(runRef)
    if (index === -1) return { ok: false, reason: "not_found" }
    if (options.threadRef !== undefined && runs[index]!.threadRef === undefined) {
      runs[index] = Schema.decodeUnknownSync(FullAutoRunSchema)({ ...runs[index]!, threadRef: options.threadRef })
    }
    const result = transitionInternal(runRef, {
      to: "running",
      actor: options.actor,
      reason: options.reason,
      correlationRef: options.correlationRef,
    })
    if (result.ok) return result
    if (result.reason === "illegal_transition") return { ok: false, reason: "illegal_transition", from: result.from }
    return result
  }

  const startNew: FullAutoRunRegistry["startNew"] = input => {
    const existingActive = activeRun()
    if (existingActive !== null) return { ok: false, reason: "active_run_conflict", activeRunRef: existingActive.runRef }
    const draft = createDraft(input)
    return start(draft.runRef, { actor: input.actor, reason: input.reason, correlationRef: input.correlationRef })
  }

  const rerun: FullAutoRunRegistry["rerun"] = (fromRunRef, input) => {
    const predecessor = get(fromRunRef)
    if (predecessor === null) return { ok: false, reason: "predecessor_not_found" }
    if (!isFullAutoRunTerminal(predecessor.state)) return { ok: false, reason: "predecessor_not_terminal" }
    return startNew({ ...input, predecessorRunRef: fromRunRef })
  }

  const bindThread: FullAutoRunRegistry["bindThread"] = (runRef, threadRef) => {
    const index = findIndex(runRef)
    if (index === -1) return null
    if (runs[index]!.threadRef !== undefined) return runs[index]!
    runs[index] = Schema.decodeUnknownSync(FullAutoRunSchema)({ ...runs[index]!, threadRef })
    persist()
    return runs[index]!
  }

  const rebindProfile: FullAutoRunRegistry["rebindProfile"] = (runRef, profile) => {
    const index = findIndex(runRef)
    if (index === -1) return null
    runs[index] = Schema.decodeUnknownSync(FullAutoRunSchema)({ ...runs[index]!, profile })
    persist()
    return runs[index]!
  }

  const touchLiveness: FullAutoRunRegistry["touchLiveness"] = (runRef, timestamp) => {
    const index = findIndex(runRef)
    if (index === -1) return null
    const next = Schema.decodeUnknownSync(FullAutoRunSchema)({ ...runs[index]!, lastLivenessCheckAt: timestamp })
    runs[index] = next
    persist()
    return next
  }

  const recordAttempt: FullAutoRunRegistry["recordAttempt"] = (runRef, outcome, options) => {
    const index = findIndex(runRef)
    if (index === -1) return null
    const current = runs[index]!
    const timestamp = now().toISOString()
    const next = Schema.decodeUnknownSync(FullAutoRunSchema)(compactRunInput({
      ...current,
      successfulAttempts: outcome === "success" ? current.successfulAttempts + 1 : current.successfulAttempts,
      failedAttempts: outcome === "failure" ? current.failedAttempts + 1 : current.failedAttempts,
      consecutiveFailures: outcome === "success" ? undefined : (current.consecutiveFailures ?? 0) + 1,
      lastFailureAt: outcome === "success" ? undefined : timestamp,
      lastProgressAt: outcome === "success" ? timestamp : current.lastProgressAt,
      pendingTurnRef: undefined,
      pendingStartedAt: undefined,
    }))
    runs[index] = next
    persist()
    return next
  }

  const reviseObjective: FullAutoRunRegistry["reviseObjective"] = (runRef, input) => {
    const index = findIndex(runRef)
    if (index === -1) return null
    const current = runs[index]!
    const timestamp = now().toISOString()
    const history = [
      ...current.objectiveHistory,
      { objective: input.objective, doneCondition: input.doneCondition, source: input.source, actor: input.actor, at: timestamp },
    ].slice(-FULL_AUTO_RUN_OBJECTIVE_HISTORY_LIMIT)
    const next = Schema.decodeUnknownSync(FullAutoRunSchema)({
      ...current,
      objective: input.objective,
      doneCondition: input.doneCondition,
      objectiveSource: input.source,
      objectiveHistory: history,
    })
    runs[index] = next
    persist()
    return next
  }

  return {
    list,
    get,
    activeRun,
    findByThreadRef,
    createDraft,
    start,
    startNew,
    rerun,
    transition: transitionInternal,
    bindThread,
    rebindProfile,
    touchLiveness,
    recordAttempt,
    reviseObjective,
  }
}

// -----------------------------------------------------------------------
// FA-AC-41: additive, idempotent migration from the legacy per-thread
// `enabled: boolean` registry.
// -----------------------------------------------------------------------

/** The exact previous generic Full Auto instruction -- migrated verbatim as
 * a visibly marked `legacy_migration` objective. Never an invented
 * user-authored goal (see the issue's migration requirements). */
export const FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE =
  "Continue Full Auto: look at this repository (README, docs, open issues) and do the next concrete useful thing."
export const FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION =
  "(legacy migration: no explicit done condition was recorded before the FullAutoRun model shipped; " +
  "this run continues under its original continuation cap until an owner reviews, Pauses, or Stops it.)"
export const FULL_AUTO_LEGACY_MIGRATION_TITLE = "Legacy Full Auto run"

export type FullAutoRunMigrationOutcome = Readonly<{
  /** Newly created runs that successfully entered Running this pass. */
  migrated: ReadonlyArray<FullAutoRun>
  /** Legacy rows with `enabled: false` -- historical/inactive, never
   * migrated to an active run merely because Desktop relaunched. */
  skippedDisabled: ReadonlyArray<string>
  /** A prior startup already migrated this exact threadRef -- idempotent
   * no-op, not a data-loss condition. */
  skippedAlreadyMigrated: ReadonlyArray<string>
  /** The v1 one-active-run-per-profile policy refused a second concurrent
   * legacy row from starting. Nothing is lost: a Draft run preserving every
   * field (workspace/profile/counters) was created for owner review, per
   * the migration requirement that data is "preserved or rejected with a
   * typed, owner-visible migration outcome." */
  preservedAsDraft: ReadonlyArray<Readonly<{ threadRef: string; runRef: string }>>
}>

export const migrateLegacyFullAutoRegistry = (input: Readonly<{
  legacyRecords: ReadonlyArray<FullAutoRecord>
  runRegistry: FullAutoRunRegistry
}>): FullAutoRunMigrationOutcome => {
  const migrated: FullAutoRun[] = []
  const skippedDisabled: string[] = []
  const skippedAlreadyMigrated: string[] = []
  const preservedAsDraft: Array<Readonly<{ threadRef: string; runRef: string }>> = []

  const alreadyMigratedThreadRefs = new Set(
    input.runRegistry.list()
      .filter(run => run.migratedFrom === "legacy_registry" && run.threadRef !== undefined)
      .map(run => run.threadRef!),
  )

  for (const legacy of input.legacyRecords) {
    if (!legacy.enabled) {
      skippedDisabled.push(legacy.threadRef)
      continue
    }
    if (alreadyMigratedThreadRefs.has(legacy.threadRef)) {
      skippedAlreadyMigrated.push(legacy.threadRef)
      continue
    }
    const createInput: FullAutoRunCreateInput = {
      title: FULL_AUTO_LEGACY_MIGRATION_TITLE,
      objective: FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE,
      doneCondition: FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION,
      objectiveSource: "legacy_migration",
      threadRef: legacy.threadRef,
      workspaceRef: legacy.workspaceRef,
      profile: legacy.profile,
      turnCap: FULL_AUTO_RUN_TURN_CAP_DEFAULT,
      migratedFrom: "legacy_registry",
    }
    const result = input.runRegistry.startNew({
      ...createInput,
      actor: "legacy_migration",
      reason: `migrated from the legacy enabled registry row for threadRef ${legacy.threadRef}`,
    })
    if (result.ok) {
      migrated.push(result.run)
      alreadyMigratedThreadRefs.add(legacy.threadRef)
      continue
    }
    // The v1 one-active-run policy is a genuine, new behavior change versus
    // the legacy model (which had no such limit). Preserve every field as a
    // Draft rather than silently dropping it -- an owner can review and
    // manually Start it once the currently active run finishes.
    const draft = input.runRegistry.createDraft(createInput)
    preservedAsDraft.push({ threadRef: legacy.threadRef, runRef: draft.runRef })
    alreadyMigratedThreadRefs.add(legacy.threadRef)
  }

  return { migrated, skippedDisabled, skippedAlreadyMigrated, preservedAsDraft }
}

// -----------------------------------------------------------------------
// Missing/orphaned thread and provider-session recovery -- a minimal typed
// fail-closed disposition (FA-AC-42's #8969-owned stub). Full liveness/SLO
// classification, retry-ETA, and owner-actionable recovery affordances are
// FA-RUN-03 (#8971)'s job; this function only guarantees the run NEVER
// silently reattaches to an unrelated thread or sits in silent Running
// forever once its bound thread record disappears or was disabled by a
// policy this module already understands.
// -----------------------------------------------------------------------

export type FullAutoRunThreadSnapshot = Readonly<{
  threadRecord: FullAutoRecord | null
  /** Whether the underlying provider turn is genuinely executing right now. */
  turnRunning: boolean
}>

export const settleFullAutoRunFromThreadState = (
  runRegistry: FullAutoRunRegistry,
  run: FullAutoRun,
  snapshot: FullAutoRunThreadSnapshot,
): FullAutoRun => {
  if (isFullAutoRunTerminal(run.state)) return run

  if (run.state === "pausing") {
    if (!snapshot.turnRunning) {
      const result = runRegistry.transition(run.runRef, {
        to: "paused",
        actor: "turn_resolution",
        reason: "the background turn resolved after Pause was requested",
      })
      return result.ok ? result.run : run
    }
    return run
  }

  if (run.state !== "running" && run.state !== "retrying") return run

  if (run.threadRef !== undefined && snapshot.threadRecord === null) {
    const result = runRegistry.transition(run.runRef, {
      to: "stalled",
      actor: "thread_state_sync",
      reason: `the bound thread record for ${run.threadRef} is missing at reconciliation`,
    })
    return result.ok ? result.run : run
  }

  if (snapshot.threadRecord !== null && !snapshot.threadRecord.enabled) {
    const disabledBy = snapshot.threadRecord.disabledBy
    const blockedReason = snapshot.threadRecord.blockedReason
    if (disabledBy === "continuation_cap") {
      const result = runRegistry.transition(run.runRef, {
        to: "cap_reached",
        actor: "continuation_cap",
        reason: blockedReason ?? "continuation cap reached",
      })
      return result.ok ? result.run : run
    }
    if (disabledBy === "dispatch_failure_limit") {
      const result = runRegistry.transition(run.runRef, {
        to: "failed",
        actor: "dispatch_failure_limit",
        reason: blockedReason ?? "dispatch failure limit reached",
      })
      return result.ok ? result.run : run
    }
    if (disabledBy === "workspace_guard") {
      const result = runRegistry.transition(run.runRef, {
        to: "failed",
        actor: "workspace_guard",
        reason: blockedReason ?? "workspace binding failed",
      })
      return result.ok ? result.run : run
    }
    // FA-GD-01 (#8991): a typed guardrail termination is a deliberate,
    // owner-configured bound being met -- Stopped (with the guardrail's
    // typed blockedReason), not Failed and not the defensive stall below.
    if (disabledBy === "guardrail") {
      const result = runRegistry.transition(run.runRef, {
        to: "stopped",
        actor: "guardrail",
        reason: blockedReason ?? "guardrail limit reached",
      })
      return result.ok ? result.run : run
    }
    // Defensive: the underlying thread was disabled by something this
    // module does not attribute to a specific run-level state (e.g. an
    // external ui_toggle/control_api disable that bypassed the run-level
    // Pause/Stop routes). Never leave the run silently claiming Running.
    const result = runRegistry.transition(run.runRef, {
      to: "stalled",
      actor: "thread_state_sync",
      reason: `the bound thread was disabled unexpectedly (disabledBy=${disabledBy ?? "unknown"})`,
    })
    return result.ok ? result.run : run
  }

  return run
}
