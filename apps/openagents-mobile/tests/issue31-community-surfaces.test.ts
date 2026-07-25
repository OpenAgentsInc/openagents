/**
 * The community room's copy and its authority boundary (omega#48).
 *
 * Two exits could not be checked before this: "the room, invitation, and
 * first-run copy say that v1 awards experience only" had no invitation surface
 * and no first-run surface to read, and "authorized mobile roles can complete
 * each non-payment lifecycle action" was ambiguous about the four actions the
 * phone deliberately cannot sign.
 *
 * These tests render the real view rather than asserting on the constants
 * alone, because a constant nothing draws is copy nobody sees.
 */
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import {
  NIP_29_PUT_USER_KIND,
  NIP_AP_PERSONA_KIND,
  attachOwnerAttestation,
} from "@openagentsinc/sarah/community";

import { defaultMobileAccessibilityProfile } from "../src/screens/khala-core.ts";
import { emptyIssue31WorkroomReadModel } from "../src/workroom/issue31-workroom-read-model.ts";
import { initialIssue31MobileNostrControlState } from "../src/workroom/issue31-mobile-nostr-runtime.ts";
import { renderMobileIssue31WorkroomView } from "../src/screens/mobile-issue31-workroom-view.ts";
import { readIssue31FullAutoProjection } from "../src/workroom/issue31-full-auto-read-model.ts";
import type {
  Issue31ConfirmedEvent,
  Issue31NostrClientSnapshot,
} from "../src/workroom/issue31-nostr-client.ts";
import {
  ISSUE31_COMMUNITY_CONTROL_KINDS,
  ISSUE31_COMMUNITY_EXPERIENCE_COPY_SURFACES,
  ISSUE31_COMMUNITY_FIRST_RUN_COPY,
  ISSUE31_COMMUNITY_INVITATION_COPY,
  ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY,
  ISSUE31_COMMUNITY_ROOM_COPY,
  projectIssue31CommunityReadModel,
  type Issue31CommunityControlKind,
  type Issue31CommunityReadModel,
} from "../src/workroom/issue31-community-read-model.ts";

const GROUP = "oa.community.surfaces.v1";
const NOW = 1_800_000_000;

const party = () => {
  const secretKey = generateSecretKey();
  return {
    secretKey,
    secretKeyHex: [...secretKey].map((b) => b.toString(16).padStart(2, "0")).join(""),
    pubkey: getPublicKey(secretKey),
  };
};

const ADMIN = party();

const sign = (
  secretKey: Uint8Array,
  input: Readonly<{
    kind: number;
    created_at?: number;
    tags: ReadonlyArray<ReadonlyArray<string>>;
    content?: string;
  }>,
): Issue31ConfirmedEvent => {
  const event = finalizeEvent(
    {
      kind: input.kind,
      created_at: input.created_at ?? NOW - 1_000,
      tags: input.tags.map((tag) => [...tag]),
      content: input.content ?? "",
    },
    secretKey,
  );
  return {
    relayUrl: "wss://relay.test",
    room: "community",
    event: event as unknown as Issue31ConfirmedEvent["event"],
    canonicalRecordId: event.id,
    privateRumorId: null,
    privateRecord: null,
    hostAnnouncement: null,
  };
};

const snapshotOf = (events: ReadonlyArray<Issue31ConfirmedEvent>): Issue31NostrClientSnapshot => ({
  devicePublicKeyHex: null,
  admittedHostPublicKeys: [],
  selectedHostPublicKeys: [],
  ownerPrivateAuthors: [],
  ownerRecipientPublicKeys: [],
  relays: [],
  confirmedEvents: events,
  storedEventIds: {},
  publishRefusals: {},
});

const projectFor = (
  viewerPubkey: string | null,
  events: ReadonlyArray<Issue31ConfirmedEvent> = [],
): Issue31CommunityReadModel =>
  projectIssue31CommunityReadModel(snapshotOf(events), {
    groupId: GROUP,
    adminPubkeys: [ADMIN.pubkey],
    scorerPubkeys: [],
    ownerAppealPubkey: null,
    viewerPubkey,
    nowUnixSeconds: NOW,
  });

const renderCommunity = (
  model: Issue31CommunityReadModel,
  historyNotice: string | null = null,
): string =>
  JSON.stringify(
    renderMobileIssue31WorkroomView(
      emptyIssue31WorkroomReadModel(),
      "community",
      { ...initialIssue31MobileNostrControlState(), communityHistoryNotice: historyNotice },
      defaultMobileAccessibilityProfile,
      { draft: "", memoryQuery: "", reminderDraft: "", transcriptLimit: 20, notice: null },
      readIssue31FullAutoProjection(null, null),
      model,
      { draft: "", subject: "", appealDraft: "", notice: null },
    ),
  );

describe("the room, invitation, and first-run copy say experience only", () => {
  test("every experience-only surface states the rule and disclaims payment", () => {
    expect(ISSUE31_COMMUNITY_EXPERIENCE_COPY_SURFACES.length).toBeGreaterThanOrEqual(4);
    for (const copy of ISSUE31_COMMUNITY_EXPERIENCE_COPY_SURFACES) {
      expect(copy).toMatch(/experience/i);
      expect(copy).toMatch(/pays no money/i);
      // Never an earning, and never a hint of a rail that does not exist in v1.
      expect(copy).not.toMatch(/earn|payout|wallet|settle|escrow|invoice|payment/i);
    }
  });

  test("somebody who has never joined is told why, before they do anything", () => {
    const stranger = party();
    const model = projectFor(stranger.pubkey, [putUserFor(party().pubkey)]);
    expect(model.viewerRole).toBe("none");

    const serialized = renderCommunity(model);
    expect(serialized).toContain(ISSUE31_COMMUNITY_FIRST_RUN_COPY);
    // Invitation-only is stated, not implied by an absent button.
    expect(ISSUE31_COMMUNITY_FIRST_RUN_COPY).toMatch(/invitation-only/i);
    expect(serialized).toContain(ISSUE31_COMMUNITY_ROOM_COPY);
  });

  test("a member who has joined is not shown the first-run explanation again", () => {
    const member = party();
    const model = projectFor(member.pubkey, [putUserFor(member.pubkey)]);
    expect(model.viewerRole).toBe("member");
    expect(renderCommunity(model)).not.toContain(ISSUE31_COMMUNITY_FIRST_RUN_COPY);
  });

  test("an admin is told what they are offering, where they offer it", () => {
    const model = projectFor(ADMIN.pubkey, [putUserFor(ADMIN.pubkey)]);
    expect(model.controls.map((control) => control.kind)).toContain("invite_member");
    const serialized = renderCommunity(model);
    expect(serialized).toContain(ISSUE31_COMMUNITY_INVITATION_COPY);
  });

  test("a member who cannot invite is not shown the invitation copy", () => {
    const member = party();
    const model = projectFor(member.pubkey, [putUserFor(member.pubkey)]);
    expect(model.controls.map((control) => control.kind)).not.toContain("invite_member");
    expect(renderCommunity(model)).not.toContain(ISSUE31_COMMUNITY_INVITATION_COPY);
  });

  test("no rendered community surface offers money", () => {
    const member = party();
    const serialized = renderCommunity(projectFor(member.pubkey, [putUserFor(member.pubkey)]));
    expect(serialized).not.toMatch(/earning|payout|wallet|settlement|escrow/i);
  });
});

describe("where each lifecycle action is signed", () => {
  test("every control kind has exactly one stated authority", () => {
    for (const kind of ISSUE31_COMMUNITY_CONTROL_KINDS) {
      const authority = ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY[kind];
      expect(authority).toBeDefined();
      expect(authority.reason.length).toBeGreaterThan(20);
      expect(authority.signingKeyHome).toBe(
        authority.signer === "operator_device" ? "this_phone" : "operator_compute",
      );
    }
    expect(Object.keys(ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY).sort()).toEqual(
      [...ISSUE31_COMMUNITY_CONTROL_KINDS].sort(),
    );
  });

  test("the split is not empty in either direction", () => {
    const kinds = ISSUE31_COMMUNITY_CONTROL_KINDS;
    const onPhone = kinds.filter(
      (kind) => ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY[kind].signer === "operator_device",
    );
    const onCompute = kinds.filter(
      (kind) => ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY[kind].signer === "agent_compute",
    );
    // Five operator actions the phone signs; four agent actions it never does.
    expect(onPhone).toEqual([
      "invite_member",
      "revoke_member",
      "revoke_agent",
      "post_message",
      "file_appeal",
    ]);
    expect(onCompute).toEqual([
      "attach_agent",
      "quote_work_unit",
      "submit_result",
      "verify_result",
    ]);
  });

  test("a projected control never contradicts the stated authority", () => {
    const operator = party();
    const agent = party();
    const model = projectFor(operator.pubkey, [
      putUserFor(operator.pubkey),
      personaFor(agent, operator),
    ]);
    expect(model.viewerRole).toBe("agent_operator");
    const all = [...model.controls, ...model.workUnits.flatMap((unit) => unit.controls)];
    expect(all.length).toBeGreaterThan(0);
    for (const control of all) {
      expect(control.signedBy).toBe(ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY[control.kind].signer);
    }
  });

  test("an agent-signed action is stated, never offered as a button", () => {
    const operator = party();
    const agent = party();
    const model = projectFor(operator.pubkey, [
      putUserFor(operator.pubkey),
      personaFor(agent, operator),
    ]);
    // `attach_agent` is the reachable agent-signed room control; the phone
    // renders it as a sentence about the operator's own compute.
    const attach = model.controls.find((control) => control.kind === "attach_agent");
    expect(attach?.signedBy).toBe("agent_compute");

    const view = renderMobileIssue31WorkroomView(
      emptyIssue31WorkroomReadModel(),
      "community",
      initialIssue31MobileNostrControlState(),
      defaultMobileAccessibilityProfile,
      { draft: "", memoryQuery: "", reminderDraft: "", transcriptLimit: 20, notice: null },
      readIssue31FullAutoProjection(null, null),
      model,
      { draft: "", subject: "", appealDraft: "", notice: null },
    );
    const buttons = collectButtonKinds(view);
    for (const kind of ISSUE31_COMMUNITY_CONTROL_KINDS) {
      if (ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY[kind].signer !== "agent_compute") continue;
      // A button here would claim this phone can assert that work happened on a
      // machine it does not hold the key for.
      expect(buttons).not.toContain(kind);
    }
    expect(JSON.stringify(view)).toContain("from your compute");
  });
});

describe("history that could not be persisted is said out loud", () => {
  test("a store refusal reaches the screen", () => {
    const member = party();
    const model = projectFor(member.pubkey, [putUserFor(member.pubkey)]);
    const notice = "A community record was not persisted: the store is full of revocations.";
    expect(renderCommunity(model, notice)).toContain(notice);
    expect(renderCommunity(model, null)).not.toContain("was not persisted");
  });
});

/** An admin admitting a key. */
function putUserFor(subject: string): Issue31ConfirmedEvent {
  return sign(ADMIN.secretKey, {
    kind: NIP_29_PUT_USER_KIND,
    created_at: NOW - 5_000,
    tags: [["h", GROUP], ["p", subject]],
  });
}

/** An agent's persona carrying its operator attestation. */
function personaFor(
  agent: ReturnType<typeof party>,
  operator: ReturnType<typeof party>,
): Issue31ConfirmedEvent {
  const authTag = attachOwnerAttestation({
    agentPubkey: agent.pubkey,
    operatorSeckeyHex: operator.secretKeyHex,
  });
  return sign(agent.secretKey, {
    kind: NIP_AP_PERSONA_KIND,
    created_at: NOW - 4_000,
    tags: [["d", "worker"], ["h", GROUP], [...authTag]],
  });
}

/**
 * Which control kinds are drawn as pressable buttons.
 *
 * Walks the rendered tree for real `Button` nodes rather than searching the
 * serialized JSON for a label, so a control rendered as descriptive text does
 * not count and a button with a reworded label still does.
 */
function collectButtonKinds(view: unknown): ReadonlyArray<Issue31CommunityControlKind> {
  const found: Issue31CommunityControlKind[] = [];
  const walk = (node: unknown): void => {
    if (Array.isArray(node)) {
      for (const child of node) walk(child);
      return;
    }
    if (node === null || typeof node !== "object") return;
    const record = node as Readonly<Record<string, unknown>>;
    const intent = record["onPress"];
    if (
      typeof record["type"] === "string" &&
      record["type"].toLowerCase().includes("button") &&
      intent !== undefined
    ) {
      const serialized = JSON.stringify(record);
      for (const kind of ISSUE31_COMMUNITY_CONTROL_KINDS) {
        if (serialized.includes(kind)) found.push(kind);
      }
    }
    for (const value of Object.values(record)) walk(value);
  };
  walk(view);
  return found;
}
