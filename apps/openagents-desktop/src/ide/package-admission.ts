import { Schema } from "effect";

const BoundedTextSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000));
const NonNegativeIntegerSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
);
const NonNegativeBytesSchema = NonNegativeIntegerSchema;

export const IdePackageDecisionSchemaVersion = Schema.Literal(
  "openagents.desktop.ide-package-decision.v1",
);

export const IdePackageCompatibilityStateSchema = Schema.Literals([
  "pass",
  "pass_with_adapter",
  "not_applicable",
  "deferred",
  "fail",
]);
export type IdePackageCompatibilityState = typeof IdePackageCompatibilityStateSchema.Type;

export const IdePackageCompatibilitySchema = Schema.Struct({
  electron43: IdePackageCompatibilityStateSchema,
  react19: IdePackageCompatibilityStateSchema,
  node24: IdePackageCompatibilityStateSchema,
  vite8: IdePackageCompatibilityStateSchema,
  typescript6: IdePackageCompatibilityStateSchema,
  restrictiveCsp: IdePackageCompatibilityStateSchema,
  rendererSandbox: IdePackageCompatibilityStateSchema,
  asar: IdePackageCompatibilityStateSchema,
  offline: IdePackageCompatibilityStateSchema,
  sourceMaps: IdePackageCompatibilityStateSchema,
}).annotate({ identifier: "IdePackageCompatibility" });
export type IdePackageCompatibility = typeof IdePackageCompatibilitySchema.Type;

/** A projection dependency may never become an application authority. */
export const IdePackageAuthorityAuditSchema = Schema.Struct({
  workspaceGrant: Schema.Literal(false),
  documentTruth: Schema.Literal(false),
  processPolicy: Schema.Literal(false),
  gitMutation: Schema.Literal(false),
  persistence: Schema.Literal(false),
  approval: Schema.Literal(false),
  receipt: Schema.Literal(false),
}).annotate({ identifier: "IdePackageAuthorityAudit" });
export type IdePackageAuthorityAudit = typeof IdePackageAuthorityAuditSchema.Type;

export const IdePackageArtifactSchema = Schema.Struct({
  packageName: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  version: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  repository: Schema.String.check(Schema.isPattern(/^https:\/\/github\.com\//u)),
  sourceCommit: Schema.NullOr(Schema.String.check(Schema.isPattern(/^[a-f0-9]{40}$/u))),
  registryIntegrity: Schema.NullOr(
    Schema.String.check(Schema.isPattern(/^sha512-[A-Za-z0-9+/=]+$/u)),
  ),
  license: Schema.Literals(["MIT", "Apache-2.0"]),
  publishedAt: Schema.NullOr(Schema.String.check(Schema.isPattern(/^\d{4}-\d{2}-\d{2}T/u))),
}).annotate({ identifier: "IdePackageArtifact" });
export type IdePackageArtifact = typeof IdePackageArtifactSchema.Type;

export const IdePackageCostSchema = Schema.Struct({
  registryUnpackedBytes: Schema.NullOr(NonNegativeBytesSchema),
  directRuntimeDependencies: NonNegativeIntegerSchema,
  newUniqueLockNodes: NonNegativeIntegerSchema,
  eagerRendererBytes: NonNegativeBytesSchema,
  lazyRendererBytes: Schema.NullOr(NonNegativeBytesSchema),
  workerBytes: Schema.NullOr(NonNegativeBytesSchema),
  startupDeltaMillisecondsP95: Schema.NullOr(Schema.Number.check(Schema.isFinite())),
  memoryDeltaBytesP95: Schema.NullOr(NonNegativeBytesSchema),
}).annotate({ identifier: "IdePackageCost" });
export type IdePackageCost = typeof IdePackageCostSchema.Type;

const decisionFields = {
  schemaVersion: IdePackageDecisionSchemaVersion,
  artifact: IdePackageArtifactSchema,
  maintenance: BoundedTextSchema,
  publicApi: BoundedTextSchema,
  securityPosture: BoundedTextSchema,
  compatibility: IdePackageCompatibilitySchema,
  cost: IdePackageCostSchema,
  workerAssetStrategy: BoundedTextSchema,
  disposalStrategy: BoundedTextSchema,
  rollback: BoundedTextSchema,
  authority: IdePackageAuthorityAuditSchema,
};

export const IdePackageDecisionSchema = Schema.TaggedUnion({
  Adopt: {
    ...decisionFields,
    adapterBoundary: BoundedTextSchema,
  },
  Reject: {
    ...decisionFields,
    rejection: BoundedTextSchema,
    fallback: BoundedTextSchema,
  },
  Defer: {
    ...decisionFields,
    plannedPacket: Schema.String.check(Schema.isPattern(/^IDE-\d{2}$/u)),
    admissionCondition: BoundedTextSchema,
  },
}).annotate({ identifier: "IdePackageDecision" });
export type IdePackageDecision = typeof IdePackageDecisionSchema.Type;

const noAuthority = IdePackageAuthorityAuditSchema.make({
  workspaceGrant: false,
  documentTruth: false,
  processPolicy: false,
  gitMutation: false,
  persistence: false,
  approval: false,
  receipt: false,
});

const unmeasuredCost = (input: {
  readonly registryUnpackedBytes: number | null;
  readonly directRuntimeDependencies: number;
  readonly newUniqueLockNodes: number;
}): IdePackageCost =>
  IdePackageCostSchema.make({
    ...input,
    eagerRendererBytes: 0,
    lazyRendererBytes: null,
    workerBytes: null,
    startupDeltaMillisecondsP95: null,
    memoryDeltaBytesP95: null,
  });

const deferredCompatibility = IdePackageCompatibilitySchema.make({
  electron43: "deferred",
  react19: "not_applicable",
  node24: "deferred",
  vite8: "deferred",
  typescript6: "deferred",
  restrictiveCsp: "deferred",
  rendererSandbox: "deferred",
  asar: "deferred",
  offline: "deferred",
  sourceMaps: "deferred",
});

/**
 * Machine-readable IDE-01 decisions. Measured bundle/worker/startup values are
 * kept in the generated IDE-01 evidence receipt because they are build-host
 * observations, not immutable package metadata.
 */
export const ide01PackageDecisions = Schema.Array(IdePackageDecisionSchema).make([
  {
    _tag: "Adopt",
    schemaVersion: "openagents.desktop.ide-package-decision.v1",
    artifact: {
      packageName: "monaco-editor",
      version: "0.55.1",
      repository: "https://github.com/microsoft/monaco-editor",
      sourceCommit: "516f350bdaf7a82f6731bd128a9ec86a6e5fa47d",
      registryIntegrity:
        "sha512-jz4x+TJNFHwHtwuV9vA9rMujcZRb0CEilTEwG2rRSpe/A7Jdkuj8xPKttCgOh+v/lkHy7HsZ64oj+q3xoAFl9A==",
      license: "MIT",
      publishedAt: "2025-11-20T20:26:31.188Z",
    },
    maintenance:
      "Microsoft-maintained public editor seam; 0.55.1 is the latest stable release in the audited lineage and fixes the 0.55.0 language-export regression.",
    publicApi:
      "Only monaco.d.ts and documented ESM language worker entries are admitted. No Code-OSS workbench, extension host, or private vs/* module enters the adapter.",
    securityPosture:
      "MIT artifact with integrity pin; its DOMPurify 3.2.7 and marked 14.0.0 runtime dependencies remain renderer-only and are covered by lockfile supply-chain policy.",
    compatibility: {
      electron43: "pass",
      react19: "not_applicable",
      node24: "pass",
      vite8: "pass_with_adapter",
      typescript6: "pass",
      restrictiveCsp: "pass_with_adapter",
      rendererSandbox: "pass",
      asar: "pass_with_adapter",
      offline: "pass",
      sourceMaps: "pass",
    },
    cost: unmeasuredCost({
      registryUnpackedBytes: 72_633_330,
      directRuntimeDependencies: 2,
      newUniqueLockNodes: 4,
    }),
    workerAssetStrategy:
      "A dedicated Vite ESM entry emits editor, JSON, CSS, HTML, and TypeScript workers under the signed renderer tree. The oa-desktop protocol serves only normalized renderer assets from that tree.",
    disposalStrategy:
      "The owned runtime disposes views and models exactly once, tracks every worker constructor, terminates workers on scope close, and can be replaced behind the same controller contract.",
    rollback:
      "Remove the exact dependency and spike entry; IDE-03 retains the existing textarea/stub compatibility path until the production controller is accepted.",
    authority: noAuthority,
    adapterBoundary:
      "App-owned Monaco runtime/controller maps opaque document refs, versions, edits, commands, theme data, and worker failures; main-owned services retain files, grants, saves, conflicts, and recovery.",
  },
  {
    _tag: "Adopt",
    schemaVersion: "openagents.desktop.ide-package-decision.v1",
    artifact: {
      packageName: "@pierre/diffs",
      version: "1.2.12",
      repository: "https://github.com/pierrecomputer/pierre",
      sourceCommit: "9466c467ae6fc03501b6bca74c12f717d70293a7",
      registryIntegrity:
        "sha512-pY/gmgWL03WnagqCyCnBi3QtRXUv4hCIY6FYqd5b1ZGaoI6a4Bsji8j+yRl2RfzPh/8Hf19rCl1GE80G6a1cLQ==",
      license: "Apache-2.0",
      publishedAt: "2026-06-29T21:35:57.166Z",
    },
    maintenance:
      "Actively maintained Pierre monorepo with React 19 peers and frequent 1.2.x releases; exact 1.2.12 is pinned rather than following beta tags.",
    publicApi:
      "Use only @pierre/diffs/react, @pierre/diffs/worker, and documented custom-theme registration. Parsing, rendering, selection, annotations, virtualization, and pool lifecycle remain behind one app adapter.",
    securityPosture:
      "Apache-2.0 artifact with integrity pin and license retained; raw unsafeCSS, remote theme loading, and package-provided mutation actions are denied at the adapter.",
    compatibility: {
      electron43: "pass",
      react19: "pass",
      node24: "pass",
      vite8: "pass_with_adapter",
      typescript6: "pass",
      restrictiveCsp: "pass_with_adapter",
      rendererSandbox: "pass",
      asar: "pass_with_adapter",
      offline: "pass",
      sourceMaps: "pass",
    },
    cost: unmeasuredCost({
      registryUnpackedBytes: 5_232_264,
      directRuntimeDependencies: 7,
      newUniqueLockNodes: 4,
    }),
    workerAssetStrategy:
      "Vite emits the documented portable worker from @pierre/diffs/worker/worker.js into the same renderer asset closure; the app supplies a bounded one-worker factory and no CDN URL.",
    disposalStrategy:
      "The app owns React roots and the WorkerPoolContext lifetime; unmount terminates the package singleton and the outer scope verifies zero tracked workers before replacement.",
    rollback:
      "Remove the exact dependency/adapter and retain the existing plain typed hunk renderer as the explicit review fallback.",
    authority: noAuthority,
    adapterBoundary:
      "Adapter accepts typed patch/file metadata and presentation callbacks only. It never receives absolute roots, grants, preload bridge objects, Git commands, apply functions, or receipt writers.",
  },
  {
    _tag: "Reject",
    schemaVersion: "openagents.desktop.ide-package-decision.v1",
    artifact: {
      packageName: "monaco-vim",
      version: "0.4.4",
      repository: "https://github.com/brijeshb42/monaco-vim",
      sourceCommit: "f7f085732795f58f0fee5d03a46bdc459d6c8a30",
      registryIntegrity:
        "sha512-LNChAb//WEm/W+eyeHG/0+pdVEHotk2hLTN+M3sQZx5E8cAlSWSgqcxpcRuQnxDybSln7pfHF9i63HmbIQvrWw==",
      license: "MIT",
      publishedAt: "2025-11-22T08:17:30.852Z",
    },
    maintenance:
      "Published recently enough to evaluate, but its compatibility surface still follows a CodeMirror-Vim adapter lineage and exposes many untyped any-valued internals.",
    publicApi:
      "The ESM bundle imports monaco-editor/esm/vs/editor/common/commands/shiftCommand, a private path outside monaco.d.ts.",
    securityPosture:
      "MIT and dependency-light, but private editor imports make upgrades unverifiable and its README explicitly warns that Ex/search/replace extra-input paths may fail.",
    compatibility: {
      electron43: "pass",
      react19: "not_applicable",
      node24: "pass",
      vite8: "fail",
      typescript6: "pass_with_adapter",
      restrictiveCsp: "pass",
      rendererSandbox: "pass",
      asar: "pass",
      offline: "pass",
      sourceMaps: "pass",
    },
    cost: unmeasuredCost({
      registryUnpackedBytes: 2_593_569,
      directRuntimeDependencies: 0,
      newUniqueLockNodes: 1,
    }),
    workerAssetStrategy: "No worker assets; rejection occurs before packaging admission.",
    disposalStrategy:
      "dispose() exists, but the package cannot be admitted while it relies on private Monaco commands and incomplete extra-input behavior.",
    rollback: "Not installed in the Desktop manifest or lockfile.",
    authority: noAuthority,
    rejection:
      "Fails the public-Monaco-API and complete first-party Vim contract gates; private ShiftCommand coupling is a release blocker, not a warning.",
    fallback:
      "IDE-03 implements an app-owned VimModeController over public Monaco commands with explicit mode, command, IME, accessibility, split, persistence, and teardown tests.",
  },
  {
    _tag: "Reject",
    schemaVersion: "openagents.desktop.ide-package-decision.v1",
    artifact: {
      packageName: "@replit/codemirror-vim",
      version: "6.3.0",
      repository: "https://github.com/replit/codemirror-vim",
      sourceCommit: null,
      registryIntegrity:
        "sha512-aTx931ULAMuJx6xLf7KQDOL7CxD+Sa05FktTDrtLaSy53uj01ll3Zf17JdKsriER248oS55GBzg0CfCTjEneAQ==",
      license: "MIT",
      publishedAt: "2026-06-18T20:59:07.141Z",
    },
    maintenance:
      "Active upstream and strong Vim coverage, but it targets CodeMirror 6 rather than Monaco.",
    publicApi:
      "Requires the CodeMirror view/state/search/commands/language runtime and cannot control Monaco through a supported adapter seam.",
    securityPosture:
      "MIT; rejected for architectural incompatibility before install or runtime evaluation.",
    compatibility: {
      electron43: "pass",
      react19: "not_applicable",
      node24: "pass",
      vite8: "pass",
      typescript6: "pass",
      restrictiveCsp: "pass",
      rendererSandbox: "pass",
      asar: "pass",
      offline: "pass",
      sourceMaps: "pass",
    },
    cost: unmeasuredCost({
      registryUnpackedBytes: 717_247,
      directRuntimeDependencies: 5,
      newUniqueLockNodes: 6,
    }),
    workerAssetStrategy: "No worker assets; rejection occurs before packaging admission.",
    disposalStrategy: "Not applicable because the CodeMirror runtime is not admitted.",
    rollback: "Not installed in the Desktop manifest or lockfile.",
    authority: noAuthority,
    rejection:
      "Admitting a second editor runtime solely for Vim would duplicate document/view authority and fail the Monaco replacement boundary.",
    fallback:
      "Use the same first-party public-Monaco VimModeController selected after monaco-vim rejection.",
  },
  ...[
    [
      "vscode-uri",
      "3.1.0",
      "IDE-06",
      "Only at the LSP URI serialization boundary after root-redaction conformance.",
    ],
    [
      "vscode-jsonrpc",
      "8.2.0",
      "IDE-06",
      "Only with the stable protocol set in a supervised utility process.",
    ],
    [
      "vscode-languageserver-protocol",
      "3.17.5",
      "IDE-06",
      "Only behind Effect schemas and generated protocol fixtures.",
    ],
    [
      "vscode-languageserver-textdocument",
      "1.0.12",
      "IDE-06",
      "May represent LSP snapshots but never canonical Desktop documents.",
    ],
  ].map(([packageName, version, plannedPacket, admissionCondition]) => ({
    _tag: "Defer" as const,
    schemaVersion: "openagents.desktop.ide-package-decision.v1" as const,
    artifact: {
      packageName: String(packageName),
      version: String(version),
      repository: "https://github.com/microsoft/vscode-languageserver-node",
      sourceCommit: null,
      registryIntegrity: null,
      license: "MIT" as const,
      publishedAt: null,
    },
    maintenance:
      "Maintained public Microsoft package; exact compatible-set provenance remains an IDE-06 responsibility.",
    publicApi: "Public package surface only; no Code-OSS workbench or private vs/* import.",
    securityPosture:
      "No runtime dependency is admitted by IDE-01; IDE-06 must re-run lockfile and process-boundary review.",
    compatibility: deferredCompatibility,
    cost: unmeasuredCost({
      registryUnpackedBytes: null,
      directRuntimeDependencies: 0,
      newUniqueLockNodes: 0,
    }),
    workerAssetStrategy:
      "No renderer worker; proposed ownership is a supervised language utility process.",
    disposalStrategy:
      "IDE-06 must bind process, connection, and document snapshots to one scoped service generation.",
    rollback: "No IDE-01 manifest or lockfile entry exists.",
    authority: noAuthority,
    plannedPacket: String(plannedPacket),
    admissionCondition: String(admissionCondition),
  })),
]);

export const decodeIdePackageDecisions = Schema.decodeUnknownSync(
  Schema.Array(IdePackageDecisionSchema),
);
