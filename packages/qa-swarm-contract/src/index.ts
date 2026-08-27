import { GraphSpec } from "@openagentsinc/arbiter-effect/core";
import { Schema as S } from "effect";

export const QA_SWARM_RUN_PROJECTION_SCHEMA = "openagents.qa_swarm.run_projection.v1" as const;

export const QaSwarmVerdict = S.Literals(["passed", "failed", "warning", "inconclusive"]);
export type QaSwarmVerdict = typeof QaSwarmVerdict.Type;

export const QaSwarmTargetVisibility = S.Literals(["public", "opaque"]);
export type QaSwarmTargetVisibility = typeof QaSwarmTargetVisibility.Type;

export class QaSwarmTargetProjection extends S.Class<QaSwarmTargetProjection>(
  "QaSwarmTargetProjection",
)({
  label: S.String,
  ref: S.optional(S.String),
  visibility: QaSwarmTargetVisibility,
}) {}

export class QaSwarmVerdictItem extends S.Class<QaSwarmVerdictItem>("QaSwarmVerdictItem")({
  label: S.String,
  receiptRef: S.String,
  summary: S.String,
  verdict: QaSwarmVerdict,
}) {}

export class QaSwarmCoverageFrontierItem extends S.Class<QaSwarmCoverageFrontierItem>(
  "QaSwarmCoverageFrontierItem",
)({
  current: S.Number,
  frontier: S.Number,
  label: S.String,
  receiptRef: S.String,
}) {}

export class QaSwarmPerfBudgetItem extends S.Class<QaSwarmPerfBudgetItem>("QaSwarmPerfBudgetItem")({
  actualMs: S.Number,
  budgetMs: S.Number,
  label: S.String,
  receiptRef: S.String,
  verdict: QaSwarmVerdict,
}) {}

export class QaSwarmVideoRef extends S.Class<QaSwarmVideoRef>("QaSwarmVideoRef")({
  label: S.String,
  posterRef: S.String,
  traceHref: S.String,
  videoRef: S.String,
}) {}

export class QaSwarmDistilledTestRef extends S.Class<QaSwarmDistilledTestRef>(
  "QaSwarmDistilledTestRef",
)({
  href: S.String,
  label: S.String,
  lifecycleState: S.Literal("landed"),
  receiptRef: S.String,
}) {}

export class QaSwarmValidatedRegressionCandidate extends S.TaggedClass<QaSwarmValidatedRegressionCandidate>()(
  "validated",
  {
    candidateRef: S.String,
    discoveryRef: S.String,
    label: S.String,
    rerunReceiptRef: S.String,
    testHref: S.String,
  },
) {}

export class QaSwarmProposedRegressionCandidate extends S.TaggedClass<QaSwarmProposedRegressionCandidate>()(
  "proposed",
  {
    candidateRef: S.String,
    commitProposalRef: S.String,
    discoveryRef: S.String,
    issueRef: S.String,
    label: S.String,
    pullRequestRef: S.String,
    rerunReceiptRef: S.String,
    testHref: S.String,
  },
) {}

export class QaSwarmLandedRegressionCandidate extends S.TaggedClass<QaSwarmLandedRegressionCandidate>()(
  "landed",
  {
    candidateRef: S.String,
    discoveryRef: S.String,
    label: S.String,
    mergedCommitRef: S.String,
    pullRequestRef: S.String,
    rerunReceiptRef: S.String,
    reviewedMergeReceiptRef: S.String,
    testHref: S.String,
  },
) {}

/**
 * A generated candidate is not a distilled/landed regression. The shared
 * projection records the review lifecycle explicitly, and the `landed`
 * variant cannot be represented without reviewed merge evidence.
 */
export const QaSwarmRegressionCandidate = S.Union([
  QaSwarmValidatedRegressionCandidate,
  QaSwarmProposedRegressionCandidate,
  QaSwarmLandedRegressionCandidate,
]);
export type QaSwarmRegressionCandidate = typeof QaSwarmRegressionCandidate.Type;

export class QaSwarmEvidenceAdmission extends S.Class<QaSwarmEvidenceAdmission>(
  "QaSwarmEvidenceAdmission",
)({
  admittedReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  resolverContract: S.Literal("qa_swarm.receipt_resolver.v1"),
}) {}

export const QaSwarmExecutionStatus = S.Literals(["scheduled", "running", "completed", "failed"]);
export type QaSwarmExecutionStatus = typeof QaSwarmExecutionStatus.Type;

export const QaSwarmExecutionTierStatus = S.Literals([
  "scheduled",
  "running",
  "passed",
  "failed",
  "skipped",
]);
export type QaSwarmExecutionTierStatus = typeof QaSwarmExecutionTierStatus.Type;

export class QaSwarmExecutionTier extends S.Class<QaSwarmExecutionTier>("QaSwarmExecutionTier")({
  backend: S.String,
  jobRef: S.optional(S.String),
  reason: S.optional(S.String),
  status: QaSwarmExecutionTierStatus,
}) {}

export class QaSwarmExecutionProjection extends S.Class<QaSwarmExecutionProjection>(
  "QaSwarmExecutionProjection",
)({
  status: QaSwarmExecutionStatus,
  tiers: S.Array(QaSwarmExecutionTier),
}) {}

export class QaSwarmRunProjection extends S.Class<QaSwarmRunProjection>("QaSwarmRunProjection")({
  boardGraph: GraphSpec,
  blockerRefs: S.Array(S.String),
  coverageFrontier: S.Array(QaSwarmCoverageFrontierItem),
  distilledTests: S.Array(QaSwarmDistilledTestRef),
  evidenceAdmission: QaSwarmEvidenceAdmission,
  execution: S.optional(QaSwarmExecutionProjection),
  generatedAt: S.String,
  nightlyArtifactRef: S.optional(S.String),
  opaqueTargetRefs: S.Array(S.String),
  perfBudgets: S.Array(QaSwarmPerfBudgetItem),
  projectionRef: S.String,
  publicSafetyRefs: S.Array(S.String),
  regressionCandidates: S.optional(S.Array(QaSwarmRegressionCandidate)),
  runRef: S.String,
  schemaVersion: S.Literal(QA_SWARM_RUN_PROJECTION_SCHEMA),
  staleness: S.Struct({
    contractVersion: S.Literal("projection_staleness.v1"),
    maxAgeHours: S.Number,
    mode: S.Literal("artifact_snapshot"),
  }),
  target: QaSwarmTargetProjection,
  title: S.String,
  traceRefs: S.Array(S.String),
  verdict: QaSwarmVerdict,
  verdictWall: S.Array(QaSwarmVerdictItem),
  videoRefs: S.Array(QaSwarmVideoRef),
}) {}

export const decodeQaSwarmRunProjection = S.decodeUnknownSync(QaSwarmRunProjection);

export const assertResolverBackedQaSwarmProjection = (value: unknown): QaSwarmRunProjection => {
  const projection = decodeQaSwarmRunProjection(value);
  const admitted = new Set(projection.evidenceAdmission.admittedReceiptRefs);
  const falseGreen = projection.boardGraph.links.find(
    (link) =>
      link.status === "evidence_backed" &&
      (link.evidenceRefs.length === 0 || link.evidenceRefs.some((ref) => !admitted.has(ref))),
  );
  if (falseGreen !== undefined) {
    throw new Error(`QA Swarm board link is not resolver-backed: ${falseGreen.id}`);
  }
  if (projection.evidenceAdmission.blockerRefs.length > 0 && projection.verdict === "passed") {
    throw new Error("QA Swarm projection cannot pass with unresolved evidence blockers");
  }
  return projection;
};
