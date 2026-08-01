import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import { forensicSha256Digest, strictDecode } from "../src/canonical.ts";
import {
  HistoricalBlockBundleSchema,
  HistoricalFingerprintDefinitionSchema,
  PrivateBitcoinCoreCapabilitySchema,
  runHistoricalFingerprintScan,
  type HistoricalBlockBundle,
  type HistoricalScanTransaction,
} from "../src/historical-scan.ts";
import { Sha256Digest } from "../src/primitives.ts";

const FixtureSchema = S.Struct({
  schema: S.Literal("openagents.coldcard_historical_scan_fixture.v1"),
  bundle: HistoricalBlockBundleSchema,
  capability: PrivateBitcoinCoreCapabilitySchema,
  definition: HistoricalFingerprintDefinitionSchema,
  workerProfileDigest: Sha256Digest,
});

const fixture = strictDecode(
  FixtureSchema,
  JSON.parse(
    readFileSync(
      new URL(
        "../../../fixtures/forensics/coldcard/historical-scan-fixture.v1.json",
        import.meta.url,
      ),
      "utf8",
    ),
  ),
);

const rebuildBundle = (
  mutate: (transaction: HistoricalScanTransaction) => HistoricalScanTransaction,
): HistoricalBlockBundle => {
  const blocks = fixture.bundle.blocks.map((block) => ({
    ...block,
    transactions: block.transactions.map(mutate),
  }));
  const digestInput = {
    blocks,
    bundleRef: fixture.bundle.bundleRef,
    endHeight: fixture.bundle.endHeight,
    genesisHash: fixture.bundle.genesisHash,
    network: fixture.bundle.network,
    sourceIdentityDigest: fixture.bundle.sourceIdentityDigest,
    startHeight: fixture.bundle.startHeight,
  };
  return strictDecode(HistoricalBlockBundleSchema, {
    ...fixture.bundle,
    blocks,
    contentDigest: forensicSha256Digest(digestInput),
  });
};

const run = (bundle = fixture.bundle) =>
  runHistoricalFingerprintScan({
    bundle,
    capability: fixture.capability,
    definition: fixture.definition,
    reportRef: "report.coldcard.historical.synthetic.v1",
    workerProfileDigest: fixture.workerProfileDigest,
  });

describe("Coldcard historical fingerprint scanner", () => {
  it("passes frozen positives before scanning and preserves the two-phase candidate set", () => {
    const report = run();
    expect(report).toMatchObject({
      outcome: "succeeded",
      selfTestStatus: "passed",
      negativeControlStatus: "passed",
      similarityClaimCeiling: "program_similarity_only",
    });
    expect(report.rawHits.map((hit) => hit.transactionRef)).toEqual([
      "transaction.known-positive",
      "transaction.wide-hit",
    ]);
    expect(report.cheapCandidateRefs).toEqual(report.expensiveResolutionRefs);
    expect(report.rawHits[0]).toMatchObject({
      feeRateSatsPerVbyte: 30,
      highRSignatures: 1,
      inputType: "p2wpkh",
      sequenceValues: [4294967295],
    });
  });

  it("fails loudly for missing fees or required prevouts instead of returning a zero hit", () => {
    const withoutFee = rebuildBundle((transaction) =>
      transaction.transactionRef === "transaction.off-fingerprint-fee"
        ? {
            inputs: transaction.inputs,
            locktime: transaction.locktime,
            outputCount: transaction.outputCount,
            outputScriptType: transaction.outputScriptType,
            outputValueSats: transaction.outputValueSats,
            prevoutDataStatus: transaction.prevoutDataStatus,
            transactionRef: transaction.transactionRef,
            txid: transaction.txid,
            version: transaction.version,
            vsize: transaction.vsize,
          }
        : transaction,
    );
    const missingFee = run(withoutFee);
    expect(missingFee.outcome).toBe("failed");
    expect(missingFee.missingDataRefs).toContain("missing.fee.transaction.off-fingerprint-fee");
    expect(missingFee.rawHits).toHaveLength(2);

    const withoutPrevout = rebuildBundle((transaction) =>
      transaction.transactionRef === "transaction.wide-hit"
        ? { ...transaction, prevoutDataStatus: "missing_required" }
        : transaction,
    );
    const missingPrevout = run(withoutPrevout);
    expect(missingPrevout.outcome).toBe("failed");
    expect(missingPrevout.missingDataRefs).toContain("missing.prevout.transaction.wide-hit");
  });

  it("refuses a wide scan when a frozen positive does not reproduce", () => {
    const mutated = rebuildBundle((transaction) =>
      transaction.transactionRef === "transaction.known-positive"
        ? { ...transaction, locktime: 1 }
        : transaction,
    );
    const report = run(mutated);
    expect(report).toMatchObject({
      outcome: "failed",
      scannedTransactions: 0,
      selfTestStatus: "failed",
    });
  });

  it("distinguishes locktime, fee-table, and sequence mutations", () => {
    const sequenceMutation = rebuildBundle((transaction) =>
      transaction.transactionRef === "transaction.wide-hit"
        ? {
            ...transaction,
            inputs: transaction.inputs.map((input) => ({ ...input, sequence: 4294967294 })),
          }
        : transaction,
    );
    expect(run(sequenceMutation).rawHits.map((hit) => hit.transactionRef)).toEqual([
      "transaction.known-positive",
    ]);
    expect(run().rawHits.map((hit) => hit.feeRateSatsPerVbyte)).toEqual([30, 2]);
  });

  it("resumes from every completed-block checkpoint with identical output digests", () => {
    const full = run();
    for (let index = 0; index < full.checkpoints.length - 1; index += 1) {
      const checkpoints = full.checkpoints.slice(0, index + 1);
      const completedHeight = checkpoints.at(-1)!.completedHeight;
      const priorRawHits = full.rawHits.filter((hit) => hit.blockHeight <= completedHeight);
      const resumed = runHistoricalFingerprintScan({
        bundle: fixture.bundle,
        capability: fixture.capability,
        definition: fixture.definition,
        priorCheckpoints: checkpoints,
        priorRawHits,
        reportRef: `report.coldcard.resumed.${completedHeight}`,
        workerProfileDigest: fixture.workerProfileDigest,
      });
      expect(resumed.rawHitsDigest).toBe(full.rawHitsDigest);
      expect(resumed.normalizedOutputDigest).toBe(full.normalizedOutputDigest);
    }
  });

  it("reports false matches per million by fee, era, script type, and revision", () => {
    const report = run();
    expect(report.baseRates.length).toBeGreaterThan(0);
    expect(
      report.baseRates.every(
        (rate) => rate.fingerprintRevisionDigest === fixture.definition.revisionDigest,
      ),
    ).toBe(true);
    expect(report.baseRates.find((rate) => rate.feeRateRef === "fee-rate.30")).toMatchObject({
      eligibleTransactions: 1,
      matches: 1,
      matchesPerMillion: 1_000_000,
      scriptType: "p2wpkh",
    });
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
