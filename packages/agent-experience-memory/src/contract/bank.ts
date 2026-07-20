import { Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import { ExperienceRecord } from "./experience.js";
import { GlobalPattern } from "./pattern.js";
import { BankId, MemoryTimestamp, OwnerScopeId, ProjectScopeId, Sha256Hex } from "./refs.js";

/**
 * The frozen eligible bank — the immutable snapshot recall reads for one turn.
 *
 * AFS-10 freezes exactly one eligible bank at turn or run start. The bank is a
 * value: once frozen it never changes, so a write during the current turn cannot
 * change the current turn's input. The `bankDigest` binds the snapshot, and the
 * turn's `effectiveAdaptationDigest` is derived from it, so the exact recall
 * substrate for a turn is auditable after the fact.
 */
export const EXPERIENCE_BANK_SCHEMA_LITERAL = "openagents.experience_bank.v1" as const;

export const ExperienceBank = S.Struct({
  schema: S.Literal(EXPERIENCE_BANK_SCHEMA_LITERAL),
  bankId: BankId,
  ownerScope: OwnerScopeId,
  projectScope: ProjectScopeId,
  frozenAt: MemoryTimestamp,
  records: S.Array(ExperienceRecord),
  patterns: S.Array(GlobalPattern),
  bankDigest: Sha256Hex,
});
export type ExperienceBank = typeof ExperienceBank.Type;

const decodeBank = S.decodeUnknownSync(ExperienceBank);
const decodeDigest = S.decodeUnknownSync(Sha256Hex);

/**
 * The content address of a bank's records, patterns, and freeze time. The digest
 * covers the exact eligible content, so a stale or altered bank is detectable.
 */
export const bankDigestOf = (args: {
  readonly frozenAt: string;
  readonly records: ReadonlyArray<ExperienceRecord>;
  readonly patterns: ReadonlyArray<GlobalPattern>;
}): Sha256Hex =>
  decodeDigest(
    sha256Hex(
      canonicalStringify({
        frozenAt: args.frozenAt,
        records: args.records.map((record) => record.digest),
        patterns: args.patterns.map((pattern) => pattern.digest),
      }),
    ),
  );

/** Freeze the eligible records and patterns for one owner+project scope into a bank. */
export const freezeExperienceBank = (args: {
  readonly bankId: typeof BankId.Type;
  readonly ownerScope: typeof OwnerScopeId.Type;
  readonly projectScope: typeof ProjectScopeId.Type;
  readonly frozenAt: string;
  readonly records: ReadonlyArray<ExperienceRecord>;
  readonly patterns: ReadonlyArray<GlobalPattern>;
}): ExperienceBank =>
  decodeBank({
    schema: EXPERIENCE_BANK_SCHEMA_LITERAL,
    bankId: args.bankId,
    ownerScope: args.ownerScope,
    projectScope: args.projectScope,
    frozenAt: args.frozenAt,
    records: args.records,
    patterns: args.patterns,
    bankDigest: bankDigestOf({
      frozenAt: args.frozenAt,
      records: args.records,
      patterns: args.patterns,
    }),
  });

/**
 * Decode an unknown bank blob from local storage, and prove its digest.
 *
 * A corrupt bank must fail closed. If the bytes do not decode, or the recomputed
 * digest does not match the stored one, this throws; the caller degrades to
 * no-memory rather than trusting altered content.
 */
export const decodeVerifiedBank = (value: unknown): ExperienceBank => {
  const bank = decodeBank(value);
  const recomputed = bankDigestOf({
    frozenAt: bank.frozenAt,
    records: bank.records,
    patterns: bank.patterns,
  });
  if (recomputed !== bank.bankDigest) {
    throw new Error("experience bank digest mismatch: the stored bank is corrupt or altered");
  }
  return bank;
};

/** An empty, valid bank for a scope. Recall over it returns nothing. */
export const emptyExperienceBank = (args: {
  readonly bankId: typeof BankId.Type;
  readonly ownerScope: typeof OwnerScopeId.Type;
  readonly projectScope: typeof ProjectScopeId.Type;
  readonly frozenAt: string;
}): ExperienceBank =>
  freezeExperienceBank({ ...args, records: [], patterns: [] });
