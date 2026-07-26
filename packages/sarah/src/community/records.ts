/**
 * Community membership as a fold over signed Nostr records (omega#48).
 *
 * `membership.ts` froze the admission model as pure in-memory helpers and left
 * the wire form to integration work. Nothing ever supplied that wire form, so
 * no client could derive a role from a signed record — the mobile community
 * room hard-coded `read_only` for every row. This module is that wire form.
 *
 * ## What grants what
 *
 * The relay is transport and storage. It is not admission authority, and this
 * fold is written so that stays true:
 *
 * - The group's **admin keys are supplied out of band** by the caller, exactly
 *   like the admitted Omega host keys on the owner-private side. An event that
 *   claims membership authority is admitted only when its author is already in
 *   that set. A relay that hands us a forged `9000` changes nothing.
 * - An **agent key is bound to its operator by NIP-OA**, verified from the
 *   attestation's own signature (`verifyAgentOwnerAttestation`). The relay does
 *   not get a vote, and neither does the admin set.
 * - Every admission runs through the `membership.ts` functions rather than
 *   writing ledger state directly, so the invitation gate, the self-attestation
 *   refusal, and the monotonic burn set all apply to records off a wire exactly
 *   as they apply in memory.
 *
 * ## Carriers
 *
 * Per the frozen contract (`docs/omega/2026-07-24-community-workroom-contract.md`
 * §4, §7.1), membership and roles travel on NIP-29 admin events. No new kind is
 * invented here:
 *
 * | Kind | Author | Meaning |
 * | --- | --- | --- |
 * | `9000` put-user | group admin | admit the tagged pubkey to the group |
 * | `9001` remove-user | group admin | revoke the tagged pubkey, immediately |
 * | `39001` / `39002` | relay | admin / member list, read as corroboration only |
 * | `30175` NIP-AP persona | the agent key itself | carries the NIP-OA owner attestation binding that agent to its operator |
 * | `9` group chat | any member | room transcript |
 *
 * A persona event is the published carrier of an attestation that NIP-AA
 * otherwise only presents live on a NIP-42 AUTH. The `auth` tag is identical in
 * both places and is verified by the same code, so a client that was not
 * present for the AUTH can still check the binding for itself.
 *
 * ## Order
 *
 * A fold does not choose the order it is handed events in. Records are sorted
 * by `created_at` then id so the projection is deterministic, and revocation is
 * applied through {@link burnAgentKey}, which does not require the binding it
 * revokes to have been seen yet. See that function's note — an ordering-
 * dependent revocation is not a revocation.
 */
import {
  NIP_29_GROUP_CHAT_KIND,
  NIP_AP_PERSONA_KIND,
  type CommunityAgentBinding,
  type CommunityMember,
} from "./types.ts";
import {
  acceptInvitation,
  attachAgent,
  burnAgentKey,
  createEmptyLedger,
  isAgentAdmitted,
  isAgentKeyBurned,
  isMemberActive,
  issueInvitation,
  revokeMember,
  type CommunityMembershipLedger,
} from "./membership.ts";
import { extractAuthTagFromAuthEvent, verifyAgentOwnerAttestation } from "./attestation.ts";

/**
 * NIP-29 admin event: create the group.
 *
 * Moderation is scoped to a group that exists, so this is the precondition for
 * every other NIP-29 kind. A relay answering `restricted: group not found` to a
 * `put-user` is reporting a missing precondition, not a missing capability.
 */
export const NIP_29_CREATE_GROUP_KIND = 9007 as const;

/** NIP-29 admin event: admit a pubkey to the group. */
export const NIP_29_PUT_USER_KIND = 9000 as const;

/** NIP-29 admin event: remove a pubkey from the group. Immediate. */
export const NIP_29_REMOVE_USER_KIND = 9001 as const;

/** NIP-29 relay-signed group metadata. */
export const NIP_29_GROUP_METADATA_KIND = 39000 as const;

/** NIP-29 relay-signed admin list. Corroboration only — never authority. */
export const NIP_29_GROUP_ADMINS_KIND = 39001 as const;

/** NIP-29 relay-signed member list. Corroboration only — never authority. */
export const NIP_29_GROUP_MEMBERS_KIND = 39002 as const;

/**
 * Kinds this fold reads. A client subscribes to at least these to derive a
 * role from a signed record rather than assuming one.
 */
export const COMMUNITY_MEMBERSHIP_RECORD_KINDS = [
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  NIP_29_GROUP_METADATA_KIND,
  NIP_29_GROUP_ADMINS_KIND,
  NIP_29_GROUP_MEMBERS_KIND,
  NIP_AP_PERSONA_KIND,
] as const;

const HEX_64 = /^[0-9a-f]{64}$/;

export interface CommunitySignedEvent {
  readonly id: string;
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
}

/** Why one record did not change the ledger. Never silence. */
export type CommunityRecordRefusalCode =
  | "wrong_group"
  | "not_group_admin"
  | "missing_subject"
  | "self_attestation"
  | "attestation_invalid"
  | "operator_not_member"
  | "agent_key_burned"
  | "already_applied"
  | "persona_invalid"
  | "unreadable";

export interface CommunityRecordRefusal {
  readonly eventId: string;
  readonly kind: number;
  readonly code: CommunityRecordRefusalCode;
  readonly detail: string;
}

export interface CommunityGroupMetadata {
  readonly groupId: string;
  readonly name: string | null;
  readonly about: string | null;
  readonly sourceEventId: string;
}

export interface CommunityLedgerFold {
  readonly ledger: CommunityMembershipLedger;
  readonly groupId: string;
  readonly metadata: CommunityGroupMetadata | null;
  /** Admin keys the caller admitted out of band. Not read off the wire. */
  readonly adminPubkeys: ReadonlyArray<string>;
  readonly members: ReadonlyArray<CommunityMember>;
  readonly agents: ReadonlyArray<CommunityAgentBinding>;
  /** Agent keys revocation burned. Rebuilt from the record stream every fold. */
  readonly burnedAgentKeys: ReadonlyArray<string>;
  readonly refusals: ReadonlyArray<CommunityRecordRefusal>;
  /** Event ids that changed the ledger, in applied order. */
  readonly appliedEventIds: ReadonlyArray<string>;
}

const tagValue = (
  event: CommunitySignedEvent,
  name: string,
): string | undefined => event.tags.find((tag) => tag[0] === name)?.[1];

const tagValues = (
  event: CommunitySignedEvent,
  name: string,
): ReadonlyArray<string> =>
  event.tags.flatMap((tag) => (tag[0] === name && tag[1] !== undefined ? [tag[1]] : []));

/**
 * Deterministic order for a record set.
 *
 * Timestamp first so a genuine sequence replays in the order it happened, then
 * event id so two records at the same second do not fold differently on two
 * devices. Nothing about the *security* of the fold rests on this — revocation
 * is order-independent by construction — but a projection two clients disagree
 * about is its own defect.
 */
const inRecordOrder = (
  events: ReadonlyArray<CommunitySignedEvent>,
): ReadonlyArray<CommunitySignedEvent> =>
  [...events].sort(
    (left, right) =>
      left.created_at - right.created_at || left.id.localeCompare(right.id),
  );

/**
 * Fold a signed community record stream into a membership ledger.
 *
 * Pure and total: it never throws for a hostile record. Anything it cannot
 * admit lands in `refusals` with a typed code, because a membership record that
 * vanishes silently is indistinguishable from one that was never sent.
 */
export const foldCommunityLedgerFromEvents = (input: {
  readonly groupId: string;
  /** Group admin keys, admitted out of band. The relay does not supply these. */
  readonly adminPubkeys: ReadonlyArray<string>;
  readonly events: ReadonlyArray<CommunitySignedEvent>;
}): CommunityLedgerFold => {
  const groupId = input.groupId;
  const adminPubkeys = [
    ...new Set(input.adminPubkeys.map((key) => key.trim().toLowerCase())),
  ].filter((key) => HEX_64.test(key));
  const admins = new Set(adminPubkeys);

  let ledger = createEmptyLedger({ groupId });
  let metadata: CommunityGroupMetadata | null = null;
  const refusals: CommunityRecordRefusal[] = [];
  const appliedEventIds: string[] = [];
  const refuse = (
    event: CommunitySignedEvent,
    code: CommunityRecordRefusalCode,
    detail: string,
  ): void => {
    refusals.push({ eventId: event.id, kind: event.kind, code, detail });
  };

  for (const event of inRecordOrder(input.events)) {
    // Every membership-bearing record names its group. A record for another
    // group cannot move this one, even from a genuine admin key.
    if (
      event.kind === NIP_29_PUT_USER_KIND ||
      event.kind === NIP_29_REMOVE_USER_KIND ||
      event.kind === NIP_29_GROUP_METADATA_KIND
    ) {
      const eventGroupId =
        event.kind === NIP_29_GROUP_METADATA_KIND
          ? (tagValue(event, "d") ?? tagValue(event, "h"))
          : tagValue(event, "h");
      if (eventGroupId !== groupId) {
        refuse(event, "wrong_group", `record names group ${eventGroupId ?? "none"}`);
        continue;
      }
    }

    if (event.kind === NIP_29_GROUP_METADATA_KIND) {
      metadata = {
        groupId,
        name: tagValue(event, "name") ?? null,
        about: tagValue(event, "about") ?? null,
        sourceEventId: event.id,
      };
      appliedEventIds.push(event.id);
      continue;
    }

    if (event.kind === NIP_29_PUT_USER_KIND || event.kind === NIP_29_REMOVE_USER_KIND) {
      // Admission authority is the out-of-band admin set, never the relay and
      // never the event's own claim about itself.
      if (!admins.has(event.pubkey)) {
        refuse(event, "not_group_admin", `author ${event.pubkey.slice(0, 12)} is not an admitted group admin`);
        continue;
      }
      const subjects = tagValues(event, "p").filter((key) => HEX_64.test(key));
      if (subjects.length === 0) {
        refuse(event, "missing_subject", "admin record names no subject pubkey");
        continue;
      }
      const at = new Date(event.created_at * 1_000).toISOString();

      if (event.kind === NIP_29_REMOVE_USER_KIND) {
        // Revocation is deliberately blunt and order-independent. The subject
        // may be a human operator, an agent key, or a key this fold has not
        // seen yet; all three burn. `burnAgentKey` does not require a live
        // binding, which is the whole point — a revocation that only lands
        // when the thing it revokes is already present is not a revocation.
        for (const subject of subjects) {
          if (isMemberActive(ledger, subject)) {
            const revoked = revokeMember(ledger, {
              operatorPubkey: subject,
              reason: "nip29_remove_user",
              revokedAt: at,
            });
            ledger = revoked.ledger;
            // Revoking the member revokes their agents; burn each key so a
            // re-invitation cannot restore the fleet they held before.
            for (const agent of revoked.member.agents) {
              ledger = burnAgentKey(ledger, {
                agentPubkey: agent.agentPubkey,
                reason: "member_revoked",
                revokedAt: at,
              }).ledger;
            }
          }
          ledger = burnAgentKey(ledger, {
            agentPubkey: subject,
            reason: "nip29_remove_user",
            revokedAt: at,
          }).ledger;
        }
        appliedEventIds.push(event.id);
        continue;
      }

      // put-user. The invitation-only gate still applies: admission is modelled
      // as the admin issuing an invitation and it being accepted in the same
      // record, so `acceptInvitation` runs its own checks rather than this
      // module writing a member row behind its back.
      let admittedAny = false;
      for (const subject of subjects) {
        if (isMemberActive(ledger, subject)) {
          refuse(event, "already_applied", `${subject.slice(0, 12)} is already an active member`);
          continue;
        }
        const invitationId = `inv.${event.id}.${subject}`;
        try {
          ledger = issueInvitation(ledger, {
            invitationId,
            inviterPubkey: event.pubkey,
            inviteePubkey: subject,
            createdAt: at,
          }).ledger;
          ledger = acceptInvitation(ledger, {
            invitationId,
            inviteePubkey: subject,
            acceptedAt: at,
          }).ledger;
          admittedAny = true;
        } catch (error) {
          refuse(
            event,
            "unreadable",
            error instanceof Error ? error.message : "admission refused",
          );
        }
      }
      if (admittedAny) appliedEventIds.push(event.id);
      continue;
    }

    if (event.kind === NIP_AP_PERSONA_KIND) {
      // The agent signs its own persona. The NIP-OA `auth` tag inside it is
      // what binds the agent key to a human operator, and it is verified from
      // its own signature — an admin cannot mint this binding and neither can
      // the relay.
      let operatorPubkey: string;
      try {
        const ownerAuthTag = extractAuthTagFromAuthEvent(event.tags);
        operatorPubkey = verifyAgentOwnerAttestation({
          agentPubkey: event.pubkey,
          ownerAuthTag,
        }).operatorPubkey;
      } catch (error) {
        const code = (error as { code?: string }).code;
        refuse(
          event,
          code === "agent_self_attestation" ? "self_attestation" : "attestation_invalid",
          error instanceof Error ? error.message : "attestation refused",
        );
        continue;
      }

      // Checked before the attach so the refusal reads as what it is, rather
      // than as a generic admission failure. `attachAgent` refuses it too — the
      // burn is enforced there, not here.
      if (isAgentKeyBurned(ledger, event.pubkey)) {
        refuse(event, "agent_key_burned", "revocation burned this agent key; a replacement key is required");
        continue;
      }
      if (!isMemberActive(ledger, operatorPubkey)) {
        refuse(event, "operator_not_member", "attested operator is not an active community member");
        continue;
      }

      const dTag = tagValue(event, "d");
      const declaredCapabilities = event.tags.flatMap((tag) =>
        tag[0] === "capability" && tag[1] !== undefined
          ? [{ capabilityRef: tag[1], label: tag[2] ?? tag[1] }]
          : [],
      );
      try {
        ledger = attachAgent(ledger, {
          operatorPubkey,
          agentPubkey: event.pubkey,
          ownerAuthTag: [...extractAuthTagFromAuthEvent(event.tags)],
          attachedAt: new Date(event.created_at * 1_000).toISOString(),
          ...(dTag === undefined
            ? {}
            : {
                persona: {
                  kind: NIP_AP_PERSONA_KIND,
                  dTag,
                  ...(tagValue(event, "name") === undefined
                    ? {}
                    : { displayName: tagValue(event, "name") }),
                  declaredCapabilities: declaredCapabilities.slice(0, 32),
                },
              }),
        }).ledger;
        appliedEventIds.push(event.id);
      } catch (error) {
        const code = (error as { code?: string }).code;
        refuse(
          event,
          code === "agent_revoked"
            ? "agent_key_burned"
            : code === "agent_already_bound"
              ? "already_applied"
              : "persona_invalid",
          error instanceof Error ? error.message : "agent attach refused",
        );
      }
      continue;
    }

    // Chat and the relay-signed lists carry no membership authority here. The
    // transcript is projected separately; the lists corroborate at most.
    if (
      event.kind !== NIP_29_GROUP_CHAT_KIND &&
      event.kind !== NIP_29_GROUP_ADMINS_KIND &&
      event.kind !== NIP_29_GROUP_MEMBERS_KIND
    ) {
      refuse(event, "unreadable", `kind ${event.kind} carries no membership meaning`);
    }
  }

  const members = [...ledger.members.values()];
  return {
    ledger,
    groupId,
    metadata,
    adminPubkeys,
    members,
    agents: members.flatMap((member) => [...member.agents]),
    burnedAgentKeys: [...ledger.burnedAgentKeys].sort(),
    refusals,
    appliedEventIds,
  };
};

/**
 * The role a pubkey holds in the community, derived from the folded record —
 * never assumed, and never taken from a client-side default.
 *
 * `verifier` is deliberately not derivable from membership alone: verifying is
 * something a member is permitted to do for a *specific* result whose producer
 * has a different operator, so it belongs to a work-unit projection, not to a
 * standing role. Returning `verifier` here would render a control that the
 * independence rule may still refuse.
 */
export type CommunityDerivedRole =
  | "owner"
  | "member"
  | "agent_operator"
  | "read_only"
  | "none";

export interface CommunityRoleState {
  readonly role: CommunityDerivedRole;
  readonly status: "active" | "revoked" | "unknown";
  readonly operatorPubkey: string | null;
  readonly isGroupAdmin: boolean;
  readonly admittedAgentPubkeys: ReadonlyArray<string>;
}

/**
 * Resolve what one pubkey may do, from signed records only.
 *
 * A group admin is reported as `owner` because the admin set is the same
 * out-of-band authority the room is bootstrapped from. A member with at least
 * one admitted agent key is an `agent_operator`; a member without one is a
 * plain `member`; a revoked member is `read_only` and explicitly `revoked`, so
 * a caller can tell "never joined" from "was removed".
 */
export const communityRoleFor = (
  fold: CommunityLedgerFold,
  pubkey: string,
): CommunityRoleState => {
  const key = pubkey.trim().toLowerCase();
  const isGroupAdmin = fold.adminPubkeys.includes(key);
  const member = fold.ledger.members.get(key);
  const admittedAgentPubkeys =
    member === undefined
      ? []
      : member.agents
          .filter((agent) => isAgentAdmitted(fold.ledger, agent.agentPubkey))
          .map((agent) => agent.agentPubkey);

  if (isGroupAdmin) {
    return {
      role: "owner",
      status: "active",
      operatorPubkey: key,
      isGroupAdmin: true,
      admittedAgentPubkeys,
    };
  }
  if (member === undefined) {
    return {
      role: "none",
      status: "unknown",
      operatorPubkey: null,
      isGroupAdmin: false,
      admittedAgentPubkeys: [],
    };
  }
  if (member.status === "revoked") {
    return {
      role: "read_only",
      status: "revoked",
      operatorPubkey: key,
      isGroupAdmin: false,
      admittedAgentPubkeys: [],
    };
  }
  if (member.status !== "active") {
    return {
      role: "read_only",
      status: "unknown",
      operatorPubkey: key,
      isGroupAdmin: false,
      admittedAgentPubkeys: [],
    };
  }
  return {
    role: admittedAgentPubkeys.length > 0 ? "agent_operator" : "member",
    status: "active",
    operatorPubkey: key,
    isGroupAdmin: false,
    admittedAgentPubkeys,
  };
};

/**
 * Resolve the operator behind an agent key, for the independence rule.
 *
 * Producer and verifier must have distinct *operators*, not merely distinct
 * keys, and one operator can hold many agent keys. Answering that question
 * needs the folded binding — comparing the two agent pubkeys does not.
 */
export const communityOperatorForAgent = (
  fold: CommunityLedgerFold,
  agentPubkey: string,
): string | null => fold.ledger.agentIndex.get(agentPubkey.trim().toLowerCase()) ?? null;

/**
 * The two-room rule as a check (contract §5.2 rule 1).
 *
 * A community group id must never equal a private conversation tag value. The
 * two rooms are event-sourced over the same relay by the same device, so an
 * equal identifier is not a cosmetic collision: a community subscription would
 * match private records and a private subscription would match community ones.
 */
export class CommunityRoomBoundaryError extends Error {}

export const assertCommunityGroupIdIsNotPrivateConversation = (input: {
  readonly groupId: string;
  readonly privateConversationRefs: ReadonlyArray<string>;
}): void => {
  const groupId = input.groupId.trim();
  for (const conversation of input.privateConversationRefs) {
    if (conversation.trim() === groupId && groupId !== "") {
      throw new CommunityRoomBoundaryError(
        "community: group id must not equal an owner-private conversation ref (two-room rule)",
      );
    }
  }
};
