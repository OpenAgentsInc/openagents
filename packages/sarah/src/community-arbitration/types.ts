/**
 * SARAH-CW-05 — Sarah community arbitration and dispute path.
 *
 * Owner decision (2026-07-24): the owner is the arbiter of last resort.
 * Sarah accepts and rejects work; she does not decide payment; she cannot be
 * the final word on a decision about her own work. Appeals go to the owner.
 *
 * Spec: docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §34, §40.1
 * Issue: OpenAgentsInc/openagents#9229
 */
import { Schema as S } from "effect";

import { AUTHORITY_DECISION_RECEIPT_SCHEMA } from "@openagentsinc/authority";

export const SARAH_CW_05_PACKET = "SARAH-CW-05" as const;
export const SARAH_CW_05_ISSUE = "OpenAgentsInc/openagents#9229" as const;

/** Public community arbitration decision projection. */
export const SARAH_ARBITRATION_DECISION_SCHEMA =
  "openagents.sarah.community_arbitration_decision.v1" as const;

/** Member dispute appeal against a Sarah arbitration decision. */
export const SARAH_DISPUTE_APPEAL_SCHEMA =
  "openagents.sarah.community_dispute_appeal.v1" as const;

/** Owner ruling as arbiter of last resort. */
export const SARAH_OWNER_RULING_SCHEMA =
  "openagents.sarah.community_owner_ruling.v1" as const;

/** Admitted owner appeal-identity registration (public key only). */
export const SARAH_OWNER_APPEAL_IDENTITY_SCHEMA =
  "openagents.sarah.owner_appeal_identity.v1" as const;

/**
 * NIP-90 feedback kind for public community arbitration lifecycle events.
 * Same transport as NIP-LBR quote/acceptance feedback; content stays empty.
 */
export const COMMUNITY_ARBITRATION_FEEDBACK_KIND = 7000 as const;

export const ARBITRATION_DECISION_ALT =
  "OpenAgents community arbitration decision" as const;
export const DISPUTE_APPEAL_ALT =
  "OpenAgents community dispute appeal" as const;
export const OWNER_RULING_ALT =
  "OpenAgents community owner ruling" as const;

/** Discriminator for kind-7000 community arbitration feedback. */
export const COMMUNITY_FEEDBACK_TYPES = [
  "arbitration_decision",
  "dispute_appeal",
  "owner_ruling",
] as const;
export type CommunityFeedbackType = (typeof COMMUNITY_FEEDBACK_TYPES)[number];

/**
 * Typed rejection reason classes. A rejection is never silence
 * (spec §34 rule 3).
 */
export const ARBITRATION_REASON_CLASSES = [
  "verification_failed",
  "independent_verifier_missing",
  "self_dealing_operators",
  "grant_expired",
  "result_replay",
  "evidence_incomplete",
  "unsafe_payload",
  "objective_not_met",
  "membership_revoked",
  "quote_not_accepted",
  "unit_budget_exceeded",
  "process_error",
] as const;
export type ArbitrationReasonClass =
  (typeof ARBITRATION_REASON_CLASSES)[number];

export const ARBITRATION_OUTCOMES = ["accepted", "rejected"] as const;
export type ArbitrationOutcome = (typeof ARBITRATION_OUTCOMES)[number];

export const APPEAL_GROUNDS = [
  "reason_disputed",
  "evidence_misread",
  "independence_violation_alleged",
  "process_error",
  "new_public_evidence",
] as const;
export type AppealGround = (typeof APPEAL_GROUNDS)[number];

export const OWNER_RULING_OUTCOMES = [
  "uphold",
  "overturn_accept",
  "overturn_reject",
  "remand",
] as const;
export type OwnerRulingOutcome = (typeof OWNER_RULING_OUTCOMES)[number];

export const APPEAL_IDENTITY_LIFECYCLES = [
  "admitted",
  "rotating",
  "revoked",
  "missing",
] as const;
export type AppealIdentityLifecycle =
  (typeof APPEAL_IDENTITY_LIFECYCLES)[number];

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const Npub = S.String.check(S.isPattern(/^npub1[a-z0-9]{58}$/));
const IsoTime = S.String.check(S.isMinLength(10), S.isMaxLength(64));
const Summary = S.Trim.check(S.isMinLength(1), S.isMaxLength(500));

export const ArbitrationReasonClassSchema = S.Literals(
  ARBITRATION_REASON_CLASSES,
);
export const ArbitrationOutcomeSchema = S.Literals(ARBITRATION_OUTCOMES);
export const AppealGroundSchema = S.Literals(APPEAL_GROUNDS);
export const OwnerRulingOutcomeSchema = S.Literals(OWNER_RULING_OUTCOMES);

/**
 * Independent-verification evidence for a unit whose output feeds a Sarah
 * claim. Producer and verifier must have distinct operators (spec anti-self-
 * dealing), not merely distinct keys.
 */
export const IndependenceEvidenceSchema = S.Struct({
  producerOperatorRef: Ref,
  producerAgentPubkey: Hex64,
  verifierOperatorRef: Ref,
  verifierAgentPubkey: Hex64,
  verificationReceiptRef: Ref,
});
export type IndependenceEvidence = S.Schema.Type<
  typeof IndependenceEvidenceSchema
>;

/**
 * Sarah's typed arbitration decision for one community work unit / LBR result.
 * Emits (or cites) the same authority receipt schema Part 2 specifies.
 * Sarah decides acceptance only — never payment.
 */
export const SarahArbitrationDecisionSchema = S.Struct({
  schema: S.Literal(SARAH_ARBITRATION_DECISION_SCHEMA),
  packet: S.Literal(SARAH_CW_05_PACKET),
  decisionRef: Ref,
  requestEventId: Hex64,
  resultEventId: Hex64,
  unitRef: Ref,
  providerPubkey: Hex64,
  /** Sarah's Nostr pubkey (requester / acceptor). */
  sarahPubkey: Hex64,
  outcome: ArbitrationOutcomeSchema,
  /** Required when outcome is rejected; must be absent or unused when accepted. */
  reasonClass: S.optional(ArbitrationReasonClassSchema),
  /** Public-safe summary of the reason (never raw logs or private paths). */
  reasonSummary: S.optional(Summary),
  /** Authority decision receipt schema id — always the Part 2 receipt. */
  authorityReceiptSchema: S.Literal(AUTHORITY_DECISION_RECEIPT_SCHEMA),
  authorityReceiptRef: Ref,
  independence: S.optional(IndependenceEvidenceSchema),
  /** Public-safe evidence refs (artifact, verification, membership). */
  evidenceRefs: S.Array(Ref),
  /** Payment stays in the platform ledger; arbitration never settles. */
  decidesPayment: S.Literal(false),
  decidedAt: IsoTime,
});
export type SarahArbitrationDecision = S.Schema.Type<
  typeof SarahArbitrationDecisionSchema
>;

/**
 * A member's typed dispute appeal. Must exist before any payout path
 * (spec §34 rule 4). Targets a prior arbitration decision.
 */
export const DisputeAppealSchema = S.Struct({
  schema: S.Literal(SARAH_DISPUTE_APPEAL_SCHEMA),
  packet: S.Literal(SARAH_CW_05_PACKET),
  appealRef: Ref,
  /** Event id or decisionRef of the arbitration decision under appeal. */
  decisionRef: Ref,
  decisionEventId: Hex64,
  requestEventId: Hex64,
  resultEventId: Hex64,
  /** Member (human operator) filing the appeal. */
  appellantOperatorRef: Ref,
  appellantPubkey: Hex64,
  grounds: AppealGroundSchema,
  groundsSummary: Summary,
  evidenceRefs: S.Array(Ref),
  /** Destination is the admitted owner appeal identity when registered. */
  arbiterOfLastResort: S.Literal("owner"),
  filedAt: IsoTime,
});
export type DisputeAppeal = S.Schema.Type<typeof DisputeAppealSchema>;

/**
 * Owner ruling as arbiter of last resort. Sarah cannot author this event —
 * the author pubkey must match the admitted owner appeal identity.
 */
export const OwnerRulingSchema = S.Struct({
  schema: S.Literal(SARAH_OWNER_RULING_SCHEMA),
  packet: S.Literal(SARAH_CW_05_PACKET),
  rulingRef: Ref,
  appealRef: Ref,
  appealEventId: Hex64,
  decisionRef: Ref,
  /** Must equal the admitted owner appeal pubkey at ruling time. */
  ownerAppealPubkey: Hex64,
  outcome: OwnerRulingOutcomeSchema,
  reasonSummary: Summary,
  evidenceRefs: S.Array(Ref),
  /** Explicitly not Sarah — for auditors who only read the projection. */
  authorRole: S.Literal("owner_arbiter_of_last_resort"),
  ruledAt: IsoTime,
});
export type OwnerRuling = S.Schema.Type<typeof OwnerRulingSchema>;

/**
 * Single admitted location for the owner's appeal public key.
 * Clients read from this registry; they must not embed a copy.
 * Rotation is modeled by superseding the prior registration with a new
 * revision and leaving the audit trail append-only.
 */
export const OwnerAppealIdentitySchema = S.Struct({
  schema: S.Literal(SARAH_OWNER_APPEAL_IDENTITY_SCHEMA),
  packet: S.Literal(SARAH_CW_05_PACKET),
  lifecycle: S.Literals(["admitted", "rotating", "revoked"]),
  /** 32-byte hex pubkey. Public only — never nsec/mnemonic. */
  pubkey: Hex64,
  /** Optional bech32 npub for human operators. */
  npub: S.optional(Npub),
  revision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  registeredAt: IsoTime,
  /** Who recorded the registration (operator ref, not a secret). */
  registeredByRef: Ref,
  /** Prior registration ref when rotating. */
  supersedesRef: S.optional(Ref),
  registrationRef: Ref,
});
export type OwnerAppealIdentity = S.Schema.Type<
  typeof OwnerAppealIdentitySchema
>;

/** Resolution when the owner appeal key is not yet registered. */
export const OwnerAppealIdentityMissingSchema = S.Struct({
  schema: S.Literal(SARAH_OWNER_APPEAL_IDENTITY_SCHEMA),
  packet: S.Literal(SARAH_CW_05_PACKET),
  lifecycle: S.Literal("missing"),
  needsOwner: S.Literal(true),
  blockerRef: S.Literal("needs_owner.owner_appeal_npub"),
  summary: S.Literal(
    "Owner Nostr public key is not registered as the community appeal identity.",
  ),
});
export type OwnerAppealIdentityMissing = S.Schema.Type<
  typeof OwnerAppealIdentityMissingSchema
>;

export type OwnerAppealIdentityResolution =
  | OwnerAppealIdentity
  | OwnerAppealIdentityMissing;

/** Unsigned NIP-01 event draft (no id/sig/pubkey). */
export interface CommunityArbitrationEventTemplate {
  readonly kind: typeof COMMUNITY_ARBITRATION_FEEDBACK_KIND;
  readonly created_at: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: "";
}

export {
  AUTHORITY_DECISION_RECEIPT_SCHEMA,
};
