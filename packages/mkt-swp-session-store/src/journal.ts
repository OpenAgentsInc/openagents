/**
 * Journaled, digest-verified record writes over the `StringKv` port.
 *
 * Durability argument (issue #9320 "justify durability"): we assume the
 * backend can tear ANY single write (crash mid-serialisation, quota abort,
 * power loss). Two mechanisms make that safe:
 *
 * 1. Every committed value is an envelope whose SHA-256 digest covers the
 *    schema version, the write sequence, and the canonical payload. A
 *    partially written string cannot verify, so a torn record is
 *    detectable and is NEVER surfaced as a loadable record.
 * 2. Writes are two-phase: the full new envelope is written to a staging
 *    key first, then to the base key, then the staging key is deleted.
 *    Whatever single write the crash tears, at least one complete committed
 *    envelope (old or new) survives, and recovery rolls forward or falls
 *    back deterministically:
 *
 *    crash point            | base key      | staging key   | recovery
 *    -----------------------|---------------|---------------|-------------------
 *    during staging write   | old, verifies | torn          | keep old, drop staging
 *    between stage & base   | old, verifies | new, verifies | roll forward to new
 *    during base write      | torn          | new, verifies | roll forward to new
 *    after base, pre-delete | new, verifies | new (stale)   | keep base, drop staging
 *
 *    Only if BOTH copies are torn — impossible under a single-crash model,
 *    reachable only through external corruption — does the load refuse
 *    loudly with `TornSessionRecordError` instead of guessing.
 *
 * This mirrors the temp-file-plus-rename protocol of the Rust lab harness
 * (immortal `crates/immortal-lab/src/state.rs`) rebuilt for backends that
 * have no atomic rename.
 */
import { Effect } from "effect";

import { canonicalJson, contentDigestHex } from "./canonical.js";
import { StorageDriverError, TornSessionRecordError } from "./errors.js";
import type { StringKv } from "./kv.js";

export interface RecordEnvelope {
  readonly schemaVersion: number;
  /** Monotonic per-key write counter; higher wins during roll-forward. */
  readonly writeSeq: number;
  readonly payload: unknown;
  /** SHA-256 over canonical `{schemaVersion, writeSeq, payload}`. */
  readonly digestHex: string;
}

const envelopeDigest = (schemaVersion: number, writeSeq: number, payload: unknown): string =>
  contentDigestHex({ payload, schemaVersion, writeSeq });

export const sealEnvelope = (
  schemaVersion: number,
  writeSeq: number,
  payload: unknown,
): RecordEnvelope => ({
  schemaVersion,
  writeSeq,
  payload,
  digestHex: envelopeDigest(schemaVersion, writeSeq, payload),
});

export const serializeEnvelope = (envelope: RecordEnvelope): string =>
  canonicalJson({
    digestHex: envelope.digestHex,
    payload: envelope.payload,
    schemaVersion: envelope.schemaVersion,
    writeSeq: envelope.writeSeq,
  });

/** Parse and verify one stored string. Returns null for a torn/invalid value. */
export const parseEnvelope = (raw: string): RecordEnvelope | null => {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return null;
  const candidate = parsed as Record<string, unknown>;
  const { schemaVersion, writeSeq, digestHex } = candidate;
  if (
    typeof schemaVersion !== "number" ||
    typeof writeSeq !== "number" ||
    typeof digestHex !== "string" ||
    !("payload" in candidate)
  ) {
    return null;
  }
  let expected: string;
  try {
    expected = envelopeDigest(schemaVersion, writeSeq, candidate.payload);
  } catch {
    return null;
  }
  if (expected !== digestHex) return null;
  return { schemaVersion, writeSeq, payload: candidate.payload, digestHex };
};

export const stagingKeyOf = (key: string): string => `${key}!next`;

export type JournalError = StorageDriverError | TornSessionRecordError;

/**
 * Commit an envelope with the two-phase protocol described above.
 */
export const commitRecord = (
  kv: StringKv,
  key: string,
  envelope: RecordEnvelope,
): Effect.Effect<void, StorageDriverError> =>
  Effect.gen(function* () {
    const serialized = serializeEnvelope(envelope);
    yield* kv.set(stagingKeyOf(key), serialized);
    yield* kv.set(key, serialized);
    yield* kv.delete(stagingKeyOf(key));
  });

/**
 * Load a record with crash recovery. Returns the committed envelope, or null
 * when the key was never written. A torn base with a verifying staged copy
 * rolls forward; both-torn refuses loudly.
 */
export const loadRecord = (
  kv: StringKv,
  key: string,
): Effect.Effect<RecordEnvelope | null, JournalError> =>
  Effect.gen(function* () {
    const baseRaw = yield* kv.get(key);
    const stagedRaw = yield* kv.get(stagingKeyOf(key));
    const base = baseRaw === null ? null : parseEnvelope(baseRaw);
    const staged = stagedRaw === null ? null : parseEnvelope(stagedRaw);

    if (base !== null) {
      if (staged !== null && staged.writeSeq > base.writeSeq) {
        // Crash between staging and base write: roll forward.
        yield* commitRecord(kv, key, staged);
        return staged;
      }
      if (stagedRaw !== null) yield* kv.delete(stagingKeyOf(key));
      return base;
    }
    if (staged !== null) {
      // Base torn or absent mid-commit; the staged copy is complete.
      yield* commitRecord(kv, key, staged);
      return staged;
    }
    if (baseRaw === null) {
      // Never committed. A torn staging remnant (crash during the very
      // first staging write) is dropped: no complete record ever existed.
      if (stagedRaw !== null) yield* kv.delete(stagingKeyOf(key));
      return null;
    }
    // A base record exists but verifies as torn, with no staged recovery
    // copy: external corruption. Refuse loudly rather than guess.
    return yield* new TornSessionRecordError({ key, reason: "digest_mismatch" });
  });

/** Delete a record and any staged copy. */
export const deleteRecord = (
  kv: StringKv,
  key: string,
): Effect.Effect<void, StorageDriverError> =>
  Effect.gen(function* () {
    yield* kv.delete(stagingKeyOf(key));
    yield* kv.delete(key);
  });
