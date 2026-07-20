import { Schema } from "effect";

const bounded = (maximum: number) => Schema.String.check(Schema.isMaxLength(maximum));
const sha = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u));

export const IdeSourceControlMetricSchema = Schema.Struct({
  name: Schema.String,
  corpus: Schema.String,
  repetitions: Schema.Number,
  p50: Schema.Number,
  p95: Schema.Number,
  p99: Schema.Number,
  thresholdP95: Schema.Number,
  thresholdP99: Schema.Number,
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeSourceControlMetric" });

export const IdeSourceControlBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-source-control-benchmark.v1"),
  issue: Schema.Literal("IDE-12"),
  candidateCommitSha: sha,
  measuredAt: Schema.String,
  environment: Schema.Struct({ platform: Schema.String, architecture: Schema.String, node: Schema.String, git: Schema.String, filesystem: Schema.String }),
  metrics: Schema.Array(IdeSourceControlMetricSchema),
  resources: Schema.Struct({ heapDeltaBytes: Schema.Number, activeHandlesDelta: Schema.Number, childProcessesAfter: Schema.Literal(0) }),
  security: Schema.Struct({ secretPathsWithheld: Schema.Literal(true), ignoredPathsWithheld: Schema.Literal(true), rawCredentialsProjected: Schema.Literal(false), privatePathsProjected: Schema.Literal(false) }),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeSourceControlBenchmarkReceipt" });

export const IdeSourceControlPackagedReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-source-control-packaged.v1"),
  issue: Schema.Literal("IDE-12"),
  candidateCommitSha: sha,
  recordedAt: Schema.String,
  artifactTreeSha256: Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/u)),
  target: Schema.Literal("darwin-arm64"),
  checks: Schema.Record(Schema.String, Schema.Boolean),
  screenshotRef: bounded(500),
  traceRef: bounded(500),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeSourceControlPackagedReceipt" });

export const IdeSourceControlAcceptanceReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-source-control-acceptance.v1"),
  issue: Schema.Literal("IDE-12"),
  candidateCommitSha: sha,
  evaluationSha: sha,
  artifactTreeSha256: Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/u)),
  generatedAt: Schema.String,
  benchmarkRef: bounded(500),
  packagedRef: bounded(500),
  screenshotRef: bounded(500),
  traceRef: bounded(500),
  evidenceRefs: Schema.Array(bounded(500)),
  rollbackTargetSha: sha,
  reviewerDisposition: Schema.Literal("unreviewed"),
  ownerDisposition: Schema.Literal("unreviewed"),
  assuranceLifecycle: Schema.Literal("proposed"),
  remainingGaps: Schema.Array(bounded(1_000)),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeSourceControlAcceptanceReceipt" });
