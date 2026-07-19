import { Schema } from "effect";

const NonNegativeIntegerSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
);

export const IdePackageSpikePhaseSchema = Schema.Literals([
  "ready",
  "disposed",
  "expected_failure",
]);

export const IdePackageSpikeSnapshotSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-package-spike.v1"),
  phase: IdePackageSpikePhaseSchema,
  cycle: NonNegativeIntegerSchema,
  themeId: Schema.Literal("tokyo-night"),
  themeBeforeEditorPaint: Schema.Literal(true),
  monaco: Schema.Struct({
    modelCount: NonNegativeIntegerSchema,
    editorsCreated: NonNegativeIntegerSchema,
    languageWorkersReady: Schema.Array(
      Schema.Literals(["editor", "json", "css", "html", "typescript"]),
    ),
    failureLabel: Schema.NullOr(Schema.String),
  }),
  pierre: Schema.Struct({
    rendered: Schema.Boolean,
    unified: Schema.Boolean,
    split: Schema.Boolean,
    annotation: Schema.Boolean,
    selectedRange: Schema.Boolean,
    workerInitialized: Schema.Boolean,
    virtualized: Schema.Boolean,
    scaleItems: NonNegativeIntegerSchema,
    renderedScaleItems: NonNegativeIntegerSchema,
  }),
  resources: Schema.Struct({
    activeWorkers: NonNegativeIntegerSchema,
    createdWorkers: NonNegativeIntegerSchema,
    externalUrls: Schema.Array(Schema.String),
    loadedUrls: Schema.Array(Schema.String),
  }),
  domNodes: NonNegativeIntegerSchema,
}).annotate({ identifier: "IdePackageSpikeSnapshot" });
export type IdePackageSpikeSnapshot = typeof IdePackageSpikeSnapshotSchema.Type;

export const IdePackageSpikeProbeReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-package-spike-probe.v1"),
  layout: Schema.Literals(["development", "asar"]),
  platform: Schema.String,
  architecture: Schema.String,
  electronVersion: Schema.String,
  cycles: Schema.Array(
    Schema.Struct({
      ready: IdePackageSpikeSnapshotSchema,
      disposed: IdePackageSpikeSnapshotSchema,
      loadMilliseconds: NonNegativeIntegerSchema,
      disposeMilliseconds: NonNegativeIntegerSchema,
      processWorkingSetBytes: NonNegativeIntegerSchema,
      rendererWorkingSetBytes: NonNegativeIntegerSchema,
    }),
  ).check(Schema.isMinLength(3), Schema.isMaxLength(3)),
  expectedFailure: IdePackageSpikeSnapshotSchema,
}).annotate({ identifier: "IdePackageSpikeProbeReceipt" });
export type IdePackageSpikeProbeReceipt = typeof IdePackageSpikeProbeReceiptSchema.Type;

export const IdePackageSpikeMatrixReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-package-spike-matrix.v1"),
  capturedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
  ),
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  development: IdePackageSpikeProbeReceiptSchema,
  asar: IdePackageSpikeProbeReceiptSchema,
  assertions: Schema.Array(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(40)),
}).annotate({ identifier: "IdePackageSpikeMatrixReceipt" });
export type IdePackageSpikeMatrixReceipt = typeof IdePackageSpikeMatrixReceiptSchema.Type;

export const decodeIdePackageSpikeSnapshot = Schema.decodeUnknownSync(
  IdePackageSpikeSnapshotSchema,
);
export const decodeIdePackageSpikeProbeReceipt = Schema.decodeUnknownSync(
  IdePackageSpikeProbeReceiptSchema,
);
export const decodeIdePackageSpikeMatrixReceipt = Schema.decodeUnknownSync(
  IdePackageSpikeMatrixReceiptSchema,
);
