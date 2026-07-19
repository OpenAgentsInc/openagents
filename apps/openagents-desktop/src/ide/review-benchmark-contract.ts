import { Schema } from "effect";

const metric = Schema.Struct({
  p50Ms: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p95Ms: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p99Ms: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  maxMs: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
});

export const IdeReviewBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-review-benchmark.v1"),
  issue: Schema.Literal("IDE-05"),
  generatedAt: Schema.String,
  runtime: Schema.Struct({ node: Schema.String, platform: Schema.String, arch: Schema.String }),
  corpus: Schema.Struct({
    sourceClasses: Schema.Array(Schema.String).check(
      Schema.isMinLength(8),
      Schema.isMaxLength(8),
    ),
    aggregateFiles: Schema.Literal(500),
    aggregatePatchBytes: Schema.Number.check(Schema.isGreaterThan(0)),
    samples: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(20)),
  }),
  latency: Schema.Struct({
    sourceProjection: metric,
    aggregateParse: metric,
  }),
  cancellationFence: Schema.Struct({
    scheduled: Schema.Literal(100),
    committed: Schema.Literal(1),
    superseded: Schema.Literal(99),
  }),
  resources: Schema.Struct({
    openCloseCycles: Schema.Literal(200),
    workerPoolDisabled: Schema.Literal(true),
    activeWorkersAfter: Schema.Literal(0),
    listenerDeltaAfter: Schema.Literal(0),
    retainedHeapBytes: Schema.Number,
  }),
  budgets: Schema.Struct({
    projectionP95Ms: Schema.Literal(20),
    aggregateParseP95Ms: Schema.Literal(250),
    retainedHeapBytes: Schema.Literal(16_777_216),
    passed: Schema.Boolean,
  }),
  offline: Schema.Struct({ remoteRequests: Schema.Literal(0) }),
}).annotate({ identifier: "IdeReviewBenchmarkReceipt" });
export type IdeReviewBenchmarkReceipt = typeof IdeReviewBenchmarkReceiptSchema.Type;
