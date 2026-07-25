/**
 * Community membership ledger: invitation-only gate, agent attach, revocation.
 *
 * In-memory pure helpers. Persistence and NIP-29 relay admin events are
 * SARAH-CW-01 / integration work. This module freezes the admission model.
 */
import {
  admitAttestedAgentKey,
  assertCommunityPublicSafe,
  assertHexPubkey,
  attachPersonaRef,
  verifyAgentOwnerAttestation,
} from "./attestation.ts";
import {
  CommunityMembershipError,
  SARAH_COMMUNITY_MEMBERSHIP_GATE,
  SARAH_COMMUNITY_MEMBERSHIP_SCHEMA,
  type CommunityAgentBinding,
  type CommunityInvitation,
  type CommunityMember,
  type CommunityMembershipGateMode,
  type CommunityOperatorRateLimit,
  type CommunityOwnerAuthTag,
  type CommunityPersonaRef,
} from "./types.ts";

export type CommunityMembershipLedger = {
  readonly gateMode: CommunityMembershipGateMode;
  readonly groupId: string;
  readonly invitations: ReadonlyMap<string, CommunityInvitation>;
  readonly members: ReadonlyMap<string, CommunityMember>;
  readonly agentIndex: ReadonlyMap<string, string>; // agentPubkey -> operatorPubkey
  readonly rateLimits: ReadonlyMap<string, CommunityOperatorRateLimit>;
  /**
   * Agent keys that revocation burned, community-wide and forever.
   *
   * This set only ever grows. It exists because the burn cannot be *derived*
   * from member rows: a member row is mutable state that re-admission replaces,
   * and an earlier version of this module read the burn out of
   * `member.agents`. Re-inviting the operator produced a fresh row with an
   * empty agent list, the burn evidence vanished with the old row, and
   * replaying the original owner attestation re-attached the revoked key.
   *
   * Revocation is a statement about the key, not about the membership event
   * that happened to carry it. Persist and replay this set with the ledger —
   * a revocation a restart undoes is not a revocation.
   */
  readonly burnedAgentKeys: ReadonlySet<string>;
};

export const createEmptyLedger = (params: {
  readonly groupId: string;
  readonly gateMode?: CommunityMembershipGateMode;
}): CommunityMembershipLedger => {
  const gateMode = params.gateMode ?? SARAH_COMMUNITY_MEMBERSHIP_GATE;
  if (gateMode !== "invitation_only") {
    throw new CommunityMembershipError(
      "gate_not_invitation_only",
      `community: admitted gate is invitation_only; got ${gateMode}`,
    );
  }
  return {
    gateMode: "invitation_only",
    groupId: params.groupId,
    invitations: new Map(),
    members: new Map(),
    agentIndex: new Map(),
    rateLimits: new Map(),
    burnedAgentKeys: new Set(),
  };
};

const nowIso = (at?: string): string => at ?? new Date().toISOString();

const cloneLedger = (
  ledger: CommunityMembershipLedger,
  patch: Partial<CommunityMembershipLedger>,
): CommunityMembershipLedger => ({
  gateMode: patch.gateMode ?? ledger.gateMode,
  groupId: patch.groupId ?? ledger.groupId,
  invitations: patch.invitations ?? ledger.invitations,
  members: patch.members ?? ledger.members,
  agentIndex: patch.agentIndex ?? ledger.agentIndex,
  rateLimits: patch.rateLimits ?? ledger.rateLimits,
  burnedAgentKeys: patch.burnedAgentKeys ?? ledger.burnedAgentKeys ?? new Set(),
});

/**
 * True when revocation burned this agent key.
 *
 * Reads the explicit burn set *and* every member row, so a ledger rebuilt from
 * persisted member rows alone still refuses a burned key, and a ledger that
 * replaced a member row still refuses one. Neither representation alone can
 * erase the burn.
 */
export const isAgentKeyBurned = (
  ledger: CommunityMembershipLedger,
  agentPubkey: string,
): boolean => {
  const key = agentPubkey.toLowerCase();
  if (ledger.burnedAgentKeys?.has(key) === true) return true;
  for (const member of ledger.members.values()) {
    if (
      member.agents.some(
        (agent) => agent.agentPubkey === key && agent.status === "revoked",
      )
    ) {
      return true;
    }
  }
  return false;
};

const withBurnedKeys = (
  ledger: CommunityMembershipLedger,
  keys: ReadonlyArray<string>,
): ReadonlySet<string> => {
  const next = new Set(ledger.burnedAgentKeys ?? []);
  for (const key of keys) next.add(key.toLowerCase());
  return next;
};

/** Assert the ledger still uses the admitted invitation-only gate. */
export const assertInvitationOnlyGate = (
  ledger: CommunityMembershipLedger,
): void => {
  if (ledger.gateMode !== "invitation_only") {
    throw new CommunityMembershipError(
      "gate_not_invitation_only",
      "community: membership gate must be invitation_only",
    );
  }
};

/**
 * Issue an invitation. Open join and application without invitation are refused.
 */
export const issueInvitation = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly invitationId: string;
    readonly inviterPubkey: string;
    readonly inviteePubkey: string;
    readonly createdAt?: string;
    readonly expiresAt?: string;
  },
): { readonly ledger: CommunityMembershipLedger; readonly invitation: CommunityInvitation } => {
  assertInvitationOnlyGate(ledger);
  const inviterPubkey = assertHexPubkey(params.inviterPubkey, "inviterPubkey");
  const inviteePubkey = assertHexPubkey(params.inviteePubkey, "inviteePubkey");
  if (ledger.invitations.has(params.invitationId)) {
    throw new CommunityMembershipError(
      "invitation_required",
      `community: invitation id already exists: ${params.invitationId}`,
    );
  }
  const invitation: CommunityInvitation = {
    invitationId: params.invitationId,
    groupId: ledger.groupId,
    inviterPubkey,
    inviteePubkey,
    status: "open",
    createdAt: nowIso(params.createdAt),
    expiresAt: params.expiresAt,
  };
  assertCommunityPublicSafe(invitation);
  const invitations = new Map(ledger.invitations);
  invitations.set(invitation.invitationId, invitation);
  return { ledger: cloneLedger(ledger, { invitations }), invitation };
};

const isExpired = (invitation: CommunityInvitation, at: string): boolean => {
  if (!invitation.expiresAt) return false;
  return Date.parse(at) > Date.parse(invitation.expiresAt);
};

/**
 * Accept an open invitation and create an active member row.
 * Without a matching open invitation, join is refused (invitation-only gate).
 */
export const acceptInvitation = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly invitationId: string;
    readonly inviteePubkey: string;
    readonly acceptedAt?: string;
  },
): { readonly ledger: CommunityMembershipLedger; readonly member: CommunityMember } => {
  assertInvitationOnlyGate(ledger);
  const inviteePubkey = assertHexPubkey(params.inviteePubkey, "inviteePubkey");
  const invitation = ledger.invitations.get(params.invitationId);
  if (!invitation) {
    throw new CommunityMembershipError(
      "invitation_required",
      "community: no invitation for this join",
    );
  }
  if (invitation.status !== "open") {
    throw new CommunityMembershipError(
      "invitation_not_open",
      `community: invitation status is ${invitation.status}`,
    );
  }
  const at = nowIso(params.acceptedAt);
  if (isExpired(invitation, at)) {
    throw new CommunityMembershipError(
      "invitation_expired",
      "community: invitation has expired",
    );
  }
  if (invitation.inviteePubkey !== inviteePubkey) {
    throw new CommunityMembershipError(
      "invitation_wrong_invitee",
      "community: invitee pubkey does not match invitation",
    );
  }
  const existing = ledger.members.get(inviteePubkey);
  if (existing?.status === "active") {
    throw new CommunityMembershipError(
      "member_already_active",
      "community: member is already active",
    );
  }
  if (existing?.status === "revoked") {
    // Re-invite path: a new invitation may re-admit after explicit re-issue.
    // The old revoked row is replaced only through a fresh open invitation.
  }

  const member: CommunityMember = {
    schema: SARAH_COMMUNITY_MEMBERSHIP_SCHEMA,
    groupId: ledger.groupId,
    operatorPubkey: inviteePubkey,
    status: "active",
    invitationId: invitation.invitationId,
    agents: [],
    joinedAt: at,
  };
  assertCommunityPublicSafe(member);

  const invitations = new Map(ledger.invitations);
  invitations.set(invitation.invitationId, {
    ...invitation,
    status: "accepted",
    acceptedAt: at,
  });
  const members = new Map(ledger.members);
  members.set(inviteePubkey, member);
  return {
    ledger: cloneLedger(ledger, { invitations, members }),
    member,
  };
};

/**
 * Refuse an open-join attempt with no invitation (explicit gate oracle).
 */
export const refuseOpenJoin = (
  _ledger: CommunityMembershipLedger,
  _operatorPubkey: string,
): never => {
  throw new CommunityMembershipError(
    "invitation_required",
    "community: membership gate is invitation_only; open join is refused",
  );
};

/**
 * Attach an agent key to an active member. Requires a valid NIP-OA tag that
 * binds the agent to that member's operator pubkey.
 */
export const attachAgent = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly operatorPubkey: string;
    readonly agentPubkey: string;
    readonly ownerAuthTag: readonly string[];
    readonly persona?: CommunityPersonaRef;
    readonly attachedAt?: string;
  },
): {
  readonly ledger: CommunityMembershipLedger;
  readonly binding: CommunityAgentBinding;
} => {
  assertInvitationOnlyGate(ledger);
  const operatorPubkey = assertHexPubkey(params.operatorPubkey, "operatorPubkey");
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");

  const member = ledger.members.get(operatorPubkey);
  if (!member) {
    throw new CommunityMembershipError(
      "member_not_found",
      "community: operator is not a member",
    );
  }
  if (member.status === "revoked") {
    throw new CommunityMembershipError(
      "member_revoked",
      "community: revoked member cannot attach agents",
    );
  }
  if (member.status !== "active") {
    throw new CommunityMembershipError(
      "invitation_required",
      "community: member must be active before attaching agents",
    );
  }

  const { operatorPubkey: attestedOp } = verifyAgentOwnerAttestation({
    agentPubkey,
    ownerAuthTag: params.ownerAuthTag,
    expectedOperatorPubkey: operatorPubkey,
  });
  if (attestedOp !== operatorPubkey) {
    throw new CommunityMembershipError(
      "agent_operator_mismatch",
      "community: agent attestation operator mismatch",
    );
  }

  // A revoked agent key is burned community-wide, and stays burned.
  //
  // `revokeAgent` clears `agentIndex`, so the duplicate check below never sees
  // a revoked key: the binding was then filtered out of `member.agents` and
  // replaced with a fresh active one, which silently erased the revocation.
  // Replaying the original attestation was enough to restore the agent.
  //
  // Scanning every member closed the direct replay and the hand-it-to-a-second-
  // operator laundering. It did not close re-admission: `acceptInvitation`
  // built a *fresh* member row with an empty agent list, so re-inviting the
  // operator deleted the only evidence the scan could see and the same replay
  // worked again. The burn is now an explicit monotonic fact on the ledger,
  // checked alongside the row scan, because a revocation that any later write
  // can erase is not a revocation.
  if (isAgentKeyBurned(ledger, agentPubkey)) {
    throw new CommunityMembershipError(
      "agent_revoked",
      "community: revoked agent key cannot be attached again",
    );
  }

  if (ledger.agentIndex.has(agentPubkey)) {
    const prior = ledger.agentIndex.get(agentPubkey)!;
    if (prior === operatorPubkey) {
      const existing = member.agents.find(
        (a) => a.agentPubkey === agentPubkey && a.status === "active",
      );
      if (existing) {
        throw new CommunityMembershipError(
          "agent_already_bound",
          "community: agent is already bound to this operator",
        );
      }
    } else {
      throw new CommunityMembershipError(
        "agent_already_bound",
        "community: agent is already bound to another operator",
      );
    }
  }

  const ownerAuthTag = [
    "auth",
    params.ownerAuthTag[1]!,
    params.ownerAuthTag[2] ?? "",
    params.ownerAuthTag[3]!,
  ] as CommunityOwnerAuthTag;

  const binding: CommunityAgentBinding = {
    agentPubkey,
    operatorPubkey,
    ownerAuthTag,
    status: "active",
    capabilityGrant: "active",
    persona: params.persona ? attachPersonaRef(params.persona) : undefined,
    attachedAt: nowIso(params.attachedAt),
  };
  assertCommunityPublicSafe(binding);

  const agents = [
    ...member.agents.filter((a) => a.agentPubkey !== agentPubkey),
    binding,
  ];
  const nextMember: CommunityMember = { ...member, agents };
  const members = new Map(ledger.members);
  members.set(operatorPubkey, nextMember);
  const agentIndex = new Map(ledger.agentIndex);
  agentIndex.set(agentPubkey, operatorPubkey);

  return {
    ledger: cloneLedger(ledger, { members, agentIndex }),
    binding,
  };
};

/**
 * Burn an agent key community-wide without requiring a current binding.
 *
 * `revokeAgent` reads `agentIndex` and throws `agent_not_found` when the key
 * has no live binding. That is correct for an operator-driven revoke of an
 * agent they can see, and wrong for anything that folds a signed record stream:
 * a fold does not choose the order it is handed events in. A revocation can be
 * admitted before the attestation it revokes — the relay returned them out of
 * order, or the original attestation aged out of the replay window and the
 * operator re-signed a fresh one with a later timestamp. In that order
 * `revokeAgent` threw, burned nothing, and the later attestation attached
 * cleanly:
 *
 *     revokeAgent before any binding threw: agent_not_found
 *     isAgentKeyBurned after that revocation: false
 *     PROBE RESULT: revoked-then-attested key is admitted = true
 *
 * This is the same law the two earlier fixes were reaching for and the third
 * place it leaked. Revocation binds the *key*. It is not a statement about the
 * binding row that happens to exist when the revocation is read, any more than
 * it was a statement about the membership event that carried it. So this burns
 * unconditionally and marks the binding revoked only if there is one to mark.
 *
 * Monotonic: the burn set only grows, and it is rebuilt from the same record
 * stream on a restart.
 */
export const burnAgentKey = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly agentPubkey: string;
    readonly reason?: string;
    readonly revokedAt?: string;
  },
): {
  readonly ledger: CommunityMembershipLedger;
  readonly binding: CommunityAgentBinding | null;
} => {
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");
  const burnedAgentKeys = withBurnedKeys(ledger, [agentPubkey]);

  const operatorPubkey = ledger.agentIndex.get(agentPubkey);
  const member =
    operatorPubkey === undefined ? undefined : ledger.members.get(operatorPubkey);
  const existing = member?.agents.find((a) => a.agentPubkey === agentPubkey);

  // No live binding: the key is burned and there is nothing else to mark. This
  // is the case `revokeAgent` refused, and refusing it is what let the key back
  // in.
  if (member === undefined || existing === undefined || operatorPubkey === undefined) {
    return { ledger: cloneLedger(ledger, { burnedAgentKeys }), binding: null };
  }
  if (existing.status === "revoked") {
    return { ledger: cloneLedger(ledger, { burnedAgentKeys }), binding: existing };
  }

  const at = nowIso(params.revokedAt);
  const binding: CommunityAgentBinding = {
    ...existing,
    status: "revoked",
    capabilityGrant: "revoked",
    revokedAt: at,
    revokeReason: params.reason,
  };
  assertCommunityPublicSafe(binding);

  const agents = member.agents.map((a) =>
    a.agentPubkey === agentPubkey ? binding : a,
  );
  const members = new Map(ledger.members);
  members.set(operatorPubkey, { ...member, agents });
  const agentIndex = new Map(ledger.agentIndex);
  agentIndex.delete(agentPubkey);

  return {
    ledger: cloneLedger(ledger, { members, agentIndex, burnedAgentKeys }),
    binding,
  };
};

/**
 * Revoke one agent. Immediate. Drops group membership and capability grant.
 * Never mutates the operator's machine, home, or credentials.
 *
 * Requires a live binding and reports `agent_not_found` without one, so an
 * operator revoking an agent they cannot see is told rather than silently
 * succeeding. Record folds must use {@link burnAgentKey} instead — see its
 * note for why the strictness is a hole on that path.
 */
export const revokeAgent = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly agentPubkey: string;
    readonly reason?: string;
    readonly revokedAt?: string;
  },
): {
  readonly ledger: CommunityMembershipLedger;
  readonly binding: CommunityAgentBinding;
} => {
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");
  const operatorPubkey = ledger.agentIndex.get(agentPubkey);
  if (!operatorPubkey) {
    throw new CommunityMembershipError(
      "agent_not_found",
      "community: agent is not registered",
    );
  }
  const member = ledger.members.get(operatorPubkey);
  if (!member) {
    throw new CommunityMembershipError(
      "member_not_found",
      "community: operator missing for agent",
    );
  }
  const existing = member.agents.find((a) => a.agentPubkey === agentPubkey);
  if (!existing) {
    throw new CommunityMembershipError(
      "agent_not_found",
      "community: agent binding missing",
    );
  }
  if (existing.status === "revoked") {
    throw new CommunityMembershipError(
      "agent_revoked",
      "community: agent is already revoked",
    );
  }

  const at = nowIso(params.revokedAt);
  const binding: CommunityAgentBinding = {
    ...existing,
    status: "revoked",
    capabilityGrant: "revoked",
    revokedAt: at,
    revokeReason: params.reason,
  };
  // Public record only — no agent-home path, no provider key, no remote wipe.
  assertCommunityPublicSafe(binding);

  const agents = member.agents.map((a) =>
    a.agentPubkey === agentPubkey ? binding : a,
  );
  const members = new Map(ledger.members);
  members.set(operatorPubkey, { ...member, agents });
  const agentIndex = new Map(ledger.agentIndex);
  agentIndex.delete(agentPubkey);

  return {
    ledger: cloneLedger(ledger, {
      members,
      agentIndex,
      burnedAgentKeys: withBurnedKeys(ledger, [agentPubkey]),
    }),
    binding,
  };
};

/**
 * Revoke a member and every agent under them. Immediate and cheap.
 * Does not reach into any operator machine.
 */
export const revokeMember = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly operatorPubkey: string;
    readonly reason?: string;
    readonly revokedAt?: string;
  },
): {
  readonly ledger: CommunityMembershipLedger;
  readonly member: CommunityMember;
} => {
  const operatorPubkey = assertHexPubkey(params.operatorPubkey, "operatorPubkey");
  const member = ledger.members.get(operatorPubkey);
  if (!member) {
    throw new CommunityMembershipError(
      "member_not_found",
      "community: member not found",
    );
  }
  if (member.status === "revoked") {
    throw new CommunityMembershipError(
      "member_revoked",
      "community: member is already revoked",
    );
  }

  const at = nowIso(params.revokedAt);
  const agents = member.agents.map((a) =>
    a.status === "revoked"
      ? a
      : {
          ...a,
          status: "revoked" as const,
          capabilityGrant: "revoked" as const,
          revokedAt: at,
          revokeReason: params.reason ?? "member_revoked",
        },
  );
  const next: CommunityMember = {
    ...member,
    status: "revoked",
    agents,
    revokedAt: at,
    revokeReason: params.reason,
  };
  assertCommunityPublicSafe(next);

  const members = new Map(ledger.members);
  members.set(operatorPubkey, next);
  const agentIndex = new Map(ledger.agentIndex);
  for (const a of member.agents) {
    agentIndex.delete(a.agentPubkey);
  }

  // Revoking a member revokes their agents, so those keys burn too. Otherwise
  // a re-invited operator could re-attach every key they had before.
  return {
    ledger: cloneLedger(ledger, {
      members,
      agentIndex,
      burnedAgentKeys: withBurnedKeys(
        ledger,
        member.agents.map((a) => a.agentPubkey),
      ),
    }),
    member: next,
  };
};

/** True when an agent may act: active binding, active grant, active member. */
export const isAgentAdmitted = (
  ledger: CommunityMembershipLedger,
  agentPubkey: string,
): boolean => {
  const op = ledger.agentIndex.get(agentPubkey.toLowerCase());
  if (!op) return false;
  const member = ledger.members.get(op);
  if (!member || member.status !== "active") return false;
  const binding = member.agents.find(
    (a) => a.agentPubkey === agentPubkey.toLowerCase(),
  );
  return (
    binding !== undefined &&
    binding.status === "active" &&
    binding.capabilityGrant === "active"
  );
};

/** True when an operator is an active community member. */
export const isMemberActive = (
  ledger: CommunityMembershipLedger,
  operatorPubkey: string,
): boolean => {
  const member = ledger.members.get(operatorPubkey.toLowerCase());
  return member?.status === "active";
};

/**
 * Check relay admission for an agent AUTH event against the membership ledger.
 * Anonymous and unattested keys are refused even if they present a pubkey.
 */
export const checkRelayAdmission = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly agentPubkey: string;
    readonly authEventTags: ReadonlyArray<ReadonlyArray<string>>;
  },
): { readonly admitted: true; readonly operatorPubkey: string } => {
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");
  const { operatorPubkey } = admitAttestedAgentKey({
    agentPubkey,
    authEventTags: params.authEventTags,
  });
  if (!isMemberActive(ledger, operatorPubkey)) {
    throw new CommunityMembershipError(
      "member_not_found",
      "community: attested operator is not an active member",
    );
  }
  if (!isAgentAdmitted(ledger, agentPubkey)) {
    throw new CommunityMembershipError(
      "agent_revoked",
      "community: agent is not admitted (missing, revoked, or grant revoked)",
    );
  }
  // Confirm the AUTH tag operator matches the ledger binding operator.
  const member = ledger.members.get(operatorPubkey)!;
  const binding = member.agents.find((a) => a.agentPubkey === agentPubkey)!;
  if (binding.operatorPubkey !== operatorPubkey) {
    throw new CommunityMembershipError(
      "agent_operator_mismatch",
      "community: ledger operator mismatch",
    );
  }
  return { admitted: true, operatorPubkey };
};

/**
 * Record a rate-limit action against the operator (not the agent key).
 * Anti-sybil: many keys under one operator share one budget.
 */
export const recordOperatorAction = (
  ledger: CommunityMembershipLedger,
  params: {
    readonly operatorPubkey: string;
    readonly at?: string;
    readonly windowSeconds?: number;
    readonly maxActions?: number;
  },
): {
  readonly ledger: CommunityMembershipLedger;
  readonly limit: CommunityOperatorRateLimit;
} => {
  const operatorPubkey = assertHexPubkey(params.operatorPubkey, "operatorPubkey");
  if (!isMemberActive(ledger, operatorPubkey)) {
    throw new CommunityMembershipError(
      "member_not_found",
      "community: rate limit only applies to active members",
    );
  }
  const at = nowIso(params.at);
  const windowSeconds = params.windowSeconds ?? 3_600;
  const maxActions = params.maxActions ?? 60;
  const existing = ledger.rateLimits.get(operatorPubkey);
  let limit: CommunityOperatorRateLimit;
  if (
    !existing ||
    Date.parse(at) - Date.parse(existing.windowStartedAt) >=
      existing.windowSeconds * 1_000
  ) {
    limit = {
      operatorPubkey,
      windowStartedAt: at,
      windowSeconds,
      maxActions,
      actionCount: 1,
    };
  } else {
    if (existing.actionCount >= existing.maxActions) {
      throw new CommunityMembershipError(
        "rate_limit_exceeded",
        "community: per-operator rate limit exceeded",
      );
    }
    limit = {
      ...existing,
      actionCount: existing.actionCount + 1,
    };
  }
  const rateLimits = new Map(ledger.rateLimits);
  rateLimits.set(operatorPubkey, limit);
  return { ledger: cloneLedger(ledger, { rateLimits }), limit };
};

/** Resolve the rate-limit unit for an agent: its operator pubkey. */
export const rateLimitUnitForAgent = (
  ledger: CommunityMembershipLedger,
  agentPubkey: string,
): string => {
  const op = ledger.agentIndex.get(agentPubkey.toLowerCase());
  if (!op) {
    throw new CommunityMembershipError(
      "agent_not_found",
      "community: unknown agent has no rate-limit unit",
    );
  }
  return op;
};
