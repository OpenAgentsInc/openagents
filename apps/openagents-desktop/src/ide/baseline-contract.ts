import { Schema } from "effect";

export const IdeBaselineSchemaVersion = Schema.Literal("openagents.desktop.ide-baseline.v1");

export const IdeBaselineTimestampSchema = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);

export const IdeBaselineMetricSchema = Schema.Struct({
  metric: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  category: Schema.Literals(["latency", "resource"]),
  unit: Schema.Literals(["milliseconds", "bytes", "count"]),
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  p50: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p95: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p99: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  minimum: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  maximum: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  sourceRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240)),
  noise: Schema.String.check(Schema.isMaxLength(400)),
});
export type IdeBaselineMetric = typeof IdeBaselineMetricSchema.Type;

export const IdeBaselineGapSchema = Schema.Struct({
  probe: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  status: Schema.Literals(["unmeasured", "partially_measured"]),
  reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
  plannedPacket: Schema.Literals([
    "IDE-01",
    "IDE-02",
    "IDE-03",
    "IDE-04",
    "IDE-05",
    "IDE-06",
    "IDE-07",
  ]),
});
export type IdeBaselineGap = typeof IdeBaselineGapSchema.Type;

export const IdeBaselineEnvironmentSchema = Schema.Struct({
  capturedAt: IdeBaselineTimestampSchema,
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  platform: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  architecture: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  nodeVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  electronVersion: Schema.NullOr(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  ),
  fixtureFiles: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  mode: Schema.Literal("public-safe deterministic local fixture"),
});
export type IdeBaselineEnvironment = typeof IdeBaselineEnvironmentSchema.Type;

export const IdeBaselineReceiptSchema = Schema.Struct({
  schemaVersion: IdeBaselineSchemaVersion,
  environment: IdeBaselineEnvironmentSchema,
  metrics: Schema.Array(IdeBaselineMetricSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(200),
  ),
  gaps: Schema.Array(IdeBaselineGapSchema).check(Schema.isMaxLength(100)),
  rawResultRefs: Schema.Array(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(30)),
  assertions: Schema.Array(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(50)),
});
export type IdeBaselineReceipt = typeof IdeBaselineReceiptSchema.Type;

export const decodeIdeBaselineReceipt = Schema.decodeUnknownEffect(IdeBaselineReceiptSchema);
