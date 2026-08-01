import { Schema as S } from "effect";

import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import {
  ForensicRef,
  ForensicTimestamp,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
} from "./primitives.ts";

export const HISTORICAL_BLOCK_BUNDLE_VERSION = "openagents.historical_block_bundle.v1" as const;
export const HISTORICAL_SCAN_REPORT_VERSION =
  "openagents.historical_fingerprint_scan_report.v1" as const;
export const PRIVATE_BITCOIN_CORE_CAPABILITY_VERSION =
  "openagents.private_bitcoin_core_capability.v1" as const;

const SatoshiString = S.String.check(S.isPattern(/^(?:0|[1-9][0-9]*)$/));
const OpaqueServerBindingRef = S.String.check(S.isPattern(/^binding\.[A-Za-z0-9][A-Za-z0-9._-]*$/));

export const PrivateBitcoinCoreCapabilitySchema = S.Struct({
  schema: S.Literal(PRIVATE_BITCOIN_CORE_CAPABILITY_VERSION),
  arbitraryEndpointAllowed: S.Literal(false),
  capabilityRef: ForensicRef,
  externalIpVisibleToGuest: S.Literal(false),
  guestNetwork: S.Literal("none"),
  methods: S.Array(
    S.Literals(["getblockchaininfo", "getblockhash", "getblock", "getrawtransaction"]),
  ).check(S.isMinLength(1), S.isMaxLength(4)),
  nodeIdentityDigest: Sha256Digest,
  secretDelivery: S.Literal("broker_bound_not_guest_visible"),
  serverBindingRef: OpaqueServerBindingRef,
  walletRpcAllowed: S.Literal(false),
}).pipe(
  S.check(
    S.makeFilter((capability) => new Set(capability.methods).size === capability.methods.length, {
      message: "private Bitcoin Core methods must be unique and explicitly read-only",
    }),
  ),
);
export type PrivateBitcoinCoreCapability = typeof PrivateBitcoinCoreCapabilitySchema.Type;

export const HistoricalScanInputSchema = S.Struct({
  inputRef: ForensicRef,
  inputType: S.Literals(["p2wpkh", "p2sh_p2wpkh", "p2pkh", "p2tr", "other"]),
  pubkeyDigest: S.optionalKey(Sha256Digest),
  sequence: NonNegativeInteger,
  signatureRWidthBytes: S.optionalKey(PositiveInteger),
});
export type HistoricalScanInput = typeof HistoricalScanInputSchema.Type;

export const HistoricalScanTransactionSchema = S.Struct({
  feeSats: S.optionalKey(SatoshiString),
  inputs: S.Array(HistoricalScanInputSchema).check(S.isMinLength(1), S.isMaxLength(10_000)),
  locktime: NonNegativeInteger,
  outputCount: PositiveInteger,
  outputScriptType: S.Literals(["p2wpkh", "p2sh", "p2pkh", "p2tr", "other"]),
  outputValueSats: SatoshiString,
  prevoutDataStatus: S.Literals(["complete", "missing_required"]),
  transactionRef: ForensicRef,
  txid: Sha256Digest,
  version: S.Number.check(S.isInt()),
  vsize: PositiveInteger,
});
export type HistoricalScanTransaction = typeof HistoricalScanTransactionSchema.Type;

export const HistoricalScanBlockSchema = S.Struct({
  blockHash: Sha256Digest,
  blockTime: ForensicTimestamp,
  height: NonNegativeInteger,
  rawBlockDigest: Sha256Digest,
  transactions: S.Array(HistoricalScanTransactionSchema).check(
    S.isMinLength(1),
    S.isMaxLength(100_000),
  ),
});
export type HistoricalScanBlock = typeof HistoricalScanBlockSchema.Type;

export const HistoricalBlockBundleSchema = S.Struct({
  schema: S.Literal(HISTORICAL_BLOCK_BUNDLE_VERSION),
  blocks: S.Array(HistoricalScanBlockSchema).check(S.isMinLength(1), S.isMaxLength(100_000)),
  bundleRef: ForensicRef,
  capturedAt: ForensicTimestamp,
  contentDigest: Sha256Digest,
  endHeight: NonNegativeInteger,
  genesisHash: Sha256Digest,
  network: S.Literal("mainnet"),
  sourceIdentityDigest: Sha256Digest,
  startHeight: NonNegativeInteger,
})
  .pipe(
    S.check(
      S.makeFilter(
        (bundle) =>
          bundle.endHeight >= bundle.startHeight &&
          bundle.blocks.length === bundle.endHeight - bundle.startHeight + 1 &&
          bundle.blocks.every((block, index) => block.height === bundle.startHeight + index),
        { message: "historical block bundle must densely cover its declared range" },
      ),
      S.makeFilter(
        (bundle) =>
          bundle.contentDigest ===
          forensicSha256Digest({
            blocks: bundle.blocks,
            bundleRef: bundle.bundleRef,
            endHeight: bundle.endHeight,
            genesisHash: bundle.genesisHash,
            network: bundle.network,
            sourceIdentityDigest: bundle.sourceIdentityDigest,
            startHeight: bundle.startHeight,
          }),
        { message: "historical block bundle digest must bind exact block bytes" },
      ),
      S.makeFilter(
        (bundle) => {
          const transactions = bundle.blocks.flatMap((block) => block.transactions);
          return (
            new Set(bundle.blocks.map((block) => block.blockHash)).size === bundle.blocks.length &&
            new Set(transactions.map((transaction) => transaction.transactionRef)).size ===
              transactions.length &&
            new Set(transactions.map((transaction) => transaction.txid)).size ===
              transactions.length
          );
        },
        { message: "historical block hashes, transaction refs, and txids must be unique" },
      ),
    ),
  )
  .annotate({ identifier: "HistoricalBlockBundle" });
export type HistoricalBlockBundle = typeof HistoricalBlockBundleSchema.Type;

export const HistoricalFingerprintDefinitionSchema = S.Struct({
  allowedSequences: S.Array(NonNegativeInteger).check(S.isMinLength(1), S.isMaxLength(16)),
  definitionRef: ForensicRef,
  eligibleInputTypes: S.Array(S.Literals(["p2wpkh", "p2sh_p2wpkh", "p2pkh"])).check(
    S.isMinLength(1),
    S.isMaxLength(3),
  ),
  feeRateMaximum: PositiveInteger,
  feeRateMinimum: PositiveInteger,
  fixedOverheadVbytes: PositiveInteger,
  inputVbytes: S.Struct({
    p2pkh: PositiveInteger,
    p2sh_p2wpkh: PositiveInteger,
    p2wpkh: PositiveInteger,
  }),
  knownPositiveControls: S.Array(
    S.Struct({ expectedFeeRateSatsPerVbyte: PositiveInteger, transactionRef: ForensicRef }),
  ).check(S.isMinLength(1), S.isMaxLength(256)),
  negativeControlBlockRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(256)),
  requiredLocktime: NonNegativeInteger,
  requiredOutputCount: PositiveInteger,
  requiredOutputScriptType: S.Literal("p2wpkh"),
  requiredVersion: S.Number.check(S.isInt()),
  revisionDigest: Sha256Digest,
});
export type HistoricalFingerprintDefinition = typeof HistoricalFingerprintDefinitionSchema.Type;

export const HistoricalRawHitSchema = S.Struct({
  blockHash: Sha256Digest,
  blockHeight: NonNegativeInteger,
  blockTime: ForensicTimestamp,
  estimatedVbytes: PositiveInteger,
  feeRateSatsPerVbyte: PositiveInteger,
  feeSats: SatoshiString,
  fingerprintRevisionDigest: Sha256Digest,
  highRSignatures: NonNegativeInteger,
  inputCount: PositiveInteger,
  inputType: S.Literals(["p2wpkh", "p2sh_p2wpkh", "p2pkh"]),
  oneSource: S.Boolean,
  sequenceValues: S.Array(NonNegativeInteger).check(S.isMinLength(1), S.isMaxLength(16)),
  transactionRef: ForensicRef,
  txid: Sha256Digest,
});
export type HistoricalRawHit = typeof HistoricalRawHitSchema.Type;

export const HistoricalScanCheckpointSchema = S.Struct({
  blockHash: Sha256Digest,
  checkpointDigest: Sha256Digest,
  completedHeight: NonNegativeInteger,
  fingerprintRevisionDigest: Sha256Digest,
  priorCheckpointDigest: S.optionalKey(Sha256Digest),
  rawHitsDigest: Sha256Digest,
}).pipe(
  S.check(
    S.makeFilter(
      (checkpoint) =>
        checkpoint.checkpointDigest ===
        forensicSha256Digest({
          blockHash: checkpoint.blockHash,
          completedHeight: checkpoint.completedHeight,
          fingerprintRevisionDigest: checkpoint.fingerprintRevisionDigest,
          priorCheckpointDigest: checkpoint.priorCheckpointDigest,
          rawHitsDigest: checkpoint.rawHitsDigest,
        }),
      { message: "historical scan checkpoint must bind its prior checkpoint and raw hits" },
    ),
  ),
);
export type HistoricalScanCheckpoint = typeof HistoricalScanCheckpointSchema.Type;

export const HistoricalBaseRateSchema = S.Struct({
  eligibleTransactions: NonNegativeInteger,
  eraRef: ForensicRef,
  feeRateRef: ForensicRef,
  fingerprintRevisionDigest: Sha256Digest,
  matches: NonNegativeInteger,
  matchesPerMillion: S.Number.check(S.isFinite(), S.isGreaterThanOrEqualTo(0)),
  scriptType: S.Literals(["p2wpkh", "p2sh_p2wpkh", "p2pkh"]),
}).pipe(
  S.check(
    S.makeFilter(
      (rate) =>
        rate.matches <= rate.eligibleTransactions &&
        rate.matchesPerMillion ===
          (rate.eligibleTransactions === 0
            ? 0
            : (rate.matches * 1_000_000) / rate.eligibleTransactions),
      { message: "historical base rate must derive from exact eligible and match counts" },
    ),
  ),
);
export type HistoricalBaseRate = typeof HistoricalBaseRateSchema.Type;

export const HistoricalFingerprintScanReportSchema = S.Struct({
  schema: S.Literal(HISTORICAL_SCAN_REPORT_VERSION),
  baseRates: S.Array(HistoricalBaseRateSchema).check(S.isMaxLength(10_000)),
  bundleDigest: Sha256Digest,
  capabilityRef: ForensicRef,
  checkpoints: S.Array(HistoricalScanCheckpointSchema).check(S.isMaxLength(100_000)),
  cheapCandidateRefs: S.Array(ForensicRef).check(S.isMaxLength(1_000_000)),
  expensiveResolutionRefs: S.Array(ForensicRef).check(S.isMaxLength(1_000_000)),
  fingerprintRevisionDigest: Sha256Digest,
  missingDataRefs: S.Array(ForensicRef).check(S.isMaxLength(1_000_000)),
  negativeControlStatus: S.Literals(["passed", "failed", "not_run"]),
  normalizedOutputDigest: Sha256Digest,
  outcome: S.Literals(["succeeded", "failed", "inconclusive"]),
  rawHits: S.Array(HistoricalRawHitSchema).check(S.isMaxLength(1_000_000)),
  rawHitsDigest: Sha256Digest,
  reportRef: ForensicRef,
  scannedTransactions: NonNegativeInteger,
  selfTestStatus: S.Literals(["passed", "failed", "not_run"]),
  similarityClaimCeiling: S.Literal("program_similarity_only"),
  workerProfileDigest: Sha256Digest,
})
  .pipe(
    S.check(
      S.makeFilter(
        (report) =>
          report.rawHitsDigest === forensicSha256Digest(report.rawHits) &&
          report.normalizedOutputDigest ===
            forensicSha256Digest(report.rawHits.map((hit) => hit.txid).toSorted()),
        { message: "historical scan output digests must rebuild from retained raw hits" },
      ),
      S.makeFilter(
        (report) =>
          new Set(report.rawHits.map((hit) => hit.txid)).size === report.rawHits.length &&
          report.rawHits.every(
            (hit, index) =>
              index === 0 ||
              report.rawHits[index - 1]!.blockHeight < hit.blockHeight ||
              (report.rawHits[index - 1]!.blockHeight === hit.blockHeight &&
                report.rawHits[index - 1]!.txid.localeCompare(hit.txid) < 0),
          ),
        { message: "historical raw hits must be unique and canonically ordered" },
      ),
      S.makeFilter(
        (report) =>
          report.checkpoints.every(
            (checkpoint, index) =>
              index === 0 ||
              (checkpoint.completedHeight === report.checkpoints[index - 1]!.completedHeight + 1 &&
                checkpoint.priorCheckpointDigest ===
                  report.checkpoints[index - 1]!.checkpointDigest),
          ),
        { message: "historical checkpoints must form a dense append-only chain" },
      ),
      S.makeFilter(
        (report) =>
          report.outcome !== "succeeded" ||
          (report.selfTestStatus === "passed" &&
            report.negativeControlStatus === "passed" &&
            report.missingDataRefs.length === 0 &&
            report.cheapCandidateRefs.length === report.expensiveResolutionRefs.length &&
            report.cheapCandidateRefs.every(
              (reference, index) => report.expensiveResolutionRefs[index] === reference,
            )),
        {
          message:
            "successful historical scans require controls, complete data, and exact funnel preservation",
        },
      ),
    ),
  )
  .annotate({ identifier: "HistoricalFingerprintScanReport" });
export type HistoricalFingerprintScanReport = typeof HistoricalFingerprintScanReportSchema.Type;

const inputVbytes = (
  definition: HistoricalFingerprintDefinition,
  inputType: HistoricalRawHit["inputType"],
): number => definition.inputVbytes[inputType];

const cheapMatch = (
  transaction: HistoricalScanTransaction,
  definition: HistoricalFingerprintDefinition,
):
  | Readonly<{ estimatedVbytes: number; feeRate: number; inputType: HistoricalRawHit["inputType"] }>
  | undefined => {
  if (
    transaction.version !== definition.requiredVersion ||
    transaction.locktime !== definition.requiredLocktime ||
    transaction.outputCount !== definition.requiredOutputCount ||
    transaction.outputScriptType !== definition.requiredOutputScriptType ||
    transaction.feeSats === undefined ||
    transaction.inputs.some((input) => !definition.allowedSequences.includes(input.sequence))
  )
    return undefined;
  const observedTypes = new Set(transaction.inputs.map((input) => input.inputType));
  if (observedTypes.size !== 1) return undefined;
  const inputType = transaction.inputs[0]?.inputType;
  if (inputType !== "p2wpkh" && inputType !== "p2sh_p2wpkh" && inputType !== "p2pkh")
    return undefined;
  if (!definition.eligibleInputTypes.includes(inputType)) return undefined;
  const estimatedVbytes =
    definition.fixedOverheadVbytes + inputVbytes(definition, inputType) * transaction.inputs.length;
  const fee = BigInt(transaction.feeSats);
  const estimate = BigInt(estimatedVbytes);
  if (fee <= 0n || fee % estimate !== 0n) return undefined;
  const feeRate = fee / estimate;
  if (
    feeRate < BigInt(definition.feeRateMinimum) ||
    feeRate > BigInt(definition.feeRateMaximum) ||
    feeRate > BigInt(Number.MAX_SAFE_INTEGER)
  )
    return undefined;
  return { estimatedVbytes, feeRate: Number(feeRate), inputType };
};

const rawHit = (
  block: HistoricalScanBlock,
  transaction: HistoricalScanTransaction,
  definition: HistoricalFingerprintDefinition,
  match: NonNullable<ReturnType<typeof cheapMatch>>,
): HistoricalRawHit =>
  strictDecode(HistoricalRawHitSchema, {
    blockHash: block.blockHash,
    blockHeight: block.height,
    blockTime: block.blockTime,
    estimatedVbytes: match.estimatedVbytes,
    feeRateSatsPerVbyte: match.feeRate,
    feeSats: transaction.feeSats,
    fingerprintRevisionDigest: definition.revisionDigest,
    highRSignatures: transaction.inputs.filter((input) => input.signatureRWidthBytes === 33).length,
    inputCount: transaction.inputs.length,
    inputType: match.inputType,
    oneSource:
      transaction.inputs.every((input) => input.pubkeyDigest !== undefined) &&
      new Set(transaction.inputs.map((input) => input.pubkeyDigest)).size === 1,
    sequenceValues: [...new Set(transaction.inputs.map((input) => input.sequence))].toSorted(
      (left, right) => left - right,
    ),
    transactionRef: transaction.transactionRef,
    txid: transaction.txid,
  });

const blockRef = (block: HistoricalScanBlock): string => `block.${block.height}`;

const baseRates = (
  blocks: ReadonlyArray<HistoricalScanBlock>,
  hits: ReadonlyArray<HistoricalRawHit>,
  definition: HistoricalFingerprintDefinition,
): ReadonlyArray<HistoricalBaseRate> => {
  const hitRefs = new Set(hits.map((hit) => hit.transactionRef));
  const strata = new Map<
    string,
    {
      eligible: number;
      matches: number;
      eraRef: string;
      feeRateRef: string;
      scriptType: HistoricalRawHit["inputType"];
    }
  >();
  for (const block of blocks) {
    const eraRef = `era.block-${Math.floor(block.height / 100_000) * 100_000}`;
    for (const transaction of block.transactions) {
      const match = cheapMatch(transaction, definition);
      const inputType = transaction.inputs[0]?.inputType;
      if (inputType !== "p2wpkh" && inputType !== "p2sh_p2wpkh" && inputType !== "p2pkh") continue;
      const feeRateRef = match === undefined ? "fee-rate.non-match" : `fee-rate.${match.feeRate}`;
      const key = `${eraRef}|${feeRateRef}|${inputType}`;
      const current = strata.get(key) ?? {
        eligible: 0,
        matches: 0,
        eraRef,
        feeRateRef,
        scriptType: inputType,
      };
      current.eligible += 1;
      if (hitRefs.has(transaction.transactionRef)) current.matches += 1;
      strata.set(key, current);
    }
  }
  return [...strata.values()].map((stratum) =>
    strictDecode(HistoricalBaseRateSchema, {
      eligibleTransactions: stratum.eligible,
      eraRef: stratum.eraRef,
      feeRateRef: stratum.feeRateRef,
      fingerprintRevisionDigest: definition.revisionDigest,
      matches: stratum.matches,
      matchesPerMillion:
        stratum.eligible === 0 ? 0 : (stratum.matches * 1_000_000) / stratum.eligible,
      scriptType: stratum.scriptType,
    }),
  );
};

export interface RunHistoricalFingerprintScanInput {
  readonly bundle: HistoricalBlockBundle;
  readonly capability: PrivateBitcoinCoreCapability;
  readonly definition: HistoricalFingerprintDefinition;
  readonly priorCheckpoints?: ReadonlyArray<HistoricalScanCheckpoint>;
  readonly priorRawHits?: ReadonlyArray<HistoricalRawHit>;
  readonly reportRef: string;
  readonly workerProfileDigest: string;
}

export const runHistoricalFingerprintScan = (
  input: RunHistoricalFingerprintScanInput,
): HistoricalFingerprintScanReport => {
  const bundle = strictDecode(HistoricalBlockBundleSchema, input.bundle);
  const capability = strictDecode(PrivateBitcoinCoreCapabilitySchema, input.capability);
  const definition = strictDecode(HistoricalFingerprintDefinitionSchema, input.definition);
  const priorCheckpoints = (input.priorCheckpoints ?? []).map((checkpoint) =>
    strictDecode(HistoricalScanCheckpointSchema, checkpoint),
  );
  const rawHits = (input.priorRawHits ?? []).map((hit) =>
    strictDecode(HistoricalRawHitSchema, hit),
  );
  const lastCheckpoint = priorCheckpoints.at(-1);
  if (
    priorCheckpoints.length > 0 &&
    (priorCheckpoints[0]!.completedHeight !== bundle.startHeight ||
      priorCheckpoints.some((checkpoint) => {
        const block = bundle.blocks.find(
          (candidate) => candidate.height === checkpoint.completedHeight,
        );
        return (
          block?.blockHash !== checkpoint.blockHash ||
          checkpoint.fingerprintRevisionDigest !== definition.revisionDigest
        );
      }))
  ) {
    throw new Error("resume checkpoints do not bind the complete current bundle and fingerprint");
  }
  if (lastCheckpoint !== undefined) {
    const block = bundle.blocks.find(
      (candidate) => candidate.height === lastCheckpoint.completedHeight,
    );
    if (
      block?.blockHash !== lastCheckpoint.blockHash ||
      lastCheckpoint.fingerprintRevisionDigest !== definition.revisionDigest ||
      lastCheckpoint.rawHitsDigest !== forensicSha256Digest(rawHits)
    )
      throw new Error(
        "resume checkpoint does not bind the current bundle, fingerprint, and raw hits",
      );
  }

  const transactions = bundle.blocks.flatMap((block) => block.transactions);
  const selfTestPassed = definition.knownPositiveControls.every((control) => {
    const transaction = transactions.find(
      (candidate) => candidate.transactionRef === control.transactionRef,
    );
    const match = transaction === undefined ? undefined : cheapMatch(transaction, definition);
    return (
      transaction?.prevoutDataStatus === "complete" &&
      match?.feeRate === control.expectedFeeRateSatsPerVbyte
    );
  });
  if (!selfTestPassed) {
    return strictDecode(HistoricalFingerprintScanReportSchema, {
      schema: HISTORICAL_SCAN_REPORT_VERSION,
      baseRates: [],
      bundleDigest: bundle.contentDigest,
      capabilityRef: capability.capabilityRef,
      checkpoints: priorCheckpoints,
      cheapCandidateRefs: [],
      expensiveResolutionRefs: [],
      fingerprintRevisionDigest: definition.revisionDigest,
      missingDataRefs: ["missing.self-test-positive"],
      negativeControlStatus: "not_run",
      normalizedOutputDigest: forensicSha256Digest(rawHits.map((hit) => hit.txid).toSorted()),
      outcome: "failed",
      rawHits,
      rawHitsDigest: forensicSha256Digest(rawHits),
      reportRef: input.reportRef,
      scannedTransactions: 0,
      selfTestStatus: "failed",
      similarityClaimCeiling: "program_similarity_only",
      workerProfileDigest: input.workerProfileDigest,
    });
  }

  const blocksToScan = bundle.blocks.filter(
    (block) => lastCheckpoint === undefined || block.height > lastCheckpoint.completedHeight,
  );
  const cheapCandidateRefs: Array<string> = [];
  const expensiveResolutionRefs: Array<string> = [];
  const missingDataRefs: Array<string> = [];
  const checkpoints = [...priorCheckpoints];
  let priorCheckpointDigest = lastCheckpoint?.checkpointDigest;
  for (const block of blocksToScan) {
    for (const transaction of block.transactions) {
      if (transaction.feeSats === undefined) {
        missingDataRefs.push(`missing.fee.${transaction.transactionRef}`);
        continue;
      }
      const match = cheapMatch(transaction, definition);
      if (match === undefined) continue;
      cheapCandidateRefs.push(transaction.transactionRef);
      expensiveResolutionRefs.push(transaction.transactionRef);
      if (transaction.prevoutDataStatus !== "complete") {
        missingDataRefs.push(`missing.prevout.${transaction.transactionRef}`);
        continue;
      }
      rawHits.push(rawHit(block, transaction, definition, match));
    }
    const checkpointValue = {
      blockHash: block.blockHash,
      completedHeight: block.height,
      fingerprintRevisionDigest: definition.revisionDigest,
      ...(priorCheckpointDigest === undefined ? {} : { priorCheckpointDigest }),
      rawHitsDigest: forensicSha256Digest(rawHits),
    };
    const checkpoint = strictDecode(HistoricalScanCheckpointSchema, {
      ...checkpointValue,
      checkpointDigest: forensicSha256Digest(checkpointValue),
    });
    checkpoints.push(checkpoint);
    priorCheckpointDigest = checkpoint.checkpointDigest;
  }

  const negativeRefs = new Set(definition.negativeControlBlockRefs);
  const negativeControlFailed = rawHits.some((hit) => {
    const block = bundle.blocks.find((candidate) => candidate.height === hit.blockHeight);
    return block !== undefined && negativeRefs.has(blockRef(block));
  });
  const deduplicatedHits = [...new Map(rawHits.map((hit) => [hit.txid, hit])).values()].sort(
    (left, right) => left.blockHeight - right.blockHeight || left.txid.localeCompare(right.txid),
  );
  const outcome = missingDataRefs.length > 0 || negativeControlFailed ? "failed" : "succeeded";
  return strictDecode(HistoricalFingerprintScanReportSchema, {
    schema: HISTORICAL_SCAN_REPORT_VERSION,
    baseRates: baseRates(bundle.blocks, deduplicatedHits, definition),
    bundleDigest: bundle.contentDigest,
    capabilityRef: capability.capabilityRef,
    checkpoints,
    cheapCandidateRefs,
    expensiveResolutionRefs,
    fingerprintRevisionDigest: definition.revisionDigest,
    missingDataRefs,
    negativeControlStatus: negativeControlFailed ? "failed" : "passed",
    normalizedOutputDigest: forensicSha256Digest(
      deduplicatedHits.map((hit) => hit.txid).toSorted(),
    ),
    outcome,
    rawHits: deduplicatedHits,
    rawHitsDigest: forensicSha256Digest(deduplicatedHits),
    reportRef: input.reportRef,
    scannedTransactions: blocksToScan.reduce(
      (count, block) => count + block.transactions.length,
      0,
    ),
    selfTestStatus: "passed",
    similarityClaimCeiling: "program_similarity_only",
    workerProfileDigest: input.workerProfileDigest,
  });
};
