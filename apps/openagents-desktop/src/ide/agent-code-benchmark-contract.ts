import { Schema } from "effect"

const nonNegative = Schema.Number.check(Schema.isFinite(), Schema.isGreaterThanOrEqualTo(0))
const positiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))
const boundedText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000))

export const IdeAgentCodeBenchmarkMetricSchema = Schema.Struct({
  metric: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  corpus: boundedText,
  unit: Schema.Literals(["milliseconds", "bytes", "count"]),
  repetitions: positiveInteger,
  warmup: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  method: boundedText,
  noise: boundedText,
  baseline: Schema.NullOr(nonNegative),
  p50: nonNegative,
  p95: nonNegative,
  p99: nonNegative,
  thresholdP95: nonNegative,
  thresholdP99: nonNegative,
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeAgentCodeBenchmarkMetric" })
export type IdeAgentCodeBenchmarkMetric = typeof IdeAgentCodeBenchmarkMetricSchema.Type

export const IdeAgentCodeBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-agent-code-benchmark.v1"),
  issue: Schema.Literal("IDE-08"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  runtime: Schema.Struct({ node: Schema.String, platform: Schema.String, arch: Schema.String }),
  corpus: Schema.Struct({
    manifestItems: positiveInteger,
    singleFileBytes: positiveInteger,
    aggregateFiles: positiveInteger,
    aggregateBytes: positiveInteger,
    faultClasses: Schema.Array(Schema.String).check(Schema.isMinLength(1), Schema.isMaxLength(100)),
  }),
  metrics: Schema.Array(IdeAgentCodeBenchmarkMetricSchema).check(Schema.isMinLength(10), Schema.isMaxLength(100)),
  resources: Schema.Struct({
    cycles: positiveInteger,
    retainedHeapBytes: Schema.Number,
    activeHandlesDelta: Schema.Number.check(Schema.isInt()),
    activeListenersAfter: Schema.Literal(0),
    proposalStreamsAfter: Schema.Literal(0),
    temporaryPreimagesAfter: Schema.Literal(0),
  }),
  offline: Schema.Struct({ remoteRequests: Schema.Literal(0), embeddingsRequired: Schema.Literal(false) }),
  budgetsPassed: Schema.Boolean,
}).annotate({ identifier: "IdeAgentCodeBenchmarkReceipt" })
export type IdeAgentCodeBenchmarkReceipt = typeof IdeAgentCodeBenchmarkReceiptSchema.Type

export const IdeAgentCodePackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-agent-code-packaged-journey.v1"),
  issue: Schema.Literal("IDE-08"),
  capturedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  artifactTreeSha256: Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u)),
  target: Schema.Literal("darwin-arm64"),
  journey: Schema.Struct({
    diagnosticObserved: Schema.Boolean,
    contextManifestDisclosed: Schema.Boolean,
    omittedContextDisclosed: Schema.Boolean,
    exactProposalAdmitted: Schema.Boolean,
    pierreReviewRendered: Schema.Boolean,
    canonicalApplyObserved: Schema.Boolean,
    evidenceSeparatedFromHarness: Schema.Boolean,
    backlinkRoundTrip: Schema.Boolean,
    undoRestoredPreimage: Schema.Boolean,
    rootWithheld: Schema.Boolean,
    keyboardOperable: Schema.Boolean,
    nonColorStateCues: Schema.Boolean,
  }),
  screenshotRef: boundedText,
  traceRef: boundedText,
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeAgentCodePackagedJourneyReceipt" })
export type IdeAgentCodePackagedJourneyReceipt = typeof IdeAgentCodePackagedJourneyReceiptSchema.Type

export const IdeAgentCodeAcceptanceReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-agent-code-acceptance.v1"),
  issue: Schema.Literal("IDE-08"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  mainEvaluationSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  artifactTreeSha256: Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u)),
  evidenceRefs: Schema.Array(boundedText).check(Schema.isMinLength(3), Schema.isMaxLength(50)),
  faultMatrix: Schema.Array(Schema.Struct({ fault: boundedText, passed: Schema.Literal(true), evidenceRef: boundedText })).check(Schema.isMinLength(10), Schema.isMaxLength(100)),
  architecture: Schema.Struct({
    oneSchemaGraph: Schema.Literal(true),
    effectServices: Schema.Literal(true),
    rendererAuthority: Schema.Literal(false),
    harnessAuthority: Schema.Literal(false),
    monacoAuthority: Schema.Literal(false),
    pierreAuthority: Schema.Literal(false),
    nativeAuthority: Schema.Literal(false),
    embeddingsRequired: Schema.Literal(false),
    publicReceiptsContainPrivateContent: Schema.Literal(false),
  }),
  accessibility: Schema.Struct({
    keyboard: Schema.Literal(true),
    screenReaderLabels: Schema.Literal(true),
    nonColorCues: Schema.Literal(true),
    reducedMotion: Schema.Literal(true),
    zoomAndMinimumWindow: Schema.Literal(true),
  }),
  assuranceLifecycle: Schema.Literal("proposed"),
  ownerDisposition: Schema.Literal("unreviewed"),
  reviewer: Schema.Struct({
    reviewerClass: Schema.Literal("deterministic_repository_oracle"),
    oracleRef: boundedText,
    producerCanOverride: Schema.Literal(false),
    disposition: Schema.Literal("pass"),
  }),
  rollbackTargetSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  claimedTargets: Schema.Tuple([Schema.Literal("darwin-arm64")]),
  laterGaps: Schema.Array(boundedText).check(Schema.isMinLength(1), Schema.isMaxLength(30)),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeAgentCodeAcceptanceReceipt" })
export type IdeAgentCodeAcceptanceReceipt = typeof IdeAgentCodeAcceptanceReceiptSchema.Type
