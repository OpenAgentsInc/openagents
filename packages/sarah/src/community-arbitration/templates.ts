/**
 * Unsigned Nostr event templates for SARAH-CW-05 arbitration lifecycle.
 * Kind 7000 feedback, empty content, public-safe tags only.
 */
import {
  ARBITRATION_DECISION_ALT,
  AUTHORITY_DECISION_RECEIPT_SCHEMA,
  COMMUNITY_ARBITRATION_FEEDBACK_KIND,
  DISPUTE_APPEAL_ALT,
  OWNER_RULING_ALT,
  type CommunityArbitrationEventTemplate,
  type DisputeAppeal,
  type OwnerRuling,
  type SarahArbitrationDecision,
} from "./types.ts";
import {
  validateArbitrationDecision,
  validateDisputeAppeal,
} from "./rules.ts";

const createdAtSeconds = (iso: string, override?: number): number => {
  if (override !== undefined) {
    return override;
  }
  const ms = Date.parse(iso);
  if (!Number.isFinite(ms)) {
    throw new Error("community_arbitration: invalid ISO timestamp");
  }
  return Math.floor(ms / 1000);
};

/**
 * Build the public kind-7000 template for a Sarah arbitration decision.
 * Callers sign with Sarah's key. Content is always empty.
 */
export const buildArbitrationDecisionTemplate = (input: {
  readonly decision: unknown;
  readonly feedsSarahClaim?: boolean;
  readonly createdAt?: number;
}): {
  readonly template: CommunityArbitrationEventTemplate;
  readonly decision: SarahArbitrationDecision;
} => {
  const decision = validateArbitrationDecision(input.decision, {
    feedsSarahClaim: input.feedsSarahClaim === true,
  });

  const tags: string[][] = [
    ["e", decision.requestEventId, "", "request"],
    ["e", decision.resultEventId, "", "result"],
    ["p", decision.providerPubkey],
    ["agent", decision.sarahPubkey],
    ["status", decision.outcome],
    ["lbr_feedback_type", "arbitration_decision"],
    ["cw_feedback_type", "arbitration_decision"],
    ["cw_decision_ref", decision.decisionRef],
    ["cw_unit_ref", decision.unitRef],
    ["cw_authority_receipt_schema", AUTHORITY_DECISION_RECEIPT_SCHEMA],
    ["cw_authority_receipt_ref", decision.authorityReceiptRef],
    ["cw_decides_payment", "false"],
    ["cw_decided_at", decision.decidedAt],
    ["alt", ARBITRATION_DECISION_ALT],
  ];

  if (decision.reasonClass !== undefined) {
    tags.push(["cw_reason_class", decision.reasonClass]);
  }
  if (decision.reasonSummary !== undefined) {
    tags.push(["cw_reason_summary", decision.reasonSummary]);
  }
  for (const ref of decision.evidenceRefs) {
    tags.push(["cw_evidence_ref", ref]);
  }
  if (decision.independence !== undefined) {
    const ind = decision.independence;
    tags.push(
      ["cw_producer_operator_ref", ind.producerOperatorRef],
      ["cw_producer_agent_pubkey", ind.producerAgentPubkey],
      ["cw_verifier_operator_ref", ind.verifierOperatorRef],
      ["cw_verifier_agent_pubkey", ind.verifierAgentPubkey],
      ["cw_verification_receipt_ref", ind.verificationReceiptRef],
    );
  }

  return {
    decision,
    template: {
      kind: COMMUNITY_ARBITRATION_FEEDBACK_KIND,
      created_at: createdAtSeconds(decision.decidedAt, input.createdAt),
      tags,
      content: "",
    },
  };
};

/**
 * Build the public kind-7000 template for a member dispute appeal.
 * Callers sign with the appellant's key.
 */
export const buildDisputeAppealTemplate = (input: {
  readonly appeal: unknown;
  readonly createdAt?: number;
}): {
  readonly template: CommunityArbitrationEventTemplate;
  readonly appeal: DisputeAppeal;
} => {
  const appeal = validateDisputeAppeal(input.appeal);

  const tags: string[][] = [
    ["e", appeal.decisionEventId, "", "decision"],
    ["e", appeal.requestEventId, "", "request"],
    ["e", appeal.resultEventId, "", "result"],
    ["p", appeal.appellantPubkey],
    ["status", "appeal_open"],
    ["lbr_feedback_type", "dispute_appeal"],
    ["cw_feedback_type", "dispute_appeal"],
    ["cw_appeal_ref", appeal.appealRef],
    ["cw_decision_ref", appeal.decisionRef],
    ["cw_appellant_operator_ref", appeal.appellantOperatorRef],
    ["cw_grounds", appeal.grounds],
    ["cw_grounds_summary", appeal.groundsSummary],
    ["cw_arbiter", "owner"],
    ["cw_filed_at", appeal.filedAt],
    ["alt", DISPUTE_APPEAL_ALT],
  ];
  for (const ref of appeal.evidenceRefs) {
    tags.push(["cw_evidence_ref", ref]);
  }

  return {
    appeal,
    template: {
      kind: COMMUNITY_ARBITRATION_FEEDBACK_KIND,
      created_at: createdAtSeconds(appeal.filedAt, input.createdAt),
      tags,
      content: "",
    },
  };
};

/**
 * Build the public kind-7000 template for an owner ruling.
 * Callers must sign with the admitted owner appeal key — never Sarah's key.
 * This function does not sign; it only shapes the unsigned template.
 */
export const buildOwnerRulingTemplate = (input: {
  readonly ruling: OwnerRuling;
  readonly createdAt?: number;
}): {
  readonly template: CommunityArbitrationEventTemplate;
  readonly ruling: OwnerRuling;
} => {
  const ruling = input.ruling;

  const tags: string[][] = [
    ["e", ruling.appealEventId, "", "appeal"],
    ["p", ruling.ownerAppealPubkey],
    ["status", ruling.outcome],
    ["lbr_feedback_type", "owner_ruling"],
    ["cw_feedback_type", "owner_ruling"],
    ["cw_ruling_ref", ruling.rulingRef],
    ["cw_appeal_ref", ruling.appealRef],
    ["cw_decision_ref", ruling.decisionRef],
    ["cw_owner_appeal_pubkey", ruling.ownerAppealPubkey],
    ["cw_author_role", "owner_arbiter_of_last_resort"],
    ["cw_reason_summary", ruling.reasonSummary],
    ["cw_ruled_at", ruling.ruledAt],
    ["alt", OWNER_RULING_ALT],
  ];
  for (const ref of ruling.evidenceRefs) {
    tags.push(["cw_evidence_ref", ref]);
  }

  return {
    ruling,
    template: {
      kind: COMMUNITY_ARBITRATION_FEEDBACK_KIND,
      created_at: createdAtSeconds(ruling.ruledAt, input.createdAt),
      tags,
      content: "",
    },
  };
};

/** Tag lookup helper for tests and decoders. */
export const tagValue = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): string | undefined => {
  for (const tag of tags) {
    if (tag[0] === name && tag[1] !== undefined) {
      return tag[1];
    }
  }
  return undefined;
};

export const tagValues = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): ReadonlyArray<string> =>
  tags.flatMap((tag) =>
    tag[0] === name && tag[1] !== undefined ? [tag[1]] : [],
  );
