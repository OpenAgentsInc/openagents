import { Schema } from "effect";

const latencyMetric = Schema.Struct({
  p50Ms: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p95Ms: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p99Ms: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  maxMs: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
});

export const IdeLanguageBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-language-benchmark.v1"),
  issue: Schema.Literal("IDE-06"),
  generatedAt: Schema.String,
  runtime: Schema.Struct({
    node: Schema.String,
    platform: Schema.String,
    arch: Schema.String,
    providerVersion: Schema.String,
    executable: Schema.String,
    placement: Schema.Literal("project_local"),
  }),
  corpus: Schema.Struct({
    files: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(100)),
    sourceBytes: Schema.Number.check(Schema.isGreaterThan(0)),
    samples: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(10)),
    capabilitiesExercised: Schema.Array(Schema.String).check(Schema.isMinLength(17), Schema.isMaxLength(17)),
  }),
  latency: Schema.Struct({
    firstDiagnosticsMs: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
    diagnostics: latencyMetric,
    documentSymbols: latencyMetric,
  }),
  cancellationFence: Schema.Struct({
    scheduled: Schema.Literal(100),
    committed: Schema.Literal(1),
    superseded: Schema.Literal(99),
  }),
  restart: Schema.Struct({
    crashObserved: Schema.Literal(true),
    recoveredServiceGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(2)),
    restartLatencyMs: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  }),
  resources: Schema.Struct({
    workersStarted: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(2)),
    activeWorkersAfter: Schema.Literal(0),
    pendingRequestsAfter: Schema.Literal(0),
  }),
  budgets: Schema.Struct({
    firstDiagnosticsMs: Schema.Literal(4_000),
    diagnosticsP95Ms: Schema.Literal(750),
    documentSymbolsP95Ms: Schema.Literal(750),
    restartMs: Schema.Literal(4_000),
    passed: Schema.Boolean,
  }),
  offline: Schema.Struct({ remoteRequests: Schema.Literal(0) }),
}).annotate({ identifier: "IdeLanguageBenchmarkReceipt" });
export type IdeLanguageBenchmarkReceipt = typeof IdeLanguageBenchmarkReceiptSchema.Type;

export const IdeLanguagePackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-language-packaged-journey.v1"),
  capturedAt: Schema.String,
  commitSha: Schema.String,
  platform: Schema.String,
  architecture: Schema.String,
  packaged: Schema.Literal(true),
  pathRef: Schema.String,
  documentLocalTierReady: Schema.Literal(true),
  projectTierReady: Schema.Literal(true),
  projectServiceGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  problemsReceiptReady: Schema.Literal(true),
  outlineReady: Schema.Literal(true),
  rootWithheld: Schema.Literal(true),
  offlineLocalWorkers: Schema.Literal(true),
  screenshotRef: Schema.String,
}).annotate({ identifier: "IdeLanguagePackagedJourneyReceipt" });
export type IdeLanguagePackagedJourneyReceipt = typeof IdeLanguagePackagedJourneyReceiptSchema.Type;
