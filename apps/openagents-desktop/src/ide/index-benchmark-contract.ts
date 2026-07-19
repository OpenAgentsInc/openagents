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
