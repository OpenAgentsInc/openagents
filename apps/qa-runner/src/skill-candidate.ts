// The distiller skill-emitter (spec §E.1): lower a `KhalaSessionTrace` into a
// GOVERNED Blueprint optimizer skill candidate.
//
// This is a CANDIDATE only — NOT a live skill, NOT a marketplace listing, and
// NOT promotable by the distiller. It is the second emitter alongside the e2e
// emitter: one capture + one distiller serves two products (spec §E). Where the
// e2e emitter renders a committable executor-style scenario, this emitter renders
// the typed Blueprint candidate that, on a FUTURE owner-gated path, would enter
// Blueprint as an `optimizer_candidate` BlueprintModuleVersion behind its
// signature, be refined by GEPA, and be promoted ONLY through a
// BlueprintReleaseGate (operator-approved; a self-promotion attempt is rejected).
//
// Invariants carried here (spec §D.3 / §G), enforced by construction:
//   - `moduleKind: 'optimizer_candidate'` — a candidate, never a promoted module.
//   - `governance.authorityBoundary: 'evidence_only'` — the candidate acts on
//     nothing; producing it moves no money, deploys nothing, mutates no source.
//   - `governance.requiresReleaseGate: true` + a named gate ref — nothing the
//     distiller produces is live until a human promotes it through the gate.
//   - `governance.selfPromotionAllowed: false` — NO self-promotion, ever. The
//     distiller cannot promote its own candidate; `assertSkillCandidateGoverned`
//     fails closed if any of these flags are wrong.
//   - honest verification class (no exactness inflation) and public-safe (the
//     candidate inherits the trace's public-safety tripwire).
//
// Settlement of any rev-share from a FUTURE use of a promoted skill stays behind
// the INERT/OWNER-GATED settlement seam (`run-settlement.ts`); this emitter never
// references live money.

import {
  assertSessionTracePublicSafe,
  type KhalaSessionTrace,
  type TypedField,
} from "./session-trace";
import type { BlueprintProgramSignature, VerificationClass } from "./distiller";

/** The Release Gate every distilled skill candidate must clear before promotion. */
export const DISTILLER_SKILL_RELEASE_GATE_REF = "blueprint.release_gate.distiller_skill_candidate.v1";

/** The verification-ladder tier a candidate is honestly placed on (NIP-SKL E/D/S/N). */
export type SkillLadderTier = "N" | "S" | "D" | "E";

/**
 * The governance posture of a distilled skill candidate. Every field is a
 * non-negotiable invariant; `assertSkillCandidateGoverned` re-checks them.
 */
export interface SkillCandidateGovernance {
  /** A distilled candidate never carries write authority. */
  readonly authorityBoundary: "evidence_only";
  /** A candidate can only ever be promoted through an operator Release Gate. */
  readonly requiresReleaseGate: true;
  /** The named gate ref that must approve before this becomes live. */
  readonly releaseGateRef: string;
  /** The distiller can NEVER promote its own candidate. */
  readonly selfPromotionAllowed: false;
  /** A candidate is not live; it is inert evidence until promoted. */
  readonly live: false;
}

/**
 * (E.1) A governed Blueprint optimizer skill candidate distilled from a trace.
 * Typed I/O contract + honest verification class + the NIP-SKL ladder tier +
 * the governance posture. It carries refs/digests only — never raw trace text.
 */
export interface SkillCandidate {
  readonly kind: "blueprint_optimizer_skill_candidate";
  /** A candidate, NOT a promoted module version. */
  readonly moduleKind: "optimizer_candidate";
  /** A url-safe slug for the candidate (mirrors the e2e slug). */
  readonly slug: string;
  /** Typed I/O contract inferred from the trace (no `any`). */
  readonly signature: BlueprintProgramSignature;
  /** The honest verification class the candidate carries (no inflation). */
  readonly verificationClass: VerificationClass;
  /**
   * The honest NIP-SKL ladder tier:
   *   E = exact_trace_replay, S = test_passed, D = seeded, N = none.
   */
  readonly ladderTier: SkillLadderTier;
  /** The source trace digest (traceability; never the raw beats). */
  readonly sourceDigest: string;
  /** The model that drove the captured session (one model: openagents/khala). */
  readonly model: string;
  /** Receipt refs the candidate inherits from the trace (public-safe). */
  readonly receiptRefs: ReadonlyArray<string>;
  /** The non-negotiable governance posture. */
  readonly governance: SkillCandidateGovernance;
}

export class SkillCandidateGovernanceError extends Error {
  constructor(reason: string) {
    super(`skill_candidate_governance_violation: ${reason}`);
    this.name = "SkillCandidateGovernanceError";
  }
}

/** Map an honest verification class to its NIP-SKL ladder tier. */
function ladderTierFor(verificationClass: VerificationClass): SkillLadderTier {
  switch (verificationClass) {
    case "exact_trace_replay":
      return "E";
    case "test_passed":
      return "S";
    case "seeded":
      return "D";
    case "none":
      return "N";
  }
}

/**
 * Emit a governed optimizer skill candidate from a trace + its already-inferred
 * signature + honest verification class. Fail-closed on a non-public-safe trace
 * (never emit a candidate from one). The governance posture is fixed at
 * construction and re-asserted before return so a candidate can never leave this
 * function self-promotable or with write authority.
 */
export function emitSkillCandidate(input: {
  readonly trace: KhalaSessionTrace;
  readonly signature: BlueprintProgramSignature;
  readonly verificationClass: VerificationClass;
  readonly slug: string;
}): SkillCandidate {
  // Fail closed: never emit a candidate from a non-public-safe trace.
  assertSessionTracePublicSafe(input.trace);

  const candidate: SkillCandidate = {
    kind: "blueprint_optimizer_skill_candidate",
    moduleKind: "optimizer_candidate",
    slug: input.slug,
    signature: input.signature,
    verificationClass: input.verificationClass,
    ladderTier: ladderTierFor(input.verificationClass),
    sourceDigest: input.trace.digest,
    model: input.trace.model,
    receiptRefs: [...input.trace.receipts],
    governance: {
      authorityBoundary: "evidence_only",
      requiresReleaseGate: true,
      releaseGateRef: DISTILLER_SKILL_RELEASE_GATE_REF,
      selfPromotionAllowed: false,
      live: false,
    },
  };

  // Defensive: the candidate must be governed (evidence-only, gated, no
  // self-promotion, not live) before it can leave this module.
  assertSkillCandidateGoverned(candidate);
  // And it must inherit the trace's public-safety (refs/digests only).
  assertSessionTracePublicSafe(candidate);
  return candidate;
}

/**
 * Re-assert the governance invariants of a skill candidate. This is the
 * "Release Gate, no self-promotion, evidence-only" guard a promotion path MUST
 * run before it could ever consider promoting a candidate. It throws (fail
 * closed) on any violation; it NEVER promotes.
 */
export function assertSkillCandidateGoverned(candidate: SkillCandidate): void {
  if (candidate.moduleKind !== "optimizer_candidate") {
    throw new SkillCandidateGovernanceError(
      `moduleKind must be 'optimizer_candidate' (a candidate, never promoted); got '${candidate.moduleKind}'`,
    );
  }
  if (candidate.governance.authorityBoundary !== "evidence_only") {
    throw new SkillCandidateGovernanceError("authorityBoundary must be 'evidence_only'");
  }
  if (candidate.governance.requiresReleaseGate !== true) {
    throw new SkillCandidateGovernanceError("requiresReleaseGate must be true");
  }
  if (candidate.governance.selfPromotionAllowed !== false) {
    throw new SkillCandidateGovernanceError("selfPromotionAllowed must be false (no self-promotion, ever)");
  }
  if (candidate.governance.live !== false) {
    throw new SkillCandidateGovernanceError("a distilled candidate must not be live");
  }
  if (candidate.governance.releaseGateRef.trim() === "") {
    throw new SkillCandidateGovernanceError("a candidate must name the Release Gate ref that gates its promotion");
  }
  // Honest typing: no `any` field may ride into a candidate signature.
  const anyTyped = [...candidate.signature.inputs, ...candidate.signature.outputs].some(
    (f: TypedField) => f.type.toLowerCase() === "any" || f.type.trim() === "",
  );
  if (anyTyped) {
    throw new SkillCandidateGovernanceError("a candidate signature field has type 'any' or empty");
  }
}

/**
 * The decision a Release Gate would return for a candidate. The distiller can
 * COMPUTE this (to prove the candidate is gated) but it can NEVER act on it: a
 * candidate is only promoted by a separate operator-approved gate run.
 */
export interface ReleaseGateDecision {
  readonly promoted: boolean;
  readonly reason: string;
}

/**
 * Evaluate the Release Gate for a candidate WITHOUT an operator approval.
 * Per spec §D.3/§D.4 this MUST reject (no self-promotion). It exists so a test
 * can prove the gate rejects an unapproved candidate; it has no path that
 * returns `promoted: true`.
 */
export function evaluateReleaseGateWithoutApproval(candidate: SkillCandidate): ReleaseGateDecision {
  assertSkillCandidateGoverned(candidate);
  return {
    promoted: false,
    reason:
      "release_gate_rejected: a distilled optimizer_candidate is never self-promoted; " +
      `promotion through ${candidate.governance.releaseGateRef} requires an explicit operator approval.`,
  };
}
