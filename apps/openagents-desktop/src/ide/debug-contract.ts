import { Exit, Schema } from "effect";

import {
  IdeAttachmentGenerationSchema,
  IdeCapabilityStateSchema,
  IdeDebugSessionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  IdeLanguageGenerationSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeServiceGenerationSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";
import { IdeRunActorSchema } from "./run-contract.ts";

const boundedText = (maximum: number) => Schema.String.check(Schema.isMaxLength(maximum));
const nonEmptyText = (maximum: number) =>
  Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(maximum));
const boundedCount = (maximum: number) =>
  Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
    Schema.isLessThanOrEqualTo(maximum),
  );
const positiveGeneration = <const Identifier extends string>(identifier: Identifier) =>
  Schema.Number.pipe(
    Schema.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
    Schema.brand(identifier),
  ).annotate({ identifier });
const boundedRef = <const Identifier extends string>(identifier: Identifier, prefix: string) =>
  Schema.String.pipe(
    Schema.check(
      Schema.isMinLength(prefix.length + 1),
      Schema.isMaxLength(192),
      Schema.isPattern(
        new RegExp(`^${prefix.replaceAll(".", "\\.")}[A-Za-z0-9][A-Za-z0-9._-]*$`, "u"),
      ),
    ),
    Schema.brand(identifier),
  ).annotate({ identifier });

export const IdeDebugSchemaVersion = Schema.Literal("openagents.desktop.ide-debug.v1");

export const IdeDebugConfigurationRefSchema = boundedRef(
  "IdeDebugConfigurationRef",
  "ide.debug-config.",
);
export type IdeDebugConfigurationRef = typeof IdeDebugConfigurationRefSchema.Type;
export const IdeDebugAdapterRefSchema = boundedRef("IdeDebugAdapterRef", "ide.debug-adapter.");
export type IdeDebugAdapterRef = typeof IdeDebugAdapterRefSchema.Type;
export const IdeDebugTargetRefSchema = boundedRef("IdeDebugTargetRef", "ide.debug-target.");
export type IdeDebugTargetRef = typeof IdeDebugTargetRefSchema.Type;
export const IdeDebugBreakpointRefSchema = boundedRef(
  "IdeDebugBreakpointRef",
  "ide.debug-breakpoint.",
);
export type IdeDebugBreakpointRef = typeof IdeDebugBreakpointRefSchema.Type;
export const IdeDebugThreadRefSchema = boundedRef("IdeDebugThreadRef", "ide.debug-thread.");
export type IdeDebugThreadRef = typeof IdeDebugThreadRefSchema.Type;
export const IdeDebugFrameRefSchema = boundedRef("IdeDebugFrameRef", "ide.debug-frame.");
export type IdeDebugFrameRef = typeof IdeDebugFrameRefSchema.Type;
export const IdeDebugScopeRefSchema = boundedRef("IdeDebugScopeRef", "ide.debug-scope.");
export type IdeDebugScopeRef = typeof IdeDebugScopeRefSchema.Type;
export const IdeDebugVariableRefSchema = boundedRef("IdeDebugVariableRef", "ide.debug-variable.");
export type IdeDebugVariableRef = typeof IdeDebugVariableRefSchema.Type;
export const IdeDebugWatchRefSchema = boundedRef("IdeDebugWatchRef", "ide.debug-watch.");
export type IdeDebugWatchRef = typeof IdeDebugWatchRefSchema.Type;
export const IdeDebugModuleRefSchema = boundedRef("IdeDebugModuleRef", "ide.debug-module.");
export type IdeDebugModuleRef = typeof IdeDebugModuleRefSchema.Type;
export const IdeDebugSourceRefSchema = boundedRef("IdeDebugSourceRef", "ide.debug-source.");
export type IdeDebugSourceRef = typeof IdeDebugSourceRefSchema.Type;
export const IdeDebugReceiptRefSchema = boundedRef("IdeDebugReceiptRef", "ide.debug-receipt.");
export type IdeDebugReceiptRef = typeof IdeDebugReceiptRefSchema.Type;
export const IdeDebugOperationRefSchema = boundedRef(
  "IdeDebugOperationRef",
  "ide.debug-operation.",
);
export type IdeDebugOperationRef = typeof IdeDebugOperationRefSchema.Type;

export const IdeDebugConfigurationGenerationSchema = positiveGeneration(
  "IdeDebugConfigurationGeneration",
);
export type IdeDebugConfigurationGeneration = typeof IdeDebugConfigurationGenerationSchema.Type;
export const IdeDebugSessionGenerationSchema = positiveGeneration("IdeDebugSessionGeneration");
export type IdeDebugSessionGeneration = typeof IdeDebugSessionGenerationSchema.Type;
export const IdeDebugAdapterGenerationSchema = positiveGeneration("IdeDebugAdapterGeneration");
export type IdeDebugAdapterGeneration = typeof IdeDebugAdapterGenerationSchema.Type;
export const IdeDebugTargetGenerationSchema = positiveGeneration("IdeDebugTargetGeneration");
export type IdeDebugTargetGeneration = typeof IdeDebugTargetGenerationSchema.Type;
export const IdeDebugSequenceSchema = positiveGeneration("IdeDebugSequence");
export type IdeDebugSequence = typeof IdeDebugSequenceSchema.Type;

export const IdeDebugBindingSchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  languageGeneration: IdeLanguageGenerationSchema,
  placementGeneration: IdePlacementGenerationSchema,
  serviceGeneration: IdeServiceGenerationSchema,
  placementRef: IdePlacementRefSchema,
  language: nonEmptyText(80),
}).annotate({ identifier: "IdeDebugBinding" });
export interface IdeDebugBinding extends Schema.Schema.Type<typeof IdeDebugBindingSchema> {}

export const IdeDebugPlacementSchema = Schema.TaggedUnion({
  Local: { hostLabel: nonEmptyText(160) },
  Container: { containerRef: nonEmptyText(192), hostLabel: nonEmptyText(160) },
  Remote: {
    hostRef: nonEmptyText(192),
    hostLabel: nonEmptyText(160),
    networkRef: nonEmptyText(192),
  },
}).annotate({ identifier: "IdeDebugPlacement" });
export type IdeDebugPlacement = typeof IdeDebugPlacementSchema.Type;

export const IdeDebugIntentSchema = Schema.TaggedUnion({
  Launch: {
    executableRef: nonEmptyText(192),
    executableLabel: nonEmptyText(320),
    argumentLabels: Schema.Array(boundedText(512)).check(Schema.isMaxLength(128)),
    prelaunchTaskRef: Schema.NullOr(nonEmptyText(192)),
    postdebugTaskRef: Schema.NullOr(nonEmptyText(192)),
  },
  Attach: {
    transportRef: nonEmptyText(192),
    targetProcessRef: nonEmptyText(192),
    targetProcessLabel: nonEmptyText(240),
    authenticationRef: Schema.NullOr(nonEmptyText(192)),
    reusedDeadAttachment: Schema.Literal(false),
  },
}).annotate({ identifier: "IdeDebugIntent" });
export type IdeDebugIntent = typeof IdeDebugIntentSchema.Type;

export const IdeDebugEnvironmentManifestSchema = Schema.Struct({
  manifestRef: nonEmptyText(192),
  admittedKeys: Schema.Array(nonEmptyText(96)).check(Schema.isMaxLength(512)),
  redactedKeys: Schema.Array(nonEmptyText(96)).check(Schema.isMaxLength(512)),
  sourceRefs: Schema.Array(nonEmptyText(192)).check(Schema.isMaxLength(64)),
  valuesExposedToRenderer: Schema.Literal(false),
  digest: nonEmptyText(96),
}).annotate({ identifier: "IdeDebugEnvironmentManifest" });
export interface IdeDebugEnvironmentManifest extends Schema.Schema.Type<
  typeof IdeDebugEnvironmentManifestSchema
> {}

export const IdeDebugSourceMapManifestSchema = Schema.Struct({
  manifestRef: nonEmptyText(192),
  sourceRoots: Schema.Array(nonEmptyText(512)).check(Schema.isMaxLength(64)),
  remoteRootRefs: Schema.Array(nonEmptyText(512)).check(Schema.isMaxLength(64)),
  generatedSourcesExplicit: Schema.Boolean,
  guessPositions: Schema.Literal(false),
  digest: nonEmptyText(96),
}).annotate({ identifier: "IdeDebugSourceMapManifest" });
export interface IdeDebugSourceMapManifest extends Schema.Schema.Type<
  typeof IdeDebugSourceMapManifestSchema
> {}

export const IdeDebugCapabilityNameSchema = Schema.Literals([
  "configuration_done",
  "conditional_breakpoints",
  "hit_conditional_breakpoints",
  "log_points",
  "function_breakpoints",
  "data_breakpoints",
  "set_variable",
  "evaluate",
  "pause",
  "step_in",
  "step_over",
  "step_out",
  "step_back",
  "run_to_cursor",
  "restart_frame",
  "restart_session",
  "continue",
  "disconnect",
  "terminate",
  "modules",
  "loaded_sources",
  "source_request",
  "cancel_request",
]);
export type IdeDebugCapabilityName = typeof IdeDebugCapabilityNameSchema.Type;
export const IdeDebugCapabilitySchema = Schema.Struct({
  capability: IdeDebugCapabilityNameSchema,
  supported: Schema.Boolean,
  reason: Schema.NullOr(nonEmptyText(300)),
}).annotate({ identifier: "IdeDebugCapability" });
export interface IdeDebugCapability extends Schema.Schema.Type<typeof IdeDebugCapabilitySchema> {}

export const IdeDebugAdapterSchema = Schema.Struct({
  adapterRef: IdeDebugAdapterRefSchema,
  adapterType: nonEmptyText(80),
  adapterVersion: nonEmptyText(80),
  executableRef: nonEmptyText(192),
  transport: Schema.Literals(["stdio", "socket", "pipe"]),
  admitted: Schema.Boolean,
  capabilities: Schema.Array(IdeDebugCapabilitySchema).check(Schema.isMaxLength(64)),
}).annotate({ identifier: "IdeDebugAdapter" });
export interface IdeDebugAdapter extends Schema.Schema.Type<typeof IdeDebugAdapterSchema> {}

export const IdeDebugConfigurationSchema = Schema.Struct({
  schemaVersion: IdeDebugSchemaVersion,
  configurationRef: IdeDebugConfigurationRefSchema,
  configurationGeneration: IdeDebugConfigurationGenerationSchema,
  label: nonEmptyText(160),
  binding: IdeDebugBindingSchema,
  intent: IdeDebugIntentSchema,
  placement: IdeDebugPlacementSchema,
  adapter: IdeDebugAdapterSchema,
  targetRef: IdeDebugTargetRefSchema,
  cwdRef: nonEmptyText(512),
  environment: IdeDebugEnvironmentManifestSchema,
  sourceMaps: IdeDebugSourceMapManifestSchema,
  timeoutMs: Schema.Number.check(
    Schema.isInt(),
    Schema.isBetween({ minimum: 100, maximum: 120_000 }),
  ),
  admitted: Schema.Boolean,
  refusalReason: Schema.NullOr(nonEmptyText(400)),
}).annotate({ identifier: "IdeDebugConfiguration" });
export interface IdeDebugConfiguration extends Schema.Schema.Type<
  typeof IdeDebugConfigurationSchema
> {}

export const IdeDebugSourceSchema = Schema.Struct({
  sourceRef: IdeDebugSourceRefSchema,
  fileRef: Schema.NullOr(IdeFileRefSchema),
  documentRef: Schema.NullOr(IdeDocumentRefSchema),
  documentGeneration: Schema.NullOr(IdeDocumentGenerationSchema),
  pathRef: nonEmptyText(512),
  label: nonEmptyText(240),
  origin: Schema.Literals(["project", "generated", "remote", "adapter"]),
  availability: Schema.Literals(["available", "loading", "unavailable", "stale"]),
  sourceMapRef: Schema.NullOr(nonEmptyText(192)),
}).annotate({ identifier: "IdeDebugSource" });
export interface IdeDebugSource extends Schema.Schema.Type<typeof IdeDebugSourceSchema> {}

export const IdeDebugLocationSchema = Schema.Struct({
  source: IdeDebugSourceSchema,
  line: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  column: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  endLine: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))),
  endColumn: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))),
}).annotate({ identifier: "IdeDebugLocation" });
export interface IdeDebugLocation extends Schema.Schema.Type<typeof IdeDebugLocationSchema> {}

const breakpointFields = {
  breakpointRef: IdeDebugBreakpointRefSchema,
  enabled: Schema.Boolean,
  condition: Schema.NullOr(boundedText(2_000)),
  hitCondition: Schema.NullOr(boundedText(300)),
  logMessage: Schema.NullOr(boundedText(2_000)),
  verified: Schema.Boolean,
  message: Schema.NullOr(boundedText(1_000)),
};
export const IdeDebugBreakpointSchema = Schema.TaggedUnion({
  Source: {
    ...breakpointFields,
    location: IdeDebugLocationSchema,
    requestedLine: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
    sourceVersion: Schema.NullOr(IdeDocumentGenerationSchema),
  },
  Function: { ...breakpointFields, functionName: nonEmptyText(1_000) },
  Data: {
    ...breakpointFields,
    dataId: nonEmptyText(1_000),
    accessType: Schema.Literals(["read", "write", "read_write"]),
  },
}).annotate({ identifier: "IdeDebugBreakpoint" });
export type IdeDebugBreakpoint = typeof IdeDebugBreakpointSchema.Type;

export const IdeDebugThreadSchema = Schema.Struct({
  threadRef: IdeDebugThreadRefSchema,
  name: nonEmptyText(240),
  state: Schema.Literals(["running", "stopped", "exited", "unavailable"]),
  stopReason: Schema.NullOr(boundedText(500)),
}).annotate({ identifier: "IdeDebugThread" });
export interface IdeDebugThread extends Schema.Schema.Type<typeof IdeDebugThreadSchema> {}
export const IdeDebugFrameSchema = Schema.Struct({
  frameRef: IdeDebugFrameRefSchema,
  threadRef: IdeDebugThreadRefSchema,
  name: nonEmptyText(500),
  location: Schema.NullOr(IdeDebugLocationSchema),
  moduleRef: Schema.NullOr(IdeDebugModuleRefSchema),
  canRestart: Schema.Boolean,
}).annotate({ identifier: "IdeDebugFrame" });
export interface IdeDebugFrame extends Schema.Schema.Type<typeof IdeDebugFrameSchema> {}
export const IdeDebugScopeSchema = Schema.Struct({
  scopeRef: IdeDebugScopeRefSchema,
  frameRef: IdeDebugFrameRefSchema,
  name: nonEmptyText(240),
  expensive: Schema.Boolean,
  variableCount: Schema.NullOr(boundedCount(1_000_000)),
  state: Schema.Literals(["loading", "ready", "unavailable", "truncated", "stale"]),
}).annotate({ identifier: "IdeDebugScope" });
export interface IdeDebugScope extends Schema.Schema.Type<typeof IdeDebugScopeSchema> {}
export const IdeDebugVariableSchema = Schema.Struct({
  variableRef: IdeDebugVariableRefSchema,
  parentRef: Schema.NullOr(IdeDebugVariableRefSchema),
  scopeRef: Schema.NullOr(IdeDebugScopeRefSchema),
  name: boundedText(500),
  value: boundedText(16_384),
  type: Schema.NullOr(boundedText(500)),
  evaluateName: Schema.NullOr(boundedText(1_000)),
  childCount: Schema.NullOr(boundedCount(1_000_000)),
  redacted: Schema.Boolean,
  truncated: Schema.Boolean,
}).annotate({ identifier: "IdeDebugVariable" });
export interface IdeDebugVariable extends Schema.Schema.Type<typeof IdeDebugVariableSchema> {}
export const IdeDebugWatchSchema = Schema.Struct({
  watchRef: IdeDebugWatchRefSchema,
  expression: boundedText(4_096),
  value: boundedText(16_384),
  type: Schema.NullOr(boundedText(500)),
  state: Schema.Literals(["pending", "ready", "failed", "stale"]),
  message: Schema.NullOr(boundedText(1_000)),
  redacted: Schema.Boolean,
  truncated: Schema.Boolean,
}).annotate({ identifier: "IdeDebugWatch" });
export interface IdeDebugWatch extends Schema.Schema.Type<typeof IdeDebugWatchSchema> {}
export const IdeDebugModuleSchema = Schema.Struct({
  moduleRef: IdeDebugModuleRefSchema,
  name: nonEmptyText(500),
  pathRef: Schema.NullOr(nonEmptyText(512)),
  version: Schema.NullOr(nonEmptyText(160)),
  symbolStatus: Schema.Literals(["loaded", "missing", "loading", "unavailable"]),
}).annotate({ identifier: "IdeDebugModule" });
export interface IdeDebugModule extends Schema.Schema.Type<typeof IdeDebugModuleSchema> {}

export const IdeDebugConsoleEntrySchema = Schema.Struct({
  sequence: IdeDebugSequenceSchema,
  category: Schema.Literals(["console", "stdout", "stderr", "telemetry", "important"]),
  text: boundedText(65_536),
  redacted: Schema.Boolean,
  truncated: Schema.Boolean,
  gapBefore: Schema.Boolean,
  observedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeDebugConsoleEntry" });
export interface IdeDebugConsoleEntry extends Schema.Schema.Type<
  typeof IdeDebugConsoleEntrySchema
> {}

export const IdeDebugLifecycleSchema = Schema.TaggedUnion({
  Validated: { validatedAt: IdeTimestampSchema },
  Starting: { startedAt: IdeTimestampSchema },
  Running: { startedAt: IdeTimestampSchema },
  Stopped: {
    reason: nonEmptyText(500),
    stoppedAt: IdeTimestampSchema,
    threadRef: Schema.NullOr(IdeDebugThreadRefSchema),
  },
  Restarting: { requestedAt: IdeTimestampSchema },
  Terminated: { terminatedAt: IdeTimestampSchema, reason: nonEmptyText(500) },
  Disconnected: { disconnectedAt: IdeTimestampSchema, targetTerminated: Schema.Boolean },
  Failed: { failedAt: IdeTimestampSchema, reason: nonEmptyText(500) },
}).annotate({ identifier: "IdeDebugLifecycle" });
export type IdeDebugLifecycle = typeof IdeDebugLifecycleSchema.Type;

export const IdeDebugControlOperationSchema = Schema.Literals([
  "continue",
  "pause",
  "step_in",
  "step_over",
  "step_out",
  "step_back",
  "run_to_cursor",
  "restart_frame",
  "restart_session",
  "disconnect",
  "terminate",
]);
export type IdeDebugControlOperation = typeof IdeDebugControlOperationSchema.Type;
const debugGenerationFenceFields = {
  operationRef: IdeDebugOperationRefSchema,
  sessionRef: IdeDebugSessionRefSchema,
  sessionGeneration: IdeDebugSessionGenerationSchema,
  adapterGeneration: IdeDebugAdapterGenerationSchema,
  targetGeneration: IdeDebugTargetGenerationSchema,
};
export const IdeDebugCommandSchema = Schema.TaggedUnion({
  Discover: { operationRef: IdeDebugOperationRefSchema, actor: IdeRunActorSchema },
  Validate: {
    operationRef: IdeDebugOperationRefSchema,
    configurationRef: IdeDebugConfigurationRefSchema,
    actor: IdeRunActorSchema,
  },
  Start: {
    operationRef: IdeDebugOperationRefSchema,
    configurationRef: IdeDebugConfigurationRefSchema,
    actor: IdeRunActorSchema,
  },
  ReplaceBreakpoints: {
    ...debugGenerationFenceFields,
    breakpoints: Schema.Array(IdeDebugBreakpointSchema).check(Schema.isMaxLength(10_000)),
    actor: IdeRunActorSchema,
  },
  Control: {
    ...debugGenerationFenceFields,
    operation: IdeDebugControlOperationSchema,
    actor: IdeRunActorSchema,
  },
  Evaluate: {
    ...debugGenerationFenceFields,
    expression: boundedText(4_096),
    frameRef: Schema.NullOr(IdeDebugFrameRefSchema),
    actor: IdeRunActorSchema,
  },
  SetVariable: {
    ...debugGenerationFenceFields,
    variableRef: IdeDebugVariableRefSchema,
    value: boundedText(16_384),
    actor: IdeRunActorSchema,
  },
  NavigateSource: {
    ...debugGenerationFenceFields,
    source: IdeDebugSourceSchema,
    actor: IdeRunActorSchema,
  },
  Cancel: {
    operationRef: IdeDebugOperationRefSchema,
    targetOperationRef: IdeDebugOperationRefSchema,
    reason: nonEmptyText(500),
    actor: IdeRunActorSchema,
  },
  DeleteRetainedData: {
    operationRef: IdeDebugOperationRefSchema,
    reason: nonEmptyText(500),
    actor: IdeRunActorSchema,
  },
  Cleanup: {
    operationRef: IdeDebugOperationRefSchema,
    reason: nonEmptyText(500),
    actor: IdeRunActorSchema,
  },
}).annotate({ identifier: "IdeDebugCommand" });
export type IdeDebugCommand = typeof IdeDebugCommandSchema.Type;

export const IdeDebugReceiptSchema = Schema.Struct({
  receiptRef: IdeDebugReceiptRefSchema,
  operationRef: IdeDebugOperationRefSchema,
  configurationRef: IdeDebugConfigurationRefSchema,
  sessionRef: Schema.NullOr(IdeDebugSessionRefSchema),
  sessionGeneration: Schema.NullOr(IdeDebugSessionGenerationSchema),
  actor: IdeRunActorSchema,
  operation: Schema.Literals([
    "validate",
    "launch",
    "attach",
    "breakpoints",
    "control",
    "evaluate",
    "set_variable",
    "source_navigation",
    "restart",
    "disconnect",
    "terminate",
    "cancel",
    "delete_retained_data",
    "cleanup",
  ]),
  disposition: Schema.Literals(["succeeded", "refused", "failed", "canceled"]),
  outcome: nonEmptyText(160),
  targetRef: IdeDebugTargetRefSchema,
  placementRef: IdePlacementRefSchema,
  environmentDigest: nonEmptyText(96),
  configurationDigest: nonEmptyText(96),
  observedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeDebugReceipt" });
export interface IdeDebugReceipt extends Schema.Schema.Type<typeof IdeDebugReceiptSchema> {}

export const IdeDebugBreakpointSetSchema = Schema.Struct({
  configurationRef: IdeDebugConfigurationRefSchema,
  breakpoints: Schema.Array(IdeDebugBreakpointSchema).check(Schema.isMaxLength(10_000)),
  updatedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeDebugBreakpointSet" });
export interface IdeDebugBreakpointSet extends Schema.Schema.Type<
  typeof IdeDebugBreakpointSetSchema
> {}

export const IdeDebugSessionSchema = Schema.Struct({
  sessionRef: IdeDebugSessionRefSchema,
  sessionGeneration: IdeDebugSessionGenerationSchema,
  adapterGeneration: IdeDebugAdapterGenerationSchema,
  targetGeneration: IdeDebugTargetGenerationSchema,
  configuration: IdeDebugConfigurationSchema,
  actor: IdeRunActorSchema,
  lifecycle: IdeDebugLifecycleSchema,
  breakpoints: Schema.Array(IdeDebugBreakpointSchema).check(Schema.isMaxLength(10_000)),
  threads: Schema.Array(IdeDebugThreadSchema).check(Schema.isMaxLength(10_000)),
  frames: Schema.Array(IdeDebugFrameSchema).check(Schema.isMaxLength(20_000)),
  scopes: Schema.Array(IdeDebugScopeSchema).check(Schema.isMaxLength(20_000)),
  variables: Schema.Array(IdeDebugVariableSchema).check(Schema.isMaxLength(50_000)),
  watches: Schema.Array(IdeDebugWatchSchema).check(Schema.isMaxLength(1_000)),
  modules: Schema.Array(IdeDebugModuleSchema).check(Schema.isMaxLength(10_000)),
  loadedSources: Schema.Array(IdeDebugSourceSchema).check(Schema.isMaxLength(20_000)),
  console: Schema.Array(IdeDebugConsoleEntrySchema).check(Schema.isMaxLength(2_048)),
  invalidatedAreas: Schema.Array(
    Schema.Literals([
      "threads",
      "stacks",
      "scopes",
      "variables",
      "watches",
      "modules",
      "sources",
      "console",
    ]),
  ).check(Schema.isMaxLength(32)),
  retainedConsoleBytes: boundedCount(16_777_216),
  droppedConsoleBytes: boundedCount(2_147_483_647),
}).annotate({ identifier: "IdeDebugSession" });
export interface IdeDebugSession extends Schema.Schema.Type<typeof IdeDebugSessionSchema> {}

export const IdeDebugSnapshotSchema = Schema.Struct({
  schemaVersion: IdeDebugSchemaVersion,
  binding: IdeDebugBindingSchema,
  capabilityState: IdeCapabilityStateSchema,
  configurations: Schema.Array(IdeDebugConfigurationSchema).check(Schema.isMaxLength(1_000)),
  breakpointSets: Schema.Array(IdeDebugBreakpointSetSchema).check(Schema.isMaxLength(1_000)),
  sessions: Schema.Array(IdeDebugSessionSchema).check(Schema.isMaxLength(128)),
  receipts: Schema.Array(IdeDebugReceiptSchema).check(Schema.isMaxLength(2_000)),
  stopped: Schema.Boolean,
}).annotate({ identifier: "IdeDebugSnapshot" });
export interface IdeDebugSnapshot extends Schema.Schema.Type<typeof IdeDebugSnapshotSchema> {}

export const IdeDebugPersistenceSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-debug-persistence.v1"),
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  configurations: Schema.Array(IdeDebugConfigurationSchema).check(Schema.isMaxLength(1_000)),
  breakpointSets: Schema.Array(IdeDebugBreakpointSetSchema).check(Schema.isMaxLength(1_000)),
  updatedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeDebugPersistence" });
export interface IdeDebugPersistence extends Schema.Schema.Type<typeof IdeDebugPersistenceSchema> {}

export const IdeDebugAdapterEventSchema = Schema.TaggedUnion({
  Initialized: {
    capabilities: Schema.Array(IdeDebugCapabilitySchema).check(Schema.isMaxLength(64)),
  },
  Stopped: {
    reason: nonEmptyText(500),
    threadRef: Schema.NullOr(IdeDebugThreadRefSchema),
    allThreadsStopped: Schema.Boolean,
  },
  Continued: {
    threadRef: Schema.NullOr(IdeDebugThreadRefSchema),
    allThreadsContinued: Schema.Boolean,
  },
  Projection: {
    threads: Schema.Array(IdeDebugThreadSchema).check(Schema.isMaxLength(10_000)),
    frames: Schema.Array(IdeDebugFrameSchema).check(Schema.isMaxLength(20_000)),
    scopes: Schema.Array(IdeDebugScopeSchema).check(Schema.isMaxLength(20_000)),
    variables: Schema.Array(IdeDebugVariableSchema).check(Schema.isMaxLength(50_000)),
    modules: Schema.Array(IdeDebugModuleSchema).check(Schema.isMaxLength(10_000)),
    loadedSources: Schema.Array(IdeDebugSourceSchema).check(Schema.isMaxLength(20_000)),
  },
  Output: {
    category: Schema.Literals(["console", "stdout", "stderr", "telemetry", "important"]),
    text: boundedText(262_144),
  },
  Invalidated: {
    areas: Schema.Array(
      Schema.Literals([
        "threads",
        "stacks",
        "scopes",
        "variables",
        "watches",
        "modules",
        "sources",
        "console",
      ]),
    ).check(Schema.isMaxLength(32)),
  },
  Terminated: { reason: nonEmptyText(500) },
  AdapterFailed: { reason: nonEmptyText(500) },
  TargetLost: { reason: nonEmptyText(500) },
}).annotate({ identifier: "IdeDebugAdapterEvent" });
export type IdeDebugAdapterEvent = typeof IdeDebugAdapterEventSchema.Type;

export const IdeDebugEventSchema = Schema.TaggedUnion({
  Snapshot: { snapshot: IdeDebugSnapshotSchema },
  StaleEventDropped: {
    sessionRef: IdeDebugSessionRefSchema,
    detail: nonEmptyText(500),
    observedAt: IdeTimestampSchema,
  },
}).annotate({ identifier: "IdeDebugEvent" });
export type IdeDebugEvent = typeof IdeDebugEventSchema.Type;

export class IdeDebugAdmissionFailure extends Schema.TaggedErrorClass<IdeDebugAdmissionFailure>()(
  "IdeDebug.AdmissionFailure",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugConfigurationFailure extends Schema.TaggedErrorClass<IdeDebugConfigurationFailure>()(
  "IdeDebug.ConfigurationFailure",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugCapabilityFailure extends Schema.TaggedErrorClass<IdeDebugCapabilityFailure>()(
  "IdeDebug.CapabilityFailure",
  { operation: Schema.String, capability: IdeDebugCapabilityNameSchema, detail: Schema.String },
) {}
export class IdeDebugStaleEvent extends Schema.TaggedErrorClass<IdeDebugStaleEvent>()(
  "IdeDebug.StaleEvent",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugSessionNotFound extends Schema.TaggedErrorClass<IdeDebugSessionNotFound>()(
  "IdeDebug.SessionNotFound",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugStopped extends Schema.TaggedErrorClass<IdeDebugStopped>()(
  "IdeDebug.Stopped",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugTransportFailure extends Schema.TaggedErrorClass<IdeDebugTransportFailure>()(
  "IdeDebug.TransportFailure",
  { operation: Schema.String, detail: Schema.String, retryable: Schema.Boolean },
) {}
export class IdeDebugProtocolFailure extends Schema.TaggedErrorClass<IdeDebugProtocolFailure>()(
  "IdeDebug.ProtocolFailure",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugSourceMappingFailure extends Schema.TaggedErrorClass<IdeDebugSourceMappingFailure>()(
  "IdeDebug.SourceMappingFailure",
  {
    operation: Schema.String,
    sourceRef: Schema.NullOr(IdeDebugSourceRefSchema),
    detail: Schema.String,
  },
) {}
export class IdeDebugAuthenticationFailure extends Schema.TaggedErrorClass<IdeDebugAuthenticationFailure>()(
  "IdeDebug.AuthenticationFailure",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugTimeoutFailure extends Schema.TaggedErrorClass<IdeDebugTimeoutFailure>()(
  "IdeDebug.TimeoutFailure",
  { operation: Schema.String, timeoutMs: Schema.Number, detail: Schema.String },
) {}
export class IdeDebugCancellation extends Schema.TaggedErrorClass<IdeDebugCancellation>()(
  "IdeDebug.Cancellation",
  { operation: Schema.String, detail: Schema.String },
) {}
export class IdeDebugAdapterFailure extends Schema.TaggedErrorClass<IdeDebugAdapterFailure>()(
  "IdeDebug.AdapterFailure",
  { operation: Schema.String, adapterRef: IdeDebugAdapterRefSchema, detail: Schema.String },
) {}
export class IdeDebugTargetLoss extends Schema.TaggedErrorClass<IdeDebugTargetLoss>()(
  "IdeDebug.TargetLoss",
  { operation: Schema.String, targetRef: IdeDebugTargetRefSchema, detail: Schema.String },
) {}
export class IdeDebugTeardownFailure extends Schema.TaggedErrorClass<IdeDebugTeardownFailure>()(
  "IdeDebug.TeardownFailure",
  { operation: Schema.String, detail: Schema.String, residuePresent: Schema.Boolean },
) {}

export type IdeDebugServiceError =
  | IdeDebugAdmissionFailure
  | IdeDebugConfigurationFailure
  | IdeDebugCapabilityFailure
  | IdeDebugStaleEvent
  | IdeDebugSessionNotFound
  | IdeDebugStopped
  | IdeDebugTransportFailure
  | IdeDebugProtocolFailure
  | IdeDebugSourceMappingFailure
  | IdeDebugAuthenticationFailure
  | IdeDebugTimeoutFailure
  | IdeDebugCancellation
  | IdeDebugAdapterFailure
  | IdeDebugTargetLoss
  | IdeDebugTeardownFailure;

export const IdeDebugCommandResultSchema = Schema.TaggedUnion({
  Succeeded: { snapshot: IdeDebugSnapshotSchema, payload: Schema.NullOr(Schema.Json) },
  Refused: {
    snapshot: Schema.NullOr(IdeDebugSnapshotSchema),
    reason: Schema.Literals([
      "invalid_input",
      "not_admitted",
      "stale_generation",
      "protocol",
      "unavailable",
    ]),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_024)),
  },
}).annotate({ identifier: "IdeDebugCommandResult" });
export type IdeDebugCommandResult = typeof IdeDebugCommandResultSchema.Type;

export const IdeDebugSnapshotChannel = "desktop:ide-debug-snapshot";
export const IdeDebugCommandChannel = "desktop:ide-debug-command";
export const IdeDebugEventChannel = "desktop:ide-debug-event";

const decodeSnapshotExit = Schema.decodeUnknownExit(IdeDebugSnapshotSchema);
const decodeCommandExit = Schema.decodeUnknownExit(IdeDebugCommandSchema);
const decodeEventExit = Schema.decodeUnknownExit(IdeDebugEventSchema);
const decodeCommandResultExit = Schema.decodeUnknownExit(IdeDebugCommandResultSchema);

export const decodeIdeDebugSnapshot = (value: unknown): IdeDebugSnapshot | null => {
  const result = decodeSnapshotExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};
export const decodeIdeDebugCommand = (value: unknown): IdeDebugCommand | null => {
  const result = decodeCommandExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};
export const decodeIdeDebugEvent = (value: unknown): IdeDebugEvent | null => {
  const result = decodeEventExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};
export const decodeIdeDebugCommandResult = (value: unknown): IdeDebugCommandResult | null => {
  const result = decodeCommandResultExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};
