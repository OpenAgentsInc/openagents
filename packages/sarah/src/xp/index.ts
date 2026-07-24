/**
 * Community workroom experience points — SARAH-CW-06.
 *
 * Three carriers:
 * - NIP-32 kind 1985 awards (namespace com.openagents.xp)
 * - NIP-85 kind 30382 rank projections (recomputable from awards)
 * - NIP-58 badges (definitions 30009, awards 8, profile 10008)
 *
 * Experience is not currency. Never call it "earnings". Settlement is deferred
 * (SARAH-CW-07 / #9230).
 *
 * @see docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §35
 */

export {
  assertXpCopyPublicSafe,
  buildWorkUnitAwardTemplate,
  buildXpAwardTemplate,
  isValidXpAward,
  isXpAwardKind,
  parseXpAwardEvent,
  pointsForAwardKind,
  XpAwardError,
  type BuildXpAwardInput,
} from "./awards.ts";

export {
  badgeDefinitionAddress,
  buildAllXpBadgeDefinitionTemplates,
  buildXpBadgeAwardTemplate,
  buildXpBadgeDefinitionTemplate,
  buildXpProfileBadgesTemplate,
  earnedBadgeIds,
  getXpBadgeDefinition,
  XP_BADGE_DEFINITIONS,
} from "./badges.ts";

export {
  awardStreamDigest,
  buildXpRankTemplate,
  fixedPointsFor,
  levelForPoints,
  parseXpRankEvent,
  projectRank,
  rankAgreesWithAwards,
  resolveRankFromAwards,
  selectContributingAwards,
  sumAwardPoints,
  XpRankError,
} from "./rank.ts";

export {
  XP_ALGORITHM_ID,
  XP_AWARD_KIND,
  XP_AWARD_KINDS,
  XP_BADGE_AWARD_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_BADGE_IDS,
  XP_FORBIDDEN_COPY_TERMS,
  XP_LEVEL_THRESHOLDS,
  XP_NAMESPACE,
  XP_POINT_TABLE,
  XP_PROFILE_BADGES_KIND,
  XP_RANK_KIND,
  XP_SETTLEMENT_DEFERRED_ISSUE,
  XP_SETTLEMENT_DEFERRED_PACKET,
  XpAwardKindSchema,
  XpAwardRecordSchema,
  XpBadgeDefinitionSpecSchema,
  XpRankProjectionSchema,
  XpWorkUnitTier,
  type XpAwardKind,
  type XpAwardRecord,
  type XpBadgeDefinitionSpec,
  type XpBadgeId,
  type XpRankProjection,
  type XpWorkUnitTier as XpWorkUnitTierType,
} from "./types.ts";
