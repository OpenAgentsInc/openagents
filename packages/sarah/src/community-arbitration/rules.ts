/**
 * Arbitration and dispute-path rules for SARAH-CW-05.
 * Four §34 bounds plus owner-as-arbiter-of-last-resort (§40.1).
 */
import { Schema as S } from "effect";

import {
  DisputeAppealSchema,
  OwnerRulingSchema,
  SarahArbitrationDecisionSchema,
  type DisputeAppeal,
  type IndependenceEvidence,
  type OwnerAppealIdentity,
  type OwnerAppealIdentityResolution,
  type OwnerRuling,
  type SarahArbitrationDecision,
} from "./types.ts";

export type ArbitrationRuleViolationCode =
  | "decides_payment_forbidden"
  | "rejection_missing_reason_class"
  | "acceptance_with_reason_class"
  | "self_dealing_operators"
  | "self_dealing_keys_only"
  | "independence_required_for_sarah_claim"
  | "sarah_authored_owner_ruling"
  | "owner_ruling_pubkey_mismatch"
  | "owner_appeal_identity_missing"
  | "appeal_window_required_before_payout"
  | "schema_invalid";

export class ArbitrationRuleError extends Error {
  readonly code: ArbitrationRuleViolationCode;

  constructor(code: ArbitrationRuleViolationCode, message: string) {
    super(message);
    this.name = "ArbitrationRuleError";
    this.code = code;
  }
}

const decodeDecision = S.decodeUnknownSync(SarahArbitrationDecisionSchema);
const decodeAppeal = S.decodeUnknownSync(DisputeAppealSchema);
const decodeRuling = S.decodeUnknownSync(OwnerRulingSchema);

/** Spec §34 rule 1: Sarah decides acceptance, never payment. */
export const assertDoesNotDecidePayment = (
  decision: SarahArbitrationDecision,
): void => {
  if (decision.decidesPayment !== false) {
    throw new ArbitrationRuleError(
      "decides_payment_forbidden",
      "Sarah arbitration must not decide payment; settlement stays in the platform ledger",
    );
  }
};

/**
 * Spec §34 rule 3: a rejection is a typed outcome with a reason class.
 * An acceptance must not smuggle a rejection reason.
 */
export const assertTypedRejection = (
  decision: SarahArbitrationDecision,
): void => {
  if (decision.outcome === "rejected") {
    if (decision.reasonClass === undefined) {
      throw new ArbitrationRuleError(
        "rejection_missing_reason_class",
        "A rejected result must carry a typed reason class, not silence",
      );
    }
    return;
  }
  if (decision.reasonClass !== undefined) {
    throw new ArbitrationRuleError(
      "acceptance_with_reason_class",
      "An accepted result must not carry a rejection reason class",
    );
  }
};

/**
 * Spec anti-self-dealing: producer and verifier must have distinct operators,
 * not merely distinct keys. A single operator running both sides is refused.
 */
export const assertIndependentOperators = (
  evidence: IndependenceEvidence,
): void => {
  if (evidence.producerOperatorRef === evidence.verifierOperatorRef) {
    throw new ArbitrationRuleError(
      "self_dealing_operators",
      "Producer and verifier must have distinct operators",
    );
  }
  if (evidence.producerAgentPubkey === evidence.verifierAgentPubkey) {
    throw new ArbitrationRuleError(
      "self_dealing_keys_only",
      "Producer and verifier agent keys must differ",
    );
  }
};

/**
 * Spec §34 rule 2: Sarah cannot verify her own production. When a unit's
 * output feeds a Sarah claim, independence evidence is required and must pass
 * the distinct-operator check.
 */
export const assertIndependenceWhenRequired = (
  decision: SarahArbitrationDecision,
  options: Readonly<{ readonly feedsSarahClaim: boolean }>,
): void => {
  if (!options.feedsSarahClaim) {
    if (decision.independence !== undefined) {
      assertIndependentOperators(decision.independence);
    }
    return;
  }
  if (decision.independence === undefined) {
    throw new ArbitrationRuleError(
      "independence_required_for_sarah_claim",
      "Units that feed a Sarah claim require an independent verifier with a distinct operator",
    );
  }
  assertIndependentOperators(decision.independence);
};

/**
 * Decode and enforce all Sarah-side arbitration rules for a decision payload.
 */
export const validateArbitrationDecision = (
  raw: unknown,
  options: Readonly<{ readonly feedsSarahClaim?: boolean }> = {},
): SarahArbitrationDecision => {
  let decision: SarahArbitrationDecision;
  try {
    decision = decodeDecision(raw, { onExcessProperty: "error" });
  } catch (cause) {
    throw new ArbitrationRuleError(
      "schema_invalid",
      cause instanceof Error ? cause.message : "invalid arbitration decision",
    );
  }
  assertDoesNotDecidePayment(decision);
  assertTypedRejection(decision);
  assertIndependenceWhenRequired(decision, {
    feedsSarahClaim: options.feedsSarahClaim === true,
  });
  return decision;
};

export const validateDisputeAppeal = (raw: unknown): DisputeAppeal => {
  try {
    return decodeAppeal(raw, { onExcessProperty: "error" });
  } catch (cause) {
    throw new ArbitrationRuleError(
      "schema_invalid",
      cause instanceof Error ? cause.message : "invalid dispute appeal",
    );
  }
};

/**
 * Spec §40.1: a ruling is a signed event from the owner appeal key.
 * Sarah cannot author one. The author pubkey must match the admitted identity.
 */
export const validateOwnerRuling = (
  raw: unknown,
  context: Readonly<{
    readonly authorPubkey: string;
    readonly appealIdentity: OwnerAppealIdentityResolution;
    /** Sarah's pubkey — must not equal the ruling author. */
    readonly sarahPubkey: string;
  }>,
): OwnerRuling => {
  let ruling: OwnerRuling;
  try {
    ruling = decodeRuling(raw, { onExcessProperty: "error" });
  } catch (cause) {
    throw new ArbitrationRuleError(
      "schema_invalid",
      cause instanceof Error ? cause.message : "invalid owner ruling",
    );
  }

  if (context.appealIdentity.lifecycle === "missing") {
    throw new ArbitrationRuleError(
      "owner_appeal_identity_missing",
      "Owner appeal identity is not registered; rulings cannot be verified",
    );
  }

  const identity = context.appealIdentity as OwnerAppealIdentity;
  if (identity.lifecycle === "revoked") {
    throw new ArbitrationRuleError(
      "owner_appeal_identity_missing",
      "Owner appeal identity is revoked; register a rotation before ruling",
    );
  }

  const author = context.authorPubkey.toLowerCase();
  if (author === context.sarahPubkey.toLowerCase()) {
    throw new ArbitrationRuleError(
      "sarah_authored_owner_ruling",
      "Sarah cannot author an owner ruling; the owner is the arbiter of last resort",
    );
  }
  if (author !== identity.pubkey.toLowerCase()) {
    throw new ArbitrationRuleError(
      "owner_ruling_pubkey_mismatch",
      "Owner ruling author pubkey must match the admitted owner appeal identity",
    );
  }
  if (ruling.ownerAppealPubkey.toLowerCase() !== identity.pubkey.toLowerCase()) {
    throw new ArbitrationRuleError(
      "owner_ruling_pubkey_mismatch",
      "Owner ruling projection must cite the admitted owner appeal pubkey",
    );
  }
  return ruling;
};

/**
 * Spec §34 rule 4: a dispute path must exist before any payout.
 * v1 awards experience only and pays nothing, but the gate still holds so a
 * later paid version cannot settle while an open appeal is possible or open.
 */
export const assertPayoutBlockedUntilDisputePath = (
  state: Readonly<{
    readonly disputePathExists: boolean;
    readonly openAppealRefs: ReadonlyArray<string>;
    readonly attemptingPayout: boolean;
  }>,
): void => {
  if (!state.attemptingPayout) {
    return;
  }
  if (!state.disputePathExists) {
    throw new ArbitrationRuleError(
      "appeal_window_required_before_payout",
      "A dispute path must exist before any payout",
    );
  }
  if (state.openAppealRefs.length > 0) {
    throw new ArbitrationRuleError(
      "appeal_window_required_before_payout",
      "Payout is blocked while an appeal is open",
    );
  }
};
