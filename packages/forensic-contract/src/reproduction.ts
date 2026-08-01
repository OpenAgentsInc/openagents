import { Schema as S } from "effect";

import {
  BoundedDigests,
  BoundedRefs,
  ClaimKind,
  CommitSha,
  EvidenceKind,
  Exactness,
  ForensicRef,
  ForensicTimestamp,
  LongText,
  NonEmptyBoundedRefs,
  NonNegativeInteger,
  PositiveDecimalString,
  PositiveInteger,
  Sha256Digest,
  ShortText,
} from "./primitives.ts";

export const COLDCARD_REPRODUCTION_MANIFEST_VERSION =
  "openagents.coldcard_reproduction_manifest.v1" as const;
export const GENERATOR_TRACE_VERSION = "openagents.generator_trace.v1" as const;
export const ENTROPY_STATE_MODEL_VERSION = "openagents.entropy_state_model.v1" as const;
export const HISTORICAL_CHAIN_SNAPSHOT_VERSION = "openagents.historical_chain_snapshot.v1" as const;
export const TRANSACTION_FINGERPRINT_VERSION = "openagents.transaction_fingerprint.v1" as const;
export const NODE_SCAN_RECEIPT_VERSION = "openagents.node_scan_receipt.v1" as const;

export const ColdcardSuite = S.Literals([
  "code_to_artifact",
  "generator_owned_fixture",
  "historical_chain_fingerprint",
  "evidence_graph",
]);
export type ColdcardSuite = typeof ColdcardSuite.Type;

const REQUIRED_COLDCARD_SUITES: ReadonlyArray<ColdcardSuite> = [
  "code_to_artifact",
  "generator_owned_fixture",
  "historical_chain_fingerprint",
  "evidence_graph",
];

const REQUIRED_PINNED_REVISION_ROLES = [
  "vulnerable_target",
  "fixed_target",
  "libngu",
  "micropython",
  "ckcc_protocol",
  "mpy_qr",
  "postmortem",
] as const;

const comparePositiveDecimals = (left: string, right: string): number => {
  const [leftInteger = "0", leftFraction = ""] = left.split(".");
  const [rightInteger = "0", rightFraction = ""] = right.split(".");
  const normalizedLeftInteger = leftInteger.replace(/^0+(?=\d)/, "");
  const normalizedRightInteger = rightInteger.replace(/^0+(?=\d)/, "");
  if (normalizedLeftInteger.length !== normalizedRightInteger.length) {
    return normalizedLeftInteger.length < normalizedRightInteger.length ? -1 : 1;
  }
  const integerOrder = normalizedLeftInteger.localeCompare(normalizedRightInteger);
  if (integerOrder !== 0) return integerOrder;
  const fractionLength = Math.max(leftFraction.length, rightFraction.length);
  return leftFraction
    .padEnd(fractionLength, "0")
    .localeCompare(rightFraction.padEnd(fractionLength, "0"));
};

export const PinnedRevisionSchema = S.Struct({
  role: S.Literals([
    "vulnerable_target",
    "fixed_target",
    "libngu",
    "micropython",
    "ckcc_protocol",
    "mpy_qr",
    "postmortem",
  ]),
  repositoryRef: ForensicRef,
  commitSha: CommitSha,
  treeDigest: Sha256Digest,
});
export interface PinnedRevision extends S.Schema.Type<typeof PinnedRevisionSchema> {}

export const ClaimGateSchema = S.Struct({
  claimKind: ClaimKind,
  requiredEvidenceKinds: S.Array(EvidenceKind).check(S.isMinLength(1), S.isMaxLength(32)),
});
export interface ClaimGate extends S.Schema.Type<typeof ClaimGateSchema> {}

export const ColdcardReproductionManifestSchema = S.Struct({
  schema: S.Literal(COLDCARD_REPRODUCTION_MANIFEST_VERSION),
  manifestRef: ForensicRef,
  pinnedRevisions: S.Array(PinnedRevisionSchema).check(S.isMinLength(7), S.isMaxLength(16)),
  suites: S.Array(ColdcardSuite).check(S.isMinLength(4), S.isMaxLength(4)),
  rawInputRefs: NonEmptyBoundedRefs,
  expectedComparisonRefs: BoundedRefs,
  controlRefs: NonEmptyBoundedRefs,
  assumptionRefs: NonEmptyBoundedRefs,
  claimGates: S.Array(ClaimGateSchema).check(S.isMinLength(8), S.isMaxLength(8)),
  disclosurePosture: S.Literal("private_manual_reporting"),
  evaluatorDigest: Sha256Digest,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (manifest) =>
          REQUIRED_COLDCARD_SUITES.every((suite) => manifest.suites.includes(suite)) &&
          new Set(manifest.suites).size === REQUIRED_COLDCARD_SUITES.length,
        { message: "Coldcard reproduction manifest requires each suite exactly once" },
      ),
      S.makeFilter(
        (manifest) => new Set(manifest.claimGates.map((gate) => gate.claimKind)).size === 8,
        { message: "Coldcard reproduction manifest requires one gate per claim rung" },
      ),
      S.makeFilter(
        (manifest) =>
          REQUIRED_PINNED_REVISION_ROLES.every((role) =>
            manifest.pinnedRevisions.some((revision) => revision.role === role),
          ) && new Set(manifest.pinnedRevisions.map((revision) => revision.role)).size === 7,
        { message: "Coldcard reproduction manifest requires one pin per required revision role" },
      ),
      S.makeFilter(
        (manifest) =>
          manifest.expectedComparisonRefs.every(
            (reference) => !manifest.rawInputRefs.includes(reference),
          ),
        { message: "Coldcard postmortem comparisons cannot enter reproduction inputs" },
      ),
    ),
  )
  .annotate({ identifier: "ColdcardReproductionManifest" });
export interface ColdcardReproductionManifest extends S.Schema.Type<
  typeof ColdcardReproductionManifestSchema
> {}

export const GeneratorCallSchema = S.Struct({
  sequence: PositiveInteger,
  operation: S.Literals(["initialize", "reseed", "shuffle", "random_bytes", "derive_seed"]),
  inputDigest: Sha256Digest,
  outputDigest: Sha256Digest,
  stateDigest: Sha256Digest,
});
export interface GeneratorCall extends S.Schema.Type<typeof GeneratorCallSchema> {}

export const GeneratorTraceSchema = S.Struct({
  schema: S.Literal(GENERATOR_TRACE_VERSION),
  traceRef: ForensicRef,
  runRef: ForensicRef,
  implementationRef: ForensicRef,
  implementationCommit: CommitSha,
  initialStateDigest: Sha256Digest,
  reseedInputsDigest: Sha256Digest,
  retainedWidthBits: NonNegativeInteger,
  calls: S.Array(GeneratorCallSchema).check(S.isMinLength(1), S.isMaxLength(100_000)),
  outputDigest: Sha256Digest,
  goldenVectorRef: ForensicRef,
  toolchainDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  receiptRefs: NonEmptyBoundedRefs,
  observedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter((trace) => trace.calls.every((call, index) => call.sequence === index + 1), {
        message: "generator calls must have a dense sequence starting at one",
      }),
    ),
  )
  .annotate({ identifier: "GeneratorTrace" });
export interface GeneratorTrace extends S.Schema.Type<typeof GeneratorTraceSchema> {}

export const EntropyInputSchema = S.Struct({
  inputRef: ForensicRef,
  kind: S.Literals(["uid", "timer", "user_interaction", "secure_element", "reseed", "call_trace"]),
  knowledge: S.Literals(["known", "bounded", "unknown", "not_present"]),
  candidateCountLower: PositiveDecimalString,
  candidateCountUpper: PositiveDecimalString,
  assumptionRef: ForensicRef,
});
export interface EntropyInput extends S.Schema.Type<typeof EntropyInputSchema> {}

export const EntropySensitivitySchema = S.Struct({
  variantRef: ForensicRef,
  changedAssumptionRefs: NonEmptyBoundedRefs,
  candidateCountLower: PositiveDecimalString,
  candidateCountUpper: PositiveDecimalString,
  entropyBitsLower: S.Number.check(
    S.isFinite(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(256),
  ),
  entropyBitsUpper: S.Number.check(
    S.isFinite(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(256),
  ),
});
export interface EntropySensitivity extends S.Schema.Type<typeof EntropySensitivitySchema> {}

export const EntropyStateModelSchema = S.Struct({
  schema: S.Literal(ENTROPY_STATE_MODEL_VERSION),
  modelRef: ForensicRef,
  hardwareClassRef: ForensicRef,
  firmwareRef: ForensicRef,
  generatorTraceRef: ForensicRef,
  inputs: S.Array(EntropyInputSchema).check(S.isMinLength(1), S.isMaxLength(128)),
  candidateCountLower: PositiveDecimalString,
  candidateCountUpper: PositiveDecimalString,
  entropyBitsLower: S.Number.check(
    S.isFinite(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(256),
  ),
  entropyBitsUpper: S.Number.check(
    S.isFinite(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(256),
  ),
  sensitivity: S.Array(EntropySensitivitySchema).check(S.isMinLength(1), S.isMaxLength(128)),
  enumerationPlanRef: ForensicRef,
  independentReviewRef: ForensicRef,
  assumptionRefs: NonEmptyBoundedRefs,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (model) =>
          model.entropyBitsLower <= model.entropyBitsUpper &&
          comparePositiveDecimals(model.candidateCountLower, model.candidateCountUpper) <= 0,
        { message: "entropy and candidate lower bounds cannot exceed upper bounds" },
      ),
    ),
  )
  .annotate({ identifier: "EntropyStateModel" });
export interface EntropyStateModel extends S.Schema.Type<typeof EntropyStateModelSchema> {}

export const ChainBlockSchema = S.Struct({
  height: NonNegativeInteger,
  blockHash: Sha256Digest,
  rawResponseDigest: Sha256Digest,
});
export interface ChainBlock extends S.Schema.Type<typeof ChainBlockSchema> {}

export const HistoricalChainSnapshotSchema = S.Struct({
  schema: S.Literal(HISTORICAL_CHAIN_SNAPSHOT_VERSION),
  snapshotRef: ForensicRef,
  network: S.Literals(["mainnet", "signet", "regtest"]),
  genesisHash: Sha256Digest,
  startHeight: NonNegativeInteger,
  endHeight: NonNegativeInteger,
  blocks: S.Array(ChainBlockSchema).check(S.isMinLength(1), S.isMaxLength(100_000)),
  sourceClass: S.Literals(["content_addressed_bundle", "private_bitcoin_core"]),
  sourceVersionRef: ForensicRef,
  sourceIdentityDigest: Sha256Digest,
  amountEncoding: S.Literal("satoshi_integer_string"),
  captureExactness: Exactness,
  capturedAt: ForensicTimestamp,
  retentionExpiresAt: ForensicTimestamp,
  materializationReceiptRef: ForensicRef,
})
  .pipe(
    S.check(
      S.makeFilter(
        (snapshot) =>
          snapshot.endHeight >= snapshot.startHeight &&
          snapshot.blocks.length === snapshot.endHeight - snapshot.startHeight + 1 &&
          snapshot.blocks.every((block, index) => block.height === snapshot.startHeight + index),
        { message: "chain snapshot blocks must densely cover the declared height range" },
      ),
    ),
  )
  .annotate({ identifier: "HistoricalChainSnapshot" });
export interface HistoricalChainSnapshot extends S.Schema.Type<
  typeof HistoricalChainSnapshotSchema
> {}

export const FingerprintFeatureSchema = S.Struct({
  featureRef: ForensicRef,
  kind: S.Literals([
    "fee_arithmetic",
    "transaction_shape",
    "locktime",
    "sequence",
    "signature_r",
    "script_type",
    "pace",
    "pooling",
  ]),
  rule: LongText,
  ruleDigest: Sha256Digest,
});
export interface FingerprintFeature extends S.Schema.Type<typeof FingerprintFeatureSchema> {}

export const FingerprintBaseRateSchema = S.Struct({
  regimeRef: ForensicRef,
  eligibleTransactions: PositiveInteger,
  matches: NonNegativeInteger,
  matchesPerMillion: S.Number.check(S.isFinite(), S.isGreaterThanOrEqualTo(0)),
  exactness: Exactness,
});
export interface FingerprintBaseRate extends S.Schema.Type<typeof FingerprintBaseRateSchema> {}

export const TransactionFingerprintSchema = S.Struct({
  schema: S.Literal(TRANSACTION_FINGERPRINT_VERSION),
  fingerprintRef: ForensicRef,
  revisionDigest: Sha256Digest,
  features: S.Array(FingerprintFeatureSchema).check(S.isMinLength(1), S.isMaxLength(64)),
  eligibilityRuleRef: ForensicRef,
  thresholdPolicyRef: ForensicRef,
  positiveControlRefs: NonEmptyBoundedRefs,
  negativeControlRefs: NonEmptyBoundedRefs,
  baseRates: S.Array(FingerprintBaseRateSchema).check(S.isMinLength(1), S.isMaxLength(128)),
  clusterRuleRef: ForensicRef,
  exclusionRefs: BoundedRefs,
  claimCeiling: S.Literal("program_fingerprint"),
  createdAt: ForensicTimestamp,
}).annotate({ identifier: "TransactionFingerprint" });
export interface TransactionFingerprint extends S.Schema.Type<
  typeof TransactionFingerprintSchema
> {}

export const ScannedRangeSchema = S.Struct({
  startHeight: NonNegativeInteger,
  endHeight: NonNegativeInteger,
  completedBlockHash: Sha256Digest,
  checkpointDigest: Sha256Digest,
}).pipe(
  S.check(
    S.makeFilter((range) => range.endHeight >= range.startHeight, {
      message: "scanned range end height cannot precede its start height",
    }),
  ),
);
export interface ScannedRange extends S.Schema.Type<typeof ScannedRangeSchema> {}

export const CandidateFunnelSchema = S.Struct({
  transactionsScanned: NonNegativeInteger,
  cheapCandidates: NonNegativeInteger,
  exactMatches: NonNegativeInteger,
  clusteredMatches: NonNegativeInteger,
  evidenceLinkedWaves: NonNegativeInteger,
}).pipe(
  S.check(
    S.makeFilter(
      (funnel) =>
        funnel.transactionsScanned >= funnel.cheapCandidates &&
        funnel.cheapCandidates >= funnel.exactMatches &&
        funnel.exactMatches >= funnel.clusteredMatches &&
        funnel.clusteredMatches >= funnel.evidenceLinkedWaves,
      { message: "candidate funnel counts must be monotonically non-increasing" },
    ),
  ),
);
export interface CandidateFunnel extends S.Schema.Type<typeof CandidateFunnelSchema> {}

export const NodeScanReceiptSchema = S.Struct({
  schema: S.Literal(NODE_SCAN_RECEIPT_VERSION),
  receiptRef: ForensicRef,
  runRef: ForensicRef,
  scanProfileRef: ForensicRef,
  chainSnapshotRef: ForensicRef,
  fingerprintRef: ForensicRef,
  completedRanges: S.Array(ScannedRangeSchema).check(S.isMaxLength(10_000)),
  selfTest: S.Literals(["passed", "failed", "not_run"]),
  negativeControls: S.Literals(["passed", "failed", "not_run"]),
  rawHitsDigest: Sha256Digest,
  normalizedDatasetDigest: Sha256Digest,
  candidateFunnel: CandidateFunnelSchema,
  resumeState: S.Literals(["not_required", "checkpointed", "resumed", "complete"]),
  missingDataRefs: BoundedRefs,
  workerReceiptRefs: NonEmptyBoundedRefs,
  outcome: S.Literals(["succeeded", "failed", "inconclusive"]),
  observedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (receipt) =>
          receipt.outcome !== "succeeded" ||
          (receipt.selfTest === "passed" &&
            receipt.negativeControls === "passed" &&
            receipt.missingDataRefs.length === 0),
        {
          message:
            "successful node scans require passed positive and negative controls and no missing data",
        },
      ),
    ),
  )
  .annotate({ identifier: "NodeScanReceipt" });
export interface NodeScanReceipt extends S.Schema.Type<typeof NodeScanReceiptSchema> {}

export const ReproductionComparisonSchema = S.Struct({
  comparisonRef: ForensicRef,
  expectedDigest: Sha256Digest,
  observedDigest: S.optionalKey(Sha256Digest),
  status: S.Literals(["match", "drift", "unavailable"]),
  note: S.optionalKey(ShortText),
  sourceDigests: BoundedDigests,
});
export interface ReproductionComparison extends S.Schema.Type<
  typeof ReproductionComparisonSchema
> {}
