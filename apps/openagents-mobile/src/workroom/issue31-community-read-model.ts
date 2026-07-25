/**
 * The community room, projected from signed community records (omega#48).
 *
 * Before this existed the room was a selector and three static cards: it never
 * opened a community subscription, derived no role, and rendered no community
 * content. Every capability row was hard-coded `read_only`, so `verifier` was
 * unreachable and "unauthorized controls do not render" held only because no
 * control existed.
 *
 * This module is the read side of that room. Three properties shape it:
 *
 * 1. **Roles come from records.** `communityRoleFor` folds NIP-29 admin records
 *    and NIP-OA attestations; nothing here assumes a role or trusts a relay to
 *    assert one. A control is offered only when the folded role, the grant, the
 *    expiration, and the current lifecycle state all permit it.
 * 2. **Member text is data.** Every member-written string on every row is
 *    carried as `UntrustedCommunityContent`, quoted at projection time. The
 *    plain string is kept beside it under a name that says what it is for
 *    (`displayText`), so a caller reaching for Sarah cannot pick it up by
 *    accident, and `buildCommunitySarahContext` refuses anything unbranded.
 * 3. **The relay decides nothing.** A quote is not an assignment, a result is
 *    not an acceptance, and an acceptance is not a settlement. Each transition
 *    needs its own signed record from the key the contract names for it.
 */
import {
  COMMUNITY_ARBITRATION_FEEDBACK_KIND,
  ARBITRATION_REASON_CLASSES,
  INDEPENDENT_VERIFICATION_FEEDBACK_TYPE,
  admitIndependentVerification,
  type ArbitrationOutcome,
  type ArbitrationReasonClass,
  type CommunityBindingResolver,
  type IndependentVerificationVerdict,
} from "@openagentsinc/sarah/community-arbitration";
import {
  NIP_29_GROUP_CHAT_KIND,
  communityOperatorForAgent,
  communityRoleFor,
  foldCommunityLedgerFromEvents,
  quoteUntrustedCommunityContent,
  type CommunityAgentBinding,
  type CommunityLedgerFold,
  type CommunityRecordRefusal,
  type CommunitySignedEvent,
  type UntrustedCommunityContent,
} from "@openagentsinc/sarah/community";
import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  decodeSarahLbrQuoteEvent,
  decodeSarahLbrWorkRequestEvent,
  type SarahLbrQuote,
  type SarahLbrWorkRequest,
} from "@openagentsinc/sarah/lbr-request-quote";
import {
  XP_AWARD_KIND,
  XP_BADGE_AWARD_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_BADGE_IDS,
  XP_RANK_KIND,
  earnedBadgeIds,
  parseXpAwardEvent,
  parseXpRankEvent,
  projectRank,
  rankAgreesWithAwards,
  type XpAwardRecord,
} from "@openagentsinc/sarah/xp";

import type { Issue31ConfirmedEvent, Issue31NostrClientSnapshot } from "./issue31-nostr-client.ts";

/** v1 pays nothing. This string is the room's standing statement of that. */
export const ISSUE31_COMMUNITY_EXPERIENCE_ONLY_COPY =
  "This room awards experience points only. v1 pays no money." as const;

/**
 * What somebody who has never joined is told.
 *
 * The room is invitation-only, so the honest first-run state is not an empty
 * transcript with a join button: it is an explanation of why there is nothing
 * to see and what would change that. It states the experience-only rule up
 * front rather than after somebody has done work, because the exit that says
 * "the room, invitation, and first-run copy say that v1 awards experience only"
 * is about what a person is told *before* they decide to take part.
 */
export const ISSUE31_COMMUNITY_FIRST_RUN_COPY =
  "This community room is invitation-only. A group admin admits your public key with a signed record before you can read the transcript or take any action here. Nothing about joining is automatic and no relay can admit you. Work completed here is recognised with experience points only. v1 pays no money." as const;

/**
 * What an admin is told before they admit somebody.
 *
 * The person doing the inviting is the person who has to be accurate about what
 * they are offering, so the promise is stated at the moment of the invitation
 * rather than only inside the room the invitee has not seen yet.
 */
export const ISSUE31_COMMUNITY_INVITATION_COPY =
  "Admitting a public key publishes a signed group record that lets that developer read this room and take part in community work units. Tell them what they are joining: contributions here are recognised with experience points only, and v1 pays no money." as const;

/** The room's own standing line, shown whether or not anybody has joined. */
export const ISSUE31_COMMUNITY_ROOM_COPY =
  "Community history and membership stay separate from the owner-private room. v1 awards experience and pays no money." as const;

/**
 * Every experience-only statement the room makes, for a copy audit.
 *
 * A test asserts against this list rather than against whichever string somebody
 * remembered to check, so a new surface that forgets the rule is a failing test
 * rather than an omission nobody notices.
 */
export const ISSUE31_COMMUNITY_EXPERIENCE_COPY_SURFACES = [
  ISSUE31_COMMUNITY_EXPERIENCE_ONLY_COPY,
  ISSUE31_COMMUNITY_FIRST_RUN_COPY,
  ISSUE31_COMMUNITY_INVITATION_COPY,
  ISSUE31_COMMUNITY_ROOM_COPY,
] as const;

export type Issue31CommunityRole =
  | "owner"
  | "member"
  | "agent_operator"
  | "verifier"
  | "read_only"
  | "none";

/**
 * Every control the community room can offer, and nothing else.
 *
 * A closed set rather than free-form strings: the view renders exactly what is
 * in `controls`, so a control that is not derivable here cannot be rendered by
 * a careless view change either.
 */
export const ISSUE31_COMMUNITY_CONTROL_KINDS = [
  "invite_member",
  "revoke_member",
  "attach_agent",
  "revoke_agent",
  "post_message",
  "quote_work_unit",
  "submit_result",
  "verify_result",
  "file_appeal",
] as const;
export type Issue31CommunityControlKind =
  (typeof ISSUE31_COMMUNITY_CONTROL_KINDS)[number];

/**
 * Which key signs this action — and therefore whether the phone can take it.
 *
 * This device holds the human operator's key. It does not hold any agent key:
 * the operator keeps compute, harness, provider accounts, credentials, and
 * agent home, and OpenAgents never receives them. So quoting, executing,
 * returning a result, and publishing a persona attestation are all signed on
 * the operator's own machine by the agent key, and the phone can only watch
 * them happen.
 *
 * Rendering those as buttons would be a lie about where authority lives, so
 * they are marked and rendered as state rather than as controls.
 */
export type Issue31CommunitySigner = "operator_device" | "agent_compute";

export interface Issue31CommunityLifecycleAuthority {
  readonly signer: Issue31CommunitySigner;
  /** Where the key that signs this action actually lives. */
  readonly signingKeyHome: "this_phone" | "operator_compute";
  readonly reason: string;
}

/**
 * Where each lifecycle action is signed — a stated boundary, not a shortfall.
 *
 * The community lifecycle has two halves, and they are split by which key can
 * honestly sign them rather than by what was convenient to build:
 *
 * - **The operator's own decisions** — joining a conversation, admitting or
 *   removing a key, appealing a ruling about work they are accountable for.
 *   These are the human's, signed by the human's key, which this phone holds.
 * - **An agent's own claims about itself and its work** — that it exists and is
 *   bound to an operator, that it will do a unit for a stated bound, that here
 *   is the result, that it checked a peer's result. These are assertions about
 *   execution that happened on the operator's compute, made by the key that did
 *   the executing.
 *
 * The phone deliberately holds no agent key. Putting one there would mean the
 * device could assert that work was done without having done it, which is the
 * one claim in this system that must stay attached to the machine that ran the
 * work. So the agent-signed half renders as observed state and never as a
 * button. This is a permanent design boundary of the mobile room, not a gap to
 * be closed later: every lifecycle action is completable by an authorized role,
 * and for four of them that role is the operator's agent rather than their
 * phone.
 */
export const ISSUE31_COMMUNITY_LIFECYCLE_AUTHORITY: Readonly<
  Record<Issue31CommunityControlKind, Issue31CommunityLifecycleAuthority>
> = {
  invite_member: {
    signer: "operator_device",
    signingKeyHome: "this_phone",
    reason: "Admitting a key is a group admin's own decision.",
  },
  revoke_member: {
    signer: "operator_device",
    signingKeyHome: "this_phone",
    reason: "Removing a key is a group admin's own decision.",
  },
  revoke_agent: {
    signer: "operator_device",
    signingKeyHome: "this_phone",
    reason:
      "Revoking an agent's access is an act against the agent, so it cannot need the agent's cooperation.",
  },
  post_message: {
    signer: "operator_device",
    signingKeyHome: "this_phone",
    reason: "A member speaks in the room as themselves.",
  },
  file_appeal: {
    signer: "operator_device",
    signingKeyHome: "this_phone",
    reason:
      "The operator is accountable for the work that was rejected, so the operator appeals it.",
  },
  attach_agent: {
    signer: "agent_compute",
    signingKeyHome: "operator_compute",
    reason:
      "An agent attests to its own existence and its binding to an operator with its own key.",
  },
  quote_work_unit: {
    signer: "agent_compute",
    signingKeyHome: "operator_compute",
    reason: "A quote is the agent committing its own compute to a bound.",
  },
  submit_result: {
    signer: "agent_compute",
    signingKeyHome: "operator_compute",
    reason:
      "A result is a claim that this agent did the work, and must be signed by the key that did it.",
  },
  verify_result: {
    signer: "agent_compute",
    signingKeyHome: "operator_compute",
    reason:
      "Verification is independent execution, so the verifying agent signs what it actually ran.",
  },
};

export interface Issue31CommunityControl {
  readonly kind: Issue31CommunityControlKind;
  /** Public-safe subject: a unit ref, an agent key, or the group. */
  readonly subjectRef: string;
  /** The signed fact that permits this control. Never a client-side default. */
  readonly permittedByRef: string;
  readonly idempotencyRef: string;
  readonly signedBy: Issue31CommunitySigner;
}

export interface Issue31CommunityMessageRow {
  readonly sourceEventId: string;
  readonly sourceCreatedAt: number;
  readonly authorPubkey: string;
  /** For rendering in the room. Never pass this to a model. */
  readonly displayText: string;
  /** The only form admitted into Sarah's context. */
  readonly untrusted: UntrustedCommunityContent;
  readonly authorRole: Issue31CommunityRole;
  readonly deepLink: string;
}

export interface Issue31CommunityMemberRow {
  readonly operatorPubkey: string;
  readonly status: "invited" | "active" | "revoked";
  readonly role: Issue31CommunityRole;
  readonly joinedAt: string | null;
  readonly revokedAt: string | null;
  readonly agentCount: number;
  readonly admittedAgentCount: number;
}

export interface Issue31CommunityAgentRow {
  readonly agentPubkey: string;
  readonly operatorPubkey: string;
  readonly status: "active" | "revoked";
  readonly capabilityGrant: "active" | "revoked";
  readonly attestedByOperator: boolean;
  readonly personaDTag: string | null;
  readonly personaDisplayName: string | null;
  readonly declaredCapabilities: ReadonlyArray<string>;
  readonly attachedAt: string;
  readonly revokedAt: string | null;
  readonly burned: boolean;
}

export interface Issue31CommunityQuoteRow {
  readonly sourceEventId: string;
  readonly quoteRef: string;
  readonly providerPubkey: string;
  readonly providerOperatorPubkey: string | null;
  readonly providerRef: string;
  readonly accepted: boolean;
  /** Provider-written, so quoted before anything can read it as instruction. */
  readonly untrustedProviderRef: UntrustedCommunityContent;
}

export interface Issue31CommunityResultRow {
  readonly sourceEventId: string;
  readonly providerPubkey: string;
  readonly providerOperatorPubkey: string | null;
  readonly createdAt: number;
  readonly untrustedSummary: UntrustedCommunityContent;
  readonly displaySummary: string;
}

export interface Issue31CommunityVerificationRow {
  readonly sourceEventId: string;
  readonly verifierPubkey: string;
  readonly verifierOperatorPubkey: string | null;
  readonly producerOperatorPubkey: string | null;
  /**
   * Distinct *operators*, not merely distinct keys. One operator holding two
   * agent keys is self-dealing, and comparing the keys would not see it.
   */
  readonly operatorsAreIndependent: boolean;
  /**
   * Why the independence claim was not admitted.
   *
   * `verifier_binding_unconfirmed` means the record does not support the
   * verifier's own claim about who operates it — an unbound key, a burned key,
   * or an agent the fold binds to a different operator.
   *
   * `verification_event_absent` means a decision claimed independence and the
   * verifier never signed anything. Before contract amendment `SARAH-CW-00-A1`
   * that was the only state there was, and it rendered as verified.
   */
  readonly refusalReason:
    | "self_dealing_operators"
    | "unknown_operator"
    | "verifier_binding_unconfirmed"
    | "verification_event_absent"
    | null;
  /**
   * What the verifier reported running. `null` only when no verification event
   * exists, which is also the `verification_event_absent` case.
   */
  readonly verdict: IndependentVerificationVerdict | null;
  /**
   * True when this row came from an event the verifier signed, rather than
   * from a decision asserting on the verifier's behalf.
   */
  readonly verifierSigned: boolean;
}

export interface Issue31CommunityDecisionRow {
  readonly sourceEventId: string;
  readonly outcome: ArbitrationOutcome;
  readonly reasonClass: ArbitrationReasonClass | null;
  readonly displayReasonSummary: string | null;
  readonly authorityReceiptRef: string | null;
  readonly decidedByPubkey: string;
  /** Where a rejected result goes next. A rejection is never a dead end. */
  readonly appealDestination: string | null;
}

export interface Issue31CommunityAppealRow {
  readonly sourceEventId: string;
  readonly appealRef: string;
  readonly appellantPubkey: string;
  readonly grounds: string;
  readonly untrustedGroundsSummary: UntrustedCommunityContent;
  readonly displayGroundsSummary: string;
}

export interface Issue31CommunityRulingRow {
  readonly sourceEventId: string;
  readonly rulingRef: string;
  readonly ownerAppealPubkey: string;
  readonly outcome: string;
  readonly authoredByAdmittedOwnerKey: boolean;
}

export type Issue31CommunityUnitLifecycle =
  | "open"
  | "quoted"
  | "accepted"
  | "delivered"
  | "verified"
  | "decided"
  | "disputed"
  | "ruled"
  | "expired";

export interface Issue31CommunityWorkUnitRow {
  readonly requestEventId: string;
  readonly unitRef: string;
  readonly grantRef: string;
  readonly idempotencyRef: string;
  readonly targetRefs: ReadonlyArray<string>;
  readonly allowedActionRefs: ReadonlyArray<string>;
  /** v1 budget is an experience tier. `budgetMsats` is always zero here. */
  readonly experienceTierCopy: string;
  readonly expiresAtUnix: number;
  readonly expired: boolean;
  readonly lifecycle: Issue31CommunityUnitLifecycle;
  readonly requesterPubkey: string;
  readonly quotes: ReadonlyArray<Issue31CommunityQuoteRow>;
  readonly acceptedProviderPubkey: string | null;
  readonly result: Issue31CommunityResultRow | null;
  readonly verification: Issue31CommunityVerificationRow | null;
  readonly decision: Issue31CommunityDecisionRow | null;
  readonly appeal: Issue31CommunityAppealRow | null;
  readonly ruling: Issue31CommunityRulingRow | null;
  readonly controls: ReadonlyArray<Issue31CommunityControl>;
  readonly deepLink: string;
}

export interface Issue31CommunityBadgeRow {
  readonly badgeId: string;
  /** Where this badge is attested. */
  readonly source: "awards_and_wire" | "awards_only" | "wire_only";
  /** The kind-8 award event, when a publisher signed one. */
  readonly awardEventId: string | null;
  /** The kind-30009 definition's issuer, when the definition was retrieved. */
  readonly issuerPubkey: string | null;
  /** From the retrieved definition. Publisher-authored, so public-safe copy. */
  readonly name: string | null;
  /**
   * True when the award stream alone supports this badge. `false` with a
   * present `awardEventId` is the case a reader must not mistake for earned
   * work.
   */
  readonly supportedByAwards: boolean;
}

export interface Issue31CommunityExperienceModel {
  /** Recomputed from the award stream alone. Never read off a rank event. */
  readonly recomputedTotalPoints: number;
  readonly recomputedLevel: number;
  readonly recomputedLevelId: string;
  readonly awardCount: number;
  /** What a scorer published, when one did. */
  readonly publishedRankPoints: number | null;
  /** Awards win. True when the published rank disagreed and was discarded. */
  readonly publishedRankDisagreed: boolean;
  /**
   * Badges the award stream supports. Unchanged: awards are the authority and
   * awards win, so this stays the derived set.
   */
  readonly badgeIds: ReadonlyArray<string>;
  /**
   * NIP-58 badges as they actually are — read off the wire and reconciled with
   * the award stream, rather than only derived from it.
   *
   * A badge is `awards_and_wire` when both agree, `awards_only` when the award
   * stream supports it and the publisher has not published the kind-8 award
   * yet, and `wire_only` when a badge publisher awarded something the award
   * stream does not support. The last one is shown and labelled rather than
   * folded into the total, because §9.2 rule 5 says awards win — a badge must
   * not become a second scoring authority by arriving on a different kind.
   */
  readonly badges: ReadonlyArray<Issue31CommunityBadgeRow>;
  readonly awards: ReadonlyArray<
    Readonly<{
      sourceEventId: string | null;
      awardKind: string;
      points: number;
      workEventId: string;
      receiptRef: string;
      deepLink: string;
    }>
  >;
}

export interface Issue31CommunityReadModel {
  readonly status: "ready" | "unavailable" | "gap";
  readonly reasonRef: string | null;
  readonly groupId: string | null;
  readonly groupName: string | null;
  readonly viewerPubkey: string | null;
  readonly viewerRole: Issue31CommunityRole;
  readonly viewerRoleStatus: "active" | "revoked" | "unknown";
  readonly experienceOnlyCopy: typeof ISSUE31_COMMUNITY_EXPERIENCE_ONLY_COPY;
  readonly transcript: ReadonlyArray<Issue31CommunityMessageRow>;
  readonly roster: ReadonlyArray<Issue31CommunityMemberRow>;
  readonly agents: ReadonlyArray<Issue31CommunityAgentRow>;
  readonly workUnits: ReadonlyArray<Issue31CommunityWorkUnitRow>;
  readonly experience: Issue31CommunityExperienceModel;
  /** Room-level controls. Unit-level controls hang off each unit row. */
  readonly controls: ReadonlyArray<Issue31CommunityControl>;
  readonly refusals: ReadonlyArray<CommunityRecordRefusal>;
  readonly rejectedRecordCount: number;
}

export interface Issue31CommunityProjectionConfig {
  readonly groupId: string | null;
  /** Group admin keys, admitted out of band. Never read off the relay. */
  readonly adminPubkeys: ReadonlyArray<string>;
  /** Keys permitted to publish XP awards and rank. */
  readonly scorerPubkeys: ReadonlyArray<string>;
  /** The admitted owner appeal identity, when registered. */
  readonly ownerAppealPubkey: string | null;
  readonly viewerPubkey: string | null;
  readonly nowUnixSeconds: number;
  readonly transcriptLimit?: number;
}

const COMMUNITY_TRANSCRIPT_PAGE_SIZE = 60;
const HEX_64 = /^[0-9a-f]{64}$/;

const deepLinkFor = (sourceEventId: string): string =>
  `openagents://omega/workroom?room=community&sourceEventId=${sourceEventId}`;

const tagValue = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): string | undefined => tags.find((tag) => tag[0] === name)?.[1];

const taggedEventId = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  marker: string,
): string | null => {
  const row = tags.find((tag) => tag[0] === "e" && tag[3] === marker);
  const id = row?.[1];
  return id !== undefined && HEX_64.test(id) ? id : null;
};

export const emptyIssue31CommunityReadModel = (
  reasonRef = "reason.issue31.community.not_configured",
): Issue31CommunityReadModel => ({
  status: "unavailable",
  reasonRef,
  groupId: null,
  groupName: null,
  viewerPubkey: null,
  viewerRole: "none",
  viewerRoleStatus: "unknown",
  experienceOnlyCopy: ISSUE31_COMMUNITY_EXPERIENCE_ONLY_COPY,
  transcript: [],
  roster: [],
  agents: [],
  workUnits: [],
  experience: {
    recomputedTotalPoints: 0,
    recomputedLevel: 0,
    recomputedLevelId: "",
    awardCount: 0,
    publishedRankPoints: null,
    publishedRankDisagreed: false,
    badgeIds: [],
    badges: [],
    awards: [],
  },
  controls: [],
  refusals: [],
  rejectedRecordCount: 0,
});

/**
 * Quote a member string, or fall back to a fixed marker when it cannot be
 * quoted at all.
 *
 * `quoteUntrustedCommunityContent` throws only for structurally impossible
 * input — a malformed author key or an over-long body. Hostile *content* is
 * never an error, because refusing to show a member's message is a denial of
 * service the attacker chooses. So the fallback keeps the row renderable while
 * still handing Sarah something branded and inert.
 */
const quoteOrMarker = (input: {
  readonly content: string;
  readonly authorPubkey: string;
  readonly origin: Parameters<typeof quoteUntrustedCommunityContent>[0]["origin"];
}): UntrustedCommunityContent => {
  try {
    return quoteUntrustedCommunityContent(input);
  } catch {
    return quoteUntrustedCommunityContent({
      content: "[community content could not be quoted for this context]",
      authorPubkey: HEX_64.test(input.authorPubkey) ? input.authorPubkey : "0".repeat(64),
      origin: input.origin,
    });
  }
};

const communityEventsFrom = (
  snapshot: Issue31NostrClientSnapshot,
): ReadonlyArray<Issue31ConfirmedEvent> =>
  snapshot.confirmedEvents.filter((event) => event.room === "community");

const asSignedRecord = (event: Issue31ConfirmedEvent): CommunitySignedEvent => ({
  id: event.event.id,
  pubkey: event.event.pubkey,
  created_at: event.event.created_at,
  kind: event.event.kind,
  tags: event.event.tags,
  content: event.event.content,
});

const roleLabel = (
  fold: CommunityLedgerFold,
  pubkey: string,
): Issue31CommunityRole => communityRoleFor(fold, pubkey).role;

const agentRows = (
  fold: CommunityLedgerFold,
): ReadonlyArray<Issue31CommunityAgentRow> =>
  fold.agents.map((agent: CommunityAgentBinding) => ({
    agentPubkey: agent.agentPubkey,
    operatorPubkey: agent.operatorPubkey,
    status: agent.status,
    capabilityGrant: agent.capabilityGrant,
    // The binding only exists because the NIP-OA tag verified during the fold.
    attestedByOperator: true,
    personaDTag: agent.persona?.dTag ?? null,
    personaDisplayName: agent.persona?.displayName ?? null,
    declaredCapabilities:
      agent.persona?.declaredCapabilities.map((capability) => capability.label) ?? [],
    attachedAt: agent.attachedAt,
    revokedAt: agent.revokedAt ?? null,
    burned: fold.burnedAgentKeys.includes(agent.agentPubkey),
  }));

const rosterRows = (
  fold: CommunityLedgerFold,
): ReadonlyArray<Issue31CommunityMemberRow> =>
  fold.members
    .map((member) => {
      const role = communityRoleFor(fold, member.operatorPubkey);
      return {
        operatorPubkey: member.operatorPubkey,
        status: member.status,
        role: role.role,
        joinedAt: member.joinedAt ?? null,
        revokedAt: member.revokedAt ?? null,
        agentCount: member.agents.length,
        admittedAgentCount: role.admittedAgentPubkeys.length,
      };
    })
    .sort((left, right) => left.operatorPubkey.localeCompare(right.operatorPubkey));

const transcriptRows = (
  events: ReadonlyArray<Issue31ConfirmedEvent>,
  fold: CommunityLedgerFold,
  groupId: string,
  limit: number,
): ReadonlyArray<Issue31CommunityMessageRow> =>
  events
    .filter(
      (event) =>
        event.event.kind === NIP_29_GROUP_CHAT_KIND &&
        event.event.tags.some((tag) => tag[0] === "h" && tag[1] === groupId),
    )
    .sort(
      (left, right) =>
        left.event.created_at - right.event.created_at ||
        left.event.id.localeCompare(right.event.id),
    )
    .slice(-limit)
    .map((event) => ({
      sourceEventId: event.event.id,
      sourceCreatedAt: event.event.created_at,
      authorPubkey: event.event.pubkey,
      displayText: event.event.content.slice(0, 4_096),
      untrusted: quoteOrMarker({
        content: event.event.content === "" ? " " : event.event.content.slice(0, 4_096),
        authorPubkey: event.event.pubkey,
        origin: "room_message",
      }),
      authorRole: roleLabel(fold, event.event.pubkey),
      deepLink: deepLinkFor(event.event.id),
    }));

const experienceTierCopy = (request: SarahLbrWorkRequest): string => {
  // v1 budgets are experience tiers. `budgetMsats` exists in the shared grant
  // shape for a later paid version and is zero here; saying "0 msats" in the
  // room would read as a price rather than as the absence of one.
  const actions = request.workUnit.allowedActionRefs.length;
  return `Experience tier work · ${actions} permitted ${actions === 1 ? "action" : "actions"} · no payment in v1`;
};

interface ArbitrationFacts {
  readonly decisionsByResultEventId: ReadonlyMap<string, Issue31CommunityDecisionRow>;
  readonly acceptedQuoteRefsByRequestId: ReadonlyMap<string, ReadonlySet<string>>;
  readonly appealsByDecisionEventId: ReadonlyMap<string, Issue31CommunityAppealRow>;
  readonly rulingsByAppealEventId: ReadonlyMap<string, Issue31CommunityRulingRow>;
  readonly verificationsByResultEventId: ReadonlyMap<string, Issue31CommunityVerificationRow>;
}

/**
 * Read the kind-7000 feedback lane.
 *
 * Kind 7000 carries four different things — quote acceptance, arbitration
 * decision, dispute appeal, and owner ruling — separated by `cw_feedback_type`.
 * Each is admitted only from the key the contract names as its authority:
 * acceptance and decisions from the requester (Sarah), rulings from the
 * admitted owner appeal key. A member cannot accept their own quote by
 * publishing the acceptance themselves.
 */
const readArbitrationLane = (
  events: ReadonlyArray<Issue31ConfirmedEvent>,
  fold: CommunityLedgerFold,
  config: Issue31CommunityProjectionConfig,
  requesterByRequestId: ReadonlyMap<string, string>,
): ArbitrationFacts => {
  const decisionsByResultEventId = new Map<string, Issue31CommunityDecisionRow>();
  const acceptedQuoteRefsByRequestId = new Map<string, Set<string>>();
  const appealsByDecisionEventId = new Map<string, Issue31CommunityAppealRow>();
  const rulingsByAppealEventId = new Map<string, Issue31CommunityRulingRow>();
  const verificationsByResultEventId = new Map<string, Issue31CommunityVerificationRow>();
  const refusedVerificationsByResultEventId = new Map<
    string,
    Issue31CommunityVerificationRow
  >();
  const independenceClaimsByResultEventId = new Map<
    string,
    {
      readonly decisionEventId: string;
      readonly verifierAgentPubkey: string;
      readonly verifierOperatorRef: string;
      readonly producerOperatorRef: string | null;
    }
  >();

  /**
   * The folded record, exposed as the two questions the verification law asks.
   *
   * A resolver rather than the ledger itself, so the admission code cannot be
   * talked into reading anything else: an event can supply tags, and tags are
   * not what answers either of these.
   */
  const binding: CommunityBindingResolver = {
    operatorForAgent: (agentPubkey) => communityOperatorForAgent(fold, agentPubkey),
    isAgentKeyBurned: (agentPubkey) =>
      fold.burnedAgentKeys.includes(agentPubkey.trim().toLowerCase()),
  };

  for (const confirmed of events) {
    const event = confirmed.event;
    if (event.kind !== COMMUNITY_ARBITRATION_FEEDBACK_KIND && event.kind !== LBR_FEEDBACK_KIND) {
      continue;
    }
    const feedbackType =
      tagValue(event.tags, "cw_feedback_type") ?? tagValue(event.tags, "lbr_feedback_type");

    if (feedbackType === "quote_acceptance" || tagValue(event.tags, "status") === "accepted_quote") {
      const requestEventId = taggedEventId(event.tags, "request") ?? tagValue(event.tags, "e") ?? null;
      const quoteRef = tagValue(event.tags, "cw_quote_ref");
      if (requestEventId === null || quoteRef === undefined) continue;
      // Exactly one authority accepts a quote: the key that requested the work.
      if (requesterByRequestId.get(requestEventId) !== event.pubkey) continue;
      const existing = acceptedQuoteRefsByRequestId.get(requestEventId) ?? new Set<string>();
      existing.add(quoteRef);
      acceptedQuoteRefsByRequestId.set(requestEventId, existing);
      continue;
    }

    if (feedbackType === "arbitration_decision") {
      const resultEventId = taggedEventId(event.tags, "result");
      const requestEventId = taggedEventId(event.tags, "request");
      const outcome = tagValue(event.tags, "status");
      if (resultEventId === null || (outcome !== "accepted" && outcome !== "rejected")) continue;
      // Sarah decides acceptance. A provider cannot accept their own result.
      if (requestEventId !== null && requesterByRequestId.get(requestEventId) !== event.pubkey) {
        continue;
      }
      const rawReasonClass = tagValue(event.tags, "cw_reason_class");
      const reasonClass =
        rawReasonClass !== undefined &&
        (ARBITRATION_REASON_CLASSES as ReadonlyArray<string>).includes(rawReasonClass)
          ? (rawReasonClass as ArbitrationReasonClass)
          : null;
      const producerOperatorRef = tagValue(event.tags, "cw_producer_operator_ref") ?? null;
      const verifierOperatorRef = tagValue(event.tags, "cw_verifier_operator_ref") ?? null;
      const verifierAgentPubkey = tagValue(event.tags, "cw_verifier_agent_pubkey") ?? null;

      decisionsByResultEventId.set(resultEventId, {
        sourceEventId: event.id,
        outcome,
        reasonClass,
        displayReasonSummary: tagValue(event.tags, "cw_reason_summary") ?? null,
        authorityReceiptRef: tagValue(event.tags, "cw_authority_receipt_ref") ?? null,
        decidedByPubkey: event.pubkey,
        // A rejection always names where it goes next.
        appealDestination:
          outcome === "rejected"
            ? config.ownerAppealPubkey === null
              ? "needs_owner.owner_appeal_npub"
              : `openagents://omega/workroom?room=community&appealTo=${config.ownerAppealPubkey}`
            : null,
      });

      if (verifierAgentPubkey !== null && verifierOperatorRef !== null) {
        // The decision names who verified. Since contract amendment
        // `SARAH-CW-00-A1` that is a *claim*, checked against an event the
        // named verifier signed. It is kept here and resolved after the whole
        // lane has been read, because the verification event may arrive in any
        // order relative to the decision citing it.
        independenceClaimsByResultEventId.set(resultEventId, {
          decisionEventId: event.id,
          verifierAgentPubkey,
          verifierOperatorRef,
          producerOperatorRef,
        });
      }
      continue;
    }

    if (feedbackType === INDEPENDENT_VERIFICATION_FEEDBACK_TYPE) {
      // Contract §8.4. The verifying agent signs this itself; nothing else in
      // the lane can produce an admitted verification. Both operators come out
      // of the fold, never off the event's own tags.
      const admission = admitIndependentVerification(event, binding);
      if (!admission.admitted) {
        if (admission.resultEventId !== null) {
          refusedVerificationsByResultEventId.set(admission.resultEventId, {
            sourceEventId: admission.sourceEventId,
            verifierPubkey: admission.verifierAgentPubkey,
            verifierOperatorPubkey: null,
            producerOperatorPubkey: null,
            operatorsAreIndependent: false,
            refusalReason:
              admission.code === "self_dealing_operators"
                ? "self_dealing_operators"
                : admission.code === "verifier_key_burned" ||
                    admission.code === "verifier_binding_unconfirmed" ||
                    admission.code === "verifier_not_author"
                  ? "verifier_binding_unconfirmed"
                  : "unknown_operator",
            verdict: null,
            verifierSigned: true,
          });
        }
        continue;
      }
      verificationsByResultEventId.set(admission.resultEventId, {
        sourceEventId: admission.sourceEventId,
        verifierPubkey: admission.verifierAgentPubkey,
        verifierOperatorPubkey: admission.verifierOperatorPubkey,
        producerOperatorPubkey: admission.producerOperatorPubkey,
        operatorsAreIndependent: true,
        refusalReason: null,
        verdict: admission.verdict,
        verifierSigned: true,
      });
      continue;
    }

    if (feedbackType === "dispute_appeal") {
      const decisionEventId = taggedEventId(event.tags, "decision");
      const appealRef = tagValue(event.tags, "cw_appeal_ref");
      if (decisionEventId === null || appealRef === undefined) continue;
      const groundsSummary = tagValue(event.tags, "cw_grounds_summary") ?? "";
      appealsByDecisionEventId.set(decisionEventId, {
        sourceEventId: event.id,
        appealRef,
        appellantPubkey: event.pubkey,
        grounds: tagValue(event.tags, "cw_grounds") ?? "process_error",
        untrustedGroundsSummary: quoteOrMarker({
          content: groundsSummary === "" ? " " : groundsSummary,
          authorPubkey: event.pubkey,
          origin: "dispute_statement",
        }),
        displayGroundsSummary: groundsSummary,
      });
      continue;
    }

    if (feedbackType === "owner_ruling") {
      const appealEventId = taggedEventId(event.tags, "appeal");
      const rulingRef = tagValue(event.tags, "cw_ruling_ref");
      if (appealEventId === null || rulingRef === undefined) continue;
      // Sarah cannot author an owner ruling, and neither can a member. The
      // author must be the admitted owner appeal key or the ruling is shown as
      // unauthored rather than treated as a decision.
      const authoredByAdmittedOwnerKey =
        config.ownerAppealPubkey !== null && event.pubkey === config.ownerAppealPubkey;
      rulingsByAppealEventId.set(appealEventId, {
        sourceEventId: event.id,
        rulingRef,
        ownerAppealPubkey: tagValue(event.tags, "cw_owner_appeal_pubkey") ?? event.pubkey,
        outcome: tagValue(event.tags, "status") ?? "uphold",
        authoredByAdmittedOwnerKey,
      });
    }
  }

  // A decision that claimed independence with nothing signed behind it is the
  // state amendment `SARAH-CW-00-A1` exists to end. It is shown refused rather
  // than rendered as verified, and rather than dropped: a claim that disappears
  // is indistinguishable from one that was never made.
  for (const [resultEventId, claim] of independenceClaimsByResultEventId) {
    if (verificationsByResultEventId.has(resultEventId)) continue;
    const refused = refusedVerificationsByResultEventId.get(resultEventId);
    if (refused !== undefined) {
      verificationsByResultEventId.set(resultEventId, refused);
      continue;
    }
    verificationsByResultEventId.set(resultEventId, {
      sourceEventId: claim.decisionEventId,
      verifierPubkey: claim.verifierAgentPubkey,
      verifierOperatorPubkey:
        communityOperatorForAgent(fold, claim.verifierAgentPubkey) ??
        claim.verifierOperatorRef,
      producerOperatorPubkey: claim.producerOperatorRef,
      operatorsAreIndependent: false,
      refusalReason: "verification_event_absent",
      verdict: null,
      verifierSigned: false,
    });
  }
  // A verification the verifier signed stands on its own; it does not need a
  // decision to have cited it. Refusals with no claim behind them are surfaced
  // for the same reason.
  for (const [resultEventId, refused] of refusedVerificationsByResultEventId) {
    if (!verificationsByResultEventId.has(resultEventId)) {
      verificationsByResultEventId.set(resultEventId, refused);
    }
  }

  return {
    decisionsByResultEventId,
    acceptedQuoteRefsByRequestId,
    appealsByDecisionEventId,
    rulingsByAppealEventId,
    verificationsByResultEventId,
  };
};

const experienceModel = (
  events: ReadonlyArray<Issue31ConfirmedEvent>,
  config: Issue31CommunityProjectionConfig,
): Issue31CommunityExperienceModel => {
  const earner = config.viewerPubkey;
  if (earner === null || !HEX_64.test(earner)) {
    return emptyIssue31CommunityReadModel().experience;
  }
  const scorerPubkeys = config.scorerPubkeys.filter((key) => HEX_64.test(key));

  const awards: XpAwardRecord[] = [];
  let publishedRankPoints: number | null = null;
  let publishedRankDisagreed = false;
  let publishedRank: ReturnType<typeof parseXpRankEvent> = null;

  /**
   * NIP-58 badge awards read off the wire, keyed by badge id.
   *
   * The contract (§4) names badge publisher keys as the only writable authority
   * for kinds 30009 / 8 / 10008, and the client already stores them
   * author-scoped to the admitted scorer set, so a member cannot award
   * themselves a badge. Nothing here reads a badge from an unadmitted key.
   */
  const wireBadgeAwards = new Map<
    string,
    { readonly awardEventId: string; readonly issuerPubkey: string }
  >();
  const wireBadgeNames = new Map<string, string>();
  const knownBadgeIds = new Set<string>(XP_BADGE_IDS as ReadonlyArray<string>);

  for (const confirmed of events) {
    const event = confirmed.event;
    if (event.kind === XP_AWARD_KIND) {
      const award = parseXpAwardEvent(event);
      if (award !== null && award.earnerPubkey === earner) awards.push(award);
      continue;
    }
    if (event.kind === XP_BADGE_DEFINITION_KIND) {
      if (!scorerPubkeys.includes(event.pubkey)) continue;
      const badgeId = tagValue(event.tags, "d");
      const name = tagValue(event.tags, "name");
      if (badgeId !== undefined && name !== undefined && knownBadgeIds.has(badgeId)) {
        wireBadgeNames.set(badgeId, name);
      }
      continue;
    }
    if (event.kind === XP_BADGE_AWARD_KIND) {
      // A badge award names its earner with `p` and its definition with the
      // NIP-33 `a` coordinate `30009:<issuer>:<badge id>`. The issuer inside
      // that coordinate must be the same key that signed the award, or a
      // publisher could award another publisher's badge.
      if (!scorerPubkeys.includes(event.pubkey)) continue;
      const earnerTag = event.tags.find((tag) => tag[0] === "p")?.[1];
      if (earnerTag !== earner) continue;
      const address = tagValue(event.tags, "a");
      if (address === undefined) continue;
      const parts = address.split(":");
      if (parts.length !== 3) continue;
      const [addressKind, issuerPubkey, badgeId] = parts as [string, string, string];
      if (
        addressKind !== String(XP_BADGE_DEFINITION_KIND) ||
        issuerPubkey !== event.pubkey ||
        !knownBadgeIds.has(badgeId)
      ) {
        continue;
      }
      wireBadgeAwards.set(badgeId, { awardEventId: event.id, issuerPubkey });
      continue;
    }
    if (event.kind === XP_RANK_KIND) {
      // Only OpenAgents scorer keys publish rank. Anything else is a member
      // asserting their own score and is not read at all.
      if (!scorerPubkeys.includes(event.pubkey)) continue;
      const rank = parseXpRankEvent(event);
      if (rank !== null && rank.earnerPubkey === earner) {
        publishedRank = rank;
        publishedRankPoints = rank.totalPoints;
      }
    }
  }

  // Rank is a projection of the awards, never a separate opinion. Recompute it
  // here and, when the published assertion disagrees, keep the recomputed value
  // and say so.
  const recomputed = projectRank(awards, { earnerPubkey: earner, scorerPubkeys });
  if (publishedRank !== null) {
    publishedRankDisagreed = !rankAgreesWithAwards(publishedRank, awards, scorerPubkeys);
  }

  // Badges: derived and published, reconciled rather than one replacing the
  // other. The derived set stays the authority for `badgeIds`; the wire adds
  // the event ids and definition names that make them renderable, and shows a
  // published badge the award stream does not support as exactly that.
  const derivedBadgeIds = new Set<string>(
    earnedBadgeIds(awards, { earnerPubkey: earner, scorerPubkeys }),
  );
  const badges: Issue31CommunityBadgeRow[] = [];
  for (const badgeId of [
    ...derivedBadgeIds,
    ...[...wireBadgeAwards.keys()].filter((id) => !derivedBadgeIds.has(id)),
  ]) {
    const wire = wireBadgeAwards.get(badgeId) ?? null;
    const supportedByAwards = derivedBadgeIds.has(badgeId);
    badges.push({
      badgeId,
      source: supportedByAwards
        ? wire === null
          ? "awards_only"
          : "awards_and_wire"
        : "wire_only",
      awardEventId: wire?.awardEventId ?? null,
      issuerPubkey: wire?.issuerPubkey ?? null,
      name: wireBadgeNames.get(badgeId) ?? null,
      supportedByAwards,
    });
  }

  return {
    recomputedTotalPoints: recomputed.totalPoints,
    recomputedLevel: recomputed.level,
    recomputedLevelId: recomputed.levelId,
    awardCount: recomputed.awardCount,
    publishedRankPoints,
    publishedRankDisagreed,
    badgeIds: [...derivedBadgeIds],
    badges,
    awards: awards.map((award) => ({
      sourceEventId: award.awardEventId ?? null,
      awardKind: award.awardKind,
      points: award.points,
      workEventId: award.workEventId,
      receiptRef: award.receiptRef,
      deepLink: deepLinkFor(award.awardEventId ?? award.workEventId),
    })),
  };
};

/**
 * Which controls this viewer may take on this unit, right now.
 *
 * Derived from the signed role, the grant expiration, and the lifecycle state
 * together. Any one of them failing removes the control — the view renders this
 * list and nothing else, so an unauthorized control has no path to the screen.
 */
const unitControls = (input: {
  readonly unit: Omit<Issue31CommunityWorkUnitRow, "controls">;
  readonly fold: CommunityLedgerFold;
  readonly viewerPubkey: string | null;
  readonly viewerRole: Issue31CommunityRole;
  readonly viewerAgentPubkeys: ReadonlyArray<string>;
}): ReadonlyArray<Issue31CommunityControl> => {
  const { unit, fold, viewerPubkey, viewerRole } = input;
  if (viewerPubkey === null) return [];
  // A revoked or unknown member gets nothing, whatever the unit state is. This
  // is the "revocation removes work-unit access immediately" exit.
  if (viewerRole !== "member" && viewerRole !== "agent_operator" && viewerRole !== "owner") {
    return [];
  }
  if (unit.expired) return [];

  const controls: Issue31CommunityControl[] = [];
  const permittedByRef = `record.community.membership:${viewerPubkey.slice(0, 12)}`;
  const isRequester = unit.requesterPubkey === viewerPubkey;
  const viewerIsAcceptedProvider =
    unit.acceptedProviderPubkey !== null &&
    (unit.acceptedProviderPubkey === viewerPubkey ||
      input.viewerAgentPubkeys.includes(unit.acceptedProviderPubkey));

  // Quoting needs an admitted agent to do the work, and the requester cannot
  // quote their own unit.
  if (
    unit.lifecycle === "open" ||
    (unit.lifecycle === "quoted" && !isRequester)
  ) {
    if (input.viewerAgentPubkeys.length > 0 && !isRequester) {
      controls.push({
        kind: "quote_work_unit",
        subjectRef: unit.unitRef,
        permittedByRef,
        idempotencyRef: `community.quote.${unit.idempotencyRef}.${viewerPubkey.slice(0, 8)}`,
        signedBy: "agent_compute",
      });
    }
  }

  if (unit.lifecycle === "accepted" && viewerIsAcceptedProvider) {
    controls.push({
      kind: "submit_result",
      subjectRef: unit.unitRef,
      permittedByRef,
      idempotencyRef: `community.result.${unit.idempotencyRef}`,
      signedBy: "agent_compute",
    });
  }

  // Verification is offered only to a member whose operator differs from the
  // producer's. Producer and verifier must have distinct operators, so a second
  // key held by the same operator does not qualify.
  if (unit.lifecycle === "delivered" && unit.result !== null) {
    const producerOperator =
      unit.result.providerOperatorPubkey ??
      communityOperatorForAgent(fold, unit.result.providerPubkey);
    if (
      producerOperator !== null &&
      producerOperator !== viewerPubkey &&
      input.viewerAgentPubkeys.length > 0
    ) {
      controls.push({
        kind: "verify_result",
        subjectRef: unit.unitRef,
        permittedByRef,
        idempotencyRef: `community.verify.${unit.idempotencyRef}.${viewerPubkey.slice(0, 8)}`,
        signedBy: "agent_compute",
      });
    }
  }

  // A rejected result can be appealed by the operator behind it, once.
  if (
    unit.decision?.outcome === "rejected" &&
    unit.appeal === null &&
    viewerIsAcceptedProvider
  ) {
    controls.push({
      kind: "file_appeal",
      subjectRef: unit.unitRef,
      permittedByRef,
      // The human operator appeals, not their agent. Sarah decided about work
      // the operator is accountable for, so the operator's own key files it.
      idempotencyRef: `community.appeal.${unit.idempotencyRef}`,
      signedBy: "operator_device",
    });
  }

  return controls;
};

const lifecycleFor = (input: {
  readonly expired: boolean;
  readonly quoteCount: number;
  readonly acceptedProviderPubkey: string | null;
  readonly result: Issue31CommunityResultRow | null;
  readonly verification: Issue31CommunityVerificationRow | null;
  readonly decision: Issue31CommunityDecisionRow | null;
  readonly appeal: Issue31CommunityAppealRow | null;
  readonly ruling: Issue31CommunityRulingRow | null;
}): Issue31CommunityUnitLifecycle => {
  if (input.ruling !== null) return "ruled";
  if (input.appeal !== null) return "disputed";
  if (input.decision !== null) return "decided";
  // Only an admitted verification moves the unit. A refused one — a decision
  // that claimed independence with nothing signed behind it, or a verifier the
  // record does not confirm — leaves the unit `delivered` and shows why. The
  // previous shape treated any verification row as proof and would have
  // rendered `verified` for a claim nobody signed.
  if (input.verification !== null && input.verification.operatorsAreIndependent) {
    return "verified";
  }
  if (input.result !== null) return "delivered";
  // Expiry is checked after the terminal states: a unit that was decided before
  // its grant lapsed did not "expire", and showing it that way would erase the
  // decision.
  if (input.expired) return "expired";
  if (input.acceptedProviderPubkey !== null) return "accepted";
  if (input.quoteCount > 0) return "quoted";
  return "open";
};

/**
 * Project the community room from the confirmed record set.
 *
 * Only `room === "community"` events are read. The owner-private room is
 * projected by its own function from its own events and the two never share a
 * ledger, a cursor, or a row.
 */
export const projectIssue31CommunityReadModel = (
  snapshot: Issue31NostrClientSnapshot,
  config: Issue31CommunityProjectionConfig,
): Issue31CommunityReadModel => {
  if (config.groupId === null || config.groupId.trim() === "") {
    return emptyIssue31CommunityReadModel("reason.issue31.community.group_not_configured");
  }
  if (config.adminPubkeys.length === 0) {
    // Without an out-of-band admin set there is no admission authority, and
    // reading membership off the relay instead is precisely what the contract
    // forbids. Nothing is shown rather than something unauthorized.
    return emptyIssue31CommunityReadModel("reason.issue31.community.admin_keys_not_configured");
  }

  const events = communityEventsFrom(snapshot);
  const fold = foldCommunityLedgerFromEvents({
    groupId: config.groupId,
    adminPubkeys: config.adminPubkeys,
    events: events.map(asSignedRecord),
  });

  const viewerPubkey =
    config.viewerPubkey !== null && HEX_64.test(config.viewerPubkey)
      ? config.viewerPubkey
      : null;
  const viewerRoleState =
    viewerPubkey === null
      ? null
      : communityRoleFor(fold, viewerPubkey);
  const viewerRole: Issue31CommunityRole = viewerRoleState?.role ?? "none";
  const viewerAgentPubkeys = viewerRoleState?.admittedAgentPubkeys ?? [];

  // ---- work units -------------------------------------------------------
  const requests = new Map<string, SarahLbrWorkRequest>();
  const requesterByRequestId = new Map<string, string>();
  let rejectedRecordCount = 0;

  for (const confirmed of events) {
    if (confirmed.event.kind !== LBR_AGENTIC_CODING_REQUEST_KIND) continue;
    try {
      const request = decodeSarahLbrWorkRequestEvent(confirmed.event);
      // A request that names another group is not this room's work.
      if (request.groupId !== undefined && request.groupId !== config.groupId) {
        rejectedRecordCount += 1;
        continue;
      }
      requests.set(confirmed.event.id, request);
      requesterByRequestId.set(confirmed.event.id, confirmed.event.pubkey);
    } catch {
      rejectedRecordCount += 1;
    }
  }

  const arbitration = readArbitrationLane(events, fold, config, requesterByRequestId);

  const quotesByRequestId = new Map<string, Issue31CommunityQuoteRow[]>();
  for (const confirmed of events) {
    if (confirmed.event.kind !== LBR_FEEDBACK_KIND) continue;
    let quote: SarahLbrQuote;
    try {
      quote = decodeSarahLbrQuoteEvent(confirmed.event);
    } catch {
      // Not every kind-7000 event is a quote; the arbitration lane reads the
      // rest. Only count a genuine decode failure of a quote-shaped event.
      if (tagValue(confirmed.event.tags, "cw_feedback_type") === undefined) {
        rejectedRecordCount += 1;
      }
      continue;
    }
    const accepted =
      arbitration.acceptedQuoteRefsByRequestId.get(quote.requestId)?.has(quote.quoteRef) === true;
    const rows = quotesByRequestId.get(quote.requestId) ?? [];
    rows.push({
      sourceEventId: confirmed.event.id,
      quoteRef: quote.quoteRef,
      providerPubkey: confirmed.event.pubkey,
      providerOperatorPubkey: communityOperatorForAgent(fold, confirmed.event.pubkey),
      providerRef: quote.providerRef,
      accepted,
      untrustedProviderRef: quoteOrMarker({
        content: quote.providerRef,
        authorPubkey: confirmed.event.pubkey,
        origin: "quote",
      }),
    });
    quotesByRequestId.set(quote.requestId, rows);
  }

  const resultsByRequestId = new Map<string, Issue31CommunityResultRow & { eventId: string }>();
  for (const confirmed of events) {
    if (confirmed.event.kind !== LBR_AGENTIC_CODING_RESULT_KIND) continue;
    const requestEventId =
      taggedEventId(confirmed.event.tags, "request") ?? tagValue(confirmed.event.tags, "e") ?? null;
    if (requestEventId === null || !requests.has(requestEventId)) {
      rejectedRecordCount += 1;
      continue;
    }
    const summary = confirmed.event.content.slice(0, 4_096);
    resultsByRequestId.set(requestEventId, {
      eventId: confirmed.event.id,
      sourceEventId: confirmed.event.id,
      providerPubkey: confirmed.event.pubkey,
      providerOperatorPubkey: communityOperatorForAgent(fold, confirmed.event.pubkey),
      createdAt: confirmed.event.created_at,
      untrustedSummary: quoteOrMarker({
        content: summary === "" ? " " : summary,
        authorPubkey: confirmed.event.pubkey,
        origin: "result_summary",
      }),
      displaySummary: summary,
    });
  }

  const workUnits: Issue31CommunityWorkUnitRow[] = [...requests.entries()].map(
    ([requestEventId, request]) => {
      const quotes = quotesByRequestId.get(requestEventId) ?? [];
      const acceptedQuote = quotes.find((quote) => quote.accepted) ?? null;
      const result = resultsByRequestId.get(requestEventId) ?? null;
      const decision =
        result === null
          ? null
          : (arbitration.decisionsByResultEventId.get(result.eventId) ?? null);
      const verification =
        result === null
          ? null
          : (arbitration.verificationsByResultEventId.get(result.eventId) ?? null);
      const appeal =
        decision === null
          ? null
          : (arbitration.appealsByDecisionEventId.get(decision.sourceEventId) ?? null);
      const ruling =
        appeal === null
          ? null
          : (arbitration.rulingsByAppealEventId.get(appeal.sourceEventId) ?? null);
      const expired = request.workUnit.expiresAtUnix <= config.nowUnixSeconds;

      const base = {
        requestEventId,
        unitRef: request.workUnit.workUnitRef,
        grantRef: request.workUnit.grantRef,
        idempotencyRef: request.workUnit.idempotencyRef,
        targetRefs: request.workUnit.repositoryRefs,
        allowedActionRefs: request.workUnit.allowedActionRefs,
        experienceTierCopy: experienceTierCopy(request),
        expiresAtUnix: request.workUnit.expiresAtUnix,
        expired,
        lifecycle: lifecycleFor({
          expired,
          quoteCount: quotes.length,
          acceptedProviderPubkey: acceptedQuote?.providerPubkey ?? null,
          result,
          verification,
          decision,
          appeal,
          ruling,
        }),
        requesterPubkey: requesterByRequestId.get(requestEventId) ?? "",
        quotes,
        acceptedProviderPubkey: acceptedQuote?.providerPubkey ?? null,
        result,
        verification,
        decision,
        appeal,
        ruling,
        deepLink: deepLinkFor(requestEventId),
      };
      return {
        ...base,
        controls: unitControls({
          unit: base,
          fold,
          viewerPubkey,
          viewerRole,
          viewerAgentPubkeys,
        }),
      };
    },
  );
  workUnits.sort(
    (left, right) => right.expiresAtUnix - left.expiresAtUnix || left.unitRef.localeCompare(right.unitRef),
  );

  // ---- room-level controls ---------------------------------------------
  const roomControls: Issue31CommunityControl[] = [];
  if (viewerPubkey !== null) {
    if (viewerRole === "owner") {
      roomControls.push(
        {
          kind: "invite_member",
          subjectRef: config.groupId,
          permittedByRef: `record.community.group_admin:${viewerPubkey.slice(0, 12)}`,
          idempotencyRef: `community.invite.${config.groupId}`,
          signedBy: "operator_device",
        },
        {
          kind: "revoke_member",
          subjectRef: config.groupId,
          permittedByRef: `record.community.group_admin:${viewerPubkey.slice(0, 12)}`,
          idempotencyRef: `community.revoke_member.${config.groupId}`,
          signedBy: "operator_device",
        },
      );
    }
    if (viewerRole === "member" || viewerRole === "agent_operator" || viewerRole === "owner") {
      roomControls.push(
        {
          kind: "post_message",
          subjectRef: config.groupId,
          permittedByRef: `record.community.membership:${viewerPubkey.slice(0, 12)}`,
          idempotencyRef: `community.message.${config.groupId}`,
          signedBy: "operator_device",
        },
        {
          kind: "attach_agent",
          subjectRef: config.groupId,
          permittedByRef: `record.community.membership:${viewerPubkey.slice(0, 12)}`,
          idempotencyRef: `community.attach_agent.${config.groupId}`,
          // The agent signs its own persona with its own key, on the operator's
          // machine. This phone never holds that key and cannot mint the
          // attestation for it.
          signedBy: "agent_compute",
        },
      );
      for (const agentPubkey of viewerAgentPubkeys) {
        roomControls.push({
          kind: "revoke_agent",
          subjectRef: agentPubkey,
          permittedByRef: `record.community.agent_binding:${agentPubkey.slice(0, 12)}`,
          idempotencyRef: `community.revoke_agent.${agentPubkey}`,
          // Revocation is the one agent-scoped act the operator takes without
          // the agent key: it removes group access and the capability grant,
          // and never reaches into the operator's machine.
          signedBy: "operator_device",
        });
      }
    }
  }

  const transcript = transcriptRows(
    events,
    fold,
    config.groupId,
    Math.max(1, Math.min(200, config.transcriptLimit ?? COMMUNITY_TRANSCRIPT_PAGE_SIZE)),
  );

  const hardRefusals = fold.refusals.filter(
    (refusal) => refusal.code !== "already_applied" && refusal.code !== "unreadable",
  );

  return {
    status: hardRefusals.length === 0 && rejectedRecordCount === 0 ? "ready" : "gap",
    reasonRef:
      hardRefusals.length > 0
        ? `reason.issue31.community.record_refused:${hardRefusals[0]?.code}`
        : rejectedRecordCount > 0
          ? "reason.issue31.community.record_unreadable"
          : null,
    groupId: config.groupId,
    groupName: fold.metadata?.name ?? null,
    viewerPubkey,
    viewerRole,
    viewerRoleStatus: viewerRoleState?.status ?? "unknown",
    experienceOnlyCopy: ISSUE31_COMMUNITY_EXPERIENCE_ONLY_COPY,
    transcript,
    roster: rosterRows(fold),
    agents: agentRows(fold),
    workUnits,
    experience: experienceModel(events, config),
    controls: roomControls,
    refusals: fold.refusals,
    rejectedRecordCount: rejectedRecordCount + hardRefusals.length,
  };
};

/**
 * Every member-written block on this room, in the only form Sarah may read.
 *
 * Returns branded values, so the caller cannot hand Sarah a raw member string
 * even by accident; `buildCommunitySarahContext` then re-checks the brand at
 * runtime before assembling. The room has no other door into her context.
 */
export const issue31CommunityUntrustedBlocks = (
  model: Issue31CommunityReadModel,
): ReadonlyArray<UntrustedCommunityContent> => [
  ...model.transcript.map((row) => row.untrusted),
  ...model.workUnits.flatMap((unit) => [
    ...unit.quotes.map((quote) => quote.untrustedProviderRef),
    ...(unit.result === null ? [] : [unit.result.untrustedSummary]),
    ...(unit.appeal === null ? [] : [unit.appeal.untrustedGroundsSummary]),
  ]),
];
