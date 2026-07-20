import { Schema } from "effect"

const sha = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))
const bounded = (maximum: number) => Schema.String.check(Schema.isMaxLength(maximum))
const percentile = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))

export const IdePortableEvidenceMetricSchema = Schema.Struct({
  operation: bounded(120),
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  p50Ms: percentile,
  p95Ms: percentile,
  p99Ms: percentile,
  thresholdP95Ms: percentile,
  thresholdP99Ms: percentile,
  passed: Schema.Boolean,
})

export const IdePortablePlacementCohortSchema = Schema.Struct({
  targetClass: Schema.Literals(["owner_local", "owner_managed", "openagents_managed", "managed_provider"]),
  evidenceClass: Schema.Literals(["real_local", "deterministic_simulator", "not_run"]),
  operatingSystem: Schema.Literals(["darwin", "windows", "linux", "unknown"]),
  architecture: Schema.Literals(["arm64", "x64", "unknown"]),
  adapterRef: bounded(256),
  capabilityState: Schema.Literals(["ready", "degraded", "unsupported", "unverified"]),
  custody: Schema.Literals(["owner_device", "owner_managed", "openagents_managed", "unverified"]),
  networkDestinations: Schema.Array(bounded(320)).check(Schema.isMaxLength(32)),
  dataDestinations: Schema.Array(bounded(320)).check(Schema.isMaxLength(32)),
  retentionSeconds: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  costFact: bounded(160),
  result: bounded(500),
})

export const IdePortableEvidenceReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-evidence.v1"),
  issue: Schema.Literal("IDE-13"),
  candidateCommitSha: sha,
  generatedAt: Schema.String,
  environment: Schema.Struct({ platform: Schema.String, architecture: Schema.String, node: Schema.String }),
  model: Schema.Struct({
    maximumDepth: Schema.Number,
    exploredStates: Schema.Number,
    exploredTransitions: Schema.Number,
    staleWriteAttempts: Schema.Number,
    counterexamples: Schema.Number,
    passed: Schema.Literal(true),
  }),
  metrics: Schema.Array(IdePortableEvidenceMetricSchema),
  placementCohorts: Schema.Array(IdePortablePlacementCohortSchema).check(
    Schema.isMinLength(4),
    Schema.isMaxLength(4),
  ),
  faultCoverage: Schema.Array(Schema.Struct({
    fault: bounded(160),
    evidence: Schema.Literals(["model", "regression", "not_run"]),
    result: Schema.Literals(["passed", "gap"]),
    evidenceRef: bounded(500),
  })),
  security: Schema.Struct({
    forbiddenMaterialProjected: Schema.Literal(false),
    optimisticAuthorityProjected: Schema.Literal(false),
    staleGenerationAccepted: Schema.Literal(false),
    rawCredentialProjected: Schema.Literal(false),
  }),
  implementationChecksPassed: Schema.Literal(true),
  acceptancePassed: Schema.Literal(false),
  remainingGaps: Schema.Array(bounded(1_000)).check(Schema.isMinLength(1)),
})
export type IdePortableEvidenceReceipt = typeof IdePortableEvidenceReceiptSchema.Type
