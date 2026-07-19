import { Schema } from "effect";

const BytesSchema = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0));
const CountSchema = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0));

export const IdePackageBundleAssetSchema = Schema.Struct({
  label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  file: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240)),
  bytes: BytesSchema,
});
export type IdePackageBundleAsset = typeof IdePackageBundleAssetSchema.Type;

export const IdePackageAuditReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-package-audit.v1"),
  capturedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
  ),
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  packages: Schema.Array(
    Schema.Struct({
      packageName: Schema.Literals(["monaco-editor", "@pierre/diffs"]),
      version: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
      license: Schema.Literals(["MIT", "Apache-2.0"]),
      sourceCommit: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
      registryIntegrity: Schema.String.check(Schema.isPattern(/^sha512-/u)),
      installedPackageBytes: BytesSchema,
      directRuntimeDependencies: CountSchema,
    }),
  ).check(Schema.isMinLength(2), Schema.isMaxLength(2)),
  bundles: Schema.Struct({
    normalBootJavaScriptBytes: BytesSchema,
    normalBootCssBytes: BytesSchema,
    fixtureRuntimeBytes: BytesSchema,
    fixtureSourceMapBytes: BytesSchema,
    fixtureAssetCount: CountSchema,
    fixtureEntryJavaScriptBytes: BytesSchema,
    workerBytes: BytesSchema,
    workers: Schema.Array(IdePackageBundleAssetSchema).check(
      Schema.isMinLength(6),
      Schema.isMaxLength(6),
    ),
  }),
  runtime: Schema.Struct({
    developmentLoadP95Milliseconds: BytesSchema,
    asarLoadP95Milliseconds: BytesSchema,
    developmentWorkingSetP95Bytes: BytesSchema,
    asarWorkingSetP95Bytes: BytesSchema,
    developmentRendererWorkingSetP95Bytes: BytesSchema,
    asarRendererWorkingSetP95Bytes: BytesSchema,
    ide00ChatFirstPaintP95Milliseconds: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
    ide01ChatFirstPaintP95Milliseconds: Schema.NullOr(
      Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
    ),
  }),
  gates: Schema.Struct({
    normalBootContainsEditorCode: Schema.Literal(false),
    fixtureBuildIsOptIn: Schema.Literal(true),
    restrictiveCspHasWorkerSelf: Schema.Literal(true),
    restrictiveCspHasUnsafeEval: Schema.Literal(false),
    fixtureHasManifest: Schema.Literal(true),
    attributedJavaScriptAssets: CountSchema,
    unattributedJavaScriptAssets: Schema.Array(Schema.String).check(Schema.isMaxLength(4)),
  }),
  findings: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500))).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(30),
  ),
});
export type IdePackageAuditReceipt = typeof IdePackageAuditReceiptSchema.Type;

export const decodeIdePackageAuditReceipt = Schema.decodeUnknownSync(IdePackageAuditReceiptSchema);
