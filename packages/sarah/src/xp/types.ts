import { Schema as S } from "effect";

/**
 * Community workroom experience points — SARAH-CW-06.
 *
 * Spec: docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §35.
 * Experience is recognition of accepted work. It is not currency and is
 * never called "earnings". Settlement stays deferred (#9230 / SARAH-CW-07).
 */

/** NIP-32 label namespace for experience awards. */
export const XP_NAMESPACE = "com.openagents.xp" as const;

/** Published scoring algorithm identity. */
export const XP_ALGORITHM_ID = "openagents.xp.v1" as const;

/** NIP-32 regular label kind (one immutable award). */
export const XP_AWARD_KIND = 1985 as const;

/** NIP-85 trusted assertion kind (running score projection). */
export const XP_RANK_KIND = 30382 as const;

/** NIP-58 badge definition kind (addressable). */
export const XP_BADGE_DEFINITION_KIND = 30009 as const;

/** NIP-58 badge award kind. */
export const XP_BADGE_AWARD_KIND = 8 as const;

/** NIP-58 profile badges kind. */
export const XP_PROFILE_BADGES_KIND = 10008 as const;

/** Deferred settlement issue — do not implement payout here. */
export const XP_SETTLEMENT_DEFERRED_ISSUE = 9230 as const;
export const XP_SETTLEMENT_DEFERRED_PACKET = "SARAH-CW-07" as const;

/**
 * Terms that must not name an experience total or imply XP is money.
 * Room copy MAY say v1 does not pay; it MUST NOT call experience "earnings".
 */
export const XP_FORBIDDEN_COPY_TERMS: ReadonlyArray<string> = [
  "earnings",
  "xp earnings",
  "experience earnings",
  "total earnings",
  "payout",
  "wage",
  "salary",
  "revenue share",
  "revenue_share",
];

/**
 * Award kinds that contribute experience. Fixed integer points only.
 * No hidden weights, no multipliers, no model in the loop.
 */
export const XP_AWARD_KINDS = [
  "accepted_work_unit.tier_1",
  "accepted_work_unit.tier_2",
  "accepted_work_unit.tier_3",
  "accepted_independent_verification",
  "reproduced_defect",
  "accepted_review",
  "first_job_type",
] as const;

export type XpAwardKind = (typeof XP_AWARD_KINDS)[number];

/** Fixed point table (§35.4). Total experience is the sum of award points. */
export const XP_POINT_TABLE: Readonly<Record<XpAwardKind, number>> = {
  "accepted_work_unit.tier_1": 10,
  "accepted_work_unit.tier_2": 20,
  "accepted_work_unit.tier_3": 40,
  accepted_independent_verification: 5,
  reproduced_defect: 8,
  accepted_review: 3,
  first_job_type: 5,
};

/**
 * Fixed level thresholds. Published beside the point table; not a quiet curve.
 * Level N requires at least `minPoints` total experience.
 */
export const XP_LEVEL_THRESHOLDS: ReadonlyArray<{
  readonly level: number;
  readonly minPoints: number;
  readonly id: string;
  readonly label: string;
}> = [
  { level: 0, minPoints: 0, id: "level_0", label: "Recruit" },
  { level: 1, minPoints: 10, id: "level_1", label: "Contributor" },
  { level: 2, minPoints: 50, id: "level_2", label: "Regular" },
  { level: 3, minPoints: 150, id: "level_3", label: "Veteran" },
  { level: 4, minPoints: 400, id: "level_4", label: "Expert" },
  { level: 5, minPoints: 1000, id: "level_5", label: "Master" },
];

/** Milestone badge ids (NIP-58 definition `d` tags). */
export const XP_BADGE_IDS = [
  "first-accepted-unit",
  "first-verification",
  "first-defect-repro",
  "first-review",
  "first-job-type",
  "level-1",
  "level-2",
  "level-3",
  "level-4",
  "level-5",
] as const;

export type XpBadgeId = (typeof XP_BADGE_IDS)[number];

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));

export const XpAwardKindSchema = S.Literals([...XP_AWARD_KINDS]);

export const XpWorkUnitTier = S.Literals([1, 2, 3]);
export type XpWorkUnitTier = S.Schema.Type<typeof XpWorkUnitTier>;

/** Parsed / validated award record used by the pure rank functions. */
export const XpAwardRecordSchema = S.Struct({
  awardKind: XpAwardKindSchema,
  points: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  earnerPubkey: Hex64,
  /** Accepted work event id (64 hex). Required for a valid award. */
  workEventId: Hex64,
  /** Acceptance / authority receipt ref or event id. Required. */
  receiptRef: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  /** Scorer pubkey that published the NIP-32 label. */
  scorerPubkey: Hex64,
  /** Event id of the award label itself (when known). */
  awardEventId: S.optional(Hex64),
  /** Work-unit tier when awardKind is accepted_work_unit.*. */
  tier: S.optional(XpWorkUnitTier),
  /** Job type id; required and unique per earner for first_job_type. */
  jobType: S.optional(S.String.check(S.isMinLength(1), S.isMaxLength(128))),
  createdAt: S.optional(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
});
export type XpAwardRecord = S.Schema.Type<typeof XpAwardRecordSchema>;

/** Recomputable rank projection (NIP-85 body / tags source). */
export const XpRankProjectionSchema = S.Struct({
  schema: S.Literal("openagents.sarah.xp_rank.v1"),
  algorithmId: S.Literal(XP_ALGORITHM_ID),
  earnerPubkey: Hex64,
  totalPoints: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  level: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  levelId: S.String,
  awardCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  /** Digest of contributing award event ids (sorted, sha256 hex) when known. */
  awardStreamDigest: S.optional(S.String.check(S.isPattern(/^[0-9a-f]{64}$/))),
});
export type XpRankProjection = S.Schema.Type<typeof XpRankProjectionSchema>;

/** Public-safe badge definition (NIP-58 kind 30009 content shape). */
export const XpBadgeDefinitionSpecSchema = S.Struct({
  id: S.Literals([...XP_BADGE_IDS]),
  name: S.String.check(S.isMinLength(1), S.isMaxLength(80)),
  description: S.String.check(S.isMinLength(1), S.isMaxLength(400)),
  /** Optional image URL or empty for text-only badges. */
  image: S.optional(S.String),
  /** Experience level that unlocks this badge, when applicable. */
  minLevel: S.optional(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
  /** Award kind that unlocks this badge on first occurrence. */
  firstAwardKind: S.optional(XpAwardKindSchema),
});
export type XpBadgeDefinitionSpec = S.Schema.Type<
  typeof XpBadgeDefinitionSpecSchema
>;
