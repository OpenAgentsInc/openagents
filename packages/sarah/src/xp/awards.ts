import { Schema as S } from "effect";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import {
  XP_ALGORITHM_ID,
  XP_AWARD_KIND,
  XP_AWARD_KINDS,
  XP_FORBIDDEN_COPY_TERMS,
  XP_NAMESPACE,
  XP_POINT_TABLE,
  XpAwardRecordSchema,
  type XpAwardKind,
  type XpAwardRecord,
  type XpWorkUnitTier,
} from "./types.ts";

const decodeAwardRecord = S.decodeUnknownSync(XpAwardRecordSchema);
const HEX64 = /^[0-9a-f]{64}$/;

export class XpAwardError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "XpAwardError";
    this.code = code;
  }
}

const fail = (code: string, message: string): never => {
  throw new XpAwardError(code, message);
};

/** Fixed points for a known award kind. */
export const pointsForAwardKind = (kind: XpAwardKind): number => {
  const points = XP_POINT_TABLE[kind];
  if (points === undefined || !Number.isInteger(points) || points <= 0) {
    fail("unknown_award_kind", `no fixed points for award kind ${kind}`);
  }
  return points;
};

export const isXpAwardKind = (value: string): value is XpAwardKind =>
  (XP_AWARD_KINDS as ReadonlyArray<string>).includes(value);

/** Reject public-safe XP copy that names experience as earnings or money. */
export const assertXpCopyPublicSafe = (value: string, field = "copy"): void => {
  const lower = value.toLowerCase();
  for (const term of XP_FORBIDDEN_COPY_TERMS) {
    if (lower.includes(term)) {
      fail(
        "forbidden_xp_copy",
        `forbidden_xp_copy: ${field} must not call experience money (found "${term}"); experience is not currency`,
      );
    }
  }
};

const tierFromAwardKind = (kind: XpAwardKind): XpWorkUnitTier | undefined => {
  if (kind === "accepted_work_unit.tier_1") return 1;
  if (kind === "accepted_work_unit.tier_2") return 2;
  if (kind === "accepted_work_unit.tier_3") return 3;
  return undefined;
};

const awardKindFromTier = (tier: XpWorkUnitTier): XpAwardKind => {
  if (tier === 1) return "accepted_work_unit.tier_1";
  if (tier === 2) return "accepted_work_unit.tier_2";
  return "accepted_work_unit.tier_3";
};

export type BuildXpAwardInput = {
  readonly awardKind: XpAwardKind;
  readonly earnerPubkey: string;
  readonly workEventId: string;
  readonly receiptRef: string;
  /** Optional relay hint for the work event. */
  readonly workRelay?: string;
  readonly jobType?: string;
  readonly createdAt?: number;
  /** Optional public-safe note (never payment language). */
  readonly note?: string;
};

/**
 * Build an unsigned NIP-32 kind-1985 award label template.
 *
 * Rules:
 * - namespace `com.openagents.xp`
 * - cites accepted work event and receipt
 * - carries fixed points and algorithm id
 * - does not transfer value; settlement is deferred
 */
export const buildXpAwardTemplate = (
  input: BuildXpAwardInput,
): {
  readonly template: SarahNostrEventTemplate;
  readonly record: XpAwardRecord;
} => {
  if (!isXpAwardKind(input.awardKind)) {
    fail("unknown_award_kind", `unknown award kind: ${input.awardKind}`);
  }
  if (!HEX64.test(input.earnerPubkey)) {
    fail("invalid_earner", "earnerPubkey must be 64 lowercase hex");
  }
  if (!HEX64.test(input.workEventId)) {
    fail("invalid_work_event", "workEventId must be 64 lowercase hex");
  }
  if (!input.receiptRef || input.receiptRef.trim().length === 0) {
    fail("missing_receipt", "every award must cite an acceptance receipt");
  }
  if (input.receiptRef.length > 256) {
    fail("invalid_receipt", "receiptRef exceeds 256 characters");
  }
  if (input.note !== undefined) {
    assertXpCopyPublicSafe(input.note, "note");
  }
  if (input.awardKind === "first_job_type") {
    if (!input.jobType || input.jobType.trim().length === 0) {
      fail("missing_job_type", "first_job_type requires jobType");
    }
  }

  const points = pointsForAwardKind(input.awardKind);
  const tier = tierFromAwardKind(input.awardKind);
  const created_at = input.createdAt ?? Math.floor(Date.now() / 1000);

  const tags: string[][] = [
    ["L", XP_NAMESPACE],
    ["l", input.awardKind, XP_NAMESPACE],
    ["p", input.earnerPubkey],
    input.workRelay
      ? ["e", input.workEventId, input.workRelay, "work"]
      : ["e", input.workEventId, "", "work"],
    ["receipt", input.receiptRef],
    ["points", String(points)],
    ["algorithm", XP_ALGORITHM_ID],
  ];
  if (tier !== undefined) {
    tags.push(["tier", String(tier)]);
  }
  if (input.jobType !== undefined && input.jobType.length > 0) {
    tags.push(["job_type", input.jobType]);
  }

  const content =
    input.note !== undefined
      ? JSON.stringify({ note: input.note, algorithmId: XP_ALGORITHM_ID })
      : "";

  const template: SarahNostrEventTemplate = {
    kind: XP_AWARD_KIND,
    created_at,
    tags,
    content,
  };
  assertSarahNostrPublicSafe(template);
  assertXpCopyPublicSafe(JSON.stringify(template), "award_template");

  // scorerPubkey is filled by the caller at sign time; use placeholder zeros
  // only for the pre-sign record shape when absent — callers should pass the
  // signed event through parseXpAwardEvent for the authoritative record.
  const record = decodeAwardRecord({
    awardKind: input.awardKind,
    points,
    earnerPubkey: input.earnerPubkey,
    workEventId: input.workEventId,
    receiptRef: input.receiptRef,
    scorerPubkey: "0".repeat(64),
    ...(tier !== undefined ? { tier } : {}),
    ...(input.jobType !== undefined ? { jobType: input.jobType } : {}),
    createdAt: created_at,
  });

  return { template, record };
};

/** Convenience: work-unit award from tier set before any quote arrives. */
export const buildWorkUnitAwardTemplate = (
  input: Omit<BuildXpAwardInput, "awardKind"> & {
    readonly tier: XpWorkUnitTier;
  },
) =>
  buildXpAwardTemplate({
    ...input,
    awardKind: awardKindFromTier(input.tier),
  });

const tagValue = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): string | undefined => {
  const row = tags.find((t) => t[0] === name);
  return row?.[1];
};

const tagValues = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
  name: string,
): ReadonlyArray<ReadonlyArray<string>> => tags.filter((t) => t[0] === name);

/**
 * Parse a NIP-32 award event into an XpAwardRecord.
 * Returns null when the event is not an OpenAgents XP award.
 */
export const parseXpAwardEvent = (event: {
  readonly id?: string;
  readonly pubkey: string;
  readonly kind: number;
  readonly created_at?: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content?: string;
}): XpAwardRecord | null => {
  if (event.kind !== XP_AWARD_KIND) return null;
  if (!HEX64.test(event.pubkey)) return null;

  const namespace = tagValue(event.tags, "L");
  if (namespace !== XP_NAMESPACE) return null;

  const labelRows = tagValues(event.tags, "l");
  const labelRow = labelRows.find(
    (row) => row[2] === XP_NAMESPACE || row[2] === undefined,
  );
  const awardKind = labelRow?.[1];
  if (!awardKind || !isXpAwardKind(awardKind)) return null;

  const earnerPubkey = tagValue(event.tags, "p");
  if (!earnerPubkey || !HEX64.test(earnerPubkey)) return null;

  const workRow = tagValues(event.tags, "e").find(
    (row) => row[3] === "work" || row[3] === undefined,
  );
  const workEventId = workRow?.[1];
  if (!workEventId || !HEX64.test(workEventId)) return null;

  const receiptRef = tagValue(event.tags, "receipt");
  if (!receiptRef || receiptRef.length === 0) return null;

  const pointsTag = tagValue(event.tags, "points");
  const expectedPoints = pointsForAwardKind(awardKind);
  if (pointsTag !== undefined && pointsTag !== String(expectedPoints)) {
    // Fixed table wins: reject self-inflated points tags.
    return null;
  }

  const algorithm = tagValue(event.tags, "algorithm");
  if (algorithm !== undefined && algorithm !== XP_ALGORITHM_ID) return null;

  if (event.content && event.content.length > 0) {
    try {
      assertXpCopyPublicSafe(event.content, "award_content");
    } catch {
      return null;
    }
  }

  const tierTag = tagValue(event.tags, "tier");
  const expectedTier = tierFromAwardKind(awardKind);
  let tier: XpWorkUnitTier | undefined;
  if (expectedTier !== undefined) {
    if (tierTag !== undefined && tierTag !== String(expectedTier)) return null;
    tier = expectedTier;
  }

  const jobType = tagValue(event.tags, "job_type");
  if (awardKind === "first_job_type" && (!jobType || jobType.length === 0)) {
    return null;
  }

  try {
    return decodeAwardRecord({
      awardKind,
      points: expectedPoints,
      earnerPubkey,
      workEventId,
      receiptRef,
      scorerPubkey: event.pubkey,
      ...(event.id && HEX64.test(event.id) ? { awardEventId: event.id } : {}),
      ...(tier !== undefined ? { tier } : {}),
      ...(jobType !== undefined ? { jobType } : {}),
      ...(event.created_at !== undefined
        ? { createdAt: event.created_at }
        : {}),
    });
  } catch {
    return null;
  }
};

/**
 * Validate an award against scorer keys and structural rules.
 * Member self-labels never enter the score.
 */
export const isValidXpAward = (
  award: XpAwardRecord,
  scorerPubkeys: ReadonlyArray<string>,
): boolean => {
  if (!scorerPubkeys.includes(award.scorerPubkey)) return false;
  if (!isXpAwardKind(award.awardKind)) return false;
  if (award.points !== pointsForAwardKind(award.awardKind)) return false;
  if (!HEX64.test(award.earnerPubkey)) return false;
  if (!HEX64.test(award.workEventId)) return false;
  if (!award.receiptRef || award.receiptRef.length === 0) return false;
  if (award.awardKind === "first_job_type" && !award.jobType) return false;
  const expectedTier = tierFromAwardKind(award.awardKind);
  if (expectedTier !== undefined && award.tier !== expectedTier) return false;
  return true;
};
