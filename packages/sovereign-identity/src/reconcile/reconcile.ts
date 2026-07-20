/**
 * IDR-04 reconciliation engine.
 *
 * `reconcileIdentities` proves a decoded candidate is the RIGHT identity BEFORE
 * it is ever imported. It derives each candidate's PUBLIC identity — the NIP-06
 * `npub` (via the IDR-06 `IdentityKeys` path) and the per-adapter Spark
 * fingerprints — inside the bounded secret scope, groups duplicate locations,
 * and compares the single distinct identity against the public local records. It
 * returns a typed `ReconciliationResult`. It NEVER imports, creates, or writes;
 * import is IDR-05.
 *
 * The classification follows the audit acceptance matrix:
 *
 * - Duplicate locations that derive the SAME `npub` collapse into one identity.
 * - Two or more DIFFERENT valid phrases → `owner_selection_required` (never
 *   auto-picked).
 * - One identity, Nostr match + Spark match → `confirmed`.
 * - Nostr match, Spark mismatch → `nostr_matches_spark_mismatch`.
 * - Spark match, Nostr mismatch → `spark_matches_nostr_mismatch`.
 * - No public source, or no field matched → `no_public_match`.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect, Result } from "effect";
import { Npub, type SparkAdapterFingerprint, type SparkAdapterKind } from "../contract/index.ts";
import type { DecodedCandidate } from "../decode/index.ts";
import { deriveLocalNostrIdentity } from "../machinery/local-signer.ts";
import type { SparkComparisonAdapter } from "./spark-adapter.ts";
import {
  type CandidatePublicIdentity,
  type ExpectedPublicIdentity,
  type GroupedIdentity,
  NoPublicMatch,
  NostrMatchesSparkMismatch,
  OwnerSelectionRequired,
  ReconciliationConfirmed,
  SparkMatchesNostrMismatch,
} from "./result.ts";

/** The reconciliation input. */
export interface ReconcileInput {
  /** The decoded candidates. Owner-attended (secret-less) candidates are skipped. */
  readonly candidates: ReadonlyArray<DecodedCandidate>;
  /** The public expected identity, or `null` when no comparison source exists. */
  readonly expected: ExpectedPublicIdentity | null;
  /** The Spark comparison adapters. Deferred adapters are recorded, not compared. */
  readonly adapters: ReadonlyArray<SparkComparisonAdapter>;
}

/**
 * Derive one candidate's PUBLIC identity inside the bounded secret scope. The
 * Nostr `npub` uses the IDR-06 `IdentityKeys` path; each exact Spark adapter adds
 * its public fingerprint; a deferred adapter is recorded, not compared. Returns
 * `null` for an owner-attended (secret-less) candidate, which cannot reconcile
 * offline. The mnemonic never leaves the `use` scope.
 */
export const deriveCandidatePublicIdentity = Effect.fn(
  "SovereignIdentity.deriveCandidatePublicIdentity",
)(function* (
  candidate: DecodedCandidate,
  adapters: ReadonlyArray<SparkComparisonAdapter>,
) {
  const secret = candidate.secret;
  if (secret === null) return null;

  // Nostr npub via the IDR-06 IdentityKeys path, inside the bounded scope.
  const nostr = yield* secret.use((mnemonic) =>
    Effect.sync(() => {
      const identity = deriveLocalNostrIdentity(mnemonic);
      return { npub: identity.npub, publicKey: identity.publicKey };
    }),
  );

  const sparkFingerprints: Array<SparkAdapterFingerprint> = [];
  const deferredAdapters: Array<SparkAdapterKind> = [];
  for (const adapter of adapters) {
    const outcome = yield* Effect.result(adapter.deriveFingerprint(secret));
    if (Result.isSuccess(outcome)) {
      sparkFingerprints.push(outcome.success);
    } else if (outcome.failure.reason === "deferred") {
      deferredAdapters.push(adapter.kind);
    } else {
      // A real derivation failure is surfaced, never swallowed.
      return yield* Effect.fail(outcome.failure);
    }
  }

  return {
    sourceLabel: candidate.result.sourcePathLabel,
    npub: Npub.make(nostr.npub),
    nostrPublicKeyHex: nostr.publicKey,
    sparkFingerprints,
    deferredAdapters,
  };
});

/** Group derived candidate identities by `npub`. Duplicate locations collapse. */
const groupByNpub = (
  identities: ReadonlyArray<CandidatePublicIdentity>,
): ReadonlyArray<GroupedIdentity> => {
  const byNpub = new Map<
    string,
    {
      npub: Npub;
      nostrPublicKeyHex: string;
      sparkFingerprints: ReadonlyArray<SparkAdapterFingerprint>;
      sourceLabels: Array<string>;
    }
  >();
  for (const identity of identities) {
    const existing = byNpub.get(identity.npub);
    if (existing) {
      existing.sourceLabels.push(identity.sourceLabel);
    } else {
      byNpub.set(identity.npub, {
        npub: identity.npub,
        nostrPublicKeyHex: identity.nostrPublicKeyHex,
        sparkFingerprints: identity.sparkFingerprints,
        sourceLabels: [identity.sourceLabel],
      });
    }
  }
  return [...byNpub.values()]
    .map((group) => ({ ...group, sourceLabels: [...group.sourceLabels].sort() }))
    .sort((left, right) => left.npub.localeCompare(right.npub));
};

/** Whether a grouped identity carries every expected Spark fingerprint. */
const sparkMatches = (
  group: GroupedIdentity,
  expected: ExpectedPublicIdentity,
): boolean =>
  expected.sparkFingerprints.every((expectedFingerprint) =>
    group.sparkFingerprints.some(
      (candidateFingerprint) =>
        candidateFingerprint.adapter === expectedFingerprint.adapter &&
        candidateFingerprint.fingerprintHex === expectedFingerprint.fingerprintHex,
    ),
  );

/**
 * Reconcile decoded candidates against the public local records. Returns a typed
 * `ReconciliationResult`. It never imports, creates, or writes.
 */
export const reconcileIdentities = Effect.fn("SovereignIdentity.reconcileIdentities")(function* (
  input: ReconcileInput,
) {
  const derived: Array<CandidatePublicIdentity> = [];
  for (const candidate of input.candidates) {
    const identity = yield* deriveCandidatePublicIdentity(candidate, input.adapters);
    if (identity !== null) derived.push(identity);
  }

  const groups = groupByNpub(derived);

  // No usable candidate, or more than one distinct valid identity: stop.
  if (groups.length === 0) {
    return NoPublicMatch.make({ candidates: [] });
  }
  if (groups.length > 1) {
    return OwnerSelectionRequired.make({ candidates: groups });
  }

  const group = groups[0]!;
  const expected = input.expected;
  if (expected === null) {
    return NoPublicMatch.make({ candidates: groups });
  }

  const hasNostrExpectation = expected.npub !== undefined;
  const hasSparkExpectation = expected.sparkFingerprints.length > 0;
  const nostrMatch = hasNostrExpectation && group.npub === expected.npub;
  const sparkMatch = hasSparkExpectation && sparkMatches(group, expected);

  if (hasNostrExpectation && hasSparkExpectation) {
    if (nostrMatch && sparkMatch) {
      return ReconciliationConfirmed.make({ identity: group });
    }
    if (nostrMatch && !sparkMatch) {
      return NostrMatchesSparkMismatch.make({ identity: group, expected });
    }
    if (!nostrMatch && sparkMatch) {
      return SparkMatchesNostrMismatch.make({ identity: group, expected });
    }
    return NoPublicMatch.make({ candidates: groups });
  }

  // Only one comparison source exists: a single match confirms; otherwise stop.
  if (nostrMatch || sparkMatch) {
    return ReconciliationConfirmed.make({ identity: group });
  }
  return NoPublicMatch.make({ candidates: groups });
});
