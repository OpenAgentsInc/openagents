import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import { Schema as S } from "effect";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { isValidXpAward, pointsForAwardKind } from "./awards.ts";
import {
  XP_ALGORITHM_ID,
  XP_LEVEL_THRESHOLDS,
  XP_RANK_KIND,
  XpRankProjectionSchema,
  type XpAwardRecord,
  type XpRankProjection,
} from "./types.ts";

const decodeRank = S.decodeUnknownSync(XpRankProjectionSchema);
const HEX64 = /^[0-9a-f]{64}$/;
const utf8 = new TextEncoder();

export class XpRankError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "XpRankError";
    this.code = code;
  }
}

/**
 * Highest published level whose minPoints is <= totalPoints.
 * Levels are fixed thresholds, not a retunable curve.
 */
export const levelForPoints = (
  totalPoints: number,
): {
  readonly level: number;
  readonly id: string;
  readonly label: string;
  readonly minPoints: number;
} => {
  if (!Number.isInteger(totalPoints) || totalPoints < 0) {
    throw new XpRankError(
      "invalid_points",
      "totalPoints must be a non-negative integer",
    );
  }
  let current = XP_LEVEL_THRESHOLDS[0]!;
  for (const row of XP_LEVEL_THRESHOLDS) {
    if (totalPoints >= row.minPoints) current = row;
  }
  return current;
};

/**
 * Filter awards that contribute to a member's score.
 *
 * - Only scorer-key awards count (self-labels never enter the score).
 * - Structural validity required (work event + receipt).
 * - `first_job_type` counts at most once per (earner, jobType).
 * - One award per workEventId+awardKind (idempotent against replay).
 */
export const selectContributingAwards = (
  awards: ReadonlyArray<XpAwardRecord>,
  input: {
    readonly earnerPubkey: string;
    readonly scorerPubkeys: ReadonlyArray<string>;
  },
): ReadonlyArray<XpAwardRecord> => {
  if (!HEX64.test(input.earnerPubkey)) {
    throw new XpRankError(
      "invalid_earner",
      "earnerPubkey must be 64 lowercase hex",
    );
  }

  const seenWork = new Set<string>();
  const seenJobTypes = new Set<string>();
  const selected: XpAwardRecord[] = [];

  // Stable order: createdAt asc, then workEventId, then awardKind.
  const ordered = [...awards].sort((a, b) => {
    const ta = a.createdAt ?? 0;
    const tb = b.createdAt ?? 0;
    if (ta !== tb) return ta - tb;
    if (a.workEventId !== b.workEventId) {
      return a.workEventId < b.workEventId ? -1 : 1;
    }
    return a.awardKind < b.awardKind ? -1 : a.awardKind > b.awardKind ? 1 : 0;
  });

  for (const award of ordered) {
    if (award.earnerPubkey !== input.earnerPubkey) continue;
    if (!isValidXpAward(award, input.scorerPubkeys)) continue;

    const workKey = `${award.awardKind}:${award.workEventId}`;
    if (seenWork.has(workKey)) continue;

    if (award.awardKind === "first_job_type") {
      const jobType = award.jobType;
      if (!jobType || seenJobTypes.has(jobType)) continue;
      seenJobTypes.add(jobType);
    }

    seenWork.add(workKey);
    selected.push(award);
  }

  return selected;
};

/** Sum of fixed points for contributing awards. Nothing else contributes. */
export const sumAwardPoints = (
  awards: ReadonlyArray<XpAwardRecord>,
  input: {
    readonly earnerPubkey: string;
    readonly scorerPubkeys: ReadonlyArray<string>;
  },
): number => {
  const contributing = selectContributingAwards(awards, input);
  return contributing.reduce((sum, award) => sum + award.points, 0);
};

/** Optional digest over known award event ids (sorted). */
export const awardStreamDigest = (
  awards: ReadonlyArray<XpAwardRecord>,
): string | undefined => {
  const ids = awards
    .map((a) => a.awardEventId)
    .filter((id): id is string => typeof id === "string" && HEX64.test(id))
    .sort();
  if (ids.length === 0) return undefined;
  return bytesToHex(sha256(utf8.encode(ids.join(","))));
};

/**
 * Pure rank projection from the award stream alone.
 * The NIP-85 rank event is a projection of this value, never a separate opinion.
 */
export const projectRank = (
  awards: ReadonlyArray<XpAwardRecord>,
  input: {
    readonly earnerPubkey: string;
    readonly scorerPubkeys: ReadonlyArray<string>;
  },
): XpRankProjection => {
  const contributing = selectContributingAwards(awards, input);
  const totalPoints = contributing.reduce((sum, a) => sum + a.points, 0);
  const level = levelForPoints(totalPoints);
  const digest = awardStreamDigest(contributing);

  return decodeRank({
    schema: "openagents.sarah.xp_rank.v1",
    algorithmId: XP_ALGORITHM_ID,
    earnerPubkey: input.earnerPubkey,
    totalPoints,
    level: level.level,
    levelId: level.id,
    awardCount: contributing.length,
    ...(digest !== undefined ? { awardStreamDigest: digest } : {}),
  });
};

/**
 * If the projection and the awards disagree, the awards win.
 * Returns true only when rank matches a recompute from awards.
 */
export const rankAgreesWithAwards = (
  rank: XpRankProjection,
  awards: ReadonlyArray<XpAwardRecord>,
  scorerPubkeys: ReadonlyArray<string>,
): boolean => {
  const recomputed = projectRank(awards, {
    earnerPubkey: rank.earnerPubkey,
    scorerPubkeys,
  });
  return (
    recomputed.totalPoints === rank.totalPoints &&
    recomputed.level === rank.level &&
    recomputed.awardCount === rank.awardCount &&
    recomputed.algorithmId === rank.algorithmId
  );
};

/**
 * Prefer the award stream when a published rank disagrees.
 * Always returns the recomputed projection.
 */
export const resolveRankFromAwards = (
  awards: ReadonlyArray<XpAwardRecord>,
  input: {
    readonly earnerPubkey: string;
    readonly scorerPubkeys: ReadonlyArray<string>;
    readonly publishedRank?: XpRankProjection;
  },
): {
  readonly projection: XpRankProjection;
  readonly usedAwards: true;
  readonly publishedDisagreed: boolean;
} => {
  const projection = projectRank(awards, input);
  const publishedDisagreed =
    input.publishedRank !== undefined &&
    !rankAgreesWithAwards(input.publishedRank, awards, input.scorerPubkeys);
  return { projection, usedAwards: true, publishedDisagreed };
};

/**
 * Build an unsigned NIP-85 kind-30382 rank assertion template.
 * Only OpenAgents scorer keys may sign and publish this.
 */
export const buildXpRankTemplate = (input: {
  readonly projection: XpRankProjection;
  readonly createdAt?: number;
}): SarahNostrEventTemplate => {
  const p = input.projection;
  if (p.algorithmId !== XP_ALGORITHM_ID) {
    throw new XpRankError("bad_algorithm", "rank algorithm must be openagents.xp.v1");
  }
  if (!HEX64.test(p.earnerPubkey)) {
    throw new XpRankError("invalid_earner", "earnerPubkey must be 64 lowercase hex");
  }

  const tags: string[][] = [
    ["d", p.earnerPubkey],
    ["p", p.earnerPubkey],
    ["rank", String(p.totalPoints)],
    ["level", String(p.level)],
    ["level_id", p.levelId],
    ["award_count", String(p.awardCount)],
    ["algorithm", XP_ALGORITHM_ID],
  ];
  if (p.awardStreamDigest) {
    tags.push(["award_stream_digest", p.awardStreamDigest]);
  }

  const template: SarahNostrEventTemplate = {
    kind: XP_RANK_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags,
    content: JSON.stringify(p),
  };
  assertSarahNostrPublicSafe(template);
  return template;
};

/**
 * Parse a NIP-85 rank event into a projection.
 * Does not trust the event over the award stream — call rankAgreesWithAwards.
 */
export const parseXpRankEvent = (event: {
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content?: string;
}): XpRankProjection | null => {
  if (event.kind !== XP_RANK_KIND) return null;

  const tag = (name: string): string | undefined =>
    event.tags.find((t) => t[0] === name)?.[1];

  const earnerPubkey = tag("d") ?? tag("p");
  const rank = tag("rank");
  const level = tag("level");
  const levelId = tag("level_id");
  const awardCount = tag("award_count");
  const algorithm = tag("algorithm");

  if (!earnerPubkey || !HEX64.test(earnerPubkey)) return null;
  if (algorithm !== undefined && algorithm !== XP_ALGORITHM_ID) return null;
  if (!rank || !/^(0|[1-9][0-9]*)$/.test(rank)) return null;
  if (!level || !/^(0|[1-9][0-9]*)$/.test(level)) return null;
  if (!awardCount || !/^(0|[1-9][0-9]*)$/.test(awardCount)) return null;

  const totalPoints = Number(rank);
  const levelNum = Number(level);
  const expected = levelForPoints(totalPoints);
  const resolvedLevelId = levelId ?? expected.id;

  try {
    return decodeRank({
      schema: "openagents.sarah.xp_rank.v1",
      algorithmId: XP_ALGORITHM_ID,
      earnerPubkey,
      totalPoints,
      level: levelNum,
      levelId: resolvedLevelId,
      awardCount: Number(awardCount),
      ...(tag("award_stream_digest")
        ? { awardStreamDigest: tag("award_stream_digest") }
        : {}),
    });
  } catch {
    return null;
  }
};

/** Re-export for callers that need the fixed table without importing types. */
export const fixedPointsFor = pointsForAwardKind;
