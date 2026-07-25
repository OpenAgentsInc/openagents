/**
 * Membership folded from signed records (omega#48).
 *
 * The security weight of this file is the revocation half. Two earlier fixes
 * closed replay and re-admission for a revocation that arrives *after* the
 * attestation it revokes. A fold does not get to choose that order, and the
 * probe that motivated `burnAgentKey` showed the other order was still open:
 *
 *     revokeAgent before any binding threw: agent_not_found
 *     isAgentKeyBurned after that revocation: false
 *     PROBE RESULT: revoked-then-attested key is admitted = true
 */
import { bytesToHex } from "@noble/hashes/utils";
import { finalizeEvent } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import {
  generateSecretKeyBytes,
  publicKeyFromSecret,
  signOwnerAuthTag,
} from "../nostr-identity/index.ts";
import {
  attachOwnerAttestation,
  burnAgentKey,
  createEmptyLedger,
  isAgentAdmitted,
  isAgentKeyBurned,
  revokeAgent,
} from "./index.ts";
import {
  NIP_29_GROUP_METADATA_KIND,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  assertCommunityGroupIdIsNotPrivateConversation,
  communityOperatorForAgent,
  communityRoleFor,
  foldCommunityLedgerFromEvents,
  type CommunitySignedEvent,
} from "./records.ts";
import { NIP_AP_PERSONA_KIND } from "./types.ts";

const GROUP = "oa.community.v1";

const party = () => {
  const secretKey = generateSecretKeyBytes();
  const secretKeyHex = bytesToHex(secretKey);
  return { secretKey, secretKeyHex, pubkey: publicKeyFromSecret(secretKey) };
};

const sign = (
  secretKey: Uint8Array,
  input: Readonly<{
    kind: number;
    created_at: number;
    tags: ReadonlyArray<ReadonlyArray<string>>;
    content?: string;
  }>,
): CommunitySignedEvent =>
  finalizeEvent(
    {
      kind: input.kind,
      created_at: input.created_at,
      tags: input.tags.map((tag) => [...tag]),
      content: input.content ?? "",
    },
    secretKey,
  ) as unknown as CommunitySignedEvent;

const putUser = (
  admin: ReturnType<typeof party>,
  subject: string,
  createdAt: number,
): CommunitySignedEvent =>
  sign(admin.secretKey, {
    kind: NIP_29_PUT_USER_KIND,
    created_at: createdAt,
    tags: [
      ["h", GROUP],
      ["p", subject],
    ],
  });

const removeUser = (
  admin: ReturnType<typeof party>,
  subject: string,
  createdAt: number,
): CommunitySignedEvent =>
  sign(admin.secretKey, {
    kind: NIP_29_REMOVE_USER_KIND,
    created_at: createdAt,
    tags: [
      ["h", GROUP],
      ["p", subject],
    ],
  });

/** An agent publishes its own persona carrying the NIP-OA owner attestation. */
const persona = (
  agent: ReturnType<typeof party>,
  operator: ReturnType<typeof party>,
  createdAt: number,
  dTag = "worker",
): CommunitySignedEvent => {
  const authTag = attachOwnerAttestation({
    agentPubkey: agent.pubkey,
    operatorSeckeyHex: operator.secretKeyHex,
  });
  return sign(agent.secretKey, {
    kind: NIP_AP_PERSONA_KIND,
    created_at: createdAt,
    tags: [
      ["d", dTag],
      ["h", GROUP],
      [...authTag],
      ["capability", "capability.community.agentic_coding", "Agentic coding"],
    ],
  });
};

const fold = (events: ReadonlyArray<CommunitySignedEvent>, admin: string) =>
  foldCommunityLedgerFromEvents({
    groupId: GROUP,
    adminPubkeys: [admin],
    events,
  });

describe("membership folds from signed community records", () => {
  test("an admin put-user admits a member and a persona binds their agent", () => {
    const admin = party();
    const operator = party();
    const agent = party();

    const result = fold(
      [putUser(admin, operator.pubkey, 1_000), persona(agent, operator, 1_100)],
      admin.pubkey,
    );

    expect(result.refusals).toEqual([]);
    expect(result.members).toHaveLength(1);
    expect(result.members[0]?.operatorPubkey).toBe(operator.pubkey);
    expect(result.agents).toHaveLength(1);
    expect(result.agents[0]?.agentPubkey).toBe(agent.pubkey);
    expect(result.agents[0]?.operatorPubkey).toBe(operator.pubkey);
    expect(result.agents[0]?.persona?.declaredCapabilities[0]?.label).toBe("Agentic coding");
    expect(isAgentAdmitted(result.ledger, agent.pubkey)).toBe(true);
    expect(communityOperatorForAgent(result, agent.pubkey)).toBe(operator.pubkey);
  });

  test("a put-user from a key outside the admitted admin set changes nothing", () => {
    const admin = party();
    const impostor = party();
    const operator = party();

    const result = fold([putUser(impostor, operator.pubkey, 1_000)], admin.pubkey);

    expect(result.members).toEqual([]);
    expect(result.refusals[0]?.code).toBe("not_group_admin");
  });

  test("a persona whose attestation names the agent itself is refused", () => {
    const admin = party();
    const agent = party();
    // Built with the raw signer rather than `attachOwnerAttestation`, which
    // refuses to mint a self-attestation at all. The fold has to refuse one
    // that arrives off a wire anyway — an attacker does not use our builder.
    const selfSigned = signOwnerAuthTag({
      agentPubkey: agent.pubkey,
      conditions: "",
      ownerSeckeyHex: agent.secretKeyHex,
    });
    expect(selfSigned[1]).toBe(agent.pubkey);

    const result = fold(
      [
        putUser(admin, agent.pubkey, 1_000),
        sign(agent.secretKey, {
          kind: NIP_AP_PERSONA_KIND,
          created_at: 1_100,
          tags: [["d", "worker"], ["h", GROUP], [...selfSigned]],
        }),
      ],
      admin.pubkey,
    );
    expect(result.agents).toEqual([]);
    expect(result.refusals.some((row) => row.code === "attestation_invalid")).toBe(true);
  });

  test("a record naming another group cannot move this group", () => {
    const admin = party();
    const operator = party();
    const foreign = sign(admin.secretKey, {
      kind: NIP_29_PUT_USER_KIND,
      created_at: 1_000,
      tags: [
        ["h", "some.other.group"],
        ["p", operator.pubkey],
      ],
    });
    const result = fold([foreign], admin.pubkey);
    expect(result.members).toEqual([]);
    expect(result.refusals[0]?.code).toBe("wrong_group");
  });

  test("group metadata projects without granting anything", () => {
    const admin = party();
    const result = fold(
      [
        sign(admin.secretKey, {
          kind: NIP_29_GROUP_METADATA_KIND,
          created_at: 900,
          tags: [
            ["d", GROUP],
            ["name", "OpenAgents community workroom"],
          ],
        }),
      ],
      admin.pubkey,
    );
    expect(result.metadata?.name).toBe("OpenAgents community workroom");
    expect(result.members).toEqual([]);
  });
});

describe("revocation binds the key, in any order the records arrive", () => {
  test("a revoked agent key cannot be restored by replaying its attestation", () => {
    const admin = party();
    const operator = party();
    const agent = party();

    const records = [
      putUser(admin, operator.pubkey, 1_000),
      persona(agent, operator, 1_100),
      removeUser(admin, agent.pubkey, 1_200),
    ];
    const revoked = fold(records, admin.pubkey);
    expect(isAgentAdmitted(revoked.ledger, agent.pubkey)).toBe(false);
    expect(isAgentKeyBurned(revoked.ledger, agent.pubkey)).toBe(true);

    // The attacker re-sends every record they ever saw, including the one that
    // granted the agent.
    const replayed = fold([...records, persona(agent, operator, 1_100)], admin.pubkey);
    expect(isAgentAdmitted(replayed.ledger, agent.pubkey)).toBe(false);
    expect(isAgentKeyBurned(replayed.ledger, agent.pubkey)).toBe(true);
  });

  test("a revocation admitted before the attestation it revokes still burns the key", () => {
    const admin = party();
    const operator = party();
    const agent = party();

    // This is the order the probe found open. The revocation is older than the
    // attestation — the operator re-signed a fresh persona after being removed,
    // or the original attestation aged out of the relay's replay window.
    const result = fold(
      [
        putUser(admin, operator.pubkey, 1_000),
        removeUser(admin, agent.pubkey, 1_100),
        persona(agent, operator, 1_200),
      ],
      admin.pubkey,
    );

    expect(isAgentKeyBurned(result.ledger, agent.pubkey)).toBe(true);
    expect(isAgentAdmitted(result.ledger, agent.pubkey)).toBe(false);
    expect(result.refusals.some((row) => row.code === "agent_key_burned")).toBe(true);
  });

  test("re-inviting the operator does not launder their burned agent key", () => {
    const admin = party();
    const operator = party();
    const agent = party();

    const result = fold(
      [
        putUser(admin, operator.pubkey, 1_000),
        persona(agent, operator, 1_100),
        removeUser(admin, operator.pubkey, 1_200),
        // Re-admission is a supported path: revocation bars the key, not the
        // person. It must not restore the fleet they held before.
        putUser(admin, operator.pubkey, 1_300),
        persona(agent, operator, 1_400),
      ],
      admin.pubkey,
    );

    expect(result.members[0]?.status).toBe("active");
    expect(isAgentKeyBurned(result.ledger, agent.pubkey)).toBe(true);
    expect(isAgentAdmitted(result.ledger, agent.pubkey)).toBe(false);
  });

  test("the burn survives a restart that replays the whole record stream from empty", () => {
    const admin = party();
    const operator = party();
    const agent = party();
    const records = [
      putUser(admin, operator.pubkey, 1_000),
      persona(agent, operator, 1_100),
      removeUser(admin, agent.pubkey, 1_200),
    ];

    // Every fold starts from `createEmptyLedger`, so this is the restart case:
    // no in-memory state carries over, only the records.
    const first = fold(records, admin.pubkey);
    const afterRestart = fold([...records].reverse(), admin.pubkey);

    expect(isAgentKeyBurned(first.ledger, agent.pubkey)).toBe(true);
    expect(isAgentKeyBurned(afterRestart.ledger, agent.pubkey)).toBe(true);
    expect(isAgentAdmitted(afterRestart.ledger, agent.pubkey)).toBe(false);
  });

  test("a revoked member loses their role immediately", () => {
    const admin = party();
    const operator = party();
    const agent = party();

    const active = fold(
      [putUser(admin, operator.pubkey, 1_000), persona(agent, operator, 1_100)],
      admin.pubkey,
    );
    expect(communityRoleFor(active, operator.pubkey).role).toBe("agent_operator");

    const revoked = fold(
      [
        putUser(admin, operator.pubkey, 1_000),
        persona(agent, operator, 1_100),
        removeUser(admin, operator.pubkey, 1_200),
      ],
      admin.pubkey,
    );
    const role = communityRoleFor(revoked, operator.pubkey);
    expect(role.role).toBe("read_only");
    expect(role.status).toBe("revoked");
    expect(role.admittedAgentPubkeys).toEqual([]);
  });

  test("burnAgentKey is monotonic and needs no live binding", () => {
    const agent = party();
    let ledger = createEmptyLedger({ groupId: GROUP });

    // The strict path refuses, which is what left the hole.
    expect(() => revokeAgent(ledger, { agentPubkey: agent.pubkey })).toThrow();
    expect(isAgentKeyBurned(ledger, agent.pubkey)).toBe(false);

    const burned = burnAgentKey(ledger, { agentPubkey: agent.pubkey });
    ledger = burned.ledger;
    expect(burned.binding).toBeNull();
    expect(isAgentKeyBurned(ledger, agent.pubkey)).toBe(true);

    // Burning twice is not an error and never un-burns.
    ledger = burnAgentKey(ledger, { agentPubkey: agent.pubkey }).ledger;
    expect(isAgentKeyBurned(ledger, agent.pubkey)).toBe(true);
  });
});

describe("roles come from the signed record, not from a default", () => {
  test("an unknown pubkey holds no role", () => {
    const admin = party();
    const stranger = party();
    const role = communityRoleFor(fold([], admin.pubkey), stranger.pubkey);
    expect(role.role).toBe("none");
    expect(role.operatorPubkey).toBeNull();
  });

  test("a member without an admitted agent is a member, not an agent operator", () => {
    const admin = party();
    const operator = party();
    const result = fold([putUser(admin, operator.pubkey, 1_000)], admin.pubkey);
    expect(communityRoleFor(result, operator.pubkey).role).toBe("member");
  });

  test("an admitted group admin holds the owner role", () => {
    const admin = party();
    const role = communityRoleFor(fold([], admin.pubkey), admin.pubkey);
    expect(role.role).toBe("owner");
    expect(role.isGroupAdmin).toBe(true);
  });
});

describe("the two rooms cannot share an identifier", () => {
  test("a group id equal to a private conversation ref is refused", () => {
    expect(() =>
      assertCommunityGroupIdIsNotPrivateConversation({
        groupId: "conversation.owner.private.1",
        privateConversationRefs: ["conversation.owner.private.1"],
      }),
    ).toThrow(/two-room rule/);
  });

  test("a distinct group id passes", () => {
    expect(() =>
      assertCommunityGroupIdIsNotPrivateConversation({
        groupId: GROUP,
        privateConversationRefs: ["conversation.owner.private.1"],
      }),
    ).not.toThrow();
  });
});
