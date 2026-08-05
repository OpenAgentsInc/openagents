/**
 * Evidence rungs (issue #9321 scope 3): `pledged -> reserved -> measured ->
 * verified -> paid -> settled`, rendering the narrowest rung the exact
 * evidence proves. Never inferred upward.
 *
 * A provider-signed status is evidence about a provider's claim, NEVER
 * settlement truth. Chain/Lightning facts are attributed to their verifying
 * source, and a status alone can never render as settled/complete
 * (`swp_settlement_overclaim` when a claim exceeds verifier evidence).
 */
import {
  AUTHORITY_RUNG_CAP,
  EVIDENCE_RUNGS,
  rungIndex,
  type EvidenceRung,
  type SwapEvidence,
} from "./model.js";
import { evidenceRequirementFor } from "./states.js";

export interface AttributedFact {
  readonly evidence: SwapEvidence;
  /** min(claimed rung, the authority's §11 cap): what this source can prove. */
  readonly provenRung: EvidenceRung;
  /** True when the evidence claimed more than its authority can establish. */
  readonly overclaim: boolean;
}

export function attributeEvidence(evidence: SwapEvidence): AttributedFact {
  const cap = AUTHORITY_RUNG_CAP[evidence.authority];
  const claimed = rungIndex(evidence.rung);
  const capped = Math.min(claimed, rungIndex(cap));
  return {
    evidence,
    provenRung: EVIDENCE_RUNGS[capped]!,
    overclaim: claimed > rungIndex(cap),
  };
}

export interface RungView {
  /** The narrowest rung the exact evidence proves; null when no evidence. */
  readonly proven: EvidenceRung | null;
  /** Every fact with its source kept visible — never merged into one narrative. */
  readonly facts: readonly AttributedFact[];
}

export function provenRungView(evidence: readonly SwapEvidence[]): RungView {
  const facts = evidence.map(attributeEvidence);
  let best = -1;
  for (const fact of facts) {
    best = Math.max(best, rungIndex(fact.provenRung));
  }
  return { proven: best < 0 ? null : EVIDENCE_RUNGS[best]!, facts };
}

export type ClaimVerdict =
  | { readonly kind: "no_evidence_required" }
  | { readonly kind: "proved" }
  | {
      readonly kind: "unproven";
      readonly error: "swp_settlement_overclaim";
      readonly requiredRung: EvidenceRung;
      readonly provenRung: EvidenceRung | null;
    };

/**
 * Whether the exact evidence proves a claimed swp_state's rail fact. Status
 * events themselves never count: their authority caps at `pledged`.
 */
export function claimVerdict(swpState: string, evidence: readonly SwapEvidence[]): ClaimVerdict {
  const requirement = evidenceRequirementFor(swpState);
  if (requirement === undefined) return { kind: "no_evidence_required" };
  const facts = evidence
    .filter((item) => requirement.classes.includes(item.class))
    .map(attributeEvidence);
  const satisfied = facts.some(
    (fact) =>
      rungIndex(fact.provenRung) >= rungIndex(requirement.minRung) &&
      (requirement.requiresFinal !== true || fact.evidence.final === true),
  );
  if (satisfied) return { kind: "proved" };
  // The rung reported for the refusal is scoped to the claim's own evidence
  // classes: settled evidence about a different artifact (e.g. the funding
  // output) must not dress up an unproven completion claim.
  let best = -1;
  for (const fact of facts) {
    best = Math.max(best, rungIndex(fact.provenRung));
  }
  return {
    kind: "unproven",
    error: "swp_settlement_overclaim",
    requiredRung: requirement.minRung,
    provenRung: best < 0 ? null : EVIDENCE_RUNGS[best]!,
  };
}
