import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { describe, expect, it } from "vite-plus/test";

import {
  generateSecretKeyBytes,
  publicKeyFromSecret,
  signOwnerAuthTag,
} from "../nostr-identity/index.ts";
import {
  SARAH_COMMUNITY_MEMBERSHIP_GATE,
  SARAH_COMMUNITY_MEMBERSHIP_SCHEMA,
  SARAH_CW_02_PACKET,
  acceptInvitation,
  admitAttestedAgentKey,
  assertCommunityPublicSafe,
  attachAgent,
  attachOwnerAttestation,
  buildCommunityAttestedAuthTemplate,
  checkRelayAdmission,
  createEmptyLedger,
  extractAuthTagFromAuthEvent,
  isAgentAdmitted,
  isMemberActive,
  issueInvitation,
  rateLimitUnitForAgent,
  recordOperatorAction,
  refuseOpenJoin,
  revokeAgent,
  revokeMember,
  verifyAgentOwnerAttestation,
  CommunityMembershipError,
} from "./index.ts";

const skHex = (): string => bytesToHex(generateSecretKeyBytes());
const pkOf = (seckeyHex: string): string =>
  publicKeyFromSecret(hexToBytes(seckeyHex));

describe("SARAH-CW-02 community membership", () => {
  it("freezes invitation_only as the admitted membership gate", () => {
    expect(SARAH_COMMUNITY_MEMBERSHIP_GATE).toBe("invitation_only");
    expect(SARAH_CW_02_PACKET).toBe("SARAH-CW-02");
    const ledger = createEmptyLedger({ groupId: "community.openagents.v1" });
    expect(ledger.gateMode).toBe("invitation_only");
    expect(() =>
      createEmptyLedger({
        groupId: "x",
        gateMode: "open_with_probation",
      }),
    ).toThrow(CommunityMembershipError);
  });

  it("refuses open join without an invitation", () => {
    const ledger = createEmptyLedger({ groupId: "g1" });
    const op = pkOf(skHex());
    expect(() => refuseOpenJoin(ledger, op)).toThrow(/invitation_only/);
    try {
      refuseOpenJoin(ledger, op);
    } catch (e) {
      expect(e).toBeInstanceOf(CommunityMembershipError);
      expect((e as CommunityMembershipError).code).toBe("invitation_required");
    }
  });

  it("admits a member only through an open invitation", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const inviter = pkOf(skHex());
    const inviteeSk = skHex();
    const invitee = pkOf(inviteeSk);

    const issued = issueInvitation(ledger, {
      invitationId: "inv-1",
      inviterPubkey: inviter,
      inviteePubkey: invitee,
      createdAt: "2026-07-24T12:00:00.000Z",
    });
    ledger = issued.ledger;
    expect(issued.invitation.status).toBe("open");

    const wrong = pkOf(skHex());
    expect(() =>
      acceptInvitation(ledger, {
        invitationId: "inv-1",
        inviteePubkey: wrong,
      }),
    ).toThrow(/invitee/);

    const accepted = acceptInvitation(ledger, {
      invitationId: "inv-1",
      inviteePubkey: invitee,
      acceptedAt: "2026-07-24T12:01:00.000Z",
    });
    ledger = accepted.ledger;
    expect(accepted.member.schema).toBe(SARAH_COMMUNITY_MEMBERSHIP_SCHEMA);
    expect(accepted.member.status).toBe("active");
    expect(isMemberActive(ledger, invitee)).toBe(true);
    expect(ledger.invitations.get("inv-1")?.status).toBe("accepted");
  });

  it("rejects expired invitations", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const inviter = pkOf(skHex());
    const invitee = pkOf(skHex());
    const issued = issueInvitation(ledger, {
      invitationId: "inv-exp",
      inviterPubkey: inviter,
      inviteePubkey: invitee,
      createdAt: "2026-07-24T10:00:00.000Z",
      expiresAt: "2026-07-24T11:00:00.000Z",
    });
    ledger = issued.ledger;
    expect(() =>
      acceptInvitation(ledger, {
        invitationId: "inv-exp",
        inviteePubkey: invitee,
        acceptedAt: "2026-07-24T12:00:00.000Z",
      }),
    ).toThrow(/expired/);
  });

  it("binds an agent with NIP-OA and builds NIP-AA AUTH for relay admission", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const inviter = pkOf(skHex());
    const operatorSk = skHex();
    const operatorPk = pkOf(operatorSk);
    const agentSk = generateSecretKeyBytes();
    const agentPk = publicKeyFromSecret(agentSk);

    ledger = issueInvitation(ledger, {
      invitationId: "inv-2",
      inviterPubkey: inviter,
      inviteePubkey: operatorPk,
    }).ledger;
    ledger = acceptInvitation(ledger, {
      invitationId: "inv-2",
      inviteePubkey: operatorPk,
    }).ledger;

    const authTag = attachOwnerAttestation({
      agentPubkey: agentPk,
      operatorSeckeyHex: operatorSk,
      conditions: "kind=9",
    });
    expect(authTag[0]).toBe("auth");
    expect(authTag[1]).toBe(operatorPk);
    const verified = verifyAgentOwnerAttestation({
      agentPubkey: agentPk,
      ownerAuthTag: authTag,
      expectedOperatorPubkey: operatorPk,
    });
    expect(verified.operatorPubkey).toBe(operatorPk);

    // Self-attestation must fail
    const selfTag = signOwnerAuthTag({
      agentPubkey: operatorPk,
      conditions: "",
      ownerSeckeyHex: operatorSk,
    });
    // ownerPk === agentPubkey when self-signing for own key as agent
    expect(() =>
      verifyAgentOwnerAttestation({
        agentPubkey: operatorPk,
        ownerAuthTag: selfTag,
      }),
    ).toThrow(/self-attestation|NIP-OA/);

    const attached = attachAgent(ledger, {
      operatorPubkey: operatorPk,
      agentPubkey: agentPk,
      ownerAuthTag: authTag,
      persona: {
        kind: 30175,
        dTag: "coder-v1",
        displayName: "Community Coder",
        declaredCapabilities: [
          { capabilityRef: "capability.unit.quote", label: "Quote work units" },
        ],
      },
      attachedAt: "2026-07-24T13:00:00.000Z",
    });
    ledger = attached.ledger;
    expect(attached.binding.status).toBe("active");
    expect(attached.binding.capabilityGrant).toBe("active");
    expect(attached.binding.persona?.dTag).toBe("coder-v1");
    expect(isAgentAdmitted(ledger, agentPk)).toBe(true);

    const authTemplate = buildCommunityAttestedAuthTemplate({
      challenge: "relay-ch-1",
      relayUrl: "wss://relay.openagents.com",
      ownerAuthTag: authTag,
    });
    expect(authTemplate.kind).toBe(22242);
    const extracted = extractAuthTagFromAuthEvent(authTemplate.tags);
    expect(extracted[1]).toBe(operatorPk);

    // Anonymous AUTH refused
    expect(() =>
      extractAuthTagFromAuthEvent([
        ["relay", "wss://relay.openagents.com"],
        ["challenge", "x"],
      ]),
    ).toThrow(/anonymous|NIP-OA/);

    const admission = checkRelayAdmission(ledger, {
      agentPubkey: agentPk,
      authEventTags: authTemplate.tags,
    });
    expect(admission.admitted).toBe(true);
    expect(admission.operatorPubkey).toBe(operatorPk);

    const admitted = admitAttestedAgentKey({
      agentPubkey: agentPk,
      authEventTags: authTemplate.tags,
    });
    expect(admitted.operatorPubkey).toBe(operatorPk);
  });

  it("revokes an agent immediately without agent-home mutation fields", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const inviter = pkOf(skHex());
    const operatorSk = skHex();
    const operatorPk = pkOf(operatorSk);
    const agentPk = pkOf(skHex());

    ledger = issueInvitation(ledger, {
      invitationId: "inv-3",
      inviterPubkey: inviter,
      inviteePubkey: operatorPk,
    }).ledger;
    ledger = acceptInvitation(ledger, {
      invitationId: "inv-3",
      inviteePubkey: operatorPk,
    }).ledger;
    const authTag = attachOwnerAttestation({
      agentPubkey: agentPk,
      operatorSeckeyHex: operatorSk,
    });
    ledger = attachAgent(ledger, {
      operatorPubkey: operatorPk,
      agentPubkey: agentPk,
      ownerAuthTag: authTag,
    }).ledger;

    const revoked = revokeAgent(ledger, {
      agentPubkey: agentPk,
      reason: "operator_removed_agent",
      revokedAt: "2026-07-24T14:00:00.000Z",
    });
    ledger = revoked.ledger;
    expect(revoked.binding.status).toBe("revoked");
    expect(revoked.binding.capabilityGrant).toBe("revoked");
    expect(isAgentAdmitted(ledger, agentPk)).toBe(false);
    // Public record stays free of home/provider fields
    assertCommunityPublicSafe(revoked.binding);
    expect(() =>
      assertCommunityPublicSafe({ agentHomePath: "/tmp/agent" }),
    ).toThrow(CommunityMembershipError);
    expect(() =>
      assertCommunityPublicSafe({ providerApiKey: "sk-test" }),
    ).toThrow(CommunityMembershipError);

    const authTemplate = buildCommunityAttestedAuthTemplate({
      challenge: "c",
      relayUrl: "wss://relay.openagents.com",
      ownerAuthTag: authTag,
    });
    expect(() =>
      checkRelayAdmission(ledger, {
        agentPubkey: agentPk,
        authEventTags: authTemplate.tags,
      }),
    ).toThrow(/not admitted|revoked/);
  });

  it("revokes a member and all agents under them", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const inviter = pkOf(skHex());
    const operatorSk = skHex();
    const operatorPk = pkOf(operatorSk);
    const agentA = pkOf(skHex());
    const agentB = pkOf(skHex());

    ledger = issueInvitation(ledger, {
      invitationId: "inv-4",
      inviterPubkey: inviter,
      inviteePubkey: operatorPk,
    }).ledger;
    ledger = acceptInvitation(ledger, {
      invitationId: "inv-4",
      inviteePubkey: operatorPk,
    }).ledger;
    for (const agent of [agentA, agentB]) {
      const tag = attachOwnerAttestation({
        agentPubkey: agent,
        operatorSeckeyHex: operatorSk,
      });
      ledger = attachAgent(ledger, {
        operatorPubkey: operatorPk,
        agentPubkey: agent,
        ownerAuthTag: tag,
      }).ledger;
    }
    expect(isAgentAdmitted(ledger, agentA)).toBe(true);
    expect(isAgentAdmitted(ledger, agentB)).toBe(true);

    const revoked = revokeMember(ledger, {
      operatorPubkey: operatorPk,
      reason: "membership_ended",
    });
    ledger = revoked.ledger;
    expect(revoked.member.status).toBe("revoked");
    expect(revoked.member.agents.every((a) => a.status === "revoked")).toBe(
      true,
    );
    expect(revoked.member.agents.every((a) => a.capabilityGrant === "revoked")).toBe(
      true,
    );
    expect(isMemberActive(ledger, operatorPk)).toBe(false);
    expect(isAgentAdmitted(ledger, agentA)).toBe(false);
    expect(isAgentAdmitted(ledger, agentB)).toBe(false);
  });

  it("rate-limits per operator, not per agent key (anti-sybil)", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const inviter = pkOf(skHex());
    const operatorSk = skHex();
    const operatorPk = pkOf(operatorSk);
    const agentA = pkOf(skHex());
    const agentB = pkOf(skHex());

    ledger = issueInvitation(ledger, {
      invitationId: "inv-5",
      inviterPubkey: inviter,
      inviteePubkey: operatorPk,
    }).ledger;
    ledger = acceptInvitation(ledger, {
      invitationId: "inv-5",
      inviteePubkey: operatorPk,
    }).ledger;
    for (const agent of [agentA, agentB]) {
      ledger = attachAgent(ledger, {
        operatorPubkey: operatorPk,
        agentPubkey: agent,
        ownerAuthTag: attachOwnerAttestation({
          agentPubkey: agent,
          operatorSeckeyHex: operatorSk,
        }),
      }).ledger;
    }

    expect(rateLimitUnitForAgent(ledger, agentA)).toBe(operatorPk);
    expect(rateLimitUnitForAgent(ledger, agentB)).toBe(operatorPk);

    // maxActions=2 shared across both agent keys
    const at = "2026-07-24T15:00:00.000Z";
    ledger = recordOperatorAction(ledger, {
      operatorPubkey: operatorPk,
      at,
      windowSeconds: 3600,
      maxActions: 2,
    }).ledger;
    ledger = recordOperatorAction(ledger, {
      operatorPubkey: operatorPk,
      at: "2026-07-24T15:00:01.000Z",
      windowSeconds: 3600,
      maxActions: 2,
    }).ledger;
    expect(() =>
      recordOperatorAction(ledger, {
        operatorPubkey: operatorPk,
        at: "2026-07-24T15:00:02.000Z",
        windowSeconds: 3600,
        maxActions: 2,
      }),
    ).toThrow(/rate limit/);
  });

  it("refuses attaching an agent for a non-member or wrong operator attestation", () => {
    let ledger = createEmptyLedger({ groupId: "g1" });
    const strangerSk = skHex();
    const strangerPk = pkOf(strangerSk);
    const agentPk = pkOf(skHex());
    const tag = attachOwnerAttestation({
      agentPubkey: agentPk,
      operatorSeckeyHex: strangerSk,
    });
    expect(() =>
      attachAgent(ledger, {
        operatorPubkey: strangerPk,
        agentPubkey: agentPk,
        ownerAuthTag: tag,
      }),
    ).toThrow(/not a member/);

    const inviter = pkOf(skHex());
    const memberSk = skHex();
    const memberPk = pkOf(memberSk);
    ledger = issueInvitation(ledger, {
      invitationId: "inv-6",
      inviterPubkey: inviter,
      inviteePubkey: memberPk,
    }).ledger;
    ledger = acceptInvitation(ledger, {
      invitationId: "inv-6",
      inviteePubkey: memberPk,
    }).ledger;

    // Tag signed by stranger, not the member
    expect(() =>
      attachAgent(ledger, {
        operatorPubkey: memberPk,
        agentPubkey: agentPk,
        ownerAuthTag: tag,
      }),
    ).toThrow(/mismatch|operator/);
  });
});
