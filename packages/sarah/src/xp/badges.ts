import { Schema as S } from "effect";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { assertXpCopyPublicSafe } from "./awards.ts";
import { projectRank, selectContributingAwards } from "./rank.ts";
import {
  XP_ALGORITHM_ID,
  XP_BADGE_AWARD_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_BADGE_IDS,
  XP_NAMESPACE,
  XP_PROFILE_BADGES_KIND,
  XpBadgeDefinitionSpecSchema,
  type XpAwardRecord,
  type XpBadgeDefinitionSpec,
  type XpBadgeId,
} from "./types.ts";

const decodeBadge = S.decodeUnknownSync(XpBadgeDefinitionSpecSchema);
const HEX64 = /^[0-9a-f]{64}$/;

/**
 * Published NIP-58 badge definition specs for experience milestones.
 * Immutable, non-transferable, displayable. Not currency.
 */
export const XP_BADGE_DEFINITIONS: ReadonlyArray<XpBadgeDefinitionSpec> = [
  decodeBadge({
    id: "first-accepted-unit",
    name: "First accepted unit",
    description:
      "Awarded when a member completes their first accepted work unit. Experience only; no payment.",
    firstAwardKind: "accepted_work_unit.tier_1",
  }),
  decodeBadge({
    id: "first-verification",
    name: "First verification",
    description:
      "Awarded for the first accepted independent verification of another member's result.",
    firstAwardKind: "accepted_independent_verification",
  }),
  decodeBadge({
    id: "first-defect-repro",
    name: "First defect reproduction",
    description: "Awarded for the first accepted reproduced defect.",
    firstAwardKind: "reproduced_defect",
  }),
  decodeBadge({
    id: "first-review",
    name: "First review",
    description:
      "Awarded for the first accepted review of another member's result.",
    firstAwardKind: "accepted_review",
  }),
  decodeBadge({
    id: "first-job-type",
    name: "First new job type",
    description:
      "Awarded when a member lands their first accepted unit in a new job type.",
    firstAwardKind: "first_job_type",
  }),
  decodeBadge({
    id: "level-1",
    name: "Contributor",
    description: "Reached experience level 1 (10 points).",
    minLevel: 1,
  }),
  decodeBadge({
    id: "level-2",
    name: "Regular",
    description: "Reached experience level 2 (50 points).",
    minLevel: 2,
  }),
  decodeBadge({
    id: "level-3",
    name: "Veteran",
    description: "Reached experience level 3 (150 points).",
    minLevel: 3,
  }),
  decodeBadge({
    id: "level-4",
    name: "Expert",
    description: "Reached experience level 4 (400 points).",
    minLevel: 4,
  }),
  decodeBadge({
    id: "level-5",
    name: "Master",
    description: "Reached experience level 5 (1000 points).",
    minLevel: 5,
  }),
];

// first-accepted-unit matches any work-unit tier, not only tier_1.
// Special-case in eligibility below.

export const getXpBadgeDefinition = (
  id: XpBadgeId,
): XpBadgeDefinitionSpec | undefined =>
  XP_BADGE_DEFINITIONS.find((b) => b.id === id);

/**
 * Which badge ids a member has earned from the award stream.
 * Pure; no side effects; no settlement.
 */
export const earnedBadgeIds = (
  awards: ReadonlyArray<XpAwardRecord>,
  input: {
    readonly earnerPubkey: string;
    readonly scorerPubkeys: ReadonlyArray<string>;
  },
): ReadonlyArray<XpBadgeId> => {
  const contributing = selectContributingAwards(awards, input);
  const kinds = new Set(contributing.map((a) => a.awardKind));
  const rank = projectRank(awards, input);
  const earned: XpBadgeId[] = [];

  for (const badge of XP_BADGE_DEFINITIONS) {
    if (badge.id === "first-accepted-unit") {
      if (
        kinds.has("accepted_work_unit.tier_1") ||
        kinds.has("accepted_work_unit.tier_2") ||
        kinds.has("accepted_work_unit.tier_3")
      ) {
        earned.push(badge.id);
      }
      continue;
    }
    if (badge.firstAwardKind && kinds.has(badge.firstAwardKind)) {
      earned.push(badge.id);
      continue;
    }
    if (badge.minLevel !== undefined && rank.level >= badge.minLevel) {
      earned.push(badge.id);
    }
  }

  return earned;
};

/** Addressable coordinate for a badge definition. */
export const badgeDefinitionAddress = (
  issuerPubkey: string,
  badgeId: XpBadgeId,
): string => `${XP_BADGE_DEFINITION_KIND}:${issuerPubkey}:${badgeId}`;

/**
 * Build an unsigned NIP-58 kind-30009 badge definition template.
 * Issuer is the OpenAgents scorer / badge authority key at sign time.
 */
export const buildXpBadgeDefinitionTemplate = (input: {
  readonly badgeId: XpBadgeId;
  readonly createdAt?: number;
}): SarahNostrEventTemplate => {
  const spec = getXpBadgeDefinition(input.badgeId);
  if (!spec) {
    throw new Error(`unknown badge id: ${input.badgeId}`);
  }
  assertXpCopyPublicSafe(spec.name, "badge_name");
  assertXpCopyPublicSafe(spec.description, "badge_description");

  const content = JSON.stringify({
    name: spec.name,
    description: spec.description,
    ...(spec.image ? { image: spec.image } : {}),
    namespace: XP_NAMESPACE,
    algorithmId: XP_ALGORITHM_ID,
  });

  const tags: string[][] = [
    ["d", spec.id],
    ["name", spec.name],
    ["description", spec.description],
    ["L", XP_NAMESPACE],
    ["algorithm", XP_ALGORITHM_ID],
  ];
  if (spec.image) {
    tags.push(["image", spec.image]);
  }

  const template: SarahNostrEventTemplate = {
    kind: XP_BADGE_DEFINITION_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags,
    content,
  };
  assertSarahNostrPublicSafe(template);
  return template;
};

/**
 * Build an unsigned NIP-58 kind-8 badge award template.
 * Targets the earner and the definition address.
 */
export const buildXpBadgeAwardTemplate = (input: {
  readonly badgeId: XpBadgeId;
  readonly issuerPubkey: string;
  readonly earnerPubkey: string;
  readonly createdAt?: number;
}): SarahNostrEventTemplate => {
  if (!HEX64.test(input.issuerPubkey)) {
    throw new Error("issuerPubkey must be 64 lowercase hex");
  }
  if (!HEX64.test(input.earnerPubkey)) {
    throw new Error("earnerPubkey must be 64 lowercase hex");
  }
  if (!(XP_BADGE_IDS as ReadonlyArray<string>).includes(input.badgeId)) {
    throw new Error(`unknown badge id: ${input.badgeId}`);
  }

  const aTag = badgeDefinitionAddress(input.issuerPubkey, input.badgeId);
  const template: SarahNostrEventTemplate = {
    kind: XP_BADGE_AWARD_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags: [
      ["a", aTag],
      ["p", input.earnerPubkey],
      ["L", XP_NAMESPACE],
      ["algorithm", XP_ALGORITHM_ID],
    ],
    content: "",
  };
  assertSarahNostrPublicSafe(template);
  return template;
};

/**
 * Build an unsigned NIP-58 kind-10008 profile badges list for an earner.
 * The earner (or their client) publishes which awards they display.
 */
export const buildXpProfileBadgesTemplate = (input: {
  readonly earnerPubkey: string;
  readonly issuerPubkey: string;
  readonly badgeIds: ReadonlyArray<XpBadgeId>;
  /** Optional award event ids parallel to badgeIds. */
  readonly awardEventIds?: ReadonlyArray<string>;
  readonly createdAt?: number;
}): SarahNostrEventTemplate => {
  if (!HEX64.test(input.earnerPubkey) || !HEX64.test(input.issuerPubkey)) {
    throw new Error("pubkeys must be 64 lowercase hex");
  }

  const tags: string[][] = [["d", "profile_badges"]];
  for (let i = 0; i < input.badgeIds.length; i++) {
    const id = input.badgeIds[i]!;
    tags.push(["a", badgeDefinitionAddress(input.issuerPubkey, id)]);
    const awardId = input.awardEventIds?.[i];
    if (awardId && HEX64.test(awardId)) {
      tags.push(["e", awardId]);
    }
  }
  tags.push(["L", XP_NAMESPACE]);

  const template: SarahNostrEventTemplate = {
    kind: XP_PROFILE_BADGES_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags,
    content: "",
  };
  assertSarahNostrPublicSafe(template);
  return template;
};

/** All published definition templates (for room publication). */
export const buildAllXpBadgeDefinitionTemplates = (createdAt?: number) =>
  XP_BADGE_IDS.map((badgeId) =>
    buildXpBadgeDefinitionTemplate(
      createdAt === undefined ? { badgeId } : { badgeId, createdAt },
    ),
  );
