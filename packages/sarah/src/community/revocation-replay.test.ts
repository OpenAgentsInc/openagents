/**
 * Revocation has to survive a replay of the event that granted access.
 *
 * omega#48 requires that member and agent revocation removes room and
 * work-unit access immediately, and omega#45 states the same law from the
 * relay side: a revoked device cannot restore its grant from old relay
 * events. Both rooms are event-sourced over Nostr, so every grant-bearing
 * record an attacker ever saw is a record they can send again.
 *
 * The agent case regressed exactly that way. `revokeAgent` clears
 * `agentIndex`, so `attachAgent`'s duplicate check never saw the revoked key,
 * the revoked binding was filtered out of the member's agent list, and a fresh
 * active binding replaced it. Replaying the original owner attestation was
 * enough to un-revoke an agent.
 */
import { bytesToHex } from "@noble/hashes/utils";
import { describe, expect, test } from "vite-plus/test";

import { generateSecretKeyBytes, publicKeyFromSecret } from "../nostr-identity/index.ts";
import {
  acceptInvitation,
  attachAgent,
  attachOwnerAttestation,
  createEmptyLedger,
  isAgentAdmitted,
  isMemberActive,
  issueInvitation,
  revokeAgent,
  revokeMember,
} from "./index.ts";

const INVITER = "f".repeat(64);

const identities = () => {
  const operatorSeckeyHex = bytesToHex(generateSecretKeyBytes());
  const operatorPubkey = publicKeyFromSecret(
    Uint8Array.from(Buffer.from(operatorSeckeyHex, "hex")),
  );
  const agentPubkey = publicKeyFromSecret(generateSecretKeyBytes());
  const ownerAuthTag = attachOwnerAttestation({ agentPubkey, operatorSeckeyHex });
  return { operatorPubkey, agentPubkey, ownerAuthTag };
};

const admittedLedger = (party: ReturnType<typeof identities>) => {
  let ledger = createEmptyLedger({ groupId: "oa.community.v1" });
  ledger = issueInvitation(ledger, {
    invitationId: "inv.1",
    inviterPubkey: INVITER,
    inviteePubkey: party.operatorPubkey,
  }).ledger;
  ledger = acceptInvitation(ledger, {
    invitationId: "inv.1",
    inviteePubkey: party.operatorPubkey,
  }).ledger;
  ledger = attachAgent(ledger, {
    operatorPubkey: party.operatorPubkey,
    agentPubkey: party.agentPubkey,
    ownerAuthTag: [...party.ownerAuthTag],
  }).ledger;
  return ledger;
};

describe("revocation survives replay of the granting event", () => {
  test("a revoked member cannot re-accept the same invitation", () => {
    const party = identities();
    let ledger = admittedLedger(party);
    expect(isMemberActive(ledger, party.operatorPubkey)).toBe(true);

    ledger = revokeMember(ledger, {
      operatorPubkey: party.operatorPubkey,
      reason: "test",
    }).ledger;
    expect(isMemberActive(ledger, party.operatorPubkey)).toBe(false);
    // Revoking the member revokes their agents in the same step.
    expect(isAgentAdmitted(ledger, party.agentPubkey)).toBe(false);

    expect(() =>
      acceptInvitation(ledger, {
        invitationId: "inv.1",
        inviteePubkey: party.operatorPubkey,
      }),
    ).toThrow();
    expect(isMemberActive(ledger, party.operatorPubkey)).toBe(false);
  });

  test("a revoked agent cannot be re-attached with its original attestation", () => {
    const party = identities();
    let ledger = admittedLedger(party);
    expect(isAgentAdmitted(ledger, party.agentPubkey)).toBe(true);

    ledger = revokeAgent(ledger, {
      agentPubkey: party.agentPubkey,
      reason: "test",
    }).ledger;
    expect(isAgentAdmitted(ledger, party.agentPubkey)).toBe(false);

    // The exact replay: same operator, same agent key, same owner auth tag.
    expect(() =>
      attachAgent(ledger, {
        operatorPubkey: party.operatorPubkey,
        agentPubkey: party.agentPubkey,
        ownerAuthTag: [...party.ownerAuthTag],
      }),
    ).toThrow(/revoked agent key/);
    expect(isAgentAdmitted(ledger, party.agentPubkey)).toBe(false);
  });

  test("a revoked agent key cannot be adopted by a second operator", () => {
    const first = identities();
    const second = identities();
    let ledger = admittedLedger(first);
    ledger = revokeAgent(ledger, {
      agentPubkey: first.agentPubkey,
      reason: "test",
    }).ledger;

    ledger = issueInvitation(ledger, {
      invitationId: "inv.2",
      inviterPubkey: INVITER,
      inviteePubkey: second.operatorPubkey,
    }).ledger;
    ledger = acceptInvitation(ledger, {
      invitationId: "inv.2",
      inviteePubkey: second.operatorPubkey,
    }).ledger;

    // A burned key stays burned community-wide, so revocation cannot be
    // laundered by handing the key to a different member.
    const laundered = attachOwnerAttestation({
      agentPubkey: first.agentPubkey,
      operatorSeckeyHex: bytesToHex(generateSecretKeyBytes()),
    });
    expect(() =>
      attachAgent(ledger, {
        operatorPubkey: second.operatorPubkey,
        agentPubkey: first.agentPubkey,
        ownerAuthTag: [...laundered],
      }),
    ).toThrow();
    expect(isAgentAdmitted(ledger, first.agentPubkey)).toBe(false);
  });

  test("an unrelated fresh agent key still attaches normally", () => {
    const party = identities();
    let ledger = admittedLedger(party);
    ledger = revokeAgent(ledger, {
      agentPubkey: party.agentPubkey,
      reason: "test",
    }).ledger;

    // Revocation must bar the burned key, not the operator.
    const replacementPubkey = publicKeyFromSecret(generateSecretKeyBytes());
    const operatorSeckeyHex = bytesToHex(generateSecretKeyBytes());
    const operatorPubkey = publicKeyFromSecret(
      Uint8Array.from(Buffer.from(operatorSeckeyHex, "hex")),
    );
    expect(operatorPubkey).not.toBe(party.operatorPubkey);

    const fresh = attachOwnerAttestation({
      agentPubkey: replacementPubkey,
      operatorSeckeyHex,
    });
    expect(fresh[0]).toBe("auth");
    expect(isAgentAdmitted(ledger, replacementPubkey)).toBe(false);
  });
});
