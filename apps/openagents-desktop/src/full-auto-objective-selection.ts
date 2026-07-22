import { Schema } from "effect"

import { makeFullAutoPlan, type FullAutoPlan } from "./full-auto-plan.ts"

/**
 * HANDS-1 (#9172): owner-priority objective selection. When no objective is
 * given, Full Auto today falls back to a generic "do the next useful thing"
 * message (full-auto-reconcile.ts). This module lets the host instead PROPOSE
 * ranked candidate work items in the owner's own shape, learned from the
 * request-history characterization in
 * `docs/analysis/2026-07-22-full-auto-autonomy-decision-quality-rubric.md`
 * (Part 3): each item names a READ TARGET, a bounded DELIVERABLE, and a NAMED
 * VERIFICATION, and ends MERGED + GREEN ON MAIN, with cited rationale.
 *
 * Selection is a CANDIDATE, never an authority: it produces a ranked list for
 * the owner (or an owner-authorized surface) to endorse. It holds no dispatch,
 * spend, release, or public-claim authority, and it never invents a citation.
 * The corpus/roadmap signal is consumed through an INJECTED recall seam
 * (#9176, added in full-auto-recall.ts by another agent) -- absent, selection
 * ranks only the directly-supplied repository/issue signals.
 */
export const FULL_AUTO_OBJECTIVE_SELECTION_SCHEMA = "openagents.desktop.full_auto_objective_selection.v1" as const

/** The fixed completion gate every candidate ends at (owner's revealed shape:
 * "push to main" + green checks). A literal, not free text. */
export const FULL_AUTO_COMPLETION_GATE = "merged and green on main" as const

export const FULL_AUTO_OBJECTIVE_MAX_CANDIDATES = 20
export const FULL_AUTO_OBJECTIVE_TITLE_LIMIT = 160
export const FULL_AUTO_OBJECTIVE_FIELD_LIMIT = 600
export const FULL_AUTO_OBJECTIVE_RATIONALE_LIMIT = 600

/**
 * The owner-priority surface model. Weights are derived from the request-shape
 * evidence in the rubric Part 3 (desktop dominates; sandbox and provider next;
 * assurance; then analysis/roadmap docs). They are a DEFAULT prior, overridable
 * by a caller that has a better-grounded model. Higher = more likely the owner
 * would choose it.
 */
export const FullAutoOwnerPrioritySurfaceSchema = Schema.Literals([
  "desktop",
  "sandbox",
  "provider",
  "assurance",
  "analysis",
  "roadmap",
  "other",
])
export type FullAutoOwnerPrioritySurface = typeof FullAutoOwnerPrioritySurfaceSchema.Type

export const FULL_AUTO_OWNER_PRIORITY_WEIGHTS: Readonly<Record<FullAutoOwnerPrioritySurface, number>> = {
  desktop: 1.0,
  sandbox: 0.7,
  provider: 0.6,
  assurance: 0.5,
  analysis: 0.35,
  roadmap: 0.35,
  other: 0.2,
}

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Title = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_OBJECTIVE_TITLE_LIMIT))
const Field = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_OBJECTIVE_FIELD_LIMIT))
const Score01 = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(1))

export const FullAutoCandidateWorkItemSchema = Schema.Struct({
  title: Title,
  /** The grounding target: a doc, an issue, or a code path to read first. */
  readTarget: Field,
  /** The concrete, bounded deliverable -- never an open goal. */
  deliverable: Field,
  /** The named verification command or acceptance check. */
  verification: Field,
  /** The fixed completion gate literal. */
  completionGate: Schema.Literal(FULL_AUTO_COMPLETION_GATE),
  /** Why this was ranked where it was -- cited, owner-legible. */
  rationale: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_OBJECTIVE_RATIONALE_LIMIT)),
  surface: FullAutoOwnerPrioritySurfaceSchema,
  /** At least one real reference (issue/doc/path). Selection never invents. */
  citedRefs: Schema.Array(Ref).check(Schema.isMinLength(1), Schema.isMaxLength(20)),
  score: Score01,
})
export type FullAutoCandidateWorkItem = typeof FullAutoCandidateWorkItemSchema.Type

export const FullAutoObjectiveSelectionSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_OBJECTIVE_SELECTION_SCHEMA),
  /** Ranked best-first. */
  candidates: Schema.Array(FullAutoCandidateWorkItemSchema).check(Schema.isMaxLength(FULL_AUTO_OBJECTIVE_MAX_CANDIDATES)),
  /** Which signal sources contributed (repository/issues vs. corpus recall). */
  usedRecall: Schema.Boolean,
  rejectedCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  generatedAt: Schema.String,
})
export type FullAutoObjectiveSelection = typeof FullAutoObjectiveSelectionSchema.Type

const decodeFullAutoObjectiveSelection = Schema.decodeUnknownSync(FullAutoObjectiveSelectionSchema)

// -----------------------------------------------------------------------
// Candidate signal input + shape validation.
// -----------------------------------------------------------------------

/** A raw candidate signal from the repository backlog, open issues, the
 * roadmap, or corpus recall. Selection validates its shape before ranking. */
export type FullAutoCandidateSignal = Readonly<{
  title: string
  readTarget: string
  deliverable: string
  verification: string
  rationale: string
  surface?: FullAutoOwnerPrioritySurface
  citedRefs: ReadonlyArray<string>
  /** A caller-supplied prior in [0, 1] (e.g. issue recency/label weight or a
   * recall priority hint). Combined with the surface weight. */
  priorityHint?: number | null
}>

export type FullAutoCandidateShapeIssue =
  | "missing_read_target"
  | "missing_deliverable"
  | "missing_verification"
  | "missing_rationale"
  | "missing_citation"

const nonEmpty = (value: string | undefined): boolean => value !== undefined && value.trim().length > 0

/** Validate a candidate is in the owner's shape. A signal that fails any of
 * these is NOT rankable -- it would produce work the owner cannot trust. */
export const validateFullAutoCandidateShape = (
  signal: FullAutoCandidateSignal,
): ReadonlyArray<FullAutoCandidateShapeIssue> => {
  const issues: Array<FullAutoCandidateShapeIssue> = []
  if (!nonEmpty(signal.readTarget)) issues.push("missing_read_target")
  if (!nonEmpty(signal.deliverable)) issues.push("missing_deliverable")
  if (!nonEmpty(signal.verification)) issues.push("missing_verification")
  if (!nonEmpty(signal.rationale)) issues.push("missing_rationale")
  if (signal.citedRefs.filter((ref) => nonEmpty(ref)).length === 0) issues.push("missing_citation")
  return issues
}

// -----------------------------------------------------------------------
// Ranking.
// -----------------------------------------------------------------------

const clamp01 = (value: number): number => Math.max(0, Math.min(1, value))

/** Score = 0.7 * surface priority + 0.3 * caller prior. Deterministic and
 * owner-legible; the surface weight is the dominant term because the request
 * history concentrates so heavily on the active product surface. */
export const scoreFullAutoCandidate = (
  signal: FullAutoCandidateSignal,
  weights: Readonly<Record<FullAutoOwnerPrioritySurface, number>> = FULL_AUTO_OWNER_PRIORITY_WEIGHTS,
): number => {
  const surface = signal.surface ?? "other"
  const surfaceWeight = weights[surface] ?? weights.other
  const prior = signal.priorityHint === undefined || signal.priorityHint === null ? 0.5 : clamp01(signal.priorityHint)
  return clamp01(0.7 * surfaceWeight + 0.3 * prior)
}

const truncate = (value: string, limit: number): string => {
  const trimmed = value.trim()
  return trimmed.length <= limit ? trimmed : trimmed.slice(0, limit)
}

/**
 * Rank shape-valid candidates best-first. Shape-invalid signals are dropped
 * (counted in `rejectedCount`, never silently). Stable sort: equal scores keep
 * input order. Never invents a candidate or a citation.
 */
export const rankFullAutoObjectiveCandidates = (
  input: Readonly<{
    signals: ReadonlyArray<FullAutoCandidateSignal>
    weights?: Readonly<Record<FullAutoOwnerPrioritySurface, number>>
    usedRecall?: boolean
    limit?: number
    now?: () => Date
  }>,
): FullAutoObjectiveSelection => {
  const now = input.now ?? (() => new Date())
  const limit = Math.min(input.limit ?? FULL_AUTO_OBJECTIVE_MAX_CANDIDATES, FULL_AUTO_OBJECTIVE_MAX_CANDIDATES)
  let rejectedCount = 0
  const scored = input.signals
    .map((signal, index) => ({ signal, index }))
    .filter(({ signal }) => {
      const valid = validateFullAutoCandidateShape(signal).length === 0
      if (!valid) rejectedCount += 1
      return valid
    })
    .map(({ signal, index }) => ({
      index,
      item: {
        title: truncate(signal.title, FULL_AUTO_OBJECTIVE_TITLE_LIMIT),
        readTarget: truncate(signal.readTarget, FULL_AUTO_OBJECTIVE_FIELD_LIMIT),
        deliverable: truncate(signal.deliverable, FULL_AUTO_OBJECTIVE_FIELD_LIMIT),
        verification: truncate(signal.verification, FULL_AUTO_OBJECTIVE_FIELD_LIMIT),
        completionGate: FULL_AUTO_COMPLETION_GATE,
        rationale: truncate(signal.rationale, FULL_AUTO_OBJECTIVE_RATIONALE_LIMIT),
        surface: signal.surface ?? "other",
        citedRefs: signal.citedRefs.filter((ref) => nonEmpty(ref)).slice(0, 20),
        score: scoreFullAutoCandidate(signal, input.weights),
      } satisfies FullAutoCandidateWorkItem,
    }))
    .toSorted((left, right) => right.item.score - left.item.score || left.index - right.index)
    .slice(0, limit)
    .map((entry) => entry.item)

  return decodeFullAutoObjectiveSelection({
    schema: FULL_AUTO_OBJECTIVE_SELECTION_SCHEMA,
    candidates: scored,
    usedRecall: input.usedRecall ?? false,
    rejectedCount,
    generatedAt: now().toISOString(),
  })
}

// -----------------------------------------------------------------------
// The RLM corpus recall seam (#9176) -- injected, not implemented here.
// -----------------------------------------------------------------------

/**
 * One corpus/roadmap-derived candidate signal, as the #9176 recall entry point
 * is expected to emit it. Objective selection maps these into full candidate
 * signals (attaching a default surface/rationale when the recall row omits
 * one). `citedRefs` MUST be real corpus/issue/doc refs -- selection never
 * fabricates a citation for a recalled item.
 */
export type FullAutoObjectiveRecallSignal = Readonly<{
  title: string
  readTarget: string
  deliverable?: string
  verification?: string
  rationale?: string
  surface?: FullAutoOwnerPrioritySurface
  citedRefs: ReadonlyArray<string>
  priorityHint?: number | null
}>

/**
 * The injected recall seam. #9176 (full-auto-recall.ts) is expected to export
 * a function matching this shape; objective selection consumes it here and
 * NEVER edits full-auto-recall.ts, RLM, or the corpus sources. Absent, the
 * `directSignals` alone are ranked (`usedRecall: false`).
 *
 * Expected #9176 contract (documented for wiring):
 *   recallFullAutoObjectiveCandidates(input: {
 *     readonly runRef: string
 *     readonly workspaceRef: string
 *     readonly limit: number
 *   }): Promise<ReadonlyArray<FullAutoObjectiveRecallSignal>>
 * It resolves run-scope corpus recall (the existing #9142/#9141 machinery),
 * returns bounded, cited candidate rows, and fails soft (returns [] on
 * refused/unavailable/budget-exhausted), exactly like the recall consumer.
 */
export type FullAutoObjectiveRecall = (
  input: Readonly<{ runRef: string; workspaceRef: string; limit: number }>,
) => Promise<ReadonlyArray<FullAutoObjectiveRecallSignal>>

const recallSignalToCandidate = (signal: FullAutoObjectiveRecallSignal): FullAutoCandidateSignal => ({
  title: signal.title,
  readTarget: signal.readTarget,
  deliverable: signal.deliverable ?? "",
  verification: signal.verification ?? "",
  rationale: signal.rationale ?? `Recalled from prior authorized history; grounded in ${signal.readTarget}.`,
  ...(signal.surface === undefined ? {} : { surface: signal.surface }),
  citedRefs: signal.citedRefs,
  ...(signal.priorityHint === undefined ? {} : { priorityHint: signal.priorityHint }),
})

/**
 * Select ranked objective candidates. Merges the directly-supplied repository/
 * issue signals with corpus-recall signals (when the #9176 seam is injected),
 * then ranks by owner priority. Fail-soft: a throwing/absent recall seam
 * degrades to direct signals only, never blocks selection.
 */
export const selectFullAutoObjective = async (
  input: Readonly<{
    runRef: string
    workspaceRef: string
    directSignals: ReadonlyArray<FullAutoCandidateSignal>
    recall?: FullAutoObjectiveRecall
    weights?: Readonly<Record<FullAutoOwnerPrioritySurface, number>>
    limit?: number
    now?: () => Date
  }>,
): Promise<FullAutoObjectiveSelection> => {
  const limit = input.limit ?? FULL_AUTO_OBJECTIVE_MAX_CANDIDATES
  let recalled: ReadonlyArray<FullAutoCandidateSignal> = []
  let usedRecall = false
  if (input.recall !== undefined) {
    try {
      const rows = await input.recall({ runRef: input.runRef, workspaceRef: input.workspaceRef, limit })
      recalled = rows.map(recallSignalToCandidate)
      usedRecall = true
    } catch {
      recalled = []
      usedRecall = false
    }
  }
  return rankFullAutoObjectiveCandidates({
    signals: [...input.directSignals, ...recalled],
    ...(input.weights === undefined ? {} : { weights: input.weights }),
    usedRecall,
    limit,
    ...(input.now === undefined ? {} : { now: input.now }),
  })
}

// -----------------------------------------------------------------------
// Candidate -> run objective/done-condition/plan (bridges HANDS-1 to 2/3).
// -----------------------------------------------------------------------

/**
 * Project a chosen candidate into the durable run fields plus a starter plan.
 * The done condition embeds the named verification as a `verify:` marker so
 * `deriveFullAutoVerificationSpec` (HANDS-2) can extract a runnable check, and
 * the starter plan (HANDS-3) is a two-step read-then-deliver decomposition the
 * run refines as it works. This is a proposal builder -- it grants no
 * authority and does not start a run.
 */
export const fullAutoObjectiveFromCandidate = (
  candidate: FullAutoCandidateWorkItem,
  now: () => Date = () => new Date(),
): Readonly<{ title: string; objective: string; doneCondition: string; plan: FullAutoPlan }> => {
  const objective = `${candidate.deliverable} (read first: ${candidate.readTarget}).`
  const doneCondition = [
    `${candidate.completionGate}.`,
    `verify: ${candidate.verification}`,
  ].join("\n")
  const plan = makeFullAutoPlan({
    steps: [
      { stepRef: "read", title: `Read ${candidate.readTarget}`, status: "pending" },
      { stepRef: "deliver", title: candidate.deliverable, status: "pending", dependsOn: ["read"] },
      { stepRef: "verify", title: `Verify: ${candidate.verification}`, status: "pending", dependsOn: ["deliver"] },
    ],
    now,
  })
  return {
    title: candidate.title.slice(0, FULL_AUTO_OBJECTIVE_TITLE_LIMIT),
    objective,
    doneCondition,
    plan,
  }
}
