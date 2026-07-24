/**
 * SARAH-CW-04 — NIP-LBR request and quote lane types for the community workroom.
 *
 * Protocol validation and kind allocation live in `@openagentsinc/nip90`
 * (and `nostr-effect/nip90`). This module only composes Sarah work-unit
 * bindings and the v1 no-spend settlement fence on top of those helpers.
 *
 * @see docs/nips/LBR.md
 * @see docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §32
 */

import { Schema as S } from "effect";

/** Wire schema id for the Sarah LBR request/quote lane. */
export const SARAH_LBR_REQUEST_QUOTE_SCHEMA =
  "openagents.sarah.lbr_request_quote.v1" as const;

/** Packet id. */
export const SARAH_LBR_REQUEST_QUOTE_PACKET = "SARAH-CW-04" as const;

/** Issue ref (public-safe). */
export const SARAH_LBR_REQUEST_QUOTE_ISSUE =
  "OpenAgentsInc/openagents#9228" as const;

/**
 * v1 settlement fence. Requests carry a budget bid per NIP-LBR, but the
 * community workroom settles nothing until SARAH-CW-07 admits a paid lane.
 */
export const SARAH_LBR_SETTLEMENT_MODE_V1 = "no_spend" as const;

/** Agentic coding job type pinned by NIP-LBR v1. */
export const SARAH_LBR_JOB_TYPE = "code_task" as const;

/** NIP-31 alt for community work requests. */
export const SARAH_LBR_REQUEST_ALT =
  "OpenAgents Sarah community work request (NIP-LBR, ref-only)" as const;

/** NIP-31 alt for community quotes. */
export const SARAH_LBR_QUOTE_ALT =
  "OpenAgents Sarah community work quote (NIP-LBR, ref-only)" as const;

/** Param keys appended by the Sarah lane (not a protocol redefinition). */
export const SARAH_LBR_PARAM = {
  workUnitRef: "lbr_work_unit_ref",
  grantRef: "lbr_grant_ref",
  idempotencyRef: "lbr_idempotency_ref",
  settlementMode: "lbr_settlement_mode",
  allowedActionRef: "lbr_allowed_action_ref",
  operatorRef: "lbr_operator_ref",
  groupRef: "lbr_group_ref",
} as const;

/**
 * Public-safe ref pattern shared with `@openagentsinc/nip90` LBR helpers:
 * `namespace.segment[.segment…][:suffix]`.
 */
export const SARAH_LBR_PUBLIC_REF_PATTERN =
  /^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/;

const PublicRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(SARAH_LBR_PUBLIC_REF_PATTERN),
);

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));

const PositiveMsats = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);

/**
 * Narrow work-unit grant that a community agent may hold.
 * Must never name Sarah principal grants (`grant.sarah.*`, `principal.sarah`).
 */
export const SarahLbrWorkUnitGrant = S.Struct({
  workUnitRef: PublicRef,
  grantRef: PublicRef,
  repositoryRefs: S.Array(PublicRef).check(S.isMinLength(1), S.isMaxLength(16)),
  allowedActionRefs: S.Array(PublicRef).check(
    S.isMinLength(1),
    S.isMaxLength(16),
  ),
  /** Max budget in millisatoshis (NIP-LBR bid). v1 records only; no settlement. */
  budgetMsats: PositiveMsats,
  /** NIP-40 expiration as a unix-seconds timestamp. */
  expiresAtUnix: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(4_102_444_800),
  ),
  idempotencyRef: PublicRef,
});
export type SarahLbrWorkUnitGrant = S.Schema.Type<typeof SarahLbrWorkUnitGrant>;

export const SarahLbrWorkRequestInput = S.Struct({
  schema: S.Literal(SARAH_LBR_REQUEST_QUOTE_SCHEMA),
  workUnit: SarahLbrWorkUnitGrant,
  objectiveRef: PublicRef,
  verificationCommandRef: PublicRef,
  requiredCapabilityRefs: S.Array(PublicRef).check(
    S.isMinLength(1),
    S.isMaxLength(16),
  ),
  /** Optional NIP-29 group id for the community room. */
  groupId: S.optional(S.String.check(S.isMinLength(1), S.isMaxLength(128))),
  /** Optional public group ref (when the room is addressable by ref). */
  groupRef: S.optional(PublicRef),
  forumTopicRef: S.optional(PublicRef),
  deadlineRef: S.optional(PublicRef),
  relays: S.optional(
    S.Array(S.String.check(S.isMinLength(1), S.isMaxLength(512))).check(
      S.isMaxLength(8),
    ),
  ),
  createdAt: S.optional(
    S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  ),
});
export type SarahLbrWorkRequestInput = S.Schema.Type<
  typeof SarahLbrWorkRequestInput
>;

export const SarahLbrQuoteInput = S.Struct({
  schema: S.Literal(SARAH_LBR_REQUEST_QUOTE_SCHEMA),
  requestId: Hex64,
  requesterPubkey: Hex64,
  workUnitRef: PublicRef,
  amountMsats: PositiveMsats,
  providerRef: PublicRef,
  capabilityRefs: S.Array(PublicRef).check(S.isMinLength(1), S.isMaxLength(16)),
  quoteRef: PublicRef,
  /** Operator (human developer) identity ref — rate limits attach here, not per key. */
  operatorRef: S.optional(PublicRef),
  expiresAtRef: S.optional(PublicRef),
  requestRelay: S.optional(S.String.check(S.isMinLength(1), S.isMaxLength(512))),
  createdAt: S.optional(
    S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  ),
});
export type SarahLbrQuoteInput = S.Schema.Type<typeof SarahLbrQuoteInput>;

export const decodeSarahLbrWorkRequestInput = S.decodeUnknownSync(
  SarahLbrWorkRequestInput,
);
export const decodeSarahLbrQuoteInput = S.decodeUnknownSync(SarahLbrQuoteInput);
export const decodeSarahLbrWorkUnitGrant = S.decodeUnknownSync(
  SarahLbrWorkUnitGrant,
);

/** Prefixes that must never appear on a community work-unit grant. */
export const SARAH_LBR_FORBIDDEN_GRANT_PREFIXES: ReadonlyArray<string> = [
  "grant.sarah.",
  "principal.sarah",
  "capability.sarah.",
  "authority.sarah.",
  "role.sarah_orchestrator",
];

export class SarahLbrLaneError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "SarahLbrLaneError";
    this.code = code;
  }
}
