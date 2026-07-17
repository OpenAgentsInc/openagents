import { createHash } from "node:crypto"
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

import { FullAutoProfileSchema } from "./full-auto-registry.ts"
import {
  isFullAutoRunActive,
  isFullAutoRunTerminal,
  FullAutoRunStateSchema,
  FullAutoRunTransitionRecordSchema,
  type FullAutoRun,
  type FullAutoRunState,
} from "./full-auto-run-registry.ts"
import {
  FullAutoRecoveryActionSchema,
  FullAutoStallCauseSchema,
  type FullAutoLivenessProjection,
  type FullAutoRecoveryAction,
  type FullAutoStallCause,
} from "./full-auto-liveness.ts"
import {
  ProviderHandoffDispositionSchema,
  ProviderHandoffTransitionRecordSchema,
  type ProviderHandoffTransitionRecord,
} from "./full-auto-provider-handoff.ts"
import {
  LocalTurnDispositionSchema,
  LocalTurnPhaseSchema,
  type LocalTurnRecord,
} from "./local-turn-journal.ts"

/**
 * FA-RUN-04 (#8972): the bounded, durable, PRIVATE `FullAutoRunReport` per
 * run, and the derived PUBLIC-SAFE `FullAutoRunReceipt` projection.
 *
 * This module is deliberately layered ON TOP of three already-durable
 * sources exactly the way FA-RUN-03 (#8971) and FA-HO-01 (#8975) are layered
 * on top of FA-RUN-01 (#8969):
 *
 *  - `FullAutoRun.transitions` (full-auto-run-registry.ts) -- the complete
 *    lifecycle-edge history, including every `liveness_monitor`-attributed
 *    transition FA-RUN-03 already writes. This module never re-derives or
 *    guesses lifecycle facts; it copies the run's own authoritative history.
 *  - `ProviderHandoffTransitionRecord` rows (full-auto-provider-handoff.ts)
 *    -- FA-HO-01's own durable receipt store, explicitly built "so a future
 *    FullAutoRunReport (#8972) can list every handoff for a run" per that
 *    module's header comment.
 *  - `LocalTurnRecord` rows (local-turn-journal.ts) -- the existing per-turn
 *    journal, projected the SAME identity/phase/disposition/timestamp-only
 *    way `full-auto-control-server.ts`'s `projectTurns` already does (never
 *    `assistantText`/`assistantSegments`).
 *
 * None of those three stores is scoped to one run: the turn journal and the
 * handoff registry each enforce their OWN global bound
 * (`LOCAL_TURN_RECORD_LIMIT`, `PROVIDER_HANDOFF_TRANSITION_LIMIT`) shared
 * across every thread/run, so a long-lived run's earlier facts can be
 * evicted by LATER, unrelated activity before anyone reads this report. The
 * report's `sync` function is therefore an incremental, idempotent MERGE
 * (union keyed by `turnRef`/`handoffRef`) rather than a stateless rebuild:
 * once a fact is captured into a run's report, it survives that upstream
 * eviction (subject only to the report's OWN bound). Callers should sync on
 * every control-API mutation that touches a run AND on every report/receipt
 * read, so organic (non-control-API) reconciliation activity in between is
 * still folded in the next time anyone looks.
 *
 * Liveness gaps and uninterrupted autonomous intervals are derived from a
 * bounded, deduplicated `livenessObservations` log this module owns -- fed
 * by the SAME typed `FullAutoLivenessProjection` FA-RUN-03 already computes
 * (`classifyFullAutoRunLiveness`/`settleFullAutoRunLiveness`), never by
 * re-parsing the free-text `reason` string on a lifecycle transition. This
 * keeps the gap/interval facts exactly as trustworthy as the existing
 * `stallCause`/`recoveryAction` control-API fields already are, with no new
 * inference.
 *
 * Fields the repository has no independent source for yet (verified
 * commit/artifact refs, objective/done-condition progress, token/cost usage)
 * are modeled as explicit, honestly-empty/`unknown` typed placeholders per
 * the issue's evidence rules ("missing evidence is explicit, not
 * fabricated"; "Unknown/truncated/unavailable evidence cannot become
 * success or zero usage") rather than omitted or guessed. Populating them is
 * out of this issue's scope (the issue names FA transcript-analysis #8973 as
 * a downstream consumer/contributor).
 */
export const FULL_AUTO_RUN_REPORT_SCHEMA = "openagents.desktop.full_auto_run_report.v1" as const
export const FULL_AUTO_RUN_RECEIPT_SCHEMA = "openagents.desktop.full_auto_run_receipt.v1" as const

/** Store-level bound: mirrors `FULL_AUTO_RUN_RECORD_LIMIT` -- one report per
 * run, evicted the same active-protected/oldest-first way. */
export const FULL_AUTO_RUN_REPORT_LIMIT = 128
/** Per-report bound on merged turn entries -- generous relative to the
 * control API's 20-turn display window since this is the durable archive. */
export const FULL_AUTO_RUN_REPORT_TURN_LIMIT = 500
/** Per-report bound on merged provider-handoff entries. */
export const FULL_AUTO_RUN_REPORT_HANDOFF_LIMIT = 200
/** Per-report bound on the deduplicated liveness-observation log. */
export const FULL_AUTO_RUN_REPORT_LIVENESS_OBSERVATION_LIMIT = 400
/** Per-report bound on derived gap/interval spans (each observation can open
 * at most one of either, so this comfortably exceeds any realistic count). */
export const FULL_AUTO_RUN_REPORT_SPAN_LIMIT = 400
export const FULL_AUTO_RUN_REPORT_VERIFIED_REF_LIMIT = 200

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))
const WorkspaceRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))
const Title = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200))
const Reason = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400))
/** sha256 hex digest -- exactly 64 lowercase hex characters. */
const Digest = Schema.String.check(Schema.isLengthBetween(64, 64))
const Count = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

/** Stable, order-independent digest so identical objective/doneCondition
 * text always produces the same report field -- callers can prove "this
 * report is about the mission I think it is" without either party ever
 * transmitting the raw text through a lower-trust channel. Never reversible
 * never logged alongside the raw text. */
export const sha256HexDigest = (value: string): string =>
  createHash("sha256").update(value, "utf8").digest("hex")

// -----------------------------------------------------------------------
// Turn entries -- identity/phase/disposition/timestamps only, mirroring
// full-auto-control-server.ts's projectTurns redaction discipline exactly.
// -----------------------------------------------------------------------

export const FullAutoRunReportTurnEntrySchema = Schema.Struct({
  turnRef: Ref,
  lane: LaneRef,
  accountRef: Schema.optional(Ref),
  model: Schema.optional(Ref),
  phase: LocalTurnPhaseSchema,
  disposition: Schema.NullOr(LocalTurnDispositionSchema),
  createdAt: Schema.String,
  updatedAt: Schema.String,
  /** Not sourced by this aggregator yet -- `local-turn-journal.ts` does not
   * track retry lineage or a selected packet/issue ref today. Always absent
   * rather than guessed; see the module header's evidence-rules note. */
  retryOfTurnRef: Schema.optional(Ref),
  selectedPacketRef: Schema.optional(Ref),
  /** Code-authored, bounded, from `phase`/`disposition` alone -- structurally
   * incapable of carrying transcript text (never assistantText/segments). */
  outcomeSummary: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type FullAutoRunReportTurnEntry = typeof FullAutoRunReportTurnEntrySchema.Type

const describeTurnOutcome = (record: LocalTurnRecord): string =>
  record.disposition !== null ? `turn ${record.disposition}` : `turn in phase ${record.phase}`

const projectReportTurn = (record: LocalTurnRecord): FullAutoRunReportTurnEntry => ({
  turnRef: record.turnRef,
  lane: record.lane,
  ...(record.accountRef === null ? {} : { accountRef: record.accountRef }),
  ...(record.model === null ? {} : { model: record.model }),
  phase: record.phase,
  disposition: record.disposition,
  createdAt: record.createdAt,
  updatedAt: record.updatedAt,
  outcomeSummary: describeTurnOutcome(record),
})

// -----------------------------------------------------------------------
// Liveness observations + derived gaps/uninterrupted intervals.
// -----------------------------------------------------------------------

export const FullAutoRunReportLivenessObservationSchema = Schema.Struct({
  at: Schema.String,
  projectedState: FullAutoRunStateSchema,
  cause: Schema.NullOr(FullAutoStallCauseSchema),
  recoveryAction: FullAutoRecoveryActionSchema,
  sinceLastProgressMs: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
})
export type FullAutoRunReportLivenessObservation =
  typeof FullAutoRunReportLivenessObservationSchema.Type

/** A span where the projected state was `stalled` or `retrying` -- no
 * useful autonomous progress was being made. `cause` is the LAST
 * non-null cause observed within the span (a span may toggle between the
 * two states without an intervening `running` observation). */
export const FullAutoRunReportLivenessGapSchema = Schema.Struct({
  enteredAt: Schema.String,
  exitedAt: Schema.NullOr(Schema.String),
  durationMs: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
  cause: Schema.NullOr(FullAutoStallCauseSchema),
})
export type FullAutoRunReportLivenessGap = typeof FullAutoRunReportLivenessGapSchema.Type

/** The complement of a gap: a span the run was healthy (Running, or any
 * other non-stalled/retrying non-terminal projection). */
export const FullAutoRunReportIntervalSchema = Schema.Struct({
  startedAt: Schema.String,
  endedAt: Schema.NullOr(Schema.String),
  durationMs: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
})
export type FullAutoRunReportInterval = typeof FullAutoRunReportIntervalSchema.Type

const isGapState = (state: FullAutoRunState): boolean =>
  state === "stalled" || state === "retrying"
const durationOf = (fromIso: string, toIso: string): number =>
  Math.max(0, Date.parse(toIso) - Date.parse(fromIso))

/** Pure derivation over a chronologically-sorted observation log, anchored
 * at the run's own started/created time (before the first observation, a
 * run is presumed productive -- it was just dispatched). Never mutates
 * called fresh on every sync so the spans always reflect the CURRENT
 * (possibly still-open) observation log. */
export const deriveFullAutoRunLivenessSpans = (
  input: Readonly<{
    observations: ReadonlyArray<FullAutoRunReportLivenessObservation>
    anchorAt: string | null
    currentState: FullAutoRunState
  }>,
): Readonly<{
  gaps: ReadonlyArray<FullAutoRunReportLivenessGap>
  intervals: ReadonlyArray<FullAutoRunReportInterval>
}> => {
  const gaps: Array<FullAutoRunReportLivenessGap> = []
  const intervals: Array<FullAutoRunReportInterval> = []
  const sorted = input.observations.toSorted((left, right) => left.at.localeCompare(right.at))

  let openGapAt: string | null = null
  let openGapCause: FullAutoStallCause | null = null
  let openIntervalAt: string | null = input.anchorAt

  for (const observation of sorted) {
    if (isGapState(observation.projectedState)) {
      if (openIntervalAt !== null) {
        intervals.push({
          startedAt: openIntervalAt,
          endedAt: observation.at,
          durationMs: durationOf(openIntervalAt, observation.at),
        })
        openIntervalAt = null
      }
      if (openGapAt === null) openGapAt = observation.at
      if (observation.cause !== null) openGapCause = observation.cause
      continue
    }
    // A non-gap observation (running, or any terminal projection) closes an
    // open gap and opens (or continues) a productive interval.
    if (openGapAt !== null) {
      gaps.push({
        enteredAt: openGapAt,
        exitedAt: observation.at,
        durationMs: durationOf(openGapAt, observation.at),
        cause: openGapCause,
      })
      openGapAt = null
      openGapCause = null
    }
    if (openIntervalAt === null) openIntervalAt = observation.at
  }

  // Ongoing spans (never closed by a later observation) stay explicitly
  // open -- exitedAt/endedAt/durationMs are null, never fabricated.
  if (openGapAt !== null && isGapState(input.currentState)) {
    gaps.push({ enteredAt: openGapAt, exitedAt: null, durationMs: null, cause: openGapCause })
  } else if (openGapAt !== null) {
    // The observation log's last entry was a gap state but the run's
    // CURRENT state (settled fresher than the log, e.g. just resumed) is
    // not -- close it defensively using the current sync time is not
    // available here, so leave it open rather than guess a false end.
    gaps.push({ enteredAt: openGapAt, exitedAt: null, durationMs: null, cause: openGapCause })
  }
  if (openIntervalAt !== null) {
    intervals.push({ startedAt: openIntervalAt, endedAt: null, durationMs: null })
  }

  return {
    gaps: gaps.slice(-FULL_AUTO_RUN_REPORT_SPAN_LIMIT),
    intervals: intervals.slice(-FULL_AUTO_RUN_REPORT_SPAN_LIMIT),
  }
}

// -----------------------------------------------------------------------
// Verified/claimed refs and usage -- honest, currently-empty placeholders.
// -----------------------------------------------------------------------

/** "A Git ref is recorded only after resolving it against the expected
 * repository/workspace; mismatch is explicit." This module does not yet
 * perform that independent resolution (no Git integration in this pass), so
 * `verification` here is always `claimed` when populated by a future
 * caller, never `verified` -- `verified` is reserved for a caller that has
 * actually checked. The array itself is always empty in this v1 aggregator
 * see the module header note on FA transcript-analysis #8973. */
export const FullAutoRunReportVerifiedRefKindSchema = Schema.Literals([
  "commit",
  "artifact",
  "receipt",
])
export type FullAutoRunReportVerifiedRefKind = typeof FullAutoRunReportVerifiedRefKindSchema.Type
export const FullAutoRunReportVerificationSchema = Schema.Literals(["verified", "claimed"])
export type FullAutoRunReportVerification = typeof FullAutoRunReportVerificationSchema.Type
export const FullAutoRunReportVerifiedRefSchema = Schema.Struct({
  ref: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  kind: FullAutoRunReportVerifiedRefKindSchema,
  verification: FullAutoRunReportVerificationSchema,
  turnRef: Schema.optional(Ref),
})
export type FullAutoRunReportVerifiedRef = typeof FullAutoRunReportVerifiedRefSchema.Type

/** `known: false` is structurally distinct from `value: 0` -- an unknown
 * usage total can never be mistaken for a genuinely free/zero-cost run. */
export const FullAutoRunReportUsageSchema = Schema.Struct({
  totalTokensKnown: Schema.Boolean,
  totalTokens: Schema.NullOr(Count),
  costUsdKnown: Schema.Boolean,
  costUsd: Schema.NullOr(Schema.Number),
})
export type FullAutoRunReportUsage = typeof FullAutoRunReportUsageSchema.Type
const UNKNOWN_USAGE: FullAutoRunReportUsage = {
  totalTokensKnown: false,
  totalTokens: null,
  costUsdKnown: false,
  costUsd: null,
}

// -----------------------------------------------------------------------
// The private report itself.
// -----------------------------------------------------------------------

export const FullAutoRunReportSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_REPORT_SCHEMA),
  runRef: Ref,
  threadRef: Schema.optional(Ref),
  title: Title,
  objectiveDigest: Digest,
  doneConditionDigest: Digest,
  objectiveRevisionCount: Count,
  workspaceRef: Schema.optional(WorkspaceRef),
  providerProfile: Schema.optional(FullAutoProfileSchema),
  turnCap: Count,
  successfulAttempts: Count,
  failedAttempts: Count,
  state: FullAutoRunStateSchema,
  terminalReason: Schema.optional(Reason),
  createdAt: Schema.String,
  startedAt: Schema.optional(Schema.String),
  endedAt: Schema.optional(Schema.String),
  lifecycleTransitions: Schema.Array(FullAutoRunTransitionRecordSchema),
  ownerActions: Schema.Array(FullAutoRunTransitionRecordSchema),
  providerTransitions: Schema.Array(ProviderHandoffTransitionRecordSchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_HANDOFF_LIMIT),
  ),
  livenessObservations: Schema.Array(FullAutoRunReportLivenessObservationSchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_LIVENESS_OBSERVATION_LIMIT),
  ),
  livenessGaps: Schema.Array(FullAutoRunReportLivenessGapSchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_SPAN_LIMIT),
  ),
  uninterruptedIntervals: Schema.Array(FullAutoRunReportIntervalSchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_SPAN_LIMIT),
  ),
  turns: Schema.Array(FullAutoRunReportTurnEntrySchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_TURN_LIMIT),
  ),
  verifiedRefs: Schema.Array(FullAutoRunReportVerifiedRefSchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_VERIFIED_REF_LIMIT),
  ),
  /** Always `"unknown"` in this v1 aggregator -- no done-condition evaluator
   * exists yet. A typed, honest placeholder rather than an omitted field. */
  progressDisposition: Schema.Literal("unknown"),
  usage: FullAutoRunReportUsageSchema,
  /** Never embedded raw content -- a pointer into Desktop's own existing
   * thread/provider-session stores for deeper (still-private) inspection.
   * Null when no thread is bound yet (a still-Draft run). */
  rawEvidenceRef: Schema.NullOr(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  ),
  reportRevision: Count,
  updatedAt: Schema.String,
})
export type FullAutoRunReport = typeof FullAutoRunReportSchema.Type
/** Hoisted so the sync hot path never recompiles this schema per call. */
const decodeFullAutoRunReport = Schema.decodeUnknownSync(FullAutoRunReportSchema)

// -----------------------------------------------------------------------
// The derived public-safe receipt. Every field is a digest, a count, a
// bounded system-minted ref/enum, or a timestamp -- structurally incapable
// of carrying free text, so no reason/title/objective/workspace-path/
// account-identity content can ever leak through it, adversarially or
// otherwise (see the redaction tests in full-auto-run-report.test.ts).
// -----------------------------------------------------------------------

export const FullAutoRunReceiptSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_RECEIPT_SCHEMA),
  runRef: Ref,
  threadRef: Schema.optional(Ref),
  objectiveDigest: Digest,
  doneConditionDigest: Digest,
  workspaceRefDigest: Schema.NullOr(Digest),
  state: FullAutoRunStateSchema,
  startedAt: Schema.optional(Schema.String),
  endedAt: Schema.optional(Schema.String),
  turnCap: Count,
  successfulAttempts: Count,
  failedAttempts: Count,
  /** Distinct provider-lane identities the run actually executed on --
   * opaque bounded refs (e.g. "codex-local"), never account/model detail. */
  providerIdentities: Schema.Array(LaneRef).check(Schema.isMaxLength(32)),
  providerTransitionCount: Count,
  providerTransitionDispositions: Schema.Array(ProviderHandoffDispositionSchema).check(
    Schema.isMaxLength(FULL_AUTO_RUN_REPORT_HANDOFF_LIMIT),
  ),
  livenessGapCount: Count,
  recoveryActionsUsed: Schema.Array(FullAutoRecoveryActionSchema).check(Schema.isMaxLength(3)),
  verifiedRefCount: Count,
  claimedRefCount: Count,
  progressDisposition: Schema.Literal("unknown"),
  usageKnown: Schema.Boolean,
  /** Ties this receipt to the exact private report state it was derived
   * from, without embedding that report's content. */
  reportRevision: Count,
  createdAt: Schema.String,
})
export type FullAutoRunReceipt = typeof FullAutoRunReceiptSchema.Type
/** Hoisted so `deriveFullAutoRunReceipt` never recompiles this schema per call. */
const decodeFullAutoRunReceipt = Schema.decodeUnknownSync(FullAutoRunReceiptSchema)

/** Pure derivation -- the exhaustive redaction-test surface. Never reads
 * anything beyond the report's own already-bounded fields. */
export const deriveFullAutoRunReceipt = (
  report: FullAutoRunReport,
  now: () => Date = () => new Date(),
): FullAutoRunReceipt => {
  const providerIdentities = [
    ...new Set([
      ...(report.providerProfile?.lane === undefined ? [] : [report.providerProfile.lane]),
      ...report.turns.map((turn) => turn.lane),
      ...report.providerTransitions.flatMap((transition) => [transition.from, transition.to]),
    ]),
  ].slice(0, 32)
  const recoveryActionsUsed = [
    ...new Set(
      report.livenessObservations
        .map((observation) => observation.recoveryAction)
        .filter((action): action is FullAutoRecoveryAction => action !== "none"),
    ),
  ]
  const verifiedRefCount = report.verifiedRefs.filter(
    (ref) => ref.verification === "verified",
  ).length
  const claimedRefCount = report.verifiedRefs.filter(
    (ref) => ref.verification === "claimed",
  ).length
  return decodeFullAutoRunReceipt({
    schema: FULL_AUTO_RUN_RECEIPT_SCHEMA,
    runRef: report.runRef,
    ...(report.threadRef === undefined ? {} : { threadRef: report.threadRef }),
    objectiveDigest: report.objectiveDigest,
    doneConditionDigest: report.doneConditionDigest,
    workspaceRefDigest:
      report.workspaceRef === undefined ? null : sha256HexDigest(report.workspaceRef),
    state: report.state,
    ...(report.startedAt === undefined ? {} : { startedAt: report.startedAt }),
    ...(report.endedAt === undefined ? {} : { endedAt: report.endedAt }),
    turnCap: report.turnCap,
    successfulAttempts: report.successfulAttempts,
    failedAttempts: report.failedAttempts,
    providerIdentities,
    providerTransitionCount: report.providerTransitions.length,
    providerTransitionDispositions: report.providerTransitions.map(
      (transition) => transition.disposition,
    ),
    livenessGapCount: report.livenessGaps.length,
    recoveryActionsUsed,
    verifiedRefCount,
    claimedRefCount,
    progressDisposition: report.progressDisposition,
    usageKnown: report.usage.totalTokensKnown,
    reportRevision: report.reportRevision,
    createdAt: now().toISOString(),
  })
}

// -----------------------------------------------------------------------
// The durable, bounded, atomic-write store -- same shape/quarantine
// discipline as full-auto-run-registry.ts and full-auto-provider-handoff.ts.
// -----------------------------------------------------------------------

const FullAutoRunReportFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_REPORT_SCHEMA),
  reports: Schema.Array(FullAutoRunReportSchema),
})
/** Hoisted so `decodeFile` never recompiles this schema per call. */
const decodeFullAutoRunReportFile = Schema.decodeUnknownSync(FullAutoRunReportFileSchema)

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
    throw new Error(
      `full auto run report store unavailable: ${error instanceof Error ? error.message : "unknown error"}`,
      { cause: error },
    )
  }
}

/** Corrupt-file quarantine, matching the FA-H10/FA-AC-41 pattern every other
 * durable Full Auto store already uses. */
const decodeFile = (filePath: string, now: () => Date): ReadonlyArray<FullAutoRunReport> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = decodeFullAutoRunReportFile(JSON.parse(readFileSync(filePath, "utf8")))
    return decoded.reports
  } catch (error) {
    const quarantinePath = `${filePath}.quarantined-${now().toISOString()}`
    try {
      renameSync(filePath, quarantinePath)
      console.error(
        `full auto run report store failed validation; quarantined the corrupt file at ${quarantinePath} and starting empty`,
        error,
      )
    } catch {
      console.error(
        `full auto run report store failed validation and the corrupt file at ${filePath} could not be quarantined; starting empty`,
        error,
      )
    }
    return []
  }
}

export type FullAutoRunReportSyncInput = Readonly<{
  run: FullAutoRun
  /** Fresh read of the local turn journal, already filtered to this run's
   * threadRef (or empty for a not-yet-bound run) -- the caller owns that
   * filter exactly like `full-auto-control-server.ts`'s `listTurns` capability. */
  turns: ReadonlyArray<LocalTurnRecord>
  /** Fresh read of the provider handoff registry, already filtered to this
   * run's runRef. */
  handoffs: ReadonlyArray<ProviderHandoffTransitionRecord>
  /** The freshly classified liveness projection for this exact settle pass,
   * when the caller already computed one (the control server always does,
   * via `settleFullAutoRunLiveness`). Omitted only for callers with no
   * liveness classifier in scope (e.g. a pure migration/backfill path)
   * omitting it never regresses previously recorded observations. */
  livenessProjection?: FullAutoLivenessProjection
}>

export type FullAutoRunReportStore = Readonly<{
  list: () => ReadonlyArray<FullAutoRunReport>
  get: (runRef: string) => FullAutoRunReport | null
  /** The single mutating entry point -- merges freshly-read facts into the
   * existing stored report (or creates one on first sync) and persists. */
  sync: (input: FullAutoRunReportSyncInput) => FullAutoRunReport
}>

const mergeByKey = <T, K>(
  existing: ReadonlyArray<T>,
  incoming: ReadonlyArray<T>,
  keyOf: (value: T) => K,
  updatedAtOf: (value: T) => string,
  limit: number,
): ReadonlyArray<T> => {
  const byKey = new Map<K, T>()
  for (const value of existing) byKey.set(keyOf(value), value)
  for (const value of incoming) {
    const current = byKey.get(keyOf(value))
    // Prefer the freshest version of a record we have already seen, but
    // never drop a record incoming no longer carries (upstream eviction).
    if (current === undefined || updatedAtOf(value) >= updatedAtOf(current))
      byKey.set(keyOf(value), value)
  }
  return [...byKey.values()]
    .toSorted((left, right) => updatedAtOf(left).localeCompare(updatedAtOf(right)))
    .slice(-limit)
}

export const openFullAutoRunReportStore = (
  file: string,
  now: () => Date = () => new Date(),
): FullAutoRunReportStore => {
  const filePath = path.resolve(file)
  let reports = [...decodeFile(filePath, now)]

  /** FA-AC-50-style eviction: never drop a report whose run is still active
   * -- extends the run registry's own eviction protection to its report. */
  const persist = (): void => {
    const sorted = reports.toSorted((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    const protectedReports = sorted.filter((report) => isFullAutoRunActive(report.state))
    const evictable = sorted.filter((report) => !isFullAutoRunActive(report.state))
    reports = [
      ...protectedReports,
      ...evictable.slice(0, Math.max(0, FULL_AUTO_RUN_REPORT_LIMIT - protectedReports.length)),
    ]
    writePrivateAtomic(filePath, { schema: FULL_AUTO_RUN_REPORT_SCHEMA, reports })
  }

  const list = (): ReadonlyArray<FullAutoRunReport> => [...reports]
  const get = (runRef: string): FullAutoRunReport | null =>
    reports.find((report) => report.runRef === runRef) ?? null

  const sync: FullAutoRunReportStore["sync"] = (input) => {
    const { run } = input
    const existing = get(run.runRef)
    const timestamp = now().toISOString()

    const mergedTurns = mergeByKey(
      existing?.turns ?? [],
      input.turns.map(projectReportTurn),
      (turn) => turn.turnRef,
      (turn) => turn.updatedAt,
      FULL_AUTO_RUN_REPORT_TURN_LIMIT,
    )
    const mergedHandoffs = mergeByKey(
      existing?.providerTransitions ?? [],
      input.handoffs,
      (handoff) => handoff.handoffRef,
      (handoff) => handoff.at,
      FULL_AUTO_RUN_REPORT_HANDOFF_LIMIT,
    )

    const priorObservations = existing?.livenessObservations ?? []
    const lastObservation = priorObservations.at(-1)
    const shouldAppendObservation =
      input.livenessProjection !== undefined &&
      (lastObservation === undefined ||
        lastObservation.projectedState !== input.livenessProjection.projectedState ||
        lastObservation.cause !== input.livenessProjection.cause ||
        lastObservation.recoveryAction !== input.livenessProjection.recoveryAction)
    const livenessObservations =
      shouldAppendObservation && input.livenessProjection !== undefined
        ? [
            ...priorObservations,
            {
              at: timestamp,
              projectedState: input.livenessProjection.projectedState,
              cause: input.livenessProjection.cause,
              recoveryAction: input.livenessProjection.recoveryAction,
              sinceLastProgressMs: input.livenessProjection.sinceLastProgressMs,
            },
          ].slice(-FULL_AUTO_RUN_REPORT_LIVENESS_OBSERVATION_LIMIT)
        : priorObservations

    const spans = deriveFullAutoRunLivenessSpans({
      observations: livenessObservations,
      anchorAt: run.startedAt ?? run.createdAt,
      currentState: run.state,
    })

    const ownerActions = run.transitions.filter(
      (transition) =>
        transition.actor === "owner_ui" ||
        transition.actor === "control_api" ||
        transition.actor === "cli" ||
        transition.actor === "mcp",
    )

    const next: FullAutoRunReport = decodeFullAutoRunReport({
      schema: FULL_AUTO_RUN_REPORT_SCHEMA,
      runRef: run.runRef,
      ...(run.threadRef === undefined ? {} : { threadRef: run.threadRef }),
      title: run.title,
      objectiveDigest: sha256HexDigest(run.objective),
      doneConditionDigest: sha256HexDigest(run.doneCondition),
      objectiveRevisionCount: run.objectiveHistory.length,
      ...(run.workspaceRef === undefined ? {} : { workspaceRef: run.workspaceRef }),
      ...(run.profile === undefined ? {} : { providerProfile: run.profile }),
      turnCap: run.turnCap,
      successfulAttempts: run.successfulAttempts,
      failedAttempts: run.failedAttempts,
      state: run.state,
      ...(run.terminalReason === undefined ? {} : { terminalReason: run.terminalReason }),
      createdAt: existing?.createdAt ?? run.createdAt,
      ...(run.startedAt === undefined ? {} : { startedAt: run.startedAt }),
      ...(isFullAutoRunTerminal(run.state)
        ? { endedAt: run.stoppedAt ?? run.completedAt ?? timestamp }
        : {}),
      lifecycleTransitions: run.transitions,
      ownerActions,
      providerTransitions: mergedHandoffs,
      livenessObservations,
      livenessGaps: spans.gaps,
      uninterruptedIntervals: spans.intervals,
      turns: mergedTurns,
      verifiedRefs: existing?.verifiedRefs ?? [],
      progressDisposition: "unknown",
      usage: existing?.usage ?? UNKNOWN_USAGE,
      rawEvidenceRef: run.threadRef === undefined ? null : `thread:${run.threadRef}`,
      reportRevision: (existing?.reportRevision ?? 0) + 1,
      updatedAt: timestamp,
    })

    const index = reports.findIndex((report) => report.runRef === run.runRef)
    if (index === -1) reports.push(next)
    else reports[index] = next
    persist()
    return next
  }

  return { list, get, sync }
}
