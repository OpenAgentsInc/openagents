import { Schema } from "effect"

/**
 * VSE-04 (#9109): fail-closed done-condition verdicts for autonomous runs.
 *
 * Full Auto's ProductSpec cuts automatic done-condition verification
 * (CUT-FA-04): "Completed" is a self-reported provider disposition, never a
 * verified-truth claim. This module is the successor contract's prototype: a
 * typed verdict, computed only by an oracle, recorded as a fact DISTINCT from
 * the provider disposition, and fail-closed by construction. An absent oracle
 * is `unavailable`; an oracle error, stale evidence, or evidence not bound to
 * the run's exact commit is `unverified`. Only fresh, commit-bound, matching
 * evidence yields `verified`. There is no permissive `pending`/`warning` state,
 * and no code path derives the verdict from the provider disposition.
 *
 * Activation — wiring this into a real Full Auto terminal state — is a separate
 * owner-gated step (Full Auto Wave 0, #8978/#8979). This library proves the
 * semantics under test first.
 */

export const DONE_CONDITION_VERDICT_SCHEMA =
  "openagents.assurance.done_condition_verdict.v1" as const

const NonEmptyString = Schema.String.check(Schema.isMinLength(1))

/**
 * Objective classes. `repository` objectives are adjudicable by the assure-repo
 * sweep machinery; `exact_replay` covers replayable work (the tassadar route);
 * `unsupported` is every objective with no sound oracle — natural-language or
 * open-ended goals — which can never be `verified`.
 */
export const DoneConditionObjectiveClassSchema = Schema.Literals([
  "repository",
  "exact_replay",
  "unsupported",
])
export type DoneConditionObjectiveClass = typeof DoneConditionObjectiveClassSchema.Type

/** The only three verdict states. No permissive `pending`/`warning`. */
export const DoneConditionStateSchema = Schema.Literals([
  "verified",
  "unverified",
  "unavailable",
])
export type DoneConditionState = typeof DoneConditionStateSchema.Type

export const DoneConditionVerdictSchema = Schema.Struct({
  schema: Schema.Literal(DONE_CONDITION_VERDICT_SCHEMA),
  objectiveClass: DoneConditionObjectiveClassSchema,
  state: DoneConditionStateSchema,
  /** Source-bound evidence reference, or null when no oracle produced one. */
  evidenceRef: Schema.NullOr(NonEmptyString),
  reason: NonEmptyString,
  at: NonEmptyString,
})
export type DoneConditionVerdict = typeof DoneConditionVerdictSchema.Type

/**
 * The objective an autonomous run was asked to satisfy. `commit` is the exact
 * revision the run targeted; a `verified` verdict must bind to it.
 */
export type DoneConditionObjective = Readonly<{
  objectiveClass: DoneConditionObjectiveClass
  commit: string
}>

/**
 * Oracle evidence, mapped from a concrete source (e.g. an assure-repo sweep
 * receipt) into the fields the adjudicator needs. Every field defaults to the
 * fail-closed reading: `present: false` means no oracle ran at all.
 */
export type DoneConditionEvidence = Readonly<{
  /** Whether an oracle ran and produced evidence at all. */
  present: boolean
  /** Whether the oracle itself errored. */
  errored: boolean
  /** The commit the evidence is bound to, or null when unbound. */
  commit: string | null
  /** Whether the evidence is fresh (within its staleness window). */
  fresh: boolean
  /** The oracle's raw pass/fail for the objective. */
  objectiveMet: boolean
  /** A source-bound reference to the evidence (e.g. a sweep-receipt ref). */
  evidenceRef: string | null
}>

const verdict = (
  objectiveClass: DoneConditionObjectiveClass,
  state: DoneConditionState,
  reason: string,
  evidenceRef: string | null,
  at: string,
): DoneConditionVerdict => ({
  schema: DONE_CONDITION_VERDICT_SCHEMA,
  objectiveClass,
  state,
  evidenceRef,
  reason,
  at,
})

/**
 * Adjudicate a done-condition verdict, fail-closed. The order of checks is the
 * contract: unsupported → unavailable; no oracle → unavailable; oracle error →
 * unverified; evidence not bound to the run commit → unverified; stale
 * evidence → unverified; objective not met → unverified; only then → verified.
 * The verdict is never derived from any provider disposition.
 */
export const adjudicateDoneCondition = (
  objective: DoneConditionObjective,
  evidence: DoneConditionEvidence,
  at: string,
): DoneConditionVerdict => {
  const { objectiveClass, commit } = objective
  if (objectiveClass === "unsupported") {
    return verdict(
      "unsupported",
      "unavailable",
      "objective has no registered oracle; not machine-adjudicable",
      null,
      at,
    )
  }
  if (!evidence.present) {
    return verdict(objectiveClass, "unavailable", "no oracle evidence available", null, at)
  }
  if (evidence.errored) {
    return verdict(objectiveClass, "unverified", "oracle errored", evidence.evidenceRef, at)
  }
  if (evidence.commit === null || evidence.commit !== commit) {
    return verdict(
      objectiveClass,
      "unverified",
      "evidence is not bound to the run's exact commit",
      evidence.evidenceRef,
      at,
    )
  }
  if (!evidence.fresh) {
    return verdict(objectiveClass, "unverified", "evidence is stale", evidence.evidenceRef, at)
  }
  if (!evidence.objectiveMet) {
    return verdict(
      objectiveClass,
      "unverified",
      "oracle observed the objective not met",
      evidence.evidenceRef,
      at,
    )
  }
  return verdict(
    objectiveClass,
    "verified",
    "fresh commit-bound evidence matches the objective",
    evidence.evidenceRef,
    at,
  )
}

/**
 * A run's terminal state records the provider disposition and the
 * done-condition verdict as two distinct facts. `completed` from the provider
 * never sets or implies a `verified` verdict — the verdict is only ever the
 * output of {@link adjudicateDoneCondition}.
 */
export type RunTerminalState = Readonly<{
  providerDisposition: string
  doneCondition: DoneConditionVerdict
}>

export const recordRunTerminalState = (
  providerDisposition: string,
  doneCondition: DoneConditionVerdict,
): RunTerminalState => ({ providerDisposition, doneCondition })

/** Objective completion is true only when the ORACLE verdict is `verified`. */
export const isObjectiveVerified = (state: RunTerminalState): boolean =>
  state.doneCondition.state === "verified"

export const decodeDoneConditionVerdict = (value: unknown): DoneConditionVerdict =>
  Schema.decodeUnknownSync(DoneConditionVerdictSchema)(value)
