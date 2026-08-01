import { Schema as S } from "effect";

import { forensicSha256Digest } from "./canonical.ts";
import {
  BoundedRefs,
  BoundedShortTexts,
  CommitSha,
  DurationTruthSchema,
  Exactness,
  ForensicPath,
  ForensicRef,
  ForensicTimestamp,
  LongText,
  NonEmptyBoundedRefs,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
  ShortText,
  UsageTruthSchema,
} from "./primitives.ts";

export const FORENSIC_TARGET_SNAPSHOT_VERSION = "openagents.forensic_target_snapshot.v1" as const;
export const FORENSIC_SOURCE_BUNDLE_VERSION = "openagents.forensic_source_bundle.v1" as const;
export const FORENSIC_COVERAGE_MANIFEST_VERSION =
  "openagents.forensic_coverage_manifest.v1" as const;
export const FORENSIC_SCAN_PROFILE_VERSION = "openagents.forensic_scan_profile.v1" as const;
export const FORENSIC_WORKER_PLACEMENT_VERSION = "openagents.forensic_worker_placement.v1" as const;
export const FORENSIC_PROMPT_ARTIFACT_VERSION = "openagents.forensic_prompt_artifact.v1" as const;
export const FORENSIC_RUN_VERSION = "openagents.forensic_run.v1" as const;
export const FORENSIC_EVIDENCE_RECEIPT_VERSION = "openagents.forensic_evidence_receipt.v1" as const;
export const FORENSIC_RUN_EVENT_VERSION = "openagents.forensic_run_event.v1" as const;

export const DirtyState = S.Literals(["clean", "dirty", "externally_prepared"]);
export type DirtyState = typeof DirtyState.Type;

export const ForensicTargetSnapshotSchema = S.Struct({
  schema: S.Literal(FORENSIC_TARGET_SNAPSHOT_VERSION),
  targetRef: ForensicRef,
  repositoryRef: ForensicRef,
  commitSha: CommitSha,
  sourceDigest: Sha256Digest,
  dirtyState: DirtyState,
  dependencyPolicyRef: ForensicRef,
  toolchainRefs: BoundedRefs,
  authorizationRefs: NonEmptyBoundedRefs,
  capturedAt: ForensicTimestamp,
}).annotate({ identifier: "ForensicTargetSnapshot" });
export interface ForensicTargetSnapshot extends S.Schema.Type<
  typeof ForensicTargetSnapshotSchema
> {}

export const ForensicSubmodulePinSchema = S.Struct({
  path: ForensicPath,
  commitSha: CommitSha,
  treeDigest: Sha256Digest,
});
export interface ForensicSubmodulePin extends S.Schema.Type<typeof ForensicSubmodulePinSchema> {}

export const ForensicSourceBundleSchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_BUNDLE_VERSION),
  bundleRef: ForensicRef,
  targetRef: ForensicRef,
  repositoryRef: ForensicRef,
  commitSha: CommitSha,
  treeDigest: Sha256Digest,
  sourceDigest: Sha256Digest,
  declaredSubmodules: S.Array(ForensicSubmodulePinSchema).check(S.isMaxLength(128)),
  dependencyManifestDigest: Sha256Digest,
  artifactRef: ForensicRef,
  builderRef: ForensicRef,
  retentionExpiresAt: ForensicTimestamp,
  materializationReceiptRef: ForensicRef,
  createdAt: ForensicTimestamp,
}).annotate({ identifier: "ForensicSourceBundle" });
export interface ForensicSourceBundle extends S.Schema.Type<typeof ForensicSourceBundleSchema> {}

export const CoverageStatus = S.Literals(["complete", "incomplete", "denied"]);
export type CoverageStatus = typeof CoverageStatus.Type;

export const CoverageEntrySchema = S.Struct({
  path: ForensicPath,
  classification: S.Literals(["target", "dependency", "generated", "excluded", "oversized"]),
  presence: S.Literals(["present", "absent", "not_applicable"]),
  required: S.Boolean,
  contentDigest: S.optionalKey(Sha256Digest),
  reasonRef: S.optionalKey(ForensicRef),
});
export interface CoverageEntry extends S.Schema.Type<typeof CoverageEntrySchema> {}

export const ForensicCoverageManifestSchema = S.Struct({
  schema: S.Literal(FORENSIC_COVERAGE_MANIFEST_VERSION),
  coverageRef: ForensicRef,
  bundleRef: ForensicRef,
  status: CoverageStatus,
  entries: S.Array(CoverageEntrySchema).check(S.isMinLength(1), S.isMaxLength(20_000)),
  incompleteReasonRefs: BoundedRefs,
  generatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (manifest) => {
          const hasMissingRequired = manifest.entries.some(
            (entry) => entry.required && entry.presence === "absent",
          );
          if (manifest.status === "complete") {
            return !hasMissingRequired && manifest.incompleteReasonRefs.length === 0;
          }
          return manifest.incompleteReasonRefs.length > 0;
        },
        {
          message:
            "complete coverage cannot omit required inputs; incomplete or denied coverage requires reasons",
        },
      ),
    ),
  )
  .annotate({ identifier: "ForensicCoverageManifest" });
export interface ForensicCoverageManifest extends S.Schema.Type<
  typeof ForensicCoverageManifestSchema
> {}

export const ForensicBudgetSchema = S.Struct({
  maxTimeSeconds: PositiveInteger,
  maxTokens: NonNegativeInteger,
  maxCostMicros: NonNegativeInteger,
  maxConcurrency: PositiveInteger,
  maxArtifactBytes: NonNegativeInteger,
  maxNetworkBytes: NonNegativeInteger,
});
export interface ForensicBudget extends S.Schema.Type<typeof ForensicBudgetSchema> {}

export const ForensicScanProfileSchema = S.Struct({
  schema: S.Literal(FORENSIC_SCAN_PROFILE_VERSION),
  profileRef: ForensicRef,
  scopeRankingRefs: NonEmptyBoundedRefs,
  vulnerabilityClasses: BoundedShortTexts.check(S.isMinLength(1)),
  modelMatrixRef: ForensicRef,
  promptArtifactRef: ForensicRef,
  toolRefs: NonEmptyBoundedRefs,
  sandboxProfileRef: ForensicRef,
  networkPolicyRef: ForensicRef,
  budget: ForensicBudgetSchema,
  createdAt: ForensicTimestamp,
}).annotate({ identifier: "ForensicScanProfile" });
export interface ForensicScanProfile extends S.Schema.Type<typeof ForensicScanProfileSchema> {}

export const WorkerPlacementState = S.Literals([
  "admission_requested",
  "refused",
  "provisioning",
  "worker_ready",
  "running",
  "stopping",
  "deleting",
  "cleaned",
  "recovery_required",
]);
export type WorkerPlacementState = typeof WorkerPlacementState.Type;

export const ForensicWorkerPlacementSchema = S.Struct({
  schema: S.Literal(FORENSIC_WORKER_PLACEMENT_VERSION),
  placementRef: ForensicRef,
  ownerRef: ForensicRef,
  tenantRef: ForensicRef,
  workUnitRef: ForensicRef,
  sandboxRef: ForensicRef,
  attachmentGeneration: NonNegativeInteger,
  resourceGeneration: NonNegativeInteger,
  targetClass: S.Literal("openagents_managed"),
  provider: S.Literal("google_cloud"),
  adapterRef: S.Literal("adapter.oa-codex-control.gce.v1"),
  isolation: S.Literal("gce_vm"),
  regionRef: ForensicRef,
  imageDigest: Sha256Digest,
  profileDigest: Sha256Digest,
  networkPolicyRef: S.Literal("network-policy-ref://openagents/managed-sandbox/broker-only-v1"),
  leaseRef: ForensicRef,
  budgetRef: ForensicRef,
  capabilityRefs: BoundedRefs,
  state: WorkerPlacementState,
  admissionReceiptRef: S.optionalKey(ForensicRef),
  readinessReceiptRef: S.optionalKey(ForensicRef),
  stopReceiptRef: S.optionalKey(ForensicRef),
  deletionReceiptRef: S.optionalKey(ForensicRef),
  cleanupReceiptRef: S.optionalKey(ForensicRef),
  updatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (placement) =>
          !["worker_ready", "running"].includes(placement.state) ||
          (placement.admissionReceiptRef !== undefined &&
            placement.readinessReceiptRef !== undefined),
        { message: "ready or running placement requires admission and readiness receipts" },
      ),
      S.makeFilter(
        (placement) =>
          placement.state !== "cleaned" ||
          (placement.deletionReceiptRef !== undefined && placement.cleanupReceiptRef !== undefined),
        { message: "cleaned placement requires deletion and cleanup receipts" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicWorkerPlacement" });
export interface ForensicWorkerPlacement extends S.Schema.Type<
  typeof ForensicWorkerPlacementSchema
> {}

export const ForensicPromptIrSchema = S.Struct({
  role: LongText,
  threatModel: LongText,
  vulnerabilityClasses: BoundedShortTexts,
  securityInvariants: BoundedShortTexts,
  evidenceRequirements: BoundedShortTexts,
  dependencyExplorationPolicy: LongText,
  uncertaintyPolicy: LongText,
  toolPolicyRefs: BoundedRefs,
  findingSchemaRef: ForensicRef,
  hypothesisSchemaRef: ForensicRef,
  pocPolicy: LongText,
  severityPolicy: LongText,
  contextPolicy: LongText,
  budgetPolicyRef: ForensicRef,
});
export interface ForensicPromptIr extends S.Schema.Type<typeof ForensicPromptIrSchema> {}

const promptArtifactCanonicalValue = (artifact: {
  readonly parentPromptArtifactRef?: string;
  readonly promptIr: ForensicPromptIr;
  readonly exampleRefs: ReadonlyArray<string>;
  readonly parameterRefs: ReadonlyArray<string>;
  readonly datasetRevisionRef: string;
  readonly compatibilityRefs: ReadonlyArray<string>;
}) => ({
  ...(artifact.parentPromptArtifactRef === undefined
    ? {}
    : { parentPromptArtifactRef: artifact.parentPromptArtifactRef }),
  promptIr: artifact.promptIr,
  exampleRefs: artifact.exampleRefs,
  parameterRefs: artifact.parameterRefs,
  datasetRevisionRef: artifact.datasetRevisionRef,
  compatibilityRefs: artifact.compatibilityRefs,
});

export const forensicPromptArtifactDigest = (
  artifact: Parameters<typeof promptArtifactCanonicalValue>[0],
): Sha256Digest => forensicSha256Digest(promptArtifactCanonicalValue(artifact));

export const ForensicPromptArtifactSchema = S.Struct({
  schema: S.Literal(FORENSIC_PROMPT_ARTIFACT_VERSION),
  promptArtifactRef: ForensicRef,
  parentPromptArtifactRef: S.optionalKey(ForensicRef),
  promptIr: ForensicPromptIrSchema,
  exampleRefs: BoundedRefs,
  parameterRefs: BoundedRefs,
  canonicalDigest: Sha256Digest,
  datasetRevisionRef: ForensicRef,
  compatibilityRefs: BoundedRefs,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (artifact) => artifact.canonicalDigest === forensicPromptArtifactDigest(artifact),
        {
          message: "prompt artifact canonical digest must bind its structured content and lineage",
        },
      ),
    ),
  )
  .annotate({ identifier: "ForensicPromptArtifact" });
export interface ForensicPromptArtifact extends S.Schema.Type<
  typeof ForensicPromptArtifactSchema
> {}

export const ForensicRunState = S.Literals([
  "draft",
  "preflight",
  "ready_inputs",
  "incomplete",
  "denied",
  "admission_requested",
  "provisioning",
  "worker_ready",
  "running",
  "settling",
  "cancel_requested",
  "cleanup_requested",
  "cleaned",
  "completed",
  "completed_incomplete",
  "cancelled",
  "failed",
  "recovery_required",
  "review",
  "candidate",
  "retained",
  "dismissed",
  "release_gate",
  "admitted",
  "rejected",
]);
export type ForensicRunState = typeof ForensicRunState.Type;

export const CleanupState = S.Literals([
  "not_requested",
  "requested",
  "observed_zero_residue",
  "failed",
]);
export type CleanupState = typeof CleanupState.Type;

const RUN_STATES_REQUIRING_CLEANUP: ReadonlySet<ForensicRunState> = new Set([
  "cleaned",
  "completed",
  "completed_incomplete",
  "cancelled",
  "review",
  "candidate",
  "retained",
  "dismissed",
  "release_gate",
  "admitted",
  "rejected",
]);

export const ForensicRunSchema = S.Struct({
  schema: S.Literal(FORENSIC_RUN_VERSION),
  runRef: ForensicRef,
  targetRef: ForensicRef,
  profileRef: ForensicRef,
  placementRef: ForensicRef,
  sourceBundleRef: ForensicRef,
  coverageRef: ForensicRef,
  coverageStatus: CoverageStatus,
  sourceDigest: Sha256Digest,
  promptDigest: Sha256Digest,
  modelDigest: Sha256Digest,
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  state: ForensicRunState,
  cleanupState: CleanupState,
  findingRefs: BoundedRefs,
  hypothesisRefs: BoundedRefs,
  errorRefs: BoundedRefs,
  startedAt: S.optionalKey(ForensicTimestamp),
  settledAt: S.optionalKey(ForensicTimestamp),
  totalDuration: S.optionalKey(DurationTruthSchema),
  usage: S.optionalKey(UsageTruthSchema),
  lastEventSequence: NonNegativeInteger,
  updatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (run) =>
          run.state !== "completed" ||
          (run.coverageStatus === "complete" && run.cleanupState === "observed_zero_residue"),
        { message: "completed runs require complete inputs and observed zero-residue cleanup" },
      ),
      S.makeFilter(
        (run) =>
          run.state !== "completed_incomplete" ||
          (run.coverageStatus === "incomplete" && run.cleanupState === "observed_zero_residue"),
        {
          message:
            "completed_incomplete runs require incomplete inputs and observed zero-residue cleanup",
        },
      ),
      S.makeFilter(
        (run) =>
          !RUN_STATES_REQUIRING_CLEANUP.has(run.state) ||
          run.cleanupState === "observed_zero_residue",
        { message: "post-cleanup run states require observed zero-residue cleanup" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicRun" });
export interface ForensicRun extends S.Schema.Type<typeof ForensicRunSchema> {}

export const ForensicEvidenceReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_EVIDENCE_RECEIPT_VERSION),
  receiptRef: ForensicRef,
  runRef: ForensicRef,
  operationRef: ForensicRef,
  commandDigest: Sha256Digest,
  inputDigests: S.Array(Sha256Digest).check(S.isMaxLength(256)),
  outcome: S.Literals(["succeeded", "failed", "refused", "inconclusive"]),
  resultDigest: S.optionalKey(Sha256Digest),
  artifactDigests: S.Array(Sha256Digest).check(S.isMaxLength(256)),
  environmentDigest: Sha256Digest,
  evidenceRefs: BoundedRefs,
  observedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (receipt) => receipt.outcome !== "succeeded" || receipt.resultDigest !== undefined,
        { message: "successful evidence receipts require a result digest" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicEvidenceReceipt" });
export interface ForensicEvidenceReceipt extends S.Schema.Type<
  typeof ForensicEvidenceReceiptSchema
> {}

export const ForensicRunEventKind = S.Literals([
  "request_accepted",
  "worker_ready",
  "coverage_ready",
  "analysis_started",
  "tranche_started",
  "turn_started",
  "tool_observed",
  "hypothesis_submitted",
  "finding_submitted",
  "verification_observed",
  "review_recorded",
  "failure_observed",
  "cleanup_requested",
  "cleanup_observed",
  "run_settled",
]);
export type ForensicRunEventKind = typeof ForensicRunEventKind.Type;

export const ForensicMetricEventContextSchema = S.Struct({
  benchmarkRevisionDigest: Sha256Digest,
  datasetSplit: S.Literals(["train", "development", "holdout", "clean_holdout"]),
  armRef: ForensicRef,
  repetition: PositiveInteger,
  targetDigest: Sha256Digest,
  sourceBundleDigest: Sha256Digest,
  promptDigest: Sha256Digest,
  modelDigest: Sha256Digest,
  modelParametersDigest: Sha256Digest,
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  sandboxRef: ForensicRef,
  resourceGeneration: NonNegativeInteger,
  evaluatorRevisionDigest: Sha256Digest,
});
export interface ForensicMetricEventContext extends S.Schema.Type<
  typeof ForensicMetricEventContextSchema
> {}

export const ForensicRunEventSchema = S.Struct({
  schema: S.Literal(FORENSIC_RUN_EVENT_VERSION),
  eventRef: ForensicRef,
  runRef: ForensicRef,
  sequence: PositiveInteger,
  kind: ForensicRunEventKind,
  actorRef: ForensicRef,
  metricContext: ForensicMetricEventContextSchema,
  relatedRefs: BoundedRefs,
  detailRefs: BoundedRefs,
  clock: S.Literals(["control_plane_server", "worker_monotonic"]),
  monotonicElapsedMilliseconds: S.optionalKey(NonNegativeInteger),
  usage: S.optionalKey(UsageTruthSchema),
  observedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (event) =>
          event.clock === "worker_monotonic"
            ? event.monotonicElapsedMilliseconds !== undefined
            : event.monotonicElapsedMilliseconds === undefined,
        { message: "only worker events may carry worker-monotonic elapsed time" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicRunEvent" });
export interface ForensicRunEvent extends S.Schema.Type<typeof ForensicRunEventSchema> {}

export const ForensicUsageExactnessSchema = S.Struct({
  exactness: Exactness,
  reasonRef: S.optionalKey(ForensicRef),
}).pipe(
  S.check(
    S.makeFilter((truth) => truth.exactness !== "unavailable" || truth.reasonRef !== undefined, {
      message: "unavailable values require a reason ref",
    }),
  ),
);
export interface ForensicUsageExactness extends S.Schema.Type<
  typeof ForensicUsageExactnessSchema
> {}

export const ForensicRunNoteSchema = S.Struct({
  noteRef: ForensicRef,
  text: ShortText,
  evidenceRefs: BoundedRefs,
});
export interface ForensicRunNote extends S.Schema.Type<typeof ForensicRunNoteSchema> {}
