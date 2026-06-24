// QA failure learning — the worker Blueprint/GEPA wiring (#6195).
//
// The qa-runner (apps/qa-runner) owns the report-side failure-learning
// strategies + the local FailurePattern capture. THIS module is the Blueprint
// surface half: it lowers a failed/REFUTED Khala program turn into a CLAIM-LEVEL
// GEPA CANDIDATE-FEEDBACK signal — the `psionic.probe_gepa_candidate_*`-class
// feedback the brain/Blueprint audit describes (StudyBench -> GEPA). The
// optimizer (`gepa_style_reflection` / `retained_failure_replay`) MAY consume it
// to refine a program/prompt/module AS A CANDIDATE.
//
// It rides ALONGSIDE the existing Khala -> Blueprint program wiring (#6188,
// `chat-program-runtime-khala.ts`): that adapter emits the evidence-only
// `BlueprintProgramRunRecord`; this one, given a run record + an honest failure
// verdict, emits the evidence-only candidate-feedback the optimizer consumes.
//
// EVIDENCE-ONLY + GOVERNED (Blueprint invariants; "Blueprint Program Run
// Evidence Authority" + the distiller skill-candidate posture). Carried, not
// weakened:
//   - authorityBoundary: 'evidence_only' — emitting it deploys nothing, sends no
//     email, spends nothing, mutates no source, promotes no public claim, and
//     changes no live behavior. It is feedback evidence only.
//   - requiresReleaseGate: true + a named gate ref — a refinement informed by
//     this feedback is live ONLY after an operator promotes it through the gate.
//   - selfPromotionAllowed: false — NO self-promotion, ever. The Release Gate
//     REJECTS an unapproved candidate (`evaluateGepaCandidateReleaseGate...`).
//   - public-safe: refs/digests/evidence summaries only (no prompts/tokens/raw
//     output); reuses the program-run record's already-public-safe refs.
//
// The signal is intentionally SMALL + claim-level (negative feedback over
// contradicted commitments), NOT a promoted candidate manifest. Failure learning
// feeds candidate FEEDBACK to the optimizer; it never assembles a live candidate.

import { Effect, Schema as S } from 'effect'

import type { BlueprintProgramRunRecord } from '../schemas/program-run'
import { blueprintProgramRunIsEvidenceOnly } from '../schemas/program-run'

/** The schema class of the candidate-feedback signal (brain-audit family). */
export const PROGRAM_FAILURE_GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION =
  'psionic.probe_gepa_candidate_feedback.v1' as const

/** The Release Gate any refinement informed by this feedback must clear. */
export const PROGRAM_FAILURE_GEPA_RELEASE_GATE_REF =
  'blueprint.release_gate.qa_failure_gepa_candidate.v1' as const

/** The honest verify-class verdict that triggered the feedback (Tassadar vocab). */
export const ProgramFailureVerdict = S.Literals(['REFUTED', 'INCONCLUSIVE', 'fail'])
export type ProgramFailureVerdict = typeof ProgramFailureVerdict.Type

/** One claim-level negative signal: a contradicted claim, public-safe. */
export const GepaCandidateFeedbackItem = S.Struct({
  /** Stable id of the contradicted claim/commitment. */
  claimId: S.String,
  /** The public-safe claim that was contradicted. */
  claim: S.String,
  /** The observed contradicting evidence (public-safe one-liner). */
  evidenceSummary: S.String,
  /** Failure learning emits NEGATIVE signal only. */
  polarity: S.Literal('negative'),
})
export type GepaCandidateFeedbackItem = typeof GepaCandidateFeedbackItem.Type

/**
 * The non-negotiable governance posture, modeled in the schema so a decode
 * pins every invariant to its only legal value (evidence-only, gated, no
 * self-promotion, not live).
 */
export const GepaCandidateFeedbackGovernance = S.Struct({
  authorityBoundary: S.Literal('evidence_only'),
  requiresReleaseGate: S.Literal(true),
  releaseGateRef: S.String,
  selfPromotionAllowed: S.Literal(false),
  live: S.Literal(false),
})
export type GepaCandidateFeedbackGovernance = typeof GepaCandidateFeedbackGovernance.Type

/**
 * The claim-level GEPA candidate-feedback signal emitted from a failed/REFUTED
 * Khala program turn. References the source program-run record (traceability)
 * and carries the per-claim negative feedback the optimizer may consume. The
 * optimizer kind it feeds is named (`gepa_style_reflection` /
 * `retained_failure_replay`) so it slots into the existing Blueprint optimizer
 * vocabulary.
 */
export const GepaCandidateFeedback = S.Struct({
  schemaVersion: S.Literal(PROGRAM_FAILURE_GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION),
  /** Stable, public-safe ref for this feedback signal. */
  feedbackRef: S.String,
  /** The source program-run record this feedback was derived from. */
  sourceProgramRunRef: S.String,
  /** The Blueprint program signature the failed run was for. */
  programSignatureId: S.String,
  /** The single model under study (one model: openagents/khala). */
  model: S.String,
  /** The honest verdict that triggered the feedback. */
  trigger: ProgramFailureVerdict,
  /** The optimizer kind this feedback feeds (existing Blueprint vocabulary). */
  optimizerKind: S.Literals(['gepa_style_reflection', 'retained_failure_replay']),
  /** The per-claim negative feedback items. */
  items: S.Array(GepaCandidateFeedbackItem),
  /** The non-negotiable governance posture. */
  governance: GepaCandidateFeedbackGovernance,
})
export type GepaCandidateFeedback = typeof GepaCandidateFeedback.Type

export class GepaCandidateFeedbackError extends S.TaggedErrorClass<GepaCandidateFeedbackError>()(
  'GepaCandidateFeedbackError',
  { reason: S.String },
) {}

/** A contradicted claim/commitment captured from the failed turn (public-safe). */
export type ProgramFailureFinding = Readonly<{
  claimId: string
  claim: string
  evidenceSummary: string
}>

export type EmitGepaCandidateFeedbackInput = Readonly<{
  /** The evidence-only program-run record the failed turn produced. */
  programRun: BlueprintProgramRunRecord
  /** The honest verdict that triggered the feedback. */
  trigger: ProgramFailureVerdict
  /** The contradicted findings (refuted commitments / failed claims). */
  findings: ReadonlyArray<ProgramFailureFinding>
  /** The optimizer kind to feed; defaults to GEPA-style reflection. */
  optimizerKind?: GepaCandidateFeedback['optimizerKind']
}>

const shortHash = (text: string): string => {
  let hash = 0x811c9dc5
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash.toString(16).padStart(8, '0')
}

/**
 * Emit a governed GEPA candidate-feedback signal from a failed/REFUTED Khala
 * program run. EVIDENCE-ONLY by construction; fails closed when:
 *   - the source program-run record is NOT evidence-only (we never derive
 *     feedback from a record that carries write authority),
 *   - the trigger is not an honest failure verdict, or
 *   - there are no contradicted findings (no signal -> no fabricated feedback).
 * The governance posture is fixed and re-asserted (decode) before return so the
 * signal can never leave self-promotable or with authority.
 */
export const emitGepaCandidateFeedback = (
  input: EmitGepaCandidateFeedbackInput,
): Effect.Effect<GepaCandidateFeedback, GepaCandidateFeedbackError> =>
  Effect.gen(function* () {
    // Fail closed: never derive feedback from a record with write authority.
    if (!blueprintProgramRunIsEvidenceOnly(input.programRun)) {
      return yield* Effect.fail(
        new GepaCandidateFeedbackError({
          reason: 'source program-run record is not evidence-only',
        }),
      )
    }
    if (input.findings.length === 0) {
      return yield* Effect.fail(
        new GepaCandidateFeedbackError({
          reason: 'no contradicted findings — failure learning never fabricates feedback',
        }),
      )
    }

    const items = input.findings.map(f => ({
      claimId: f.claimId,
      claim: f.claim,
      evidenceSummary: f.evidenceSummary,
      polarity: 'negative' as const,
    }))

    const digest = shortHash(
      JSON.stringify({ run: input.programRun.id, items, trigger: input.trigger }),
    )

    const candidate: GepaCandidateFeedback = {
      schemaVersion: PROGRAM_FAILURE_GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION,
      feedbackRef: `gepa_candidate_feedback.khala_program.${input.programRun.id}.${digest}`,
      sourceProgramRunRef: input.programRun.id,
      programSignatureId: input.programRun.programSignatureId,
      model:
        typeof input.programRun.metadata.model === 'string'
          ? input.programRun.metadata.model
          : 'openagents/khala',
      trigger: input.trigger,
      optimizerKind: input.optimizerKind ?? 'gepa_style_reflection',
      items,
      governance: {
        authorityBoundary: 'evidence_only',
        requiresReleaseGate: true,
        releaseGateRef: PROGRAM_FAILURE_GEPA_RELEASE_GATE_REF,
        selfPromotionAllowed: false,
        live: false,
      },
    }

    // Re-assert the whole shape (and thus every governance invariant) by decode;
    // the schema literals make any tampering a decode failure (fail closed).
    return yield* S.decodeUnknownEffect(GepaCandidateFeedback)(candidate).pipe(
      Effect.mapError(
        error =>
          new GepaCandidateFeedbackError({ reason: `feedback failed governance decode: ${String(error)}` }),
      ),
    )
  })

/** The decision a Release Gate returns for a candidate-feedback signal. */
export type GepaCandidateReleaseGateDecision = Readonly<{
  promoted: boolean
  reason: string
}>

/**
 * Evaluate the Release Gate for a candidate-feedback signal WITHOUT an operator
 * approval. Per the Blueprint evidence-only invariants this MUST reject (no
 * self-promotion). It exists so a test can prove the gate rejects unapproved
 * candidate-feedback; it has NO path that returns `promoted: true`.
 */
export const evaluateGepaCandidateReleaseGateWithoutApproval = (
  feedback: GepaCandidateFeedback,
): GepaCandidateReleaseGateDecision => ({
  promoted: false,
  reason:
    'release_gate_rejected: QA failure-learning candidate-feedback is never self-promoted; ' +
    `promotion of any refinement through ${feedback.governance.releaseGateRef} requires an explicit operator approval.`,
})
