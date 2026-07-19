import { Schema } from "effect";

const NonNegativeSchema = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0));
const PositiveIntegerSchema = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));

export const IdeIndexBenchmarkMetricSchema = Schema.Struct({
  metric: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  unit: Schema.Literal("milliseconds"),
  repetitions: PositiveIntegerSchema,
  p50: NonNegativeSchema,
  p95: NonNegativeSchema,
  p99: NonNegativeSchema,
  minimum: NonNegativeSchema,
  maximum: NonNegativeSchema,
  thresholdP95: NonNegativeSchema,
  passed: Schema.Boolean,
});
export type IdeIndexBenchmarkMetric = typeof IdeIndexBenchmarkMetricSchema.Type;

export const IdeIndexPlacementDecisionSchema = Schema.TaggedUnion({
  Select: {
    runtime: Schema.Literal("typescript"),
    scope: Schema.Literals(["IDE-01 benchmark", "IDE-02 project index"]),
    rationale: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    replacementGate: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
  },
  Reject: {
    runtime: Schema.Literal("rust"),
    scope: Schema.Literals(["IDE-01 benchmark", "IDE-02 project index"]),
    rationale: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    reconsiderationGate: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
  },
});
export type IdeIndexPlacementDecision = typeof IdeIndexPlacementDecisionSchema.Type;

export const IdeIndexBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-index-benchmark.v1"),
  capturedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
  ),
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  platform: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  architecture: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  nodeVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  fixtureFiles: PositiveIntegerSchema,
  baselinePathSearchP95: NonNegativeSchema,
  metrics: Schema.Array(IdeIndexBenchmarkMetricSchema).check(
    Schema.isMinLength(4),
    Schema.isMaxLength(4),
  ),
  placement: Schema.Array(IdeIndexPlacementDecisionSchema).check(
    Schema.isMinLength(2),
    Schema.isMaxLength(2),
  ),
  assertions: Schema.Array(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(20)),
});
export type IdeIndexBenchmarkReceipt = typeof IdeIndexBenchmarkReceiptSchema.Type;

export const decodeIdeIndexBenchmarkReceipt = Schema.decodeUnknownSync(
  IdeIndexBenchmarkReceiptSchema,
);

export const IdePathIndexDeliveryReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-path-index-benchmark.v1"),
  capturedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
  ),
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  platform: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  architecture: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  nodeVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  fixtureFiles: PositiveIntegerSchema,
  fixtureDirectories: PositiveIntegerSchema,
  metrics: Schema.Array(IdeIndexBenchmarkMetricSchema).check(
    Schema.isMinLength(4),
    Schema.isMaxLength(4),
  ),
  resources: Schema.Struct({
    indexedNodes: PositiveIntegerSchema,
    estimatedBytes: PositiveIntegerSchema,
    heapDeltaBytes: Schema.Number,
    activeResourcesBefore: NonNegativeSchema,
    activeResourcesAfter: NonNegativeSchema,
    activeResourceDelta: Schema.Number,
    sourceSubscriptionCountAfter: Schema.Literal(0),
    stoppedAccessRefused: Schema.Literal(true),
  }),
  journeys: Schema.Array(Schema.Struct({
    mode: Schema.Literals(["pointer", "keyboard", "screen_reader", "reduced_motion", "zoom_200"]),
    passed: Schema.Boolean,
    traceRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240)),
  })).check(Schema.isMinLength(5), Schema.isMaxLength(5)),
  placement: Schema.Array(IdeIndexPlacementDecisionSchema).check(
    Schema.isMinLength(2),
    Schema.isMaxLength(2),
  ),
  assertions: Schema.Array(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(30)),
});
export type IdePathIndexDeliveryReceipt = typeof IdePathIndexDeliveryReceiptSchema.Type;

export const decodeIdePathIndexDeliveryReceipt = Schema.decodeUnknownSync(
  IdePathIndexDeliveryReceiptSchema,
);

export const IdePathIndexPackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-path-index-packaged-journey.v1"),
  capturedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
  ),
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  platform: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  architecture: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  packaged: Schema.Literal(true),
  sourceCorpus: Schema.Literal("tracked-openagents-archive"),
  sourceEntries: PositiveIntegerSchema,
  indexedNodes: PositiveIntegerSchema,
  indexState: Schema.Literal("ready"),
  pointerActivation: Schema.Literal(true),
  keyboardHomeEnd: Schema.Literal(true),
  keyboardContextMenu: Schema.Literal(true),
  screenReaderTree: Schema.Literal(true),
  rootWithheld: Schema.Literal(true),
  screenshotRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240)),
});
export type IdePathIndexPackagedJourneyReceipt = typeof IdePathIndexPackagedJourneyReceiptSchema.Type;

export const decodeIdePathIndexPackagedJourneyReceipt = Schema.decodeUnknownSync(
  IdePathIndexPackagedJourneyReceiptSchema,
);
