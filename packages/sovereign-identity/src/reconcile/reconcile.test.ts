import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import { mnemonicToSeedSync } from "@scure/bip39";
import { Effect, Result } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { deriveSovereignIdentityPublic } from "../contract/derivation.ts";
import { Npub, type SparkAdapterFingerprint } from "../contract/index.ts";
import { PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE, PUBLIC_TEST_MNEMONIC } from "../contract/vectors.ts";
import { RecoveredSecret } from "../decode/boundary.ts";
import type { DecodedCandidate } from "../decode/result.ts";
import { deriveLocalNostrIdentity } from "../machinery/local-signer.ts";
import { deriveCandidatePublicIdentity, reconcileIdentities } from "./reconcile.ts";
import type { ExpectedPublicIdentity, ReconciliationResult } from "./result.ts";
import { isReconciliationConflict } from "./result.ts";
import {
  type BreezSeedFingerprintDeriver,
  ldkSparkComparisonAdapter,
  makeBreezSparkComparisonAdapter,
  rustSparkComparisonAdapter,
} from "./spark-adapter.ts";

/**
 * IDR-04 reconciliation tests.
 *
 * SAFETY: every candidate is built from a PUBLIC, published BIP-39 TEST
 * mnemonic — never a real secret. The tests assert the reconciliation result
 * NEVER carries any mnemonic word or seed byte (the secret-logging tripwire).
 */

/** A second published BIP-39 TEST mnemonic (all-ones entropy). NOT a real secret. */
const SECOND_TEST_MNEMONIC = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

/** Build a decoded plain-mnemonic candidate for a public test mnemonic. */
const candidateFrom = (mnemonic: string, sourceLabel: string): DecodedCandidate => ({
  result: {
    format: "plain_mnemonic_file",
    formatLabel: "Plain BIP-39 mnemonic file",
    formatVersion: "1",
    sourcePathLabel: sourceLabel,
    status: "decoded",
    decoded: true,
    publicIdentity: null,
  },
  secret: new RecoveredSecret(mnemonic, "plain_mnemonic_file", "1"),
});

/** A deterministic OFFLINE stand-in for the bounded Breez SDK deriver. */
const testBreezDeriver: BreezSeedFingerprintDeriver = {
  deriveSeedFingerprint: (seed) =>
    Effect.succeed({
      fingerprintHex: bytesToHex(sha256(seed).slice(0, 4)),
      publicKeyHex: bytesToHex(sha256(seed)),
    }),
};
const breezAdapter = makeBreezSparkComparisonAdapter(testBreezDeriver);

const allAdapters = [rustSparkComparisonAdapter, breezAdapter, ldkSparkComparisonAdapter] as const;

// The frozen public identity of PUBLIC_TEST_MNEMONIC, re-derived here.
const reference = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
const referenceNpub = deriveLocalNostrIdentity(PUBLIC_TEST_MNEMONIC).npub;

const rustFingerprint: SparkAdapterFingerprint = {
  adapter: "rust_spark",
  fingerprintHex: reference.sparkBip32FingerprintHex,
  publicKeyHex: reference.sparkPublicKeyHex,
};

const referenceSeed = mnemonicToSeedSync(PUBLIC_TEST_MNEMONIC, "");
const breezFingerprint: SparkAdapterFingerprint = {
  adapter: "breez_spark",
  fingerprintHex: bytesToHex(sha256(referenceSeed).slice(0, 4)),
  publicKeyHex: bytesToHex(sha256(referenceSeed)),
};

const expectedFull: ExpectedPublicIdentity = {
  label: "public local records",
  npub: Npub.make(referenceNpub),
  sparkFingerprints: [rustFingerprint, breezFingerprint],
};

describe("IDR-04 reconciliation classifies each mismatch class", () => {
  test("clean Nostr + Spark match reconciles to a single confirmed identity", () => {
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "primary historical candidate")],
        expected: expectedFull,
        adapters: allAdapters,
      }),
    );
    expect(result._tag).toBe("confirmed");
    if (result._tag !== "confirmed") throw new Error("unreachable");
    expect(result.identity.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub);
    expect(result.identity.npub).toBe(referenceNpub);
    expect(result.identity.sourceLabels).toEqual(["primary historical candidate"]);
    expect(isReconciliationConflict(result)).toBe(false);
  });

  test("duplicate locations for one identity group and select a single identity", () => {
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [
          candidateFrom(PUBLIC_TEST_MNEMONIC, "location-b"),
          candidateFrom(PUBLIC_TEST_MNEMONIC, "location-a"),
        ],
        expected: expectedFull,
        adapters: allAdapters,
      }),
    );
    expect(result._tag).toBe("confirmed");
    if (result._tag !== "confirmed") throw new Error("unreachable");
    // Two locations, ONE identity, source labels grouped and sorted.
    expect(result.identity.sourceLabels).toEqual(["location-a", "location-b"]);
  });

  test("two different valid phrases stop for owner selection and are never auto-picked", () => {
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [
          candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate-1"),
          candidateFrom(SECOND_TEST_MNEMONIC, "candidate-2"),
        ],
        expected: expectedFull,
        adapters: allAdapters,
      }),
    );
    expect(result._tag).toBe("owner_selection_required");
    if (result._tag !== "owner_selection_required") throw new Error("unreachable");
    // Both distinct identities are surfaced; neither is chosen.
    expect(result.candidates.length).toBe(2);
    const npubs = result.candidates.map((candidate) => candidate.npub);
    expect(npubs).toContain(referenceNpub);
    expect(isReconciliationConflict(result)).toBe(true);
  });

  test("Nostr matches but Spark does not → nostr_matches_spark_mismatch", () => {
    const wrongSpark: ExpectedPublicIdentity = {
      label: "public local records",
      npub: Npub.make(referenceNpub),
      sparkFingerprints: [{ adapter: "rust_spark", fingerprintHex: "deadbeef" }],
    };
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate")],
        expected: wrongSpark,
        adapters: [rustSparkComparisonAdapter],
      }),
    );
    expect(result._tag).toBe("nostr_matches_spark_mismatch");
    expect(isReconciliationConflict(result)).toBe(true);
  });

  test("a mismatched Breez profile alone (Rust still matches) reports a Spark mismatch", () => {
    // Rust matches, Breez does not: sparkMatch requires EVERY expected
    // fingerprint, so the Rust-versus-Breez independence is honored.
    const mismatchedBreez: ExpectedPublicIdentity = {
      label: "public local records",
      npub: Npub.make(referenceNpub),
      sparkFingerprints: [rustFingerprint, { adapter: "breez_spark", fingerprintHex: "00000000" }],
    };
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate")],
        expected: mismatchedBreez,
        adapters: allAdapters,
      }),
    );
    expect(result._tag).toBe("nostr_matches_spark_mismatch");
  });

  test("Spark matches but Nostr does not → spark_matches_nostr_mismatch", () => {
    const wrongNostr: ExpectedPublicIdentity = {
      label: "public local records",
      npub: Npub.make(deriveLocalNostrIdentity(SECOND_TEST_MNEMONIC).npub),
      sparkFingerprints: [rustFingerprint],
    };
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate")],
        expected: wrongNostr,
        adapters: [rustSparkComparisonAdapter],
      }),
    );
    expect(result._tag).toBe("spark_matches_nostr_mismatch");
    expect(isReconciliationConflict(result)).toBe(true);
  });

  test("no public comparison source → no_public_match", () => {
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate")],
        expected: null,
        adapters: allAdapters,
      }),
    );
    expect(result._tag).toBe("no_public_match");
  });

  test("owner-attended (secret-less) candidates are skipped, not reconciled", () => {
    const ownerAttended: DecodedCandidate = {
      result: {
        format: "electron_safe_storage_record",
        formatLabel: "Electron safe-storage record",
        formatVersion: "1",
        sourcePathLabel: "electron opaque record",
        status: "owner_attended_required",
        decoded: false,
        publicIdentity: null,
      },
      secret: null,
    };
    const result = Effect.runSync(
      reconcileIdentities({
        candidates: [ownerAttended],
        expected: expectedFull,
        adapters: allAdapters,
      }),
    );
    expect(result._tag).toBe("no_public_match");
  });
});

describe("IDR-04 comparison adapters", () => {
  test("the exact Rust and Breez adapters derive public fingerprints; LDK is deferred", () => {
    const identity = Effect.runSync(
      deriveCandidatePublicIdentity(candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate"), allAdapters),
    );
    expect(identity).not.toBeNull();
    if (identity === null) throw new Error("unreachable");

    const kinds = identity.sparkFingerprints.map((fingerprint) => fingerprint.adapter).sort();
    expect(kinds).toEqual(["breez_spark", "rust_spark"]);
    // LDK is registered but deferred: recorded, not compared.
    expect(identity.deferredAdapters).toEqual(["ldk"]);

    const rust = identity.sparkFingerprints.find((f) => f.adapter === "rust_spark");
    expect(rust?.fingerprintHex).toBe(reference.sparkBip32FingerprintHex);

    const breez = identity.sparkFingerprints.find((f) => f.adapter === "breez_spark");
    expect(breez?.fingerprintHex).toBe(breezFingerprint.fingerprintHex);
    // The two Spark profiles derive DIFFERENT fingerprints from the same phrase.
    expect(breez?.fingerprintHex).not.toBe(rust?.fingerprintHex);
  });

  test("the deferred LDK adapter fails with the deferred reason and never touches the secret", () => {
    const outcome = Effect.runSync(
      Effect.result(
        ldkSparkComparisonAdapter.deriveFingerprint(
          new RecoveredSecret(PUBLIC_TEST_MNEMONIC, "plain_mnemonic_file", "1"),
        ),
      ),
    );
    expect(Result.isFailure(outcome)).toBe(true);
    if (!Result.isFailure(outcome)) throw new Error("unreachable");
    expect(outcome.failure.reason).toBe("deferred");
    expect(outcome.failure.adapter).toBe("ldk");
  });
});

describe("IDR-04 secret-logging tripwire", () => {
  const forbiddenNeedles = ["abandon", "zoo", "wrong", bytesToHex(referenceSeed)];

  const assertNoSecret = (value: unknown): void => {
    const serialized = JSON.stringify(value);
    for (const needle of forbiddenNeedles) {
      expect(serialized.includes(needle)).toBe(false);
    }
  };

  test("no reconciliation result carries any mnemonic word or seed byte", () => {
    const results: ReconciliationResult[] = [
      Effect.runSync(
        reconcileIdentities({
          candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate")],
          expected: expectedFull,
          adapters: allAdapters,
        }),
      ),
      Effect.runSync(
        reconcileIdentities({
          candidates: [
            candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate-1"),
            candidateFrom(SECOND_TEST_MNEMONIC, "candidate-2"),
          ],
          expected: expectedFull,
          adapters: allAdapters,
        }),
      ),
      Effect.runSync(
        reconcileIdentities({
          candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate")],
          expected: null,
          adapters: allAdapters,
        }),
      ),
    ];
    for (const result of results) assertNoSecret(result);
  });

  test("the derived candidate identity is public-only", () => {
    const identity = Effect.runSync(
      deriveCandidatePublicIdentity(candidateFrom(PUBLIC_TEST_MNEMONIC, "candidate"), allAdapters),
    );
    assertNoSecret(identity);
  });
});
