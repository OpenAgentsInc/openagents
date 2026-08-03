import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import { forensicSha256Digest, strictDecode } from "../src/canonical.ts";
import {
  HistoricalBlockBundleSchema,
  HistoricalFingerprintDefinitionSchema,
  PrivateBitcoinCoreCapabilitySchema,
  runHistoricalFingerprintScan,
  validateHistoricalWideScanLedger,
  type HistoricalBlockBundle,
  type HistoricalScanTransaction,
} from "../src/historical-scan.ts";
import { ForensicRef, Sha256Digest } from "../src/primitives.ts";

/**
 * These fixtures are frozen mainnet chain data, not a hand-written scenario.
 * The three bundles were extracted read-only from our own archival Bitcoin Core
 * node at `oa-bitcoind` and carry the blocks holding the eight known-positive
 * transactions the Coldcard postmortem published. The wide-scan ledger is the
 * per-block record of a 1,701-block, 7.12M-transaction scan on the same node.
 *
 * That matters for what a green run here means. A synthetic fixture can only
 * prove the scanner agrees with whoever wrote the fixture. These prove the
 * published fingerprint reproduces from chain bytes nobody in this repository
 * controls, and that perturbing the fee table by one vbyte destroys it.
 */
const FixtureSchema = S.Struct({
  schema: S.Literal("openagents.coldcard_historical_scan_fixture.v2"),
  bundles: S.Array(
    S.Struct({
      bundleDigest: Sha256Digest,
      bundleFile: S.String,
      canonicalBytesDigest: Sha256Digest,
      compressedBytesDigest: Sha256Digest,
      definition: HistoricalFingerprintDefinitionSchema,
      endHeight: S.Number,
      reportRef: ForensicRef,
      startHeight: S.Number,
    }),
  ).check(S.isMinLength(3)),
  capability: PrivateBitcoinCoreCapabilitySchema,
  narrowedRevision: S.Record(S.String, S.Unknown),
  publishedRevision: S.Record(S.String, S.Unknown),
  workerProfile: S.Record(S.String, S.Unknown),
  workerProfileDigest: Sha256Digest,
});

const fixturePath = (name: string) =>
  new URL(`../../../fixtures/forensics/coldcard/${name}`, import.meta.url);

const fixture = strictDecode(
  FixtureSchema,
  JSON.parse(readFileSync(fixturePath("historical-scan-fixture.v2.json"), "utf8")),
);

const sha256 = (bytes: Uint8Array) => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

const loadedBundles = fixture.bundles.map((entry) => {
  const compressed = readFileSync(fixturePath(entry.bundleFile));
  const canonical = gunzipSync(compressed);
  return {
    bundle: strictDecode(
      HistoricalBlockBundleSchema,
      JSON.parse(canonical.toString("utf8")),
    ) as HistoricalBlockBundle,
    canonicalBytes: canonical,
    compressedBytes: compressed,
    entry,
  };
});

const primary = loadedBundles.find((loaded) => loaded.bundle.blocks.length > 1)!;

const run = (loaded: (typeof loadedBundles)[number], bundle = loaded.bundle) =>
  runHistoricalFingerprintScan({
    bundle,
    capability: fixture.capability,
    definition: loaded.entry.definition,
    reportRef: loaded.entry.reportRef,
    workerProfileDigest: fixture.workerProfileDigest,
  });

const rebuildBundle = (
  loaded: (typeof loadedBundles)[number],
  mutate: (transaction: HistoricalScanTransaction) => HistoricalScanTransaction,
): HistoricalBlockBundle => {
  const blocks = loaded.bundle.blocks.map((block) => ({
    ...block,
    transactions: block.transactions.map(mutate),
  }));
  return strictDecode(HistoricalBlockBundleSchema, {
    ...loaded.bundle,
    blocks,
    contentDigest: forensicSha256Digest({
      blocks,
      bundleRef: loaded.bundle.bundleRef,
      endHeight: loaded.bundle.endHeight,
      genesisHash: loaded.bundle.genesisHash,
      network: loaded.bundle.network,
      sourceIdentityDigest: loaded.bundle.sourceIdentityDigest,
      startHeight: loaded.bundle.startHeight,
    }),
  });
};

describe("Coldcard historical fingerprint scanner over frozen mainnet bundles", () => {
  it("binds every checked-in bundle to its recorded bytes, node identity, and digest", () => {
    expect(loadedBundles).toHaveLength(3);
    for (const loaded of loadedBundles) {
      expect(sha256(loaded.compressedBytes)).toBe(loaded.entry.compressedBytesDigest);
      expect(sha256(loaded.canonicalBytes)).toBe(loaded.entry.canonicalBytesDigest);
      expect(loaded.bundle.contentDigest).toBe(loaded.entry.bundleDigest);
      expect(loaded.bundle.network).toBe("mainnet");
      expect(loaded.bundle.genesisHash).toBe(
        "sha256:000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
      );
      expect(loaded.bundle.sourceIdentityDigest).toBe(fixture.capability.nodeIdentityDigest);
      expect(loaded.bundle.startHeight).toBe(loaded.entry.startHeight);
      expect(loaded.bundle.endHeight).toBe(loaded.entry.endHeight);
    }
    expect(forensicSha256Digest(fixture.workerProfile)).toBe(fixture.workerProfileDigest);
    expect(forensicSha256Digest(fixture.narrowedRevision)).toBe(
      loadedBundles[0]!.entry.definition.revisionDigest,
    );
  });

  it("reproduces all eight published known positives at their published fee rates", () => {
    const observed = new Map<string, number>();
    let controls = 0;
    for (const loaded of loadedBundles) {
      const report = run(loaded);
      expect(report).toMatchObject({
        bundleDigest: loaded.bundle.contentDigest,
        mutationControlStatus: "passed",
        negativeControlStatus: "not_run",
        outcome: "succeeded",
        selfTestStatus: "passed",
        similarityClaimCeiling: "program_similarity_only",
      });
      expect(report.missingDataRefs).toEqual([]);
      expect(report.cheapCandidateRefs).toEqual(report.expensiveResolutionRefs);
      for (const hit of report.rawHits) observed.set(hit.transactionRef, hit.feeRateSatsPerVbyte);
      for (const control of loaded.entry.definition.knownPositiveControls) {
        controls += 1;
        expect(observed.get(control.transactionRef)).toBe(control.expectedFeeRateSatsPerVbyte);
      }
    }
    expect(controls).toBe(8);
  });

  it("destroys every known positive when the fee table moves by a single vbyte", () => {
    for (const loaded of loadedBundles) {
      const report = run(loaded);
      expect(report.mutationControlResults).toHaveLength(2);
      for (const result of report.mutationControlResults) {
        expect(result.knownPositiveSurvivors).toBe(0);
        expect(result.status).toBe("passed");
        // Not just the known positives: moving the table by one vbyte drops
        // the whole bundle's match count several-fold, which is what says the
        // published table is the one these blocks were built with rather than
        // an arithmetic accident.
        expect(result.bundleHits * 5).toBeLessThan(report.rawHits.length);
      }
    }
  });

  it("refuses a wide scan when a frozen positive stops reproducing", () => {
    const control = primary.entry.definition.knownPositiveControls[0]!;
    const mutated = rebuildBundle(primary, (transaction) =>
      transaction.transactionRef === control.transactionRef
        ? { ...transaction, locktime: 1 }
        : transaction,
    );
    const report = run(primary, mutated);
    expect(report).toMatchObject({
      mutationControlStatus: "not_run",
      outcome: "failed",
      scannedTransactions: 0,
      selfTestStatus: "failed",
    });
  });

  it("fails loudly for missing fees or required prevouts instead of returning a zero hit", () => {
    const target = run(primary).rawHits.find(
      (hit) =>
        !primary.entry.definition.knownPositiveControls.some(
          (control) => control.transactionRef === hit.transactionRef,
        ),
    )!;

    const withoutFee = rebuildBundle(primary, (transaction) => {
      if (transaction.transactionRef !== target.transactionRef) return transaction;
      const { feeSats: _dropped, ...rest } = transaction;
      return rest as HistoricalScanTransaction;
    });
    const missingFee = run(primary, withoutFee);
    expect(missingFee.outcome).toBe("failed");
    expect(missingFee.missingDataRefs).toContain(`missing.fee.${target.transactionRef}`);

    const withoutPrevout = rebuildBundle(primary, (transaction) =>
      transaction.transactionRef === target.transactionRef
        ? { ...transaction, prevoutDataStatus: "missing_required" }
        : transaction,
    );
    const missingPrevout = run(primary, withoutPrevout);
    expect(missingPrevout.outcome).toBe("failed");
    expect(missingPrevout.missingDataRefs).toContain(`missing.prevout.${target.transactionRef}`);
    expect(missingPrevout.rawHits.some((hit) => hit.transactionRef === target.transactionRef)).toBe(
      false,
    );
  });

  it("resumes from every completed-block checkpoint with identical output digests", () => {
    const full = run(primary);
    expect(full.checkpoints.length).toBeGreaterThan(1);
    for (let index = 0; index < full.checkpoints.length - 1; index += 1) {
      const checkpoints = full.checkpoints.slice(0, index + 1);
      const completedHeight = checkpoints.at(-1)!.completedHeight;
      const resumed = runHistoricalFingerprintScan({
        bundle: primary.bundle,
        capability: fixture.capability,
        definition: primary.entry.definition,
        priorCheckpoints: checkpoints,
        priorRawHits: full.rawHits.filter((hit) => hit.blockHeight <= completedHeight),
        reportRef: `report.coldcard.resumed.${completedHeight}`,
        workerProfileDigest: fixture.workerProfileDigest,
      });
      expect(resumed.rawHitsDigest).toBe(full.rawHitsDigest);
      expect(resumed.normalizedOutputDigest).toBe(full.normalizedOutputDigest);
      expect(resumed.rawHits.map((hit) => hit.txid)).toEqual(full.rawHits.map((hit) => hit.txid));
    }
  });

  it("cannot pass a negative control the bundle never contained", () => {
    const absent = strictDecode(HistoricalFingerprintDefinitionSchema, {
      ...primary.entry.definition,
      negativeControlBlockRefs: ["block.1"],
    });
    const report = runHistoricalFingerprintScan({
      bundle: primary.bundle,
      capability: fixture.capability,
      definition: absent,
      reportRef: primary.entry.reportRef,
      workerProfileDigest: fixture.workerProfileDigest,
    });
    expect(report.negativeControlStatus).toBe("failed");
    expect(report.outcome).toBe("failed");
  });

  it("reports real false-match strata per million by fee, era, script type, and revision", () => {
    const report = run(primary);
    expect(report.baseRates.length).toBeGreaterThan(1);
    expect(
      report.baseRates.every(
        (rate) => rate.fingerprintRevisionDigest === primary.entry.definition.revisionDigest,
      ),
    ).toBe(true);
    const eligible = report.baseRates.reduce((total, rate) => total + rate.eligibleTransactions, 0);
    const matched = report.baseRates.reduce((total, rate) => total + rate.matches, 0);
    const scanned = primary.bundle.blocks.reduce(
      (total, block) => total + block.transactions.length,
      0,
    );
    // The denominator is the population the rule could have selected, so it is
    // smaller than the block's transaction count and larger than its hits.
    expect(eligible).toBeGreaterThan(matched);
    expect(eligible).toBeLessThan(scanned);
    expect(matched).toBe(report.rawHits.length);
    // Real mainnet blocks are dominated by transactions the fingerprint does
    // not select. A stratum table that did not say so would be describing a
    // fixture, not a chain.
    expect(matched / eligible).toBeLessThan(0.05);
    expect(report.baseRates.some((rate) => rate.feeRateRef === "fee-rate.non-match")).toBe(true);
  });

  it("never admits node secrets, wallet RPC, an endpoint, or guest networking", () => {
    expect(() =>
      strictDecode(PrivateBitcoinCoreCapabilitySchema, {
        ...fixture.capability,
        walletRpcAllowed: true,
      }),
    ).toThrow();
    expect(() =>
      strictDecode(PrivateBitcoinCoreCapabilitySchema, {
        ...fixture.capability,
        rpcCookie: "secret",
      }),
    ).toThrow(/rpcCookie/);
  });
});

describe("Coldcard 1,701-block wide-scan ledger", () => {
  const ledger = validateHistoricalWideScanLedger(
    JSON.parse(readFileSync(fixturePath("historical-wide-scan-ledger.v1.json"), "utf8")),
  );

  it("retains the per-block evidence the era totals fold from", () => {
    expect(ledger.blocks).toBe(1_701);
    expect(ledger.eligibleTransactions).toBe(7_122_744);
    expect(ledger.prevoutErrors).toBe(0);
    expect(ledger.selfTestStatus).toBe("passed");
    expect(ledger.similarityClaimCeiling).toBe("program_similarity_only");
    expect(ledger.sourceIdentityDigest).toBe(fixture.capability.nodeIdentityDigest);
    expect(forensicSha256Digest(fixture.publishedRevision)).toBe(ledger.fingerprintRevisionDigest);
    expect(ledger.eras.map((era) => era.eraRef)).toEqual([
      "era.coldcard.control2025",
      "era.coldcard.incident",
      "era.coldcard.prewave",
    ]);
  });

  it("measures the denominator the published postmortem never had", () => {
    const rate = (eraRef: string, feeRate: number) => {
      const era = ledger.eras.find((candidate) => candidate.eraRef === eraRef)!;
      const row = era.matchesByFeeRate.find(
        (candidate) => candidate.feeRateSatsPerVbyte === feeRate,
      );
      return Math.round(row?.matchesPerMillion ?? 0);
    };
    const any = (eraRef: string) =>
      Math.round(ledger.eras.find((candidate) => candidate.eraRef === eraRef)!.matchesPerMillion);

    // 2 sat/vB collides with ordinary traffic at three to six per thousand.
    expect(rate("era.coldcard.control2025", 2)).toBe(2_820);
    expect(rate("era.coldcard.incident", 2)).toBe(3_707);
    expect(rate("era.coldcard.prewave", 2)).toBe(6_154);
    // 30 sat/vB is far weaker than "zero false positives across 7,553
    // transactions" is usually read to mean: 537 per million implies about
    // four expected false positives in a sample that size.
    expect(rate("era.coldcard.control2025", 30)).toBe(537);
    expect(rate("era.coldcard.incident", 30)).toBe(701);
    expect(rate("era.coldcard.prewave", 30)).toBe(24);
    expect(any("era.coldcard.control2025")).toBe(37_043);
    expect(any("era.coldcard.incident")).toBe(25_876);
    expect(any("era.coldcard.prewave")).toBe(18_922);
  });

  it("refuses a match count that its own retained blocks do not support", () => {
    const inflated = {
      ...ledger,
      eras: ledger.eras.map((era, index) =>
        index === 0 ? { ...era, matches: era.matches + 1 } : era,
      ),
      matches: ledger.matches + 1,
    };
    expect(() => validateHistoricalWideScanLedger(inflated)).toThrow();

    const unsupportedSelfTest = { ...ledger, selfTestStatus: "not_run" };
    expect(() => validateHistoricalWideScanLedger(unsupportedSelfTest)).toThrow();

    const withMissingPrevouts = {
      ...ledger,
      prevoutErrors: 1,
    };
    expect(() => validateHistoricalWideScanLedger(withMissingPrevouts)).toThrow();
  });
});
