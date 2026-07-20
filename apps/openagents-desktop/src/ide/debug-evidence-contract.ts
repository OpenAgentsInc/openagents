import { Schema } from "effect";

import { IdeTimestampSchema } from "./project-contract.ts";

const boundedText = (maximum: number) =>
  Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(maximum));
const nonNegative = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0));
const positiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1));
const sha = Schema.String.check(
  Schema.isMinLength(40),
  Schema.isMaxLength(64),
  Schema.isPattern(/^[a-f0-9]+$/u),
);

export const IDE_DEBUG_METRIC_NAMES = [
  "configuration-validation",
  "adapter-launch",
  "adapter-attach",
  "first-stopped-paint",
  "breakpoint-round-trip",
  "step-continue",
  "stack-scope-variable-expansion",
  "evaluate-watch",
  "source-navigation",
  "restart-terminate",
  "memory-cpu-sample",
  "teardown",
] as const;

export const IDE_DEBUG_FAULT_NAMES = [
  "invalid-configuration",
  "missing-configuration",
  "malicious-variable-substitution",
  "secret-bearing-environment",
  "secret-bearing-arguments",
  "missing-adapter",
  "unsupported-capability",
  "malformed-protocol-message",
  "out-of-order-protocol-message",
  "oversized-protocol-message",
  "request-timeout",
  "cancellation-with-late-response",
  "adapter-crash",
  "adapter-restart",
  "target-crash-or-exit",
  "attach-auth-loss",
  "attach-network-loss",
  "process-id-reuse",
  "breakpoint-verification-churn",
  "dynamic-threads",
  "huge-deep-cyclic-variables",
  "expensive-evaluation",
  "source-map-failure",
  "remote-unavailable-changed-source",
  "prelaunch-failure",
  "postdebug-failure",
  "project-worktree-attachment-restart",
] as const;

export const IDE_DEBUG_ACCESSIBILITY_NAMES = [
  "keyboard-controls",
  "keyboard-pane-navigation",
  "screen-reader-state",
  "screen-reader-progress",
  "screen-reader-errors",
  "focus-restoration",
  "zoom",
  "reduced-motion",
  "vim-editor-key-boundary",
  "theme-contrast-and-non-color-cues",
  "minimum-window",
  "huge-tree-degradation",
] as const;

export const IDE_DEBUG_LIFECYCLE_NAMES = [
  "cancel",
  "adapter-restart",
  "target-loss",
  "project-switch",
  "app-restart",
] as const;

export const IDE_DEBUG_CONTROL_NAMES = [
  "continue",
  "pause",
  "step-in",
  "step-over",
  "step-out",
  "evaluate",
  "restart",
  "disconnect",
  "terminate",
] as const;

export const IDE_DEBUG_SOURCE_KINDS = [
  "source-map",
  "changed",
  "unavailable",
  "remote",
  "generated",
] as const;

export const IdeDebugDesktopTargetSchema = Schema.Literals([
  "macos-arm64",
  "macos-x64",
  "windows-arm64",
  "windows-x64",
  "linux-arm64",
  "linux-x64",
]);
export type IdeDebugDesktopTarget = typeof IdeDebugDesktopTargetSchema.Type;

export const IdeDebugMetricSchema = Schema.Struct({
  name: Schema.Literals(IDE_DEBUG_METRIC_NAMES),
  unit: Schema.Literals(["milliseconds", "bytes", "count", "percent"]),
  repetitions: positiveInteger,
  warmup: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  p50: nonNegative,
  p95: nonNegative,
  p99: nonNegative,
  thresholdP50: nonNegative,
  thresholdP95: nonNegative,
  thresholdP99: nonNegative,
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugMetric" });
export interface IdeDebugMetric extends Schema.Schema.Type<typeof IdeDebugMetricSchema> {}

export const IdeDebugControlObservationSchema = Schema.Struct({
  control: Schema.Literals(IDE_DEBUG_CONTROL_NAMES),
  supported: Schema.Boolean,
  capabilityNegotiated: Schema.Literal(true),
  cancellable: Schema.Literal(true),
  receiptRef: boundedText(360),
  unsupportedStateHonest: Schema.Literal(true),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugControlObservation" });
export interface IdeDebugControlObservation extends Schema.Schema.Type<
  typeof IdeDebugControlObservationSchema
> {}

export const IdeDebugSourceObservationSchema = Schema.Struct({
  kind: Schema.Literals(IDE_DEBUG_SOURCE_KINDS),
  canonicalIdentityUsed: Schema.Literal(true),
  guessedPosition: Schema.Literal(false),
  explicitState: Schema.Literal(true),
  evidenceRef: boundedText(360),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugSourceObservation" });
export interface IdeDebugSourceObservation extends Schema.Schema.Type<
  typeof IdeDebugSourceObservationSchema
> {}

export const IdeDebugMatrixRowSchema = Schema.Struct({
  name: boundedText(120),
  evidenceRef: boundedText(360),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugMatrixRow" });
export interface IdeDebugMatrixRow extends Schema.Schema.Type<typeof IdeDebugMatrixRowSchema> {}

export const IdeDebugLifecycleObservationSchema = Schema.Struct({
  transition: Schema.Literals(IDE_DEBUG_LIFECYCLE_NAMES),
  oldGeneration: positiveInteger,
  newGeneration: positiveInteger,
  lateEventSent: Schema.Literal(true),
  lateEventRejected: Schema.Literal(true),
  currentStateUnchanged: Schema.Literal(true),
  cleanupReceiptRef: boundedText(360),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugLifecycleObservation" });
export interface IdeDebugLifecycleObservation extends Schema.Schema.Type<
  typeof IdeDebugLifecycleObservationSchema
> {}

export const IdeDebugJourneySchema = Schema.Struct({
  journeyRef: boundedText(240),
  adapterKind: Schema.Literals(["deterministic-fake", "representative-real"]),
  adapterName: boundedText(120),
  adapterVersion: boundedText(80),
  language: boundedText(80),
  languageVersion: boundedText(80),
  mode: Schema.Literals(["launch", "attach"]),
  desktopTarget: IdeDebugDesktopTargetSchema,
  targetKind: Schema.Literals(["local-process", "remote-process", "container"]),
  transport: Schema.Literals(["stdio", "tcp", "pipe"]),
  configurationRef: boundedText(240),
  effectiveConfigurationDigest: sha,
  dataSourceRefs: Schema.Array(boundedText(240)).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(32),
  ),
  environmentValueRefsOnly: Schema.Literal(true),
  generations: Schema.Struct({
    project: positiveInteger,
    worktree: positiveInteger,
    attachment: positiveInteger,
    language: positiveInteger,
    target: positiveInteger,
    placement: positiveInteger,
    service: positiveInteger,
  }),
  capabilities: Schema.Struct({
    supported: Schema.Array(boundedText(120)).check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    unsupported: Schema.Array(boundedText(120)).check(
      Schema.isMinLength(1),
      Schema.isMaxLength(80),
    ),
    negotiatedBeforeCommands: Schema.Literal(true),
  }),
  projections: Schema.Struct({
    breakpoints: Schema.Literal(true),
    threads: Schema.Literal(true),
    stacks: Schema.Literal(true),
    scopes: Schema.Literal(true),
    variables: Schema.Literal(true),
    watches: Schema.Literal(true),
    console: Schema.Literal(true),
    modules: Schema.Literal(true),
    loadedSources: Schema.Literal(true),
  }),
  screenshotRef: boundedText(360),
  traceRef: boundedText(360),
  receiptRef: boundedText(360),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugJourney" });
export interface IdeDebugJourney extends Schema.Schema.Type<typeof IdeDebugJourneySchema> {}

export const IdeDebugTargetFactSchema = Schema.Struct({
  target: IdeDebugDesktopTargetSchema,
  claimed: Schema.Boolean,
  packagedJourneyRef: Schema.NullOr(boundedText(360)),
  nativeHelper: Schema.Literal(false),
  typescriptFallback: Schema.Literal(true),
  disposition: Schema.Literals(["packaged-journey-passed", "not-claimed"]),
}).annotate({ identifier: "IdeDebugTargetFact" });
export interface IdeDebugTargetFact extends Schema.Schema.Type<typeof IdeDebugTargetFactSchema> {}

const IdeDebugCapturedEvidenceFields = {
  recordedAt: IdeTimestampSchema,
  candidateCommitSha: sha,
  environment: Schema.Struct({
    platform: boundedText(40),
    architecture: boundedText(40),
    node: boundedText(40),
    electron: boundedText(40),
    appVersion: boundedText(40),
    runtime: Schema.Literal("Effect v4 + supervised DAP transport"),
    corpusRef: boundedText(360),
  }),
  artifact: Schema.Struct({
    treeSha256: sha,
    files: positiveInteger,
    bytes: positiveInteger,
    artifactRef: boundedText(360),
  }),
  journeys: Schema.Array(IdeDebugJourneySchema).check(
    Schema.isMinLength(4),
    Schema.isMaxLength(24),
  ),
  controls: Schema.Array(IdeDebugControlObservationSchema).check(
    Schema.isMinLength(IDE_DEBUG_CONTROL_NAMES.length),
    Schema.isMaxLength(IDE_DEBUG_CONTROL_NAMES.length),
  ),
  sources: Schema.Array(IdeDebugSourceObservationSchema).check(
    Schema.isMinLength(IDE_DEBUG_SOURCE_KINDS.length),
    Schema.isMaxLength(IDE_DEBUG_SOURCE_KINDS.length),
  ),
  lifecycle: Schema.Array(IdeDebugLifecycleObservationSchema).check(
    Schema.isMinLength(IDE_DEBUG_LIFECYCLE_NAMES.length),
    Schema.isMaxLength(IDE_DEBUG_LIFECYCLE_NAMES.length),
  ),
  faultMatrix: Schema.Array(IdeDebugMatrixRowSchema).check(
    Schema.isMinLength(IDE_DEBUG_FAULT_NAMES.length),
    Schema.isMaxLength(IDE_DEBUG_FAULT_NAMES.length),
  ),
  accessibilityMatrix: Schema.Array(IdeDebugMatrixRowSchema).check(
    Schema.isMinLength(IDE_DEBUG_ACCESSIBILITY_NAMES.length),
    Schema.isMaxLength(IDE_DEBUG_ACCESSIBILITY_NAMES.length),
  ),
  metrics: Schema.Array(IdeDebugMetricSchema).check(
    Schema.isMinLength(IDE_DEBUG_METRIC_NAMES.length),
    Schema.isMaxLength(IDE_DEBUG_METRIC_NAMES.length),
  ),
  policy: Schema.Struct({
    oneSchemaGraph: Schema.Literal(true),
    effectAuthority: Schema.Literal(true),
    rendererProjectionOnly: Schema.Literal(true),
    adapterMechanicsOnly: Schema.Literal(true),
    exactConfigurationDisclosed: Schema.Literal(true),
    exactGenerationsBound: Schema.Literal(true),
    launchAttachSeparatePaths: Schema.Literal(true),
    humanAgentSamePolicy: Schema.Literal(true),
    humanAgentSameBudgets: Schema.Literal(true),
    humanAgentSameIntervention: Schema.Literal(true),
    humanAgentSameObservability: Schema.Literal(true),
    humanAgentSameCleanup: Schema.Literal(true),
  }),
  security: Schema.Struct({
    secretsRemainReferences: Schema.Literal(true),
    projectedDataRedacted: Schema.Literal(true),
    protocolQueueBounded: Schema.Literal(true),
    consoleRetentionBounded: Schema.Literal(true),
    variableDepthBounded: Schema.Literal(true),
    variableCountBounded: Schema.Literal(true),
    retainedDataDeleted: Schema.Literal(true),
    rendererReceivesCredentials: Schema.Literal(false),
    evidenceContainsForbiddenMaterial: Schema.Literal(false),
  }),
  resources: Schema.Struct({
    activeHandlesAfter: Schema.Literal(0),
    adapterProcessesAfter: Schema.Literal(0),
    subscriptionsAfter: Schema.Literal(0),
    queuedProtocolMessagesAfter: Schema.Literal(0),
    retainedVariableBytesAfterDeletion: Schema.Literal(0),
    peakHeapBytes: positiveInteger,
    peakCpuPercent: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(100),
    ),
  }),
  targets: Schema.Array(IdeDebugTargetFactSchema).check(
    Schema.isMinLength(6),
    Schema.isMaxLength(6),
  ),
  nativeDecision: Schema.Struct({
    rustAdmitted: Schema.Literal(false),
    ac47AdmissionEvidencePresent: Schema.Literal(false),
    reason: boundedText(600),
  }),
  ownerDisposition: Schema.Literal("unreviewed"),
  assuranceLifecycle: Schema.Literal("proposed"),
} as const;

export const IdeDebugEvidenceInputSchema = Schema.TaggedUnion({
  Unexecuted: {
    schemaVersion: Schema.Literal("openagents.desktop.ide-debug-evidence-input.v1"),
    issue: Schema.Literal("IDE-11"),
    reason: boundedText(600),
    requiredRunner: boundedText(240),
  },
  Captured: {
    schemaVersion: Schema.Literal("openagents.desktop.ide-debug-evidence-input.v1"),
    issue: Schema.Literal("IDE-11"),
    ...IdeDebugCapturedEvidenceFields,
  },
}).annotate({ identifier: "IdeDebugEvidenceInput" });
export type IdeDebugEvidenceInput = typeof IdeDebugEvidenceInputSchema.Type;

export const IdeDebugBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-debug-benchmark.v1"),
  issue: Schema.Literal("IDE-11"),
  ...IdeDebugCapturedEvidenceFields,
  sourceEvidenceRef: boundedText(360),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugBenchmarkReceipt" });
export interface IdeDebugBenchmarkReceipt extends Schema.Schema.Type<
  typeof IdeDebugBenchmarkReceiptSchema
> {}

export const IdeDebugPackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-debug-packaged.v1"),
  issue: Schema.Literal("IDE-11"),
  recordedAt: IdeTimestampSchema,
  candidateCommitSha: sha,
  artifact: IdeDebugBenchmarkReceiptSchema.fields.artifact,
  environment: IdeDebugBenchmarkReceiptSchema.fields.environment,
  journeys: IdeDebugBenchmarkReceiptSchema.fields.journeys,
  controls: IdeDebugBenchmarkReceiptSchema.fields.controls,
  sources: IdeDebugBenchmarkReceiptSchema.fields.sources,
  lifecycle: IdeDebugBenchmarkReceiptSchema.fields.lifecycle,
  faultMatrix: IdeDebugBenchmarkReceiptSchema.fields.faultMatrix,
  accessibilityMatrix: IdeDebugBenchmarkReceiptSchema.fields.accessibilityMatrix,
  screenshotRefs: Schema.Array(boundedText(360)).check(
    Schema.isMinLength(4),
    Schema.isMaxLength(24),
  ),
  traceRefs: Schema.Array(boundedText(360)).check(Schema.isMinLength(4), Schema.isMaxLength(24)),
  sourceEvidenceRef: boundedText(360),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugPackagedJourneyReceipt" });
export interface IdeDebugPackagedJourneyReceipt extends Schema.Schema.Type<
  typeof IdeDebugPackagedJourneyReceiptSchema
> {}

export const IdeDebugAcceptanceReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-debug-acceptance.v1"),
  issue: Schema.Literal("IDE-11"),
  generatedAt: IdeTimestampSchema,
  candidateCommitSha: sha,
  mainEvaluationSha: sha,
  artifactTreeSha256: sha,
  benchmarkRef: boundedText(360),
  packagedRef: boundedText(360),
  evidenceRefs: Schema.Array(boundedText(360)).check(
    Schema.isMinLength(12),
    Schema.isMaxLength(100),
  ),
  exactReviewerRef: boundedText(360),
  ownerDisposition: Schema.Literal("unreviewed"),
  assuranceLifecycle: Schema.Literal("proposed"),
  rollbackTargetSha: sha,
  rustAdmitted: Schema.Literal(false),
  remainingGaps: Schema.Array(boundedText(600)).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(20),
  ),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeDebugAcceptanceReceipt" });
export interface IdeDebugAcceptanceReceipt extends Schema.Schema.Type<
  typeof IdeDebugAcceptanceReceiptSchema
> {}

const requireExactNames = (
  label: string,
  actual: ReadonlyArray<string>,
  required: ReadonlyArray<string>,
): void => {
  const actualSet = new Set(actual);
  const missing = required.filter((name) => !actualSet.has(name));
  const duplicateCount = actual.length - actualSet.size;
  if (missing.length > 0 || duplicateCount > 0) {
    throw new Error(
      `${label} is incomplete: missing=${missing.join(",") || "none"} duplicates=${duplicateCount}`,
    );
  }
};

type IdeDebugEvidenceValidationShape = Pick<
  Extract<IdeDebugEvidenceInput, { readonly _tag: "Captured" }>,
  | "metrics"
  | "controls"
  | "sources"
  | "lifecycle"
  | "faultMatrix"
  | "accessibilityMatrix"
  | "targets"
  | "journeys"
>;

export const validateIdeDebugCapturedEvidence = (
  evidence: IdeDebugEvidenceValidationShape,
): void => {
  requireExactNames(
    "IDE-11 metrics",
    evidence.metrics.map((row) => row.name),
    IDE_DEBUG_METRIC_NAMES,
  );
  requireExactNames(
    "IDE-11 controls",
    evidence.controls.map((row) => row.control),
    IDE_DEBUG_CONTROL_NAMES,
  );
  requireExactNames(
    "IDE-11 sources",
    evidence.sources.map((row) => row.kind),
    IDE_DEBUG_SOURCE_KINDS,
  );
  requireExactNames(
    "IDE-11 lifecycle matrix",
    evidence.lifecycle.map((row) => row.transition),
    IDE_DEBUG_LIFECYCLE_NAMES,
  );
  requireExactNames(
    "IDE-11 fault matrix",
    evidence.faultMatrix.map((row) => row.name),
    IDE_DEBUG_FAULT_NAMES,
  );
  requireExactNames(
    "IDE-11 accessibility matrix",
    evidence.accessibilityMatrix.map((row) => row.name),
    IDE_DEBUG_ACCESSIBILITY_NAMES,
  );
  requireExactNames(
    "IDE-11 target table",
    evidence.targets.map((row) => row.target),
    ["macos-arm64", "macos-x64", "windows-arm64", "windows-x64", "linux-arm64", "linux-x64"],
  );

  const fakeJourneys = evidence.journeys.filter(
    (journey) => journey.adapterKind === "deterministic-fake",
  );
  const realJourneys = evidence.journeys.filter(
    (journey) => journey.adapterKind === "representative-real",
  );
  const modes = new Set(evidence.journeys.map((journey) => journey.mode));
  const realLanguages = new Set(realJourneys.map((journey) => journey.language));
  if (
    fakeJourneys.length === 0 ||
    realJourneys.length < 2 ||
    realLanguages.size < 2 ||
    !modes.has("launch") ||
    !modes.has("attach")
  ) {
    throw new Error(
      "IDE-11 corpus must include a deterministic fake, two representative real languages, launch, and attach",
    );
  }
  if (evidence.lifecycle.some((row) => row.newGeneration <= row.oldGeneration)) {
    throw new Error("IDE-11 lifecycle observations must advance the generation");
  }
  if (
    evidence.metrics.some(
      (row) =>
        row.p50 > row.p95 ||
        row.p95 > row.p99 ||
        row.p50 > row.thresholdP50 ||
        row.p95 > row.thresholdP95 ||
        row.p99 > row.thresholdP99,
    )
  ) {
    throw new Error("IDE-11 metric percentiles must be ordered and remain within all thresholds");
  }
  if (
    !evidence.targets.some(
      (row) =>
        row.claimed &&
        row.disposition === "packaged-journey-passed" &&
        row.packagedJourneyRef !== null,
    )
  ) {
    throw new Error("IDE-11 target table must include at least one claimed packaged target");
  }
  if (
    evidence.targets.some(
      (row) =>
        row.claimed !== (row.disposition === "packaged-journey-passed") ||
        row.claimed !== (row.packagedJourneyRef !== null),
    )
  ) {
    throw new Error("IDE-11 target claims must bind an exact packaged journey");
  }
  const claimedTargets = new Set(
    evidence.targets.filter((row) => row.claimed).map((row) => row.target),
  );
  if (evidence.journeys.some((journey) => !claimedTargets.has(journey.desktopTarget))) {
    throw new Error("IDE-11 journeys must use a target with an exact packaged claim");
  }
  const journeyRefs = new Set(evidence.journeys.map((journey) => journey.journeyRef));
  if (journeyRefs.size !== evidence.journeys.length) {
    throw new Error("IDE-11 journey references must be unique");
  }
  if (
    evidence.journeys.some((journey) => {
      const supported = new Set(journey.capabilities.supported);
      const unsupported = new Set(journey.capabilities.unsupported);
      return (
        supported.size !== journey.capabilities.supported.length ||
        unsupported.size !== journey.capabilities.unsupported.length ||
        [...supported].some((capability) => unsupported.has(capability))
      );
    })
  ) {
    throw new Error("IDE-11 capability tables must be unique and disjoint");
  }
};
