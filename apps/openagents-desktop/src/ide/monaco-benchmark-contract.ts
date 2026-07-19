import { Schema } from "effect"

const NonNegative = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))
const NonNegativeInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))

export const IdeMonacoBenchmarkMetricSchema = Schema.Struct({
  metric: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  unit: Schema.Literal("milliseconds"),
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  p50: NonNegative,
  p95: NonNegative,
  p99: NonNegative,
  minimum: NonNegative,
  maximum: NonNegative,
  thresholdP95: NonNegative,
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeMonacoBenchmarkMetric" })
export type IdeMonacoBenchmarkMetric = typeof IdeMonacoBenchmarkMetricSchema.Type

export const IdeMonacoBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-monaco-benchmark.v1"),
  capturedAt: Schema.String,
  commitSha: Schema.String,
  platform: Schema.String,
  architecture: Schema.String,
  nodeVersion: Schema.String,
  fixtureBytes: NonNegativeInt,
  fixtureTabs: NonNegativeInt,
  metrics: Schema.Array(IdeMonacoBenchmarkMetricSchema).check(Schema.isMinLength(1)),
  resources: Schema.Struct({
    heapDeltaBytes: Schema.Number,
    activeResourcesBefore: NonNegativeInt,
    activeResourcesAfter: NonNegativeInt,
    activeResourceDelta: Schema.Number.check(Schema.isInt()),
    ordinaryBootJavaScriptBytes: NonNegativeInt,
    editorJavaScriptBytes: NonNegativeInt,
    editorCssBytes: NonNegativeInt,
    workerBytes: NonNegativeInt,
    ordinaryBootContainsMonacoGraph: Schema.Literal(false),
    stoppedResourceSnapshot: Schema.Struct({
      models: Schema.Literal(0),
      views: Schema.Literal(0),
      workers: Schema.Literal(0),
      listeners: Schema.Literal(0),
    }),
  }),
  placement: Schema.Struct({
    selected: Schema.Literal("typescript"),
    rejected: Schema.Literal("rust"),
    rationale: Schema.String,
    replacementGate: Schema.String,
  }),
  assertions: Schema.Array(Schema.String.check(Schema.isMinLength(1))).check(Schema.isMinLength(1)),
}).annotate({ identifier: "IdeMonacoBenchmarkReceipt" })
export type IdeMonacoBenchmarkReceipt = typeof IdeMonacoBenchmarkReceiptSchema.Type

export const IdeMonacoPackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-monaco-packaged-journey.v1"),
  capturedAt: Schema.String,
  commitSha: Schema.String,
  platform: Schema.String,
  architecture: Schema.String,
  packaged: Schema.Literal(true),
  pathRef: Schema.String,
  editorReady: Schema.Literal(true),
  edited: Schema.Literal(true),
  vimToggled: Schema.Literal(true),
  splitViews: Schema.Literal(2),
  recoveryReloaded: Schema.Literal(true),
  offlinePrivateScheme: Schema.Literal(true),
  rootWithheld: Schema.Literal(true),
  legacyTextareaAbsent: Schema.Literal(true),
  resourcesAfterClose: Schema.Struct({
    models: Schema.Literal(0),
    views: Schema.Literal(0),
    workers: Schema.Literal(0),
    listeners: Schema.Literal(0),
  }),
  screenshotRef: Schema.String,
}).annotate({ identifier: "IdeMonacoPackagedJourneyReceipt" })
export type IdeMonacoPackagedJourneyReceipt = typeof IdeMonacoPackagedJourneyReceiptSchema.Type
