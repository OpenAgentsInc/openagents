/**
 * Sarah community workroom surfaces (v2, SARAH-CW-*).
 *
 * CW-02: invitation-only membership, NIP-OA attestation, revocation.
 * CW-03: pure tick → bounded work-unit decomposition.
 *
 * Spec: docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §31, §33, §38.2
 * @see docs/omega/2026-07-24-sarah-community-membership.md
 */

// SARAH-CW-02 — membership, attestation, revocation
export {
  admitAttestedAgentKey,
  assertCommunityPublicSafe,
  assertHexPubkey,
  attachOwnerAttestation,
  attachPersonaRef,
  buildCommunityAttestedAuthTemplate,
  extractAuthTagFromAuthEvent,
  verifyAgentOwnerAttestation,
} from "./attestation.ts";
export {
  acceptInvitation,
  assertInvitationOnlyGate,
  attachAgent,
  checkRelayAdmission,
  createEmptyLedger,
  isAgentAdmitted,
  isMemberActive,
  issueInvitation,
  rateLimitUnitForAgent,
  recordOperatorAction,
  refuseOpenJoin,
  revokeAgent,
  revokeMember,
  type CommunityMembershipLedger,
} from "./membership.ts";
export {
  CommunityAgentBinding,
  CommunityAgentStatus,
  CommunityCapabilityGrantStatus,
  CommunityDeclaredCapability,
  CommunityInvitation,
  CommunityInvitationStatus,
  CommunityMember,
  CommunityMembershipError,
  CommunityMembershipGateMode,
  CommunityMemberStatus,
  CommunityOperatorRateLimit,
  CommunityOwnerAuthTag,
  CommunityPersonaRef,
  FORBIDDEN_COMMUNITY_SECRET_FIELDS,
  NIP_29_GROUP_CHAT_KIND,
  NIP_AP_MANAGED_INSTANCE_KIND,
  NIP_AP_PERSONA_KIND,
  SARAH_COMMUNITY_MEMBERSHIP_GATE,
  SARAH_COMMUNITY_MEMBERSHIP_SCHEMA,
  SARAH_CW_02_ISSUE,
  SARAH_CW_02_PACKET,
  type CommunityMembershipErrorCode,
  type CommunityAgentBinding as CommunityAgentBindingType,
  type CommunityInvitation as CommunityInvitationType,
  type CommunityMember as CommunityMemberType,
  type CommunityOperatorRateLimit as CommunityOperatorRateLimitType,
  type CommunityOwnerAuthTag as CommunityOwnerAuthTagType,
  type CommunityPersonaRef as CommunityPersonaRefType,
} from "./types.ts";

// SARAH-CW-03 — tick → work-unit decomposition
export * from "./work-units.ts";
export * from "./untrusted.ts";
