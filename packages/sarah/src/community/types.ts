import { Schema as S } from "effect";

/** Wire schema for the community membership record. */
export const SARAH_COMMUNITY_MEMBERSHIP_SCHEMA =
  "openagents.sarah.community_membership.v1" as const;

/** Packet this module implements. */
export const SARAH_CW_02_PACKET = "SARAH-CW-02" as const;

/** Spec pointer. */
export const SARAH_CW_02_ISSUE = "OpenAgentsInc/openagents#9227" as const;

/**
 * Owner decision 2026-07-24 (§40): membership is invitation only for now.
 * Application and open/probation modes stay reserved until a new decision.
 */
export const SARAH_COMMUNITY_MEMBERSHIP_GATE = "invitation_only" as const;

/** NIP-AP persona kind (public addressable blueprint). */
export const NIP_AP_PERSONA_KIND = 30175 as const;

/** NIP-AP managed-agent instance projection kind. */
export const NIP_AP_MANAGED_INSTANCE_KIND = 30177 as const;

/** NIP-29 group message kind (chat). Membership is admin-event driven. */
export const NIP_29_GROUP_CHAT_KIND = 9 as const;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const IsoTime = S.String.check(S.isMinLength(10), S.isMaxLength(64));
const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Summary = S.String.check(S.isMinLength(1), S.isMaxLength(1_000));

/** Membership gate modes. Only invitation_only is admitted. */
export const CommunityMembershipGateMode = S.Literals([
  "invitation_only",
  "application_with_review",
  "open_with_probation",
]);
export type CommunityMembershipGateMode = S.Schema.Type<
  typeof CommunityMembershipGateMode
>;

export const CommunityInvitationStatus = S.Literals([
  "open",
  "accepted",
  "revoked",
  "expired",
]);
export type CommunityInvitationStatus = S.Schema.Type<
  typeof CommunityInvitationStatus
>;

export const CommunityMemberStatus = S.Literals([
  "invited",
  "active",
  "revoked",
]);
export type CommunityMemberStatus = S.Schema.Type<typeof CommunityMemberStatus>;

export const CommunityAgentStatus = S.Literals(["active", "revoked"]);
export type CommunityAgentStatus = S.Schema.Type<typeof CommunityAgentStatus>;

export const CommunityCapabilityGrantStatus = S.Literals([
  "active",
  "revoked",
]);
export type CommunityCapabilityGrantStatus = S.Schema.Type<
  typeof CommunityCapabilityGrantStatus
>;

/**
 * NIP-OA auth tag wire form: ["auth", ownerPubkey, conditions, sig].
 * Owner here is the human operator of the community agent, not OpenAgents.
 */
export const CommunityOwnerAuthTag = S.Tuple([
  S.Literal("auth"),
  Hex64,
  S.String.check(S.isMaxLength(2_048)),
  S.String.check(S.isPattern(/^[0-9a-f]{128}$/)),
]);
export type CommunityOwnerAuthTag = S.Schema.Type<typeof CommunityOwnerAuthTag>;

/** Declared public capability labels (NIP-AP persona surface). No secrets. */
export const CommunityDeclaredCapability = S.Struct({
  capabilityRef: Ref,
  label: S.String.check(S.isMinLength(1), S.isMaxLength(80)),
});
export type CommunityDeclaredCapability = S.Schema.Type<
  typeof CommunityDeclaredCapability
>;

/**
 * Public-safe persona reference for NIP-AP.
 * Secrets, provider keys, and agent-home paths are forbidden.
 */
export const CommunityPersonaRef = S.Struct({
  kind: S.Literal(NIP_AP_PERSONA_KIND),
  /** Plaintext persona slug (`d` tag). Not blinded (NIP-AP discovery rule). */
  dTag: S.String.check(
    S.isPattern(/^[a-z0-9][a-z0-9_-]{0,63}$/),
  ),
  displayName: S.optional(S.String.check(S.isMinLength(1), S.isMaxLength(80))),
  declaredCapabilities: S.Array(CommunityDeclaredCapability).check(
    S.isMaxLength(32),
  ),
});
export type CommunityPersonaRef = S.Schema.Type<typeof CommunityPersonaRef>;

/**
 * One agent key bound to its human operator by NIP-OA.
 * Revoking this row never reaches into the operator machine.
 */
export const CommunityAgentBinding = S.Struct({
  agentPubkey: Hex64,
  operatorPubkey: Hex64,
  ownerAuthTag: CommunityOwnerAuthTag,
  status: CommunityAgentStatus,
  capabilityGrant: CommunityCapabilityGrantStatus,
  persona: S.optional(CommunityPersonaRef),
  attachedAt: IsoTime,
  revokedAt: S.optional(IsoTime),
  revokeReason: S.optional(Summary),
});
export type CommunityAgentBinding = S.Schema.Type<typeof CommunityAgentBinding>;

/** Invitation to join the community group (NIP-29 h). */
export const CommunityInvitation = S.Struct({
  invitationId: Ref,
  groupId: Ref,
  inviterPubkey: Hex64,
  inviteePubkey: Hex64,
  status: CommunityInvitationStatus,
  createdAt: IsoTime,
  expiresAt: S.optional(IsoTime),
  acceptedAt: S.optional(IsoTime),
  revokedAt: S.optional(IsoTime),
});
export type CommunityInvitation = S.Schema.Type<typeof CommunityInvitation>;

/**
 * A community member is a human developer with a Nostr identity.
 * Agents hang off the member. Rate limits key on the operator, not the agent.
 */
export const CommunityMember = S.Struct({
  schema: S.Literal(SARAH_COMMUNITY_MEMBERSHIP_SCHEMA),
  groupId: Ref,
  operatorPubkey: Hex64,
  status: CommunityMemberStatus,
  invitationId: Ref,
  agents: S.Array(CommunityAgentBinding).check(S.isMaxLength(64)),
  joinedAt: S.optional(IsoTime),
  revokedAt: S.optional(IsoTime),
  revokeReason: S.optional(Summary),
});
export type CommunityMember = S.Schema.Type<typeof CommunityMember>;

/**
 * Per-operator rate-limit unit (anti-sybil).
 * One operator minting many agent keys must not multiply throughput.
 */
export const CommunityOperatorRateLimit = S.Struct({
  operatorPubkey: Hex64,
  windowStartedAt: IsoTime,
  windowSeconds: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  maxActions: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  actionCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
});
export type CommunityOperatorRateLimit = S.Schema.Type<
  typeof CommunityOperatorRateLimit
>;

/** Typed failure reasons for membership operations. */
export type CommunityMembershipErrorCode =
  | "gate_not_invitation_only"
  | "invitation_required"
  | "invitation_not_open"
  | "invitation_expired"
  | "invitation_wrong_invitee"
  | "member_already_active"
  | "member_revoked"
  | "member_not_found"
  | "agent_not_attested"
  | "agent_self_attestation"
  | "agent_operator_mismatch"
  | "agent_already_bound"
  | "agent_not_found"
  | "agent_revoked"
  | "anonymous_pubkey_refused"
  | "rate_limit_exceeded"
  | "provider_key_forbidden"
  | "agent_home_mutation_forbidden"
  | "secret_shaped_payload";

export class CommunityMembershipError extends Error {
  readonly code: CommunityMembershipErrorCode;
  constructor(code: CommunityMembershipErrorCode, message: string) {
    super(message);
    this.name = "CommunityMembershipError";
    this.code = code;
  }
}

/** Forbidden field names on public community records (same law as SW-02). */
export const FORBIDDEN_COMMUNITY_SECRET_FIELDS: ReadonlyArray<string> = [
  "mnemonic",
  "nsec",
  "privateKey",
  "privateKeyHex",
  "privateKeyBytes",
  "seckey",
  "secretKey",
  "secretKeyHex",
  "seed",
  "seedHex",
  "rawKey",
  "providerKey",
  "providerApiKey",
  "OPENAI_API_KEY",
  "ANTHROPIC_API_KEY",
  "agentHome",
  "agentHomePath",
  "CODEX_HOME",
];
