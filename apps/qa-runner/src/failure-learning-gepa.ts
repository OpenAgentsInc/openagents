// QA failure learning — the Blueprint/GEPA half (#6195).
//
// This is the part that makes failure learning OURS, not a droid-control copy: a
// failed/REFUTED run (or a low eval) emits a CLAIM-LEVEL GEPA CANDIDATE-FEEDBACK
// signal — the `psionic.probe_gepa_candidate_manifest.v1`-class feedback the
// brain/Blueprint audit describes (StudyBench -> GEPA). The optimizer can then
// refine the program/prompt/module AS A CANDIDATE.
//
// Strictly EVIDENCE-ONLY + GOVERNED (Blueprint invariants; mirrors the distiller
// skill-candidate posture in `skill-candidate.ts`):
//   - authorityBoundary: 'evidence_only' — the feedback acts on nothing; emitting
//     it moves no money, deploys nothing, mutates no source, changes no live
//     behavior. It is a candidate-feedback record the optimizer MAY consume.
//   - requiresReleaseGate: true + a named gate ref — nothing this produces is
//     live until a human promotes a refined candidate through the gate.
//   - selfPromotionAllowed: false — NO self-promotion, ever. The failure-learning
//     loop cannot promote its own candidate-feedback into live behavior. The
//     Release Gate REJECTS an unapproved candidate (`evaluateGepaReleaseGate...`).
//   - PUBLIC-SAFE: refs/digests/evidence summaries only (no prompts/tokens). The
//     same forbidden-field tripwire (`assertPublicSafeResult`) re-checks it.
//
// SCHEMA CLASS: the signal is namespaced in the `psionic.probe_gepa_candidate_*`
// family (the brain audit's vocabulary). It is intentionally a SMALL, claim-level
// FEEDBACK record (not the full probe-benchmark candidate manifest) — failure
// learning feeds candidate FEEDBACK to the optimizer; it does not assemble a
// promoted candidate manifest itself.

import { assertPublicSafeResult } from "./result";
import type { FailurePattern } from "./failure-learning";

/** The schema class of the candidate-feedback signal (brain-audit family). */
export const GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION =
  "psionic.probe_gepa_candidate_feedback.v1" as const;

/** The Release Gate every GEPA candidate-feedback signal must clear before any promotion. */
export const QA_FAILURE_GEPA_RELEASE_GATE_REF =
  "blueprint.release_gate.qa_failure_gepa_candidate.v1" as const;

/**
 * The non-negotiable governance posture of a candidate-feedback signal. Every
 * field is an invariant re-checked by `assertGepaCandidateFeedbackGoverned`.
 */
export interface GepaCandidateFeedbackGovernance {
  /** The signal carries no write authority. */
  readonly authorityBoundary: "evidence_only";
  /** It can only ever inform a candidate that is promoted through a Release Gate. */
  readonly requiresReleaseGate: true;
  /** The named gate ref that must approve before any refinement becomes live. */
  readonly releaseGateRef: string;
  /** The loop can NEVER promote its own feedback. */
  readonly selfPromotionAllowed: false;
  /** Not live: it is inert evidence the optimizer MAY consume. */
  readonly live: false;
}

/** One claim-level negative signal: a refuted/failed claim, public-safe. */
export interface GepaCandidateFeedbackItem {
  /** Stable id of the contradicted claim/step (from the FailurePattern). */
  readonly claimId: string;
  /** The public-safe claim that was contradicted. */
  readonly claim: string;
  /** The observed contradicting evidence (public-safe one-liner). */
  readonly evidenceSummary: string;
  /** The honest feedback polarity. Failure learning emits negative signal only. */
  readonly polarity: "negative";
}

/**
 * The claim-level GEPA candidate-feedback signal emitted from a failed/REFUTED
 * run. It references the source failure pattern (traceability) and carries the
 * per-claim negative feedback the optimizer may use to refine a candidate. It
 * is evidence-only and governed by construction.
 */
export interface GepaCandidateFeedback {
  readonly schemaVersion: typeof GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION;
  /** Stable, public-safe ref for this feedback signal. */
  readonly feedbackRef: string;
  /** The failure pattern this feedback was derived from (public-safe ref). */
  readonly sourcePatternRef: string;
  /** The single model under study (one model: openagents/khala). */
  readonly model: string;
  /** What triggered the feedback (mirrors the pattern source). */
  readonly trigger: FailurePattern["source"];
  /** The per-claim negative feedback items. */
  readonly items: ReadonlyArray<GepaCandidateFeedbackItem>;
  /** The non-negotiable governance posture. */
  readonly governance: GepaCandidateFeedbackGovernance;
}

export class GepaCandidateFeedbackGovernanceError extends Error {
  constructor(reason: string) {
    super(`gepa_candidate_feedback_governance_violation: ${reason}`);
    this.name = "GepaCandidateFeedbackGovernanceError";
  }
}

/** The single model under study (one model, no variants — workspace invariant). */
export const QA_FAILURE_GEPA_MODEL = "openagents/khala" as const;

/**
 * Emit a governed GEPA candidate-feedback signal from a captured failure
 * pattern. Evidence-only, Release-Gate-gated, never self-promoted, not live.
 * The governance posture is fixed at construction and re-asserted before return
 * so the signal can never leave this function self-promotable or with authority.
 *
 * Public-safe by construction; the tripwire re-checks before return.
 */
export function emitGepaCandidateFeedback(input: {
  readonly pattern: FailurePattern;
  readonly model?: string;
}): GepaCandidateFeedback {
  // Inherit the pattern's public-safety (fail closed on a non-public-safe input).
  assertPublicSafeResult(input.pattern);

  const items: GepaCandidateFeedbackItem[] = input.pattern.findings.map((f) => ({
    claimId: f.id,
    claim: f.claim,
    evidenceSummary: f.evidenceSummary,
    polarity: "negative",
  }));

  const feedback: GepaCandidateFeedback = {
    schemaVersion: GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION,
    feedbackRef: input.pattern.patternRef.replace(
      "failure_pattern:qa_runner:",
      "gepa_candidate_feedback:qa_runner:",
    ),
    sourcePatternRef: input.pattern.patternRef,
    model: input.model ?? QA_FAILURE_GEPA_MODEL,
    trigger: input.pattern.source,
    items,
    governance: {
      authorityBoundary: "evidence_only",
      requiresReleaseGate: true,
      releaseGateRef: QA_FAILURE_GEPA_RELEASE_GATE_REF,
      selfPromotionAllowed: false,
      live: false,
    },
  };

  // Defensive: the signal must be governed before it can leave this module.
  assertGepaCandidateFeedbackGoverned(feedback);
  // And it must be public-safe (refs/summaries only).
  assertPublicSafeResult(feedback);
  return feedback;
}

/**
 * Re-assert the governance invariants of a candidate-feedback signal. This is
 * the "evidence-only, Release Gate, no self-promotion" guard a promotion path
 * MUST run before it could ever consider acting on the feedback. Throws (fail
 * closed) on any violation; it NEVER promotes.
 */
export function assertGepaCandidateFeedbackGoverned(
  feedback: GepaCandidateFeedback,
): void {
  const g = feedback.governance;
  if (g.authorityBoundary !== "evidence_only") {
    throw new GepaCandidateFeedbackGovernanceError("authorityBoundary must be 'evidence_only'");
  }
  if (g.requiresReleaseGate !== true) {
    throw new GepaCandidateFeedbackGovernanceError("requiresReleaseGate must be true");
  }
  if (g.selfPromotionAllowed !== false) {
    throw new GepaCandidateFeedbackGovernanceError(
      "selfPromotionAllowed must be false (no self-promotion, ever)",
    );
  }
  if (g.live !== false) {
    throw new GepaCandidateFeedbackGovernanceError("candidate-feedback must not be live");
  }
  if (g.releaseGateRef.trim() === "") {
    throw new GepaCandidateFeedbackGovernanceError(
      "candidate-feedback must name the Release Gate ref that gates any promotion",
    );
  }
  // Failure learning emits NEGATIVE signal only; a positive/promoting item is a
  // governance violation (it would be self-promotion of a refinement).
  if (feedback.items.some((i) => i.polarity !== "negative")) {
    throw new GepaCandidateFeedbackGovernanceError(
      "every feedback item must be negative polarity (failure learning never self-promotes a refinement)",
    );
  }
}

/**
 * The decision a Release Gate would return for a candidate-feedback signal. The
 * loop can COMPUTE this (to prove the feedback is gated) but it can NEVER act on
 * it: a refinement is only promoted by a separate operator-approved gate run.
 */
export interface GepaReleaseGateDecision {
  readonly promoted: boolean;
  readonly reason: string;
}

/**
 * Evaluate the Release Gate for a candidate-feedback signal WITHOUT an operator
 * approval. This MUST reject (no self-promotion). It exists so a test can prove
 * the gate rejects unapproved candidate-feedback; it has NO path that returns
 * `promoted: true`.
 */
export function evaluateGepaReleaseGateWithoutApproval(
  feedback: GepaCandidateFeedback,
): GepaReleaseGateDecision {
  assertGepaCandidateFeedbackGoverned(feedback);
  return {
    promoted: false,
    reason:
      "release_gate_rejected: QA failure-learning candidate-feedback is never self-promoted; " +
      `promotion of any refinement through ${feedback.governance.releaseGateRef} requires an explicit operator approval.`,
  };
}
