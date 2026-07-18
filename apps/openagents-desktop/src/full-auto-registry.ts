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

/**
 * Full Auto (#8853): durable per-thread continuation state, owned by main.
 * This is the piece that makes Full Auto survive an app restart -- the
 * renderer no longer decides whether to continue; it only tells main "this
 * thread is enabled" (or not), and main re-evaluates that durable fact both
 * after every completed turn and again at startup, exactly the way
 * local-turn-journal.ts already owns interrupted-turn recovery truth.
 *
 * Wave 2 (FA-H2 #8875, FA-H3 #8876, FA-H5 #8878, FA-H6 #8879) extends the
 * record from an enabled-only flag toward the roadmap's durable next-turn
 * record: granted-workspace identity, bound execution profile, a durable
 * dispatch lease, and typed failure/backoff state. Every new field is
 * OPTIONAL so an existing v1 registry file still decodes -- the FA-H10
 * quarantine path must never eat a user's state because of this upgrade.
 */
export const FULL_AUTO_REGISTRY_SCHEMA = "openagents.desktop.full_auto_registry.v1" as const
export const FULL_AUTO_RECORD_LIMIT = 128
export const FULL_AUTO_BLOCKED_REASON_LIMIT = 300
/** FA-RT-01 (#8987): bound on the ordered routing-policy candidate list. */
export const FULL_AUTO_ROUTING_POLICY_LIMIT = 8
/** FA-RT-01 (#8987): bound on the durable rotation history (oldest evicted). */
export const FULL_AUTO_ROTATION_HISTORY_LIMIT = 20
/** FA-GD-01 (#8991): bound on the durable per-continuation decision history
 * (oldest evicted, same discipline as rotationHistory). */
export const FULL_AUTO_DECISION_HISTORY_LIMIT = 40

/** #8928: durable provenance for every transition to disabled. Older rows
 * legitimately omit this additive field; every current disable path supplies
 * one so an operator never has to infer whether a stop came from UI, API, or
 * a safety policy. */
export const FullAutoDisabledBySchema = Schema.Literals([
  "ui_toggle",
  "control_api",
  "workspace_guard",
  "continuation_cap",
  "dispatch_failure_limit",
  /** FA-GD-01 (#8991): a typed owner-configured guardrail (max wall clock,
   * max turns) terminated the loop. The generalized failure budget keeps the
   * existing dispatch_failure_limit attribution -- same failure class. */
  "guardrail",
  /** MOB-FA-02 (#8994): a typed Pause/Resume/Stop intent dispatched from
   * OpenAgents mobile and applied by Desktop's control-intent consumer. */
  "mobile",
])
export type FullAutoDisabledBy = typeof FullAutoDisabledBySchema.Type

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Cursor = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

/**
 * FA-H6 (#8879): the execution profile the loop was granted with. Bound from
 * the initiating (renderer-sent) Full Auto turn; continuations replay it so a
 * restart-resumed loop runs on the same account/model/effort the owner
 * started it with. Fields are durable plain strings (revalidated against the
 * live contract enums at dispatch time) so a future enum change can never
 * corrupt-fail the whole registry file.
 */
/**
 * FA-RT-01 (#8987): one admitted routing candidate — an existing ProviderLane
 * ref plus an optional account ref on that lane. Candidates are validated
 * (unknown/unadmitted/Full-Auto-ineligible lanes fail closed) at policy BIND
 * time by full-auto-routing.ts, never trusted at decode time: a durable row
 * whose lane later lost admission simply fails its dispatch attempt typed.
 */
export const FullAutoRoutingCandidateSchema = Schema.Struct({
  lane: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  accountRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
})
export type FullAutoRoutingCandidate = typeof FullAutoRoutingCandidateSchema.Type

/**
 * FA-RT-01 (#8987): the typed dispatch-failure classes that permit rotating
 * to the next admitted candidate in the SAME reconciliation pass instead of
 * consuming FA-H5 failure budget. Every other failure keeps the existing
 * budget/backoff/disable semantics unchanged.
 */
export const FullAutoRotationReasonSchema = Schema.Literals([
  "account_exhausted",
  "rate_limited",
  "provider_error",
])
export type FullAutoRotationReason = typeof FullAutoRotationReasonSchema.Type

/**
 * FA-RT-01 (#8987): one durable rotation fact. Public-safe by construction —
 * lane refs, a typed reason, and an ISO timestamp only; never prompts,
 * models, paths, tokens, or raw provider detail.
 */
export const FullAutoRotationRecordSchema = Schema.Struct({
  fromLane: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  toLane: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  reason: FullAutoRotationReasonSchema,
  at: Schema.String,
})
export type FullAutoRotationRecord = typeof FullAutoRotationRecordSchema.Type

/** FA-RT-01 (#8987): the bind-side bounded policy shape. The RECORD field
 * below deliberately carries no length checks (same FA-H10 rationale as the
 * record bound itself: decode must never quarantine a legitimately written
 * file); bounds are enforced write-side by `bindRoutingPolicy`. */
export const FullAutoRoutingPolicySchema = Schema.Array(FullAutoRoutingCandidateSchema).check(
  Schema.isMinLength(1),
  Schema.isMaxLength(FULL_AUTO_ROUTING_POLICY_LIMIT),
)

const PositiveCount = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

/**
 * FA-GD-01 (#8991): the OWNER-CONFIGURABLE guardrail set. Every field is
 * optional; an absent field means "the built-in default applies" (the 20-turn
 * cap and 5-consecutive-failure disable keep their exact existing semantics
 * when the corresponding field is absent -- see full-auto-reconcile.ts).
 *
 * This schema is deliberately the COMPLETE configurable surface. The
 * non-overridable core guardrails (workspace binding, own-capacity-only
 * dispatch, no rate-limit-reset triggering -- see
 * FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS in full-auto-reconcile.ts) have NO
 * field here on purpose: there is structurally nothing to write, in config or
 * env, that relaxes them. Unknown keys on a durable/hand-edited guardrails
 * object are dropped at decode and can never reach enforcement.
 */
export const FullAutoGuardrailsSchema = Schema.Struct({
  /** Hard wall-clock bound for the whole enabled span, measured from the
   * record's durable `enabledAt` anchor. */
  maxWallClockMs: Schema.optional(PositiveCount),
  /** Generalizes the FULL_AUTO_MAX_CONTINUATIONS (20) cap. Absent = the
   * existing cap semantics, byte-for-byte. */
  maxTurns: Schema.optional(PositiveCount),
  /** Generalizes FULL_AUTO_MAX_CONSECUTIVE_FAILURES (5). Absent = existing
   * failure-budget semantics unchanged. */
  maxPerTurnFailures: Schema.optional(PositiveCount),
  /** Durable pointer to an external token/spend budget. Desktop has no local
   * token-usage source to enforce against yet (the run report's usage block
   * is honestly `unknown`), so this is carried as an owner-visible ref for a
   * future enforcer -- stored, surfaced, never fabricated into enforcement. */
  tokenBudgetRef: Schema.optional(Ref),
})
export type FullAutoGuardrails = typeof FullAutoGuardrailsSchema.Type

/**
 * FA-GD-01 (#8991): the typed per-continuation decision vocabulary. Exactly
 * one durable decision fact is appended per between-turn continuation
 * decision the reconciler takes (continue on success, rotate on a typed
 * same-pass failover, pause_low_confidence on the no-progress detector,
 * stop_guardrail on any guardrail/cap termination).
 */
export const FullAutoContinuationDecisionKindSchema = Schema.Literals([
  "continue",
  "rotate",
  "pause_low_confidence",
  "stop_guardrail",
])
export type FullAutoContinuationDecisionKind = typeof FullAutoContinuationDecisionKindSchema.Type

export const FullAutoContinuationDecisionSchema = Schema.Struct({
  at: Schema.String,
  decision: FullAutoContinuationDecisionKindSchema,
  reason: Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FULL_AUTO_BLOCKED_REASON_LIMIT),
  ),
  /** Turn-budget slots remaining AFTER this decision, against the effective
   * (guardrail-or-default) turn cap. Absent when not meaningful. */
  budgetRemaining: Schema.optional(Cursor),
  /** Optional pointer to the governing goal/run (e.g. a FullAutoRun runRef).
   * The thread registry itself has no goal source; the reconciler leaves it
   * absent rather than inventing one. */
  goalRef: Schema.optional(Ref),
})
export type FullAutoContinuationDecision = typeof FullAutoContinuationDecisionSchema.Type

/** FA-GD-01 (#8991): who resumed a low-confidence pause. Resume is always an
 * explicit command by one of the owner-facing surfaces, never the loop. */
export const FullAutoResumeActorSchema = Schema.Literals([
  "owner_ui",
  "control_api",
  "cli",
  "mcp",
])
export type FullAutoResumeActor = typeof FullAutoResumeActorSchema.Type

export const FullAutoProfileSchema = Schema.Struct({
  /** L6 #8901: durable ProviderLane.laneRef. Optional keeps every rev-7 row decodable. */
  lane: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  accountRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  model: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  reasoningEffort: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40))),
})
export type FullAutoProfile = typeof FullAutoProfileSchema.Type

export const FullAutoRecordSchema = Schema.Struct({
  threadRef: Ref,
  enabled: Schema.Boolean,
  /**
   * Consecutive auto-dispatched continuations since Full Auto was last enabled
   * for this thread (toggling off resets it). A manual send while the toggle
   * stays on does NOT reset the count -- see FA-H7 (#8880) and the pinning
   * test in tests/full-auto-registry.test.ts. Since FA-H5 (#8878) the count
   * increments only on a SUCCESSFUL dispatch: a failed dispatch never consumes
   * a cap slot (it consumes failure/backoff budget instead).
   */
  continuationCount: Cursor,
  updatedAt: Schema.String,
  /**
   * FA-H2 (#8875): the absolute workspace path granted when Full Auto was
   * enabled. Dispatch refuses (and disables the record) when the currently
   * resolved workspace differs; a record with no workspaceRef fails closed.
   */
  workspaceRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))),
  /** FA-H6 (#8879): execution profile bound by the initiating flagged turn. */
  profile: Schema.optional(FullAutoProfileSchema),
  /**
   * FA-H3 (#8876): the durable dispatch lease. While a continuation turn ref
   * is claimed here, no other reconcile pass may dispatch this thread. Cleared
   * on dispatch completion (success or failure); a stale lease whose turn ref
   * never reached the local-turn journal is cleared at startup reconciliation.
   */
  pendingTurnRef: Schema.optional(Schema.NullOr(Ref)),
  pendingStartedAt: Schema.optional(Schema.String),
  /** FA-H5 (#8878): failure/backoff state. */
  lastFailureAt: Schema.optional(Schema.String),
  consecutiveFailures: Schema.optional(Cursor),
  /** FA-H2/FA-H5: typed, owner-visible reason the loop is blocked/disabled. */
  blockedReason: Schema.optional(Schema.NullOr(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FULL_AUTO_BLOCKED_REASON_LIMIT),
  ))),
  disabledBy: Schema.optional(FullAutoDisabledBySchema),
  disabledAt: Schema.optional(Schema.String),
  /**
   * FA-RT-01 (#8987): ordered multi-lane routing policy. OPTIONAL and
   * additive: a v1/v2-era registry file without it decodes and behaves
   * exactly as single-lane (the bound `profile` alone). When present and
   * non-empty, reconciliation may rotate through these candidates on a typed
   * account_exhausted/rate_limited/provider_error dispatch failure.
   */
  routingPolicy: Schema.optional(Schema.Array(FullAutoRoutingCandidateSchema)),
  /**
   * FA-RT-01 (#8987): bounded rotation history, most recent last. Capped at
   * FULL_AUTO_ROTATION_HISTORY_LIMIT write-side (oldest evicted); decode
   * carries no cap for the same never-quarantine reason as the record bound.
   */
  rotationHistory: Schema.optional(Schema.Array(FullAutoRotationRecordSchema)),
  /**
   * FA-GD-01 (#8991): when the record was last granted (transitioned
   * disabled -> enabled, or created enabled). The durable anchor for the
   * maxWallClockMs guardrail and the low-confidence no-progress detector.
   * Optional so every pre-#8991 row still decodes.
   */
  enabledAt: Schema.optional(Schema.String),
  /** FA-GD-01 (#8991): owner-configurable guardrails; see the schema doc for
   * why the non-overridable core set has no field here. */
  guardrails: Schema.optional(FullAutoGuardrailsSchema),
  /**
   * FA-GD-01 (#8991): the durable low-confidence pause. A paused record stays
   * `enabled: true` (it keeps eviction protection and the owner's grant) but
   * reconciliation never dispatches it until an explicit resume clears these
   * fields. `pausedReason` present <=> the record is paused.
   */
  pausedReason: Schema.optional(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FULL_AUTO_BLOCKED_REASON_LIMIT),
  )),
  pausedAt: Schema.optional(Schema.String),
  /** FA-GD-01 (#8991): stamped by resume; the no-progress detector anchors
   * here (falling back to enabledAt) so pre-resume evidence can never
   * immediately re-pause a just-resumed loop. */
  lastResumedAt: Schema.optional(Schema.String),
  resumedBy: Schema.optional(FullAutoResumeActorSchema),
  /**
   * FA-GD-01 (#8991): bounded per-continuation decision history, most recent
   * last. Capped at FULL_AUTO_DECISION_HISTORY_LIMIT write-side (oldest
   * evicted); decode carries no cap for the same never-quarantine reason as
   * the record bound.
   */
  decisionHistory: Schema.optional(Schema.Array(FullAutoContinuationDecisionSchema)),
})
export type FullAutoRecord = typeof FullAutoRecordSchema.Type

/**
 * FA-RT-01 (#8987): the public-safe projection of a record's rotation
 * history for the control-API status/turns surfaces. Explicit field-by-field
 * mapping so nothing beyond lane refs, the typed reason, and the timestamp
 * can ever ride along, and the projection is bounded even against a
 * hand-edited over-long durable file.
 */
export const projectFullAutoRotationHistory = (
  record: FullAutoRecord,
): ReadonlyArray<FullAutoRotationRecord> =>
  (record.rotationHistory ?? []).slice(-FULL_AUTO_ROTATION_HISTORY_LIMIT).map(entry => ({
    fromLane: entry.fromLane,
    toLane: entry.toLane,
    reason: entry.reason,
    at: entry.at,
  }))

/**
 * FA-GD-01 (#8991): the public-safe projection of a record's continuation
 * decision history, mirroring projectFullAutoRotationHistory's explicit
 * field-by-field discipline. Exported as the seam for the control-API
 * status/turns surfaces to wire (a follow-up; this issue touches no control
 * server files).
 */
export const projectFullAutoDecisionHistory = (
  record: FullAutoRecord,
): ReadonlyArray<FullAutoContinuationDecision> =>
  (record.decisionHistory ?? []).slice(-FULL_AUTO_DECISION_HISTORY_LIMIT).map(entry => ({
    at: entry.at,
    decision: entry.decision,
    reason: entry.reason,
    ...(entry.budgetRemaining === undefined ? {} : { budgetRemaining: entry.budgetRemaining }),
    ...(entry.goalRef === undefined ? {} : { goalRef: entry.goalRef }),
  }))

/**
 * The record bound (FULL_AUTO_RECORD_LIMIT) is enforced write-side and applies
 * only to the disabled tail (FA-H10 #8883): enabled records are never evicted,
 * so a legitimately persisted file may exceed the limit when more than
 * FULL_AUTO_RECORD_LIMIT threads are enabled at once. The decode schema
 * therefore carries no max-length check.
 */
const FullAutoRegistryFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_REGISTRY_SCHEMA),
  records: Schema.Array(FullAutoRecordSchema),
})

export type FullAutoSetOptions = Readonly<{
  /** FA-H2: bind the granted workspace at enable time. */
  workspaceRef?: string
  /** FA-H6: bind the execution profile at enable time. */
  profile?: FullAutoProfile
  /** FA-RT-01 (#8987): bind the validated routing policy at enable time.
   * Callers MUST validate through full-auto-routing.ts first; this option is
   * durable plumbing, not the admission gate. */
  routingPolicy?: ReadonlyArray<FullAutoRoutingCandidate>
  /** FA-GD-01 (#8991): bind the owner-configured guardrails at enable time.
   * Like routingPolicy, this option only applies on ENABLE; guardrails
   * otherwise survive transitions durably. */
  guardrails?: FullAutoGuardrails
  /** FA-H2/FA-H5: typed reason recorded when DISABLING a record. */
  blockedReason?: string
  /** #8928: required by every current call that transitions to disabled. */
  disabledBy?: FullAutoDisabledBy
}>

export type FullAutoRegistry = Readonly<{
  list: () => ReadonlyArray<FullAutoRecord>
  get: (threadRef: string) => boolean
  record: (threadRef: string) => FullAutoRecord | null
  enabledThreads: () => ReadonlyArray<string>
  set: (threadRef: string, enabled: boolean, options?: FullAutoSetOptions) => FullAutoRecord
  incrementContinuation: (threadRef: string) => number
  /**
   * FA-H3: claim the durable dispatch lease with the continuation's exact
   * turn ref. Returns false (no write) when the record is missing or another
   * lease is already held -- the caller must then skip the thread.
   */
  claimPending: (threadRef: string, turnRef: string) => boolean
  clearPending: (threadRef: string) => void
  /**
   * FA-H5: record a failed dispatch -- increments consecutiveFailures, stamps
   * lastFailureAt, sets blockedReason, and releases the lease. Returns the new
   * consecutive-failure count (0 when the record is missing).
   */
  recordFailure: (threadRef: string, reason: string) => number
  /** FA-H5: a successful dispatch clears failure state and the lease. */
  recordSuccess: (threadRef: string) => void
  bindWorkspace: (threadRef: string, workspaceRef: string) => FullAutoRecord | null
  bindProfile: (threadRef: string, profile: FullAutoProfile) => FullAutoRecord | null
  /**
   * FA-RT-01 (#8987): bind (or, with null, clear) the ordered routing
   * policy. Bounds are enforced fail-closed here (1..FULL_AUTO_ROUTING_
   * POLICY_LIMIT candidates); admission validation is the caller's duty via
   * full-auto-routing.ts BEFORE binding. Missing record is a null no-op.
   */
  bindRoutingPolicy: (
    threadRef: string,
    policy: ReadonlyArray<FullAutoRoutingCandidate> | null,
  ) => FullAutoRecord | null
  /**
   * FA-RT-01 (#8987): append one typed rotation fact (stamped with now()),
   * evicting the oldest entry beyond FULL_AUTO_ROTATION_HISTORY_LIMIT.
   * Missing record is a null no-op.
   */
  recordRotation: (
    threadRef: string,
    rotation: Readonly<{ fromLane: string; toLane: string; reason: FullAutoRotationReason }>,
  ) => FullAutoRecord | null
  /**
   * FA-GD-01 (#8991): bind (or, with null, clear) the owner-configurable
   * guardrails. Invalid shapes (non-positive limits, unknown keys are
   * stripped by decode) fail closed here. Missing record is a null no-op.
   */
  bindGuardrails: (
    threadRef: string,
    guardrails: FullAutoGuardrails | null,
  ) => FullAutoRecord | null
  /**
   * FA-GD-01 (#8991): append one typed continuation decision fact (stamped
   * with now()), evicting the oldest beyond FULL_AUTO_DECISION_HISTORY_LIMIT.
   * The reason is truncated to the blocked-reason bound. Missing record is a
   * null no-op.
   */
  recordDecision: (
    threadRef: string,
    decision: Readonly<{
      decision: FullAutoContinuationDecisionKind
      reason: string
      budgetRemaining?: number
      goalRef?: string
    }>,
  ) => FullAutoRecord | null
  /**
   * FA-GD-01 (#8991): transition an ENABLED record to the durable
   * low-confidence paused state (pausedReason + pausedAt), releasing any
   * held dispatch lease. Null no-op when the record is missing, disabled, or
   * already paused -- pausing never invents or re-stamps state.
   */
  pause: (threadRef: string, reason: string) => FullAutoRecord | null
  /**
   * FA-GD-01 (#8991): the explicit resume command. Clears the paused fields,
   * stamps lastResumedAt/resumedBy, and returns the record. Null no-op when
   * the record is missing or not currently paused -- resume can never be
   * used to re-enable a disabled record or to touch a healthy one.
   */
  resume: (threadRef: string, actor: FullAutoResumeActor) => FullAutoRecord | null
}>

export class FullAutoRegistryError extends Error {
  readonly _tag = "FullAutoRegistryError"
  override readonly name = "FullAutoRegistryError"

  constructor(
    readonly reason: "storage_unavailable" | "missing_disable_attribution",
    message: string,
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
    throw new FullAutoRegistryError(
      "storage_unavailable",
      error instanceof Error ? error.message : "full auto registry unavailable",
    )
  }
}

/**
 * FA-H10 (#8883): a corrupt or schema-invalid registry file must never block
 * app initialization -- Full Auto is a non-critical automation preference, so
 * decode failure fails closed for the feature (empty registry, nothing
 * enabled) and open for the app. The bad file is quarantined beside the
 * registry (best-effort) so the evidence survives for diagnosis instead of
 * being silently overwritten by the next persist.
 */
const decodeFile = (filePath: string, now: () => Date): ReadonlyArray<FullAutoRecord> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = Schema.decodeUnknownSync(FullAutoRegistryFileSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    )
    return decoded.records
  } catch (error) {
    const quarantinePath = `${filePath}.quarantined-${now().toISOString()}`
    try {
      renameSync(filePath, quarantinePath)
      console.error(
        `full auto registry failed validation; quarantined the corrupt file at ${quarantinePath} and starting with an empty registry (Full Auto disabled for all threads)`,
        error,
      )
    } catch {
      console.error(
        `full auto registry failed validation and the corrupt file at ${filePath} could not be quarantined; starting with an empty registry (Full Auto disabled for all threads)`,
        error,
      )
    }
    return []
  }
}

/** Drop `undefined`-valued keys so exact-optional record decodes stay clean
 * and cleared fields disappear from the durable file instead of persisting
 * as explicit nulls forever. */
const compactRecordInput = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(Object.entries(value).filter(([, entry]) => entry !== undefined))

export const openFullAutoRegistry = (file: string, now: () => Date = () => new Date()): FullAutoRegistry => {
  const filePath = path.resolve(file)
  let records = [...decodeFile(filePath, now)]

  /**
   * FA-H10 (#8883): eviction never drops an `enabled: true` record -- an
   * owner-enabled thread must survive to the next restart no matter how many
   * other records were touched more recently. Only the disabled tail is
   * bounded: all enabled records are kept, then remaining capacity is filled
   * with the most-recently-updated disabled records.
   */
  const persist = (): void => {
    const sorted = [...records].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    const enabled = sorted.filter(record => record.enabled)
    const disabled = sorted.filter(record => !record.enabled)
    records = [...enabled, ...disabled.slice(0, Math.max(0, FULL_AUTO_RECORD_LIMIT - enabled.length))]
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    writePrivateAtomic(filePath, { schema: FULL_AUTO_REGISTRY_SCHEMA, records })
  }
  const findIndex = (threadRef: string): number => records.findIndex(record => record.threadRef === threadRef)

  const update = (index: number, patch: Record<string, unknown>): FullAutoRecord => {
    const next = Schema.decodeUnknownSync(FullAutoRecordSchema)(
      compactRecordInput({ ...records[index]!, ...patch, updatedAt: now().toISOString() }),
    )
    records[index] = next
    persist()
    return next
  }

  return {
    list: () => [...records],
    get: threadRef => {
      const index = findIndex(threadRef)
      return index === -1 ? false : records[index]!.enabled
    },
    record: threadRef => {
      const index = findIndex(threadRef)
      return index === -1 ? null : records[index]!
    },
    enabledThreads: () => records.filter(record => record.enabled).map(record => record.threadRef),
    set: (threadRef, enabled, options) => {
      if (!enabled && options?.disabledBy === undefined) {
        throw new FullAutoRegistryError(
          "missing_disable_attribution",
          "refusing to disable Full Auto without durable disable attribution",
        )
      }
      const index = findIndex(threadRef)
      const existing = index === -1 ? null : records[index]!
      const timestamp = now().toISOString()
      // Enabling is a fresh grant: failure/backoff state clears so a
      // previously blocked loop the owner re-enables can run immediately.
      // Disabling zeroes the cap counter (FA-H7 pinned semantic), releases
      // any lease, and records the typed blockedReason when the disable was
      // a policy stop (workspace mismatch, failure limit, cap) rather than
      // an owner toggle-off (which passes attribution but no blocked reason).
      const next = Schema.decodeUnknownSync(FullAutoRecordSchema)(compactRecordInput({
        threadRef,
        enabled,
        continuationCount: existing === null || !enabled ? 0 : existing.continuationCount,
        updatedAt: timestamp,
        workspaceRef: options?.workspaceRef ?? existing?.workspaceRef,
        profile: options?.profile ?? existing?.profile,
        // FA-RT-01 (#8987): the routing policy and rotation history survive
        // enable/disable transitions -- the policy is a durable grant like
        // workspaceRef, and the history is diagnosis evidence like
        // blockedReason. An enable-time routingPolicy option rebinds it.
        routingPolicy: (enabled ? options?.routingPolicy : undefined) ?? existing?.routingPolicy,
        rotationHistory: existing?.rotationHistory,
        // FA-GD-01 (#8991): guardrails are a durable grant like routingPolicy
        // (enable-time option rebinds); decision history is evidence and
        // always survives; enabledAt re-stamps only on a fresh disabled ->
        // enabled grant (the wall-clock anchor must not reset on a redundant
        // re-enable); the paused fields NEVER survive a set() -- enabling is
        // a fresh grant and a disabled record is not paused.
        guardrails: (enabled ? options?.guardrails : undefined) ?? existing?.guardrails,
        decisionHistory: existing?.decisionHistory,
        enabledAt: enabled
          ? (existing?.enabled === true ? existing.enabledAt ?? timestamp : timestamp)
          : existing?.enabledAt,
        pendingTurnRef: enabled ? existing?.pendingTurnRef ?? undefined : undefined,
        pendingStartedAt: enabled ? existing?.pendingStartedAt ?? undefined : undefined,
        lastFailureAt: enabled ? undefined : existing?.lastFailureAt,
        consecutiveFailures: enabled ? undefined : existing?.consecutiveFailures,
        blockedReason: enabled ? undefined : options?.blockedReason,
        disabledBy: enabled ? undefined : options?.disabledBy,
        disabledAt: enabled ? undefined : timestamp,
      }))
      if (index === -1) records.push(next)
      else records[index] = next
      persist()
      return next
    },
    incrementContinuation: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return 0
      return update(index, { continuationCount: records[index]!.continuationCount + 1 }).continuationCount
    },
    claimPending: (threadRef, turnRef) => {
      const index = findIndex(threadRef)
      if (index === -1) return false
      const existing = records[index]!
      if (typeof existing.pendingTurnRef === "string") return false
      update(index, { pendingTurnRef: turnRef, pendingStartedAt: now().toISOString() })
      return true
    },
    clearPending: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return
      if (records[index]!.pendingTurnRef === undefined && records[index]!.pendingStartedAt === undefined) return
      update(index, { pendingTurnRef: undefined, pendingStartedAt: undefined })
    },
    recordFailure: (threadRef, reason) => {
      const index = findIndex(threadRef)
      if (index === -1) return 0
      const failures = (records[index]!.consecutiveFailures ?? 0) + 1
      update(index, {
        consecutiveFailures: failures,
        lastFailureAt: now().toISOString(),
        blockedReason: reason.slice(0, FULL_AUTO_BLOCKED_REASON_LIMIT),
        pendingTurnRef: undefined,
        pendingStartedAt: undefined,
      })
      return failures
    },
    recordSuccess: threadRef => {
      const index = findIndex(threadRef)
      if (index === -1) return
      update(index, {
        consecutiveFailures: undefined,
        lastFailureAt: undefined,
        blockedReason: undefined,
        pendingTurnRef: undefined,
        pendingStartedAt: undefined,
      })
    },
    bindWorkspace: (threadRef, workspaceRef) => {
      const index = findIndex(threadRef)
      return index === -1 ? null : update(index, { workspaceRef })
    },
    bindProfile: (threadRef, profile) => {
      const index = findIndex(threadRef)
      return index === -1 ? null : update(index, { profile })
    },
    bindRoutingPolicy: (threadRef, policy) => {
      const index = findIndex(threadRef)
      if (index === -1) return null
      if (policy === null) return update(index, { routingPolicy: undefined })
      // Fail closed on bounds: an empty or over-long policy throws here
      // rather than persisting an unreconcilable durable shape.
      return update(index, { routingPolicy: Schema.decodeUnknownSync(FullAutoRoutingPolicySchema)(policy) })
    },
    recordRotation: (threadRef, rotation) => {
      const index = findIndex(threadRef)
      if (index === -1) return null
      const entry = Schema.decodeUnknownSync(FullAutoRotationRecordSchema)({
        fromLane: rotation.fromLane,
        toLane: rotation.toLane,
        reason: rotation.reason,
        at: now().toISOString(),
      })
      const history = [...(records[index]!.rotationHistory ?? []), entry]
        .slice(-FULL_AUTO_ROTATION_HISTORY_LIMIT)
      return update(index, { rotationHistory: history })
    },
    bindGuardrails: (threadRef, guardrails) => {
      const index = findIndex(threadRef)
      if (index === -1) return null
      if (guardrails === null) return update(index, { guardrails: undefined })
      // Fail closed on shape: non-positive limits throw here rather than
      // persisting an unenforceable durable guardrail.
      return update(index, {
        guardrails: Schema.decodeUnknownSync(FullAutoGuardrailsSchema)(guardrails),
      })
    },
    recordDecision: (threadRef, decision) => {
      const index = findIndex(threadRef)
      if (index === -1) return null
      const entry = Schema.decodeUnknownSync(FullAutoContinuationDecisionSchema)(compactRecordInput({
        at: now().toISOString(),
        decision: decision.decision,
        reason: decision.reason.slice(0, FULL_AUTO_BLOCKED_REASON_LIMIT),
        budgetRemaining: decision.budgetRemaining,
        goalRef: decision.goalRef,
      }))
      const history = [...(records[index]!.decisionHistory ?? []), entry]
        .slice(-FULL_AUTO_DECISION_HISTORY_LIMIT)
      return update(index, { decisionHistory: history })
    },
    pause: (threadRef, reason) => {
      const index = findIndex(threadRef)
      if (index === -1) return null
      const existing = records[index]!
      if (!existing.enabled || existing.pausedReason !== undefined) return null
      return update(index, {
        pausedReason: reason.slice(0, FULL_AUTO_BLOCKED_REASON_LIMIT),
        pausedAt: now().toISOString(),
        pendingTurnRef: undefined,
        pendingStartedAt: undefined,
      })
    },
    resume: (threadRef, actor) => {
      const index = findIndex(threadRef)
      if (index === -1) return null
      const existing = records[index]!
      if (!existing.enabled || existing.pausedReason === undefined) return null
      return update(index, {
        pausedReason: undefined,
        pausedAt: undefined,
        lastResumedAt: now().toISOString(),
        resumedBy: actor,
      })
    },
  }
}
