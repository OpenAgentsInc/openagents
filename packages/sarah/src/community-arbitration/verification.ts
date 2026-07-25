/**
 * The independent-verification event, signed by the verifying agent (omega#48).
 *
 * ## Why this exists, and why it is an amendment rather than an invention
 *
 * `SARAH-CW-00` §4 already reserves a writable-authority row:
 *
 * > | Independent verification | verification result event bound to the unit |
 * > | a verifier with a distinct operator identity |
 *
 * The row was frozen with its authority named and its **wire form left
 * unspecified**. Nothing ever supplied one, so every client read independence
 * off Sarah's arbitration decision instead — which means the deciding key was
 * asserting, on the verifier's behalf, that a verification happened. That is
 * exactly the shape of claim this contract refuses everywhere else: a result is
 * signed by the provider, a persona is signed by the agent, a ruling is signed
 * by the owner. Verification was the one claim in the lifecycle that nobody
 * signed.
 *
 * This module supplies that wire form. It is deliberately an *amendment to an
 * existing row* (`SARAH-CW-00-A1`, contract §8.4), not a new authority:
 *
 * - **No new kind.** Kind `7000` is already the contract's carrier for the
 *   quote, the quote acceptance, the arbitration decision, the dispute appeal
 *   and the owner ruling.
 * - **No new tag family.** `cw_feedback_type` is the discriminator those five
 *   already use. This adds a fourth value to an enumeration, not a mechanism.
 * - **No new authority.** Acceptance still belongs to Sarah (§4). This event
 *   decides nothing: it carries no authority receipt, it cannot say `accepted`,
 *   and `cw_decides_payment` is pinned false. It is testimony, not a verdict.
 *
 * ## What the signature buys
 *
 * One thing, and it is the thing the exit asks for: the verifier says "I ran
 * this" in its own hand. A decision can still *claim* independence, but the
 * claim is now checkable against an event the claimed verifier signed, and a
 * claim with no such event is refused rather than rendered.
 *
 * ## What the signature does not buy
 *
 * It does not make the tags authoritative. `cw_verifier_operator_ref` and
 * `cw_producer_agent_pubkey` are what the *verifier* asserts, and an agent key
 * asserting its own operator is precisely the self-dealing shape that was fixed
 * on the decision path. Both operators are therefore re-resolved from the
 * folded membership binding by {@link admitIndependentVerification}; the tags
 * are only ever compared against the record, never trusted in place of it. A
 * burned key is refused whatever it signs, because a revocation binds the
 * subject regardless of arrival order.
 */
import { Schema as S } from "effect";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import {
  ARBITRATION_REASON_CLASSES,
  ArbitrationReasonClassSchema,
  COMMUNITY_ARBITRATION_FEEDBACK_KIND,
  SARAH_CW_05_PACKET,
  type ArbitrationReasonClass,
} from "./types.ts";

/** Amendment packet id. Cited by contract §8.4. */
export const SARAH_CW_00_A1_PACKET = "SARAH-CW-00-A1" as const;

/** Verifier-signed independent verification of one community result. */
export const SARAH_INDEPENDENT_VERIFICATION_SCHEMA =
  "openagents.sarah.community_independent_verification.v1" as const;

/** `cw_feedback_type` value for this event. Fourth value, existing tag. */
export const INDEPENDENT_VERIFICATION_FEEDBACK_TYPE =
  "independent_verification" as const;

export const INDEPENDENT_VERIFICATION_ALT =
  "OpenAgents community independent verification" as const;

/**
 * The verifier's verdict on the result it checked.
 *
 * Deliberately not `accepted` / `rejected`: those are Sarah's words and this
 * event must not be mistakable for the decision. A verifier reports what it
 * observed running the check; whether that is grounds to accept the unit is
 * somebody else's call.
 */
export const INDEPENDENT_VERIFICATION_VERDICTS = [
  "reproduced",
  "not_reproduced",
  "inconclusive",
] as const;
export type IndependentVerificationVerdict =
  (typeof INDEPENDENT_VERIFICATION_VERDICTS)[number];

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const IsoTime = S.String.check(S.isMinLength(10), S.isMaxLength(64));
const Summary = S.Trim.check(S.isMinLength(1), S.isMaxLength(500));

export const IndependentVerificationVerdictSchema = S.Literals(
  INDEPENDENT_VERIFICATION_VERDICTS,
);

/**
 * What the verifying agent signs.
 *
 * `verifierAgentPubkey` is required and must equal the signing key: an event
 * that names a verifier other than its own author is somebody speaking for the
 * verifier, which is the state this amendment exists to end.
 */
export const IndependentVerificationSchema = S.Struct({
  schema: S.Literal(SARAH_INDEPENDENT_VERIFICATION_SCHEMA),
  packet: S.Literal(SARAH_CW_00_A1_PACKET),
  verificationRef: Ref,
  unitRef: Ref,
  requestEventId: Hex64,
  resultEventId: Hex64,
  /** The agent whose result was checked. */
  producerAgentPubkey: Hex64,
  /** The signing agent. Asserted here, re-resolved from the fold on read. */
  verifierAgentPubkey: Hex64,
  /** The operator the verifier claims. Corroboration only — never authority. */
  verifierOperatorRef: Ref,
  verdict: IndependentVerificationVerdictSchema,
  /** Required when the verdict is not `reproduced`. */
  reasonClass: S.optional(ArbitrationReasonClassSchema),
  /** Public-safe. Treated as untrusted member content by every reader. */
  reasonSummary: S.optional(Summary),
  verificationReceiptRef: Ref,
  evidenceRefs: S.Array(Ref),
  /** Pinned false. A verification never settles and never accepts. */
  decidesPayment: S.Literal(false),
  verifiedAt: IsoTime,
});
export type IndependentVerification = S.Schema.Type<
  typeof IndependentVerificationSchema
>;

export type IndependentVerificationRuleCode =
  | "schema_invalid"
  | "verifier_not_author"
  | "self_dealing_keys_only"
  | "missing_reason_class"
  | "decides_payment_forbidden";

export class IndependentVerificationRuleError extends Error {
  readonly code: IndependentVerificationRuleCode;

  constructor(code: IndependentVerificationRuleCode, message: string) {
    super(message);
    this.name = "IndependentVerificationRuleError";
    this.code = code;
  }
}

const decodeVerification = S.decodeUnknownSync(IndependentVerificationSchema);

/**
 * Validate the record the verifier is about to sign.
 *
 * Key-level self-dealing is caught here because it needs no ledger: an agent
 * cannot verify itself. Operator-level self-dealing is *not* decidable from
 * this record alone and is deliberately left to
 * {@link admitIndependentVerification}, which has the fold — one operator
 * holding two agent keys passes every check available here.
 */
export const validateIndependentVerification = (
  raw: unknown,
): IndependentVerification => {
  let verification: IndependentVerification;
  try {
    verification = decodeVerification(raw);
  } catch (error) {
    throw new IndependentVerificationRuleError(
      "schema_invalid",
      error instanceof Error ? error.message : "independent verification invalid",
    );
  }
  if (verification.decidesPayment !== false) {
    throw new IndependentVerificationRuleError(
      "decides_payment_forbidden",
      "an independent verification never decides payment",
    );
  }
  if (verification.verifierAgentPubkey === verification.producerAgentPubkey) {
    throw new IndependentVerificationRuleError(
      "self_dealing_keys_only",
      "an agent cannot independently verify its own result",
    );
  }
  if (
    verification.verdict !== "reproduced" &&
    verification.reasonClass === undefined
  ) {
    throw new IndependentVerificationRuleError(
      "missing_reason_class",
      "a verification that did not reproduce must name a typed reason class",
    );
  }
  return verification;
};

/**
 * Build the unsigned kind-7000 template. The **verifying agent** signs it, on
 * the operator's own compute — never Sarah, never the phone, never the
 * producer.
 */
export const buildIndependentVerificationTemplate = (input: {
  readonly verification: unknown;
  readonly createdAt?: number;
}): {
  readonly template: SarahNostrEventTemplate;
  readonly verification: IndependentVerification;
} => {
  const verification = validateIndependentVerification(input.verification);

  const createdAt = ((): number => {
    if (input.createdAt !== undefined) return input.createdAt;
    const ms = Date.parse(verification.verifiedAt);
    if (!Number.isFinite(ms)) {
      throw new IndependentVerificationRuleError(
        "schema_invalid",
        "independent verification: invalid ISO timestamp",
      );
    }
    return Math.floor(ms / 1000);
  })();

  const tags: string[][] = [
    ["e", verification.requestEventId, "", "request"],
    ["e", verification.resultEventId, "", "result"],
    ["p", verification.producerAgentPubkey],
    ["agent", verification.verifierAgentPubkey],
    ["status", verification.verdict],
    ["lbr_feedback_type", INDEPENDENT_VERIFICATION_FEEDBACK_TYPE],
    ["cw_feedback_type", INDEPENDENT_VERIFICATION_FEEDBACK_TYPE],
    ["cw_verification_ref", verification.verificationRef],
    ["cw_unit_ref", verification.unitRef],
    ["cw_producer_agent_pubkey", verification.producerAgentPubkey],
    ["cw_verifier_agent_pubkey", verification.verifierAgentPubkey],
    ["cw_verifier_operator_ref", verification.verifierOperatorRef],
    ["cw_verification_receipt_ref", verification.verificationReceiptRef],
    ["cw_decides_payment", "false"],
    ["cw_verified_at", verification.verifiedAt],
    ["alt", INDEPENDENT_VERIFICATION_ALT],
  ];
  if (verification.reasonClass !== undefined) {
    tags.push(["cw_reason_class", verification.reasonClass]);
  }
  if (verification.reasonSummary !== undefined) {
    tags.push(["cw_reason_summary", verification.reasonSummary]);
  }
  for (const ref of verification.evidenceRefs) {
    tags.push(["cw_evidence_ref", ref]);
  }

  return {
    verification,
    template: {
      kind: COMMUNITY_ARBITRATION_FEEDBACK_KIND,
      created_at: createdAt,
      tags,
      content: "",
    },
  };
};

/** A kind-7000 event as it arrives from a relay. */
export interface IndependentVerificationEvent {
  readonly id: string;
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
}

export type IndependentVerificationRefusalCode =
  | "not_a_verification_event"
  | "malformed"
  | "verifier_not_author"
  | "verifier_key_burned"
  | "verifier_binding_unconfirmed"
  | "producer_binding_unconfirmed"
  | "self_dealing_operators"
  | "decides_payment_forbidden";

export interface AdmittedIndependentVerification {
  readonly admitted: true;
  readonly sourceEventId: string;
  readonly resultEventId: string;
  readonly requestEventId: string | null;
  readonly unitRef: string | null;
  readonly verificationRef: string | null;
  readonly verifierAgentPubkey: string;
  /** From the fold. Never the tag. */
  readonly verifierOperatorPubkey: string;
  readonly producerAgentPubkey: string;
  /** From the fold. Never the tag. */
  readonly producerOperatorPubkey: string;
  readonly verdict: IndependentVerificationVerdict;
  readonly reasonClass: ArbitrationReasonClass | null;
  readonly reasonSummary: string | null;
  readonly verificationReceiptRef: string | null;
  readonly verifiedAtUnix: number;
}

export interface RefusedIndependentVerification {
  readonly admitted: false;
  readonly sourceEventId: string;
  readonly resultEventId: string | null;
  readonly verifierAgentPubkey: string;
  readonly producerAgentPubkey: string | null;
  readonly code: IndependentVerificationRefusalCode;
  readonly detail: string;
}

export type IndependentVerificationAdmission =
  | AdmittedIndependentVerification
  | RefusedIndependentVerification;

/**
 * The binding a caller resolves from its folded community record.
 *
 * Passed as functions rather than as a ledger so this module stays free of the
 * membership fold and can be exercised from either side of the contract.
 */
export interface CommunityBindingResolver {
  /** Operator pubkey the record binds this agent key to, or null. */
  readonly operatorForAgent: (agentPubkey: string) => string | null;
  /** True when a revocation burned this agent key, in any order. */
  readonly isAgentKeyBurned: (agentPubkey: string) => boolean;
}

const tagValue = (
  event: IndependentVerificationEvent,
  name: string,
): string | undefined => event.tags.find((tag) => tag[0] === name)?.[1];

const taggedEventId = (
  event: IndependentVerificationEvent,
  marker: string,
): string | null =>
  event.tags.find((tag) => tag[0] === "e" && tag[3] === marker)?.[1] ?? null;

const HEX_64 = /^[0-9a-f]{64}$/;

/**
 * Admit one signed verification event against the folded record.
 *
 * Every operator on both sides comes from `binding`, never from a tag. The two
 * refusals that matter are named separately on purpose:
 * `verifier_binding_unconfirmed` means the record does not support the
 * verifier's own claim about who operates it; `self_dealing_operators` means
 * the record *does* support it and the answer is the producer's operator.
 */
export const admitIndependentVerification = (
  event: IndependentVerificationEvent,
  binding: CommunityBindingResolver,
): IndependentVerificationAdmission => {
  const verifierKey = event.pubkey.trim().toLowerCase();
  const refuse = (
    code: IndependentVerificationRefusalCode,
    detail: string,
    producer: string | null,
    resultEventId: string | null,
  ): RefusedIndependentVerification => ({
    admitted: false,
    sourceEventId: event.id,
    resultEventId,
    verifierAgentPubkey: verifierKey,
    producerAgentPubkey: producer,
    code,
    detail,
  });

  const feedbackType =
    tagValue(event, "cw_feedback_type") ?? tagValue(event, "lbr_feedback_type");
  if (
    event.kind !== COMMUNITY_ARBITRATION_FEEDBACK_KIND ||
    feedbackType !== INDEPENDENT_VERIFICATION_FEEDBACK_TYPE
  ) {
    return refuse(
      "not_a_verification_event",
      `kind ${event.kind} / ${feedbackType ?? "no feedback type"} is not an independent verification`,
      null,
      null,
    );
  }

  const resultEventId = taggedEventId(event, "result");
  const producerRaw = tagValue(event, "cw_producer_agent_pubkey") ?? tagValue(event, "p");
  const producerKey = producerRaw?.trim().toLowerCase() ?? null;
  const claimedVerifier = tagValue(event, "cw_verifier_agent_pubkey")?.trim().toLowerCase();
  const verdictRaw = tagValue(event, "status");

  if (
    resultEventId === null ||
    !HEX_64.test(resultEventId) ||
    producerKey === null ||
    !HEX_64.test(producerKey) ||
    verdictRaw === undefined ||
    !(INDEPENDENT_VERIFICATION_VERDICTS as ReadonlyArray<string>).includes(verdictRaw)
  ) {
    return refuse(
      "malformed",
      "verification must bind a result event, a producer key, and a known verdict",
      producerKey,
      resultEventId,
    );
  }
  const verdict = verdictRaw as IndependentVerificationVerdict;

  if (tagValue(event, "cw_decides_payment") !== "false") {
    return refuse(
      "decides_payment_forbidden",
      "a verification that does not disclaim payment is not admitted",
      producerKey,
      resultEventId,
    );
  }

  // The whole point of the amendment. Somebody else's signature over "X
  // verified this" is the state we are leaving, not a state we accept.
  if (claimedVerifier !== undefined && claimedVerifier !== verifierKey) {
    return refuse(
      "verifier_not_author",
      "the verification names a verifier other than its own author",
      producerKey,
      resultEventId,
    );
  }
  if (verifierKey === producerKey) {
    return refuse(
      "self_dealing_operators",
      "an agent cannot independently verify its own result",
      producerKey,
      resultEventId,
    );
  }

  // Revocation binds the subject whatever it signs and whenever it arrives.
  // Checked before the binding lookup so a burned key that still has a live
  // binding row cannot slip through as merely "unconfirmed".
  if (binding.isAgentKeyBurned(verifierKey)) {
    return refuse(
      "verifier_key_burned",
      "revocation burned this verifier key",
      producerKey,
      resultEventId,
    );
  }

  const verifierOperator = binding.operatorForAgent(verifierKey);
  if (verifierOperator === null) {
    return refuse(
      "verifier_binding_unconfirmed",
      "no signed record binds this verifier key to an operator",
      producerKey,
      resultEventId,
    );
  }
  const claimedOperator = tagValue(event, "cw_verifier_operator_ref")?.trim().toLowerCase();
  if (claimedOperator !== undefined && claimedOperator !== verifierOperator) {
    return refuse(
      "verifier_binding_unconfirmed",
      "the verifier claims an operator the record does not bind it to",
      producerKey,
      resultEventId,
    );
  }

  const producerOperator = binding.operatorForAgent(producerKey);
  if (producerOperator === null) {
    return refuse(
      "producer_binding_unconfirmed",
      "no signed record binds the produced result's key to an operator",
      producerKey,
      resultEventId,
    );
  }

  // Distinct *operators*, not merely distinct keys. One operator holding two
  // agent keys passes every key comparison above.
  if (producerOperator === verifierOperator) {
    return refuse(
      "self_dealing_operators",
      "producer and verifier resolve to the same operator",
      producerKey,
      resultEventId,
    );
  }

  const rawReasonClass = tagValue(event, "cw_reason_class");
  const reasonClass =
    rawReasonClass !== undefined &&
    (ARBITRATION_REASON_CLASSES as ReadonlyArray<string>).includes(rawReasonClass)
      ? (rawReasonClass as ArbitrationReasonClass)
      : null;

  return {
    admitted: true,
    sourceEventId: event.id,
    resultEventId,
    requestEventId: taggedEventId(event, "request"),
    unitRef: tagValue(event, "cw_unit_ref") ?? null,
    verificationRef: tagValue(event, "cw_verification_ref") ?? null,
    verifierAgentPubkey: verifierKey,
    verifierOperatorPubkey: verifierOperator,
    producerAgentPubkey: producerKey,
    producerOperatorPubkey: producerOperator,
    verdict,
    reasonClass,
    reasonSummary: tagValue(event, "cw_reason_summary") ?? null,
    verificationReceiptRef: tagValue(event, "cw_verification_receipt_ref") ?? null,
    verifiedAtUnix: event.created_at,
  };
};
