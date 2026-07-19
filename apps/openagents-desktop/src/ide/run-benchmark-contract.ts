import { Schema } from "effect"

import { IdeTimestampSchema } from "./project-contract.ts"

const nonEmpty = (maximum: number) => Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(maximum),
)
const nonNegative = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))

export const IdeRunBenchmarkMetricSchema = Schema.Struct({
  name: nonEmpty(120),
  unit: Schema.Literals(["milliseconds", "bytes", "count"]),
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  warmup: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  p50: nonNegative,
  p95: nonNegative,
  p99: nonNegative,
  thresholdP95: nonNegative,
  thresholdP99: nonNegative,
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeRunBenchmarkMetric" })
export interface IdeRunBenchmarkMetric extends Schema.Schema.Type<typeof IdeRunBenchmarkMetricSchema> {}

export const IdeRunTargetFactSchema = Schema.Struct({
  target: Schema.Literals([
    "macos-arm64",
    "macos-x64",
    "windows-arm64",
    "windows-x64",
    "linux-arm64",
    "linux-x64",
  ]),
  nativeHelper: Schema.Literal(false),
  typescriptFallback: Schema.Literal(true),
  disposition: Schema.Literal("not_claimed_native_helper_unnecessary"),
}).annotate({ identifier: "IdeRunTargetFact" })
export interface IdeRunTargetFact extends Schema.Schema.Type<typeof IdeRunTargetFactSchema> {}

export const IdeRunBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-run-benchmark.v1"),
  issue: Schema.Literal("IDE-10"),
  measuredAt: IdeTimestampSchema,
  candidateCommitSha: nonEmpty(64),
  environment: Schema.Struct({
    platform: nonEmpty(40),
    architecture: nonEmpty(40),
    node: nonEmpty(40),
    shell: nonEmpty(120),
    runtime: Schema.Literal("Effect v4 + Node child_process"),
    corpus: Schema.Literal("deterministic declared-task and output fixture"),
  }),
  metrics: Schema.Array(IdeRunBenchmarkMetricSchema).check(Schema.isMinLength(8)),
  outputFacts: Schema.Struct({
    sequenceMonotonic: Schema.Literal(true),
    boundedRetention: Schema.Literal(true),
    gapAccounted: Schema.Literal(true),
    redactionObserved: Schema.Literal(true),
    invalidEncodingAccounted: Schema.Literal(true),
    rendererReceivesEnvironmentValues: Schema.Literal(false),
    inheritedAllHostVariables: Schema.Literal(false),
  }),
  resources: Schema.Struct({
    activeHandlesDelta: Schema.Number,
    heapDeltaBytes: Schema.Number,
    runningProcessesAfter: Schema.Literal(0),
    subscriptionsAfter: Schema.Literal(0),
  }),
  nativeDecision: Schema.Struct({
    rustAdmitted: Schema.Literal(false),
    reason: nonEmpty(600),
  targets: Schema.Array(IdeRunTargetFactSchema).check(
    Schema.isMinLength(6),
    Schema.isMaxLength(6),
  ),
  }),
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeRunBenchmarkReceipt" })
export interface IdeRunBenchmarkReceipt extends Schema.Schema.Type<typeof IdeRunBenchmarkReceiptSchema> {}

export const IdeRunPackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-run-packaged.v1"),
  issue: Schema.Literal("IDE-10"),
  recordedAt: IdeTimestampSchema,
  candidateCommitSha: nonEmpty(64),
  artifactTreeSha256: nonEmpty(64),
  artifactFiles: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  artifactBytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  target: Schema.Literal("darwin-arm64"),
  checks: Schema.Struct({
    khalaDefault: Schema.Literal(true),
    tokyoNightFallbackPresent: Schema.Literal(true),
    xtermProjection: Schema.Literal(true),
    terminalInput: Schema.Literal(true),
    terminalSearch: Schema.Literal(true),
    tasksDiscovered: Schema.Literal(true),
    taskSucceeded: Schema.Literal(true),
    dependencySucceeded: Schema.Literal(true),
    outputRedacted: Schema.Literal(true),
    outputGapVisible: Schema.Literal(true),
    testTreeVisible: Schema.Literal(true),
    keyboardNavigation: Schema.Literal(true),
    processCleanup: Schema.Literal(true),
    privateRootWithheld: Schema.Literal(true),
  }),
  screenshotRef: nonEmpty(240),
  traceRef: nonEmpty(240),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeRunPackagedJourneyReceipt" })
export interface IdeRunPackagedJourneyReceipt extends Schema.Schema.Type<typeof IdeRunPackagedJourneyReceiptSchema> {}

export const IdeRunAcceptanceReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-run-acceptance.v1"),
  issue: Schema.Literal("IDE-10"),
  generatedAt: IdeTimestampSchema,
  candidateCommitSha: nonEmpty(64),
  mainEvaluationSha: nonEmpty(64),
  artifactTreeSha256: nonEmpty(64),
  benchmarkRef: nonEmpty(240),
  packagedRef: nonEmpty(240),
  screenshotRef: nonEmpty(240),
  traceRef: nonEmpty(240),
  evidenceRefs: Schema.Array(nonEmpty(240)).check(Schema.isMinLength(10)),
  faultMatrix: Schema.Array(Schema.Struct({
    fault: nonEmpty(400),
    evidenceRef: nonEmpty(400),
    passed: Schema.Literal(true),
  })).check(Schema.isMinLength(20)),
  architecture: Schema.Struct({
    oneSchemaGraph: Schema.Literal(true),
    effectAuthority: Schema.Literal(true),
    xtermProjectionOnly: Schema.Literal(true),
    explicitEnvironment: Schema.Literal(true),
    semanticSuccess: Schema.Literal(true),
    actorParity: Schema.Literal(true),
    rustAdmitted: Schema.Literal(false),
  }),
  ownerDisposition: Schema.Literal("unreviewed"),
  assuranceLifecycle: Schema.Literal("proposed"),
  rollbackTargetSha: nonEmpty(64),
  laterGaps: Schema.Array(nonEmpty(400)).check(Schema.isMinLength(1)),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeRunAcceptanceReceipt" })
export interface IdeRunAcceptanceReceipt extends Schema.Schema.Type<typeof IdeRunAcceptanceReceiptSchema> {}
