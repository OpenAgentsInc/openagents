import { Schema as S } from "effect";

import {
  BoundedRefs,
  CommitSha,
  EvidenceTier,
  ForensicPath,
  ForensicRef,
  ForensicTimestamp,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
  ShortText,
} from "./primitives.ts";
import { ColdcardSuite } from "./reproduction.ts";

export const COLDCARD_BENCHMARK_MANIFEST_VERSION =
  "openagents.coldcard_benchmark_manifest.v1" as const;
export const COLDCARD_SUITE_MANIFEST_VERSION = "openagents.coldcard_suite_manifest.v1" as const;
export const COLDCARD_HISTORICAL_IMPORT_VERSION =
  "openagents.coldcard_historical_import.v1" as const;

export const ColdcardArmKind = S.Literals([
  "incomplete_clone",
  "complete_vulnerable",
  "fixed_clone",
  "structural_variants",
  "clean_controls",
]);
export type ColdcardArmKind = typeof ColdcardArmKind.Type;

const REQUIRED_ARM_KINDS: ReadonlyArray<ColdcardArmKind> = [
  "incomplete_clone",
  "complete_vulnerable",
  "fixed_clone",
  "structural_variants",
  "clean_controls",
];

const REQUIRED_DATASET_SPLITS = ["train", "development", "holdout", "clean_holdout"] as const;

export const ColdcardBenchmarkArmSchema = S.Struct({
  armRef: ForensicRef,
  kind: ColdcardArmKind,
  datasetSplit: S.Literals(["development", "control"]),
  targetRevisionRole: S.Literals(["vulnerable_target", "fixed_target", "fixture"]),
  fixtureDigest: Sha256Digest,
  coverageStatus: S.Literals(["complete", "incomplete"]),
  missingDependencyPaths: S.Array(ForensicPath).check(S.isMaxLength(32)),
  expectedRunState: S.Literals(["completed", "completed_incomplete"]),
  expectedVerdict: S.Literals([
    "not_scored_incomplete",
    "source_hit_unverified",
    "historical_finding_absent",
    "semantic_variant_hit",
    "no_false_positive",
  ]),
  expectedEvidenceTier: EvidenceTier,
  finalLinkOutcome: S.Literals(["not_proven", "not_applicable"]),
  requiredCausalLinkRefs: BoundedRefs,
  exactMatchResistanceRefs: BoundedRefs,
  controlCaseRefs: BoundedRefs,
  modelCallPolicy: S.Literals(["blocked_before_inference", "allowed"]),
}).pipe(
  S.check(
    S.makeFilter(
      (arm) =>
        arm.kind !== "incomplete_clone" ||
        (arm.coverageStatus === "incomplete" &&
          arm.missingDependencyPaths.length > 0 &&
          arm.expectedRunState === "completed_incomplete" &&
          arm.expectedVerdict === "not_scored_incomplete" &&
          arm.modelCallPolicy === "blocked_before_inference"),
      { message: "incomplete Coldcard arm must remain blocked and completed_incomplete" },
    ),
    S.makeFilter(
      (arm) =>
        arm.kind !== "complete_vulnerable" ||
        (arm.coverageStatus === "complete" &&
          arm.expectedVerdict === "source_hit_unverified" &&
          arm.expectedEvidenceTier === "source_observed" &&
          arm.finalLinkOutcome === "not_proven"),
      { message: "complete vulnerable arm is an unverified source hit, not artifact proof" },
    ),
    S.makeFilter(
      (arm) => arm.kind !== "fixed_clone" || arm.expectedVerdict === "historical_finding_absent",
      { message: "fixed Coldcard arm must reject the historical finding" },
    ),
    S.makeFilter(
      (arm) => arm.kind !== "structural_variants" || arm.exactMatchResistanceRefs.length >= 5,
      { message: "structural variants must resist every frozen exact-match shortcut" },
    ),
    S.makeFilter((arm) => arm.kind !== "clean_controls" || arm.controlCaseRefs.length >= 3, {
      message: "clean controls require hardware, unavailable, and non-secret cases",
    }),
  ),
);
export interface ColdcardBenchmarkArm extends S.Schema.Type<typeof ColdcardBenchmarkArmSchema> {}

export const ColdcardCausalLinkSchema = S.Struct({
  causalLinkRef: ForensicRef,
  sequence: PositiveInteger,
  proposition: ShortText,
  requiredSourcePaths: S.Array(ForensicPath).check(S.isMinLength(1), S.isMaxLength(32)),
});

export const ColdcardRubricSchema = S.Struct({
  rubricRef: ForensicRef,
  rubricDigest: Sha256Digest,
  causalLinks: S.Array(ColdcardCausalLinkSchema).check(S.isMinLength(6), S.isMaxLength(6)),
  sourceHitRequiresAllCausalLinks: S.Literal(true),
  genericRandomnessAdviceIsHit: S.Literal(false),
  sourceTierFinalLinkOutcome: S.Literal("not_proven"),
  fixedHistoricalFindingAllowed: S.Literal(false),
  incompleteComprehensiveClaimAllowed: S.Literal(false),
  structuralExactMatchAllowed: S.Literal(false),
  maxTimeSeconds: PositiveInteger,
  maxTokens: PositiveInteger,
  maxCostMicros: NonNegativeInteger,
  maxConcurrency: S.Literal(1),
}).pipe(
  S.check(
    S.makeFilter(
      (rubric) => rubric.causalLinks.every((link, index) => link.sequence === index + 1),
      { message: "Coldcard causal links must have a dense frozen sequence" },
    ),
  ),
);

export const ColdcardDatasetSplitSchema = S.Struct({
  split: S.Literals(REQUIRED_DATASET_SPLITS),
  ownerRef: ForensicRef,
  manifestDigest: Sha256Digest,
  optimizerVisibility: S.Literals(["optimizer_visible", "evaluator_only"]),
  benchmarkArmRefs: BoundedRefs,
});

export const ColdcardSuiteBindingSchema = S.Struct({
  suite: ColdcardSuite,
  manifestRef: ForensicRef,
  manifestDigest: Sha256Digest,
});

export const ColdcardBenchmarkManifestSchema = S.Struct({
  schema: S.Literal(COLDCARD_BENCHMARK_MANIFEST_VERSION),
  benchmarkRef: ForensicRef,
  vulnerableCommit: CommitSha,
  fixedCommit: CommitSha,
  reproductionManifestRef: ForensicRef,
  reproductionManifestDigest: Sha256Digest,
  treeDigestAlgorithm: S.Literal("sha256_git_ls_tree_r_z_v1"),
  arms: S.Array(ColdcardBenchmarkArmSchema).check(S.isMinLength(5), S.isMaxLength(5)),
  rubric: ColdcardRubricSchema,
  datasetSplits: S.Array(ColdcardDatasetSplitSchema).check(S.isMinLength(4), S.isMaxLength(4)),
  suites: S.Array(ColdcardSuiteBindingSchema).check(S.isMinLength(4), S.isMaxLength(4)),
  historicalImportRef: ForensicRef,
  historicalImportDigest: Sha256Digest,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (manifest) =>
          REQUIRED_ARM_KINDS.every((kind) => manifest.arms.some((arm) => arm.kind === kind)) &&
          new Set(manifest.arms.map((arm) => arm.kind)).size === REQUIRED_ARM_KINDS.length,
        { message: "Coldcard benchmark requires each arm exactly once" },
      ),
      S.makeFilter(
        (manifest) =>
          REQUIRED_DATASET_SPLITS.every((split) =>
            manifest.datasetSplits.some((candidate) => candidate.split === split),
          ) && new Set(manifest.datasetSplits.map((split) => split.split)).size === 4,
        { message: "Coldcard benchmark requires each dataset split exactly once" },
      ),
      S.makeFilter(
        (manifest) =>
          manifest.datasetSplits
            .filter((split) => split.split === "holdout" || split.split === "clean_holdout")
            .every(
              (split) =>
                split.optimizerVisibility === "evaluator_only" &&
                split.benchmarkArmRefs.length === 0,
            ),
        { message: "Coldcard development arms cannot enter evaluator-only holdouts" },
      ),
      S.makeFilter(
        (manifest) =>
          new Set(manifest.datasetSplits.map((split) => split.manifestDigest)).size === 4 &&
          new Set(manifest.datasetSplits.map((split) => split.ownerRef)).size === 4,
        { message: "dataset splits require separately owned digest pins" },
      ),
      S.makeFilter(
        (manifest) =>
          new Set(manifest.suites.map((suite) => suite.suite)).size === 4 &&
          new Set(manifest.suites.map((suite) => suite.manifestDigest)).size === 4,
        { message: "Coldcard suite manifests must be complete and independently pinned" },
      ),
    ),
  )
  .annotate({ identifier: "ColdcardBenchmarkManifest" });
export interface ColdcardBenchmarkManifest extends S.Schema.Type<
  typeof ColdcardBenchmarkManifestSchema
> {}

export const ColdcardSuiteInputSchema = S.Struct({
  inputRef: ForensicRef,
  status: S.Literals(["available", "required_unmaterialized"]),
  digest: S.optionalKey(Sha256Digest),
  unavailableReasonRef: S.optionalKey(ForensicRef),
}).pipe(
  S.check(
    S.makeFilter(
      (input) =>
        (input.status === "available" &&
          input.digest !== undefined &&
          input.unavailableReasonRef === undefined) ||
        (input.status === "required_unmaterialized" &&
          input.digest === undefined &&
          input.unavailableReasonRef !== undefined),
      { message: "suite input availability must carry exactly the matching proof" },
    ),
  ),
);

export const ColdcardSuiteManifestSchema = S.Struct({
  schema: S.Literal(COLDCARD_SUITE_MANIFEST_VERSION),
  suiteRef: ForensicRef,
  suite: ColdcardSuite,
  executionTarget: S.Literals(["openagents_gce", "source_only"]),
  inputs: S.Array(ColdcardSuiteInputSchema).check(S.isMinLength(1), S.isMaxLength(128)),
  evaluatorInputRefs: BoundedRefs,
  expectedComparisonRefs: BoundedRefs,
  controlRefs: BoundedRefs.check(S.isMinLength(1)),
  requiredCheckRefs: BoundedRefs.check(S.isMinLength(1)),
  outputContractRefs: BoundedRefs.check(S.isMinLength(1)),
  permittedClaimRefs: BoundedRefs,
  forbiddenClaimRefs: BoundedRefs.check(S.isMinLength(1)),
  evaluatorDigest: Sha256Digest,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (manifest) =>
          manifest.expectedComparisonRefs.every(
            (reference) =>
              !manifest.evaluatorInputRefs.includes(reference) &&
              !manifest.inputs.some((input) => input.inputRef === reference),
          ),
        { message: "postmortem comparison outputs cannot enter derivation or evaluator inputs" },
      ),
      S.makeFilter(
        (manifest) =>
          manifest.suite !== "historical_chain_fingerprint" ||
          manifest.executionTarget === "openagents_gce",
        { message: "historical chain suite is admitted only on OpenAgents GCE" },
      ),
    ),
  )
  .annotate({ identifier: "ColdcardSuiteManifest" });
export interface ColdcardSuiteManifest extends S.Schema.Type<typeof ColdcardSuiteManifestSchema> {}

const UnavailableMeasurementSchema = S.Struct({
  exactness: S.Literal("unavailable"),
  unavailableReasonRef: ForensicRef,
});
const AvailableMeasurementSchema = S.Struct({
  exactness: S.Literals(["exact", "estimated", "upper_bound"]),
  value: NonNegativeInteger,
});
export const HistoricalMeasurementSchema = S.Union([
  UnavailableMeasurementSchema,
  AvailableMeasurementSchema,
]);

export const ColdcardHistoricalRunSchema = S.Struct({
  runRef: ForensicRef,
  armRef: ForensicRef,
  inputCompleteness: S.Literals(["complete", "incomplete"]),
  importedRunState: S.Literals(["completed", "completed_incomplete"]),
  verdict: S.Literals(["hit", "miss"]),
  findingCount: NonNegativeInteger,
  highSeverityCount: NonNegativeInteger,
  mediumSeverityCount: NonNegativeInteger,
  qualifiedFindingRefs: BoundedRefs,
  evidenceTier: EvidenceTier,
  verificationState: S.Literal("disabled"),
  wallDurationMilliseconds: HistoricalMeasurementSchema,
  tokenUsage: HistoricalMeasurementSchema,
}).pipe(
  S.check(
    S.makeFilter(
      (run) =>
        run.inputCompleteness !== "incomplete" ||
        (run.importedRunState === "completed_incomplete" && run.qualifiedFindingRefs.length === 0),
      { message: "historical incomplete run cannot import a qualified complete result" },
    ),
    S.makeFilter(
      (run) =>
        run.verdict !== "hit" ||
        (run.evidenceTier === "source_observed" && run.qualifiedFindingRefs.length > 0),
      { message: "historical hit is source-observed and requires a finding ref" },
    ),
  ),
);

export const ColdcardHistoricalImportSchema = S.Struct({
  schema: S.Literal(COLDCARD_HISTORICAL_IMPORT_VERSION),
  importRef: ForensicRef,
  episodeRef: S.Literal("docs/transcripts/264.md"),
  resultsRef: S.Literal("docs/loupe/2026-08-01-coldcard-prefix-experiment-results.md"),
  sourceExperimentRef: S.Literal("docs/loupe/2026-08-01-coldcard-prefix-experiment.md"),
  runs: S.Array(ColdcardHistoricalRunSchema).check(S.isMinLength(2), S.isMaxLength(2)),
  importedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (historicalImport) =>
          historicalImport.runs.some(
            (run) =>
              run.armRef === "arm.coldcard.incomplete" &&
              run.importedRunState === "completed_incomplete" &&
              run.verdict === "miss",
          ) &&
          historicalImport.runs.some(
            (run) =>
              run.armRef === "arm.coldcard.complete-vulnerable" &&
              run.inputCompleteness === "complete" &&
              run.verdict === "hit" &&
              run.evidenceTier === "source_observed",
          ),
        { message: "Episode 264 import requires honest incomplete and source-hit arms" },
      ),
    ),
  )
  .annotate({ identifier: "ColdcardHistoricalImport" });
export interface ColdcardHistoricalImport extends S.Schema.Type<
  typeof ColdcardHistoricalImportSchema
> {}
