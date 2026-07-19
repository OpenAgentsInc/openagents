import { Exit, Schema } from "@effect-native/core/effect"

import {
  IdeAttachmentGenerationSchema,
  IdeEvidenceRefSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"

const boundedText = (maximum: number) =>
  Schema.String.check(Schema.isMaxLength(maximum))

const nonEmptyText = (maximum: number) =>
  Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(maximum))

const boundedRef = <const Identifier extends string>(identifier: Identifier, prefix: string) =>
  Schema.String.pipe(
    Schema.check(
      Schema.isMinLength(prefix.length + 1),
      Schema.isMaxLength(192),
      Schema.isPattern(new RegExp(`^${prefix.replaceAll(".", "\\.")}[A-Za-z0-9][A-Za-z0-9._-]*$`, "u")),
    ),
    Schema.brand(identifier),
  ).annotate({ identifier })

const positiveGeneration = <const Identifier extends string>(identifier: Identifier) =>
  Schema.Number.pipe(
    Schema.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
    Schema.brand(identifier),
  ).annotate({ identifier })

export const IdeRunSchemaVersion = Schema.Literal("openagents.desktop.ide-run.v1")

export const IdeTerminalSessionRefSchema = boundedRef("IdeTerminalSessionRef", "ide.terminal.")
export type IdeTerminalSessionRef = typeof IdeTerminalSessionRefSchema.Type

export const IdeTerminalProfileRefSchema = boundedRef("IdeTerminalProfileRef", "ide.terminal-profile.")
export type IdeTerminalProfileRef = typeof IdeTerminalProfileRefSchema.Type

export const IdeTerminalSplitRefSchema = boundedRef("IdeTerminalSplitRef", "ide.terminal-split.")
export type IdeTerminalSplitRef = typeof IdeTerminalSplitRefSchema.Type

export const IdeEnvironmentManifestRefSchema = boundedRef("IdeEnvironmentManifestRef", "ide.environment.")
export type IdeEnvironmentManifestRef = typeof IdeEnvironmentManifestRefSchema.Type

export const IdeExecutableRefSchema = boundedRef("IdeExecutableRef", "ide.executable.")
export type IdeExecutableRef = typeof IdeExecutableRefSchema.Type

export const IdeTaskDefinitionRefSchema = boundedRef("IdeTaskDefinitionRef", "ide.task-definition.")
export type IdeTaskDefinitionRef = typeof IdeTaskDefinitionRefSchema.Type

export const IdeTaskRunRefSchema = boundedRef("IdeTaskRunRef", "ide.task-run.")
export type IdeTaskRunRef = typeof IdeTaskRunRefSchema.Type

export const IdeTestControllerRefSchema = boundedRef("IdeTestControllerRef", "ide.test-controller.")
export type IdeTestControllerRef = typeof IdeTestControllerRefSchema.Type

export const IdeTestItemRefSchema = boundedRef("IdeTestItemRef", "ide.test-item.")
export type IdeTestItemRef = typeof IdeTestItemRefSchema.Type

export const IdeTestRunRefSchema = boundedRef("IdeTestRunRef", "ide.test-run.")
export type IdeTestRunRef = typeof IdeTestRunRefSchema.Type

export const IdeOutputChannelRefSchema = boundedRef("IdeOutputChannelRef", "ide.output-channel.")
export type IdeOutputChannelRef = typeof IdeOutputChannelRefSchema.Type

export const IdeArtifactRefSchema = boundedRef("IdeArtifactRef", "ide.artifact.")
export type IdeArtifactRef = typeof IdeArtifactRefSchema.Type

export const IdeRunReceiptRefSchema = boundedRef("IdeRunReceiptRef", "ide.run-receipt.")
export type IdeRunReceiptRef = typeof IdeRunReceiptRefSchema.Type

export const IdeEnvironmentGenerationSchema = positiveGeneration("IdeEnvironmentGeneration")
export type IdeEnvironmentGeneration = typeof IdeEnvironmentGenerationSchema.Type

export const IdeTerminalReconnectGenerationSchema = positiveGeneration("IdeTerminalReconnectGeneration")
export type IdeTerminalReconnectGeneration = typeof IdeTerminalReconnectGenerationSchema.Type

export const IdeTaskDiscoveryGenerationSchema = positiveGeneration("IdeTaskDiscoveryGeneration")
export type IdeTaskDiscoveryGeneration = typeof IdeTaskDiscoveryGenerationSchema.Type

export const IdeTestDiscoveryGenerationSchema = positiveGeneration("IdeTestDiscoveryGeneration")
export type IdeTestDiscoveryGeneration = typeof IdeTestDiscoveryGenerationSchema.Type

export const IdeOutputSequenceSchema = Schema.Number.pipe(
  Schema.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  Schema.brand("IdeOutputSequence"),
).annotate({ identifier: "IdeOutputSequence" })
export type IdeOutputSequence = typeof IdeOutputSequenceSchema.Type

export const IdeRunBindingSchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  placementGeneration: IdePlacementGenerationSchema,
  placementRef: IdePlacementRefSchema,
  cwdRef: nonEmptyText(512),
  cwdLabel: nonEmptyText(160),
}).annotate({ identifier: "IdeRunBinding" })
export interface IdeRunBinding extends Schema.Schema.Type<typeof IdeRunBindingSchema> {}

export const IdeEnvironmentSourceSchema = Schema.TaggedUnion({
  HostSafe: {
    precedence: Schema.Literal(10),
    keys: Schema.Array(nonEmptyText(96)),
  },
  Profile: {
    precedence: Schema.Literal(20),
    profileRef: IdeTerminalProfileRefSchema,
    keys: Schema.Array(nonEmptyText(96)),
  },
  Project: {
    precedence: Schema.Literal(30),
    sourceRef: nonEmptyText(192),
    keys: Schema.Array(nonEmptyText(96)),
  },
  TaskInput: {
    precedence: Schema.Literal(40),
    inputRef: nonEmptyText(192),
    keys: Schema.Array(nonEmptyText(96)),
  },
}).annotate({ identifier: "IdeEnvironmentSource" })
export type IdeEnvironmentSource = typeof IdeEnvironmentSourceSchema.Type

export const IdeEnvironmentManifestSchema = Schema.Struct({
  manifestRef: IdeEnvironmentManifestRefSchema,
  generation: IdeEnvironmentGenerationSchema,
  sources: Schema.Array(IdeEnvironmentSourceSchema),
  admittedKeys: Schema.Array(nonEmptyText(96)),
  redactedKeys: Schema.Array(nonEmptyText(96)),
  inheritedAllHostVariables: Schema.Literal(false),
  valuesExposedToRenderer: Schema.Literal(false),
  digest: nonEmptyText(96),
}).annotate({ identifier: "IdeEnvironmentManifest" })
export interface IdeEnvironmentManifest extends Schema.Schema.Type<typeof IdeEnvironmentManifestSchema> {}

export const IdeExecutableAdmissionSchema = Schema.Struct({
  executableRef: IdeExecutableRefSchema,
  executable: nonEmptyText(512),
  argv: Schema.Array(boundedText(4_096)).check(Schema.isMaxLength(128)),
  displayLabel: nonEmptyText(240),
  source: Schema.Literals(["profile", "task_definition", "test_controller"]),
  shellInterpolation: Schema.Literal(false),
  admitted: Schema.Boolean,
  refusalReason: Schema.NullOr(nonEmptyText(400)),
}).annotate({ identifier: "IdeExecutableAdmission" })
export interface IdeExecutableAdmission extends Schema.Schema.Type<typeof IdeExecutableAdmissionSchema> {}

export const IdeTerminalProfileSchema = Schema.Struct({
  profileRef: IdeTerminalProfileRefSchema,
  label: nonEmptyText(120),
  shellLabel: nonEmptyText(120),
  executable: IdeExecutableAdmissionSchema,
  environmentKeys: Schema.Array(nonEmptyText(96)),
  isDefault: Schema.Boolean,
}).annotate({ identifier: "IdeTerminalProfile" })
export interface IdeTerminalProfile extends Schema.Schema.Type<typeof IdeTerminalProfileSchema> {}

export const IdeOutputProducerSchema = Schema.TaggedUnion({
  Terminal: { sessionRef: IdeTerminalSessionRefSchema },
  Task: { runRef: IdeTaskRunRefSchema },
  Test: { runRef: IdeTestRunRefSchema },
  System: { producerRef: nonEmptyText(192) },
}).annotate({ identifier: "IdeOutputProducer" })
export type IdeOutputProducer = typeof IdeOutputProducerSchema.Type

export const IdeOutputLocationSchema = Schema.Struct({
  pathRef: nonEmptyText(512),
  line: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  column: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  label: nonEmptyText(240),
}).annotate({ identifier: "IdeOutputLocation" })
export interface IdeOutputLocation extends Schema.Schema.Type<typeof IdeOutputLocationSchema> {}

export const IdeOutputChunkSchema = Schema.Struct({
  channelRef: IdeOutputChannelRefSchema,
  producer: IdeOutputProducerSchema,
  sequence: IdeOutputSequenceSchema,
  stream: Schema.Literals(["pty", "stdout", "stderr", "system"]),
  text: boundedText(65_536),
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  redacted: Schema.Boolean,
  truncated: Schema.Boolean,
  gapBefore: Schema.Boolean,
  invalidEncoding: Schema.Boolean,
  locations: Schema.Array(IdeOutputLocationSchema).check(Schema.isMaxLength(64)),
  observedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeOutputChunk" })
export interface IdeOutputChunk extends Schema.Schema.Type<typeof IdeOutputChunkSchema> {}

export const IdeOutputChannelSchema = Schema.Struct({
  channelRef: IdeOutputChannelRefSchema,
  label: nonEmptyText(160),
  producer: IdeOutputProducerSchema,
  firstSequence: Schema.NullOr(IdeOutputSequenceSchema),
  lastSequence: Schema.NullOr(IdeOutputSequenceSchema),
  chunks: Schema.Array(IdeOutputChunkSchema).check(Schema.isMaxLength(2_048)),
  retainedBytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  retentionByteLimit: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  droppedBytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  gap: Schema.Boolean,
  redactionCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  disposed: Schema.Boolean,
}).annotate({ identifier: "IdeOutputChannel" })
export interface IdeOutputChannel extends Schema.Schema.Type<typeof IdeOutputChannelSchema> {}

export const IdeTerminalLifecycleSchema = Schema.TaggedUnion({
  Starting: { startedAt: IdeTimestampSchema },
  Running: { startedAt: IdeTimestampSchema, pidPresent: Schema.Boolean },
  Exited: {
    startedAt: IdeTimestampSchema,
    exitedAt: IdeTimestampSchema,
    exitCode: Schema.NullOr(Schema.Number),
    signal: Schema.NullOr(nonEmptyText(80)),
  },
  Recovered: { recoveredAt: IdeTimestampSchema, processStateLost: Schema.Literal(true) },
  Closing: { requestedAt: IdeTimestampSchema, graceMs: Schema.Number },
  Closed: { closedAt: IdeTimestampSchema, reason: Schema.Literals(["user", "project_stale", "app_quit", "spawn_failed"]) },
}).annotate({ identifier: "IdeTerminalLifecycle" })
export type IdeTerminalLifecycle = typeof IdeTerminalLifecycleSchema.Type

export const IdeTerminalSessionSchema = Schema.Struct({
  sessionRef: IdeTerminalSessionRefSchema,
  title: nonEmptyText(160),
  profileRef: IdeTerminalProfileRefSchema,
  splitRef: IdeTerminalSplitRefSchema,
  binding: IdeRunBindingSchema,
  environment: IdeEnvironmentManifestSchema,
  executable: IdeExecutableAdmissionSchema,
  outputChannelRef: IdeOutputChannelRefSchema,
  cols: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 1_000 })),
  rows: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 1_000 })),
  reconnectGeneration: IdeTerminalReconnectGenerationSchema,
  shellIntegration: Schema.Array(Schema.Literals(["command_boundaries", "cwd", "exit_status", "links"])),
  lifecycle: IdeTerminalLifecycleSchema,
}).annotate({ identifier: "IdeTerminalSession" })
export interface IdeTerminalSession extends Schema.Schema.Type<typeof IdeTerminalSessionSchema> {}

export const IdeProblemMatcherSchema = Schema.Struct({
  matcherRef: nonEmptyText(192),
  kind: Schema.Literals(["typescript", "rustc", "python", "generic_location"]),
  severity: Schema.Literals(["error", "warning", "info"]),
}).annotate({ identifier: "IdeProblemMatcher" })
export interface IdeProblemMatcher extends Schema.Schema.Type<typeof IdeProblemMatcherSchema> {}

export const IdeTaskDefinitionSchema = Schema.Struct({
  definitionRef: IdeTaskDefinitionRefSchema,
  discoveryGeneration: IdeTaskDiscoveryGenerationSchema,
  version: Schema.Literal(1),
  label: nonEmptyText(160),
  group: Schema.Literals(["build", "test", "lint", "run", "other"]),
  dependencies: Schema.Array(IdeTaskDefinitionRefSchema).check(Schema.isMaxLength(32)),
  binding: IdeRunBindingSchema,
  executable: IdeExecutableAdmissionSchema,
  environment: IdeEnvironmentManifestSchema,
  problemMatchers: Schema.Array(IdeProblemMatcherSchema).check(Schema.isMaxLength(16)),
  background: Schema.Struct({
    enabled: Schema.Boolean,
    readinessPattern: Schema.NullOr(nonEmptyText(240)),
  }),
  timeoutMs: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1_000, maximum: 86_400_000 })),
  maxRetries: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum: 5 })),
  artifactPatterns: Schema.Array(nonEmptyText(512)).check(Schema.isMaxLength(32)),
  exactRerunLabel: nonEmptyText(512),
}).annotate({ identifier: "IdeTaskDefinition" })
export interface IdeTaskDefinition extends Schema.Schema.Type<typeof IdeTaskDefinitionSchema> {}

export const IdeRunActorSchema = Schema.TaggedUnion({
  Human: { actorRef: nonEmptyText(192) },
  Agent: { actorRef: nonEmptyText(192), turnRef: nonEmptyText(192) },
}).annotate({ identifier: "IdeRunActor" })
export type IdeRunActor = typeof IdeRunActorSchema.Type

export const IdeArtifactSchema = Schema.Struct({
  artifactRef: IdeArtifactRefSchema,
  label: nonEmptyText(240),
  pathRef: nonEmptyText(512),
  mediaType: nonEmptyText(160),
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  digest: nonEmptyText(96),
  available: Schema.Boolean,
}).annotate({ identifier: "IdeArtifact" })
export interface IdeArtifact extends Schema.Schema.Type<typeof IdeArtifactSchema> {}

export const IdeTaskOutcomeSchema = Schema.TaggedUnion({
  Running: { startedAt: IdeTimestampSchema, attempt: Schema.Number },
  Ready: { startedAt: IdeTimestampSchema, readyAt: IdeTimestampSchema, attempt: Schema.Number },
  Succeeded: { completedAt: IdeTimestampSchema, exitCode: Schema.Literal(0), semanticChecksPassed: Schema.Literal(true) },
  Failed: { completedAt: IdeTimestampSchema, exitCode: Schema.NullOr(Schema.Number), reason: nonEmptyText(400) },
  Cancelled: { completedAt: IdeTimestampSchema, actor: IdeRunActorSchema },
  TimedOut: { completedAt: IdeTimestampSchema, timeoutMs: Schema.Number },
  Refused: { completedAt: IdeTimestampSchema, reason: nonEmptyText(400) },
}).annotate({ identifier: "IdeTaskOutcome" })
export type IdeTaskOutcome = typeof IdeTaskOutcomeSchema.Type

export const IdeTaskRunSchema = Schema.Struct({
  runRef: IdeTaskRunRefSchema,
  definitionRef: IdeTaskDefinitionRefSchema,
  actor: IdeRunActorSchema,
  binding: IdeRunBindingSchema,
  outputChannelRef: IdeOutputChannelRefSchema,
  outcome: IdeTaskOutcomeSchema,
  problems: Schema.Array(IdeOutputLocationSchema).check(Schema.isMaxLength(1_000)),
  artifacts: Schema.Array(IdeArtifactSchema).check(Schema.isMaxLength(128)),
  evidenceRefs: Schema.Array(IdeEvidenceRefSchema).check(Schema.isMaxLength(128)),
}).annotate({ identifier: "IdeTaskRun" })
export interface IdeTaskRun extends Schema.Schema.Type<typeof IdeTaskRunSchema> {}

export const IdeTestItemSchema = Schema.Struct({
  itemRef: IdeTestItemRefSchema,
  controllerRef: IdeTestControllerRefSchema,
  discoveryGeneration: IdeTestDiscoveryGenerationSchema,
  parentRef: Schema.NullOr(IdeTestItemRefSchema),
  label: nonEmptyText(240),
  kind: Schema.Literals(["root", "file", "suite", "test"]),
  location: Schema.NullOr(IdeOutputLocationSchema),
  runnable: Schema.Boolean,
  debugSupported: Schema.Boolean,
}).annotate({ identifier: "IdeTestItem" })
export interface IdeTestItem extends Schema.Schema.Type<typeof IdeTestItemSchema> {}

export const IdeTestControllerSchema = Schema.Struct({
  controllerRef: IdeTestControllerRefSchema,
  label: nonEmptyText(160),
  discoveryGeneration: IdeTestDiscoveryGenerationSchema,
  binding: IdeRunBindingSchema,
  executable: IdeExecutableAdmissionSchema,
  environment: IdeEnvironmentManifestSchema,
  items: Schema.Array(IdeTestItemSchema).check(Schema.isMaxLength(10_000)),
  profiles: Schema.Array(Schema.Literals(["run", "debug", "coverage"])),
  discoveryComplete: Schema.Boolean,
  discoveryError: Schema.NullOr(nonEmptyText(400)),
}).annotate({ identifier: "IdeTestController" })
export interface IdeTestController extends Schema.Schema.Type<typeof IdeTestControllerSchema> {}

export const IdeTestItemResultSchema = Schema.Struct({
  itemRef: IdeTestItemRefSchema,
  status: Schema.Literals(["queued", "running", "passed", "failed", "skipped", "cancelled"]),
  durationMs: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
  message: Schema.NullOr(nonEmptyText(2_000)),
  location: Schema.NullOr(IdeOutputLocationSchema),
}).annotate({ identifier: "IdeTestItemResult" })
export interface IdeTestItemResult extends Schema.Schema.Type<typeof IdeTestItemResultSchema> {}

export const IdeTestOutcomeSchema = Schema.TaggedUnion({
  Running: { startedAt: IdeTimestampSchema, profile: Schema.Literals(["run", "debug", "coverage"]) },
  Succeeded: { completedAt: IdeTimestampSchema, exitCode: Schema.Literal(0), assertionsObserved: Schema.Literal(true) },
  Failed: { completedAt: IdeTimestampSchema, exitCode: Schema.NullOr(Schema.Number), reason: nonEmptyText(400) },
  Cancelled: { completedAt: IdeTimestampSchema, actor: IdeRunActorSchema },
  Refused: { completedAt: IdeTimestampSchema, reason: nonEmptyText(400) },
}).annotate({ identifier: "IdeTestOutcome" })
export type IdeTestOutcome = typeof IdeTestOutcomeSchema.Type

export const IdeTestRunSchema = Schema.Struct({
  runRef: IdeTestRunRefSchema,
  controllerRef: IdeTestControllerRefSchema,
  requestedItemRefs: Schema.Array(IdeTestItemRefSchema).check(Schema.isMaxLength(1_000)),
  discoveryGeneration: IdeTestDiscoveryGenerationSchema,
  actor: IdeRunActorSchema,
  binding: IdeRunBindingSchema,
  outputChannelRef: IdeOutputChannelRefSchema,
  outcome: IdeTestOutcomeSchema,
  results: Schema.Array(IdeTestItemResultSchema).check(Schema.isMaxLength(10_000)),
  artifacts: Schema.Array(IdeArtifactSchema).check(Schema.isMaxLength(128)),
  coveragePercent: Schema.NullOr(Schema.Number.check(Schema.isBetween({ minimum: 0, maximum: 100 }))),
  retryOf: Schema.NullOr(IdeTestRunRefSchema),
  evidenceRefs: Schema.Array(IdeEvidenceRefSchema).check(Schema.isMaxLength(128)),
}).annotate({ identifier: "IdeTestRun" })
export interface IdeTestRun extends Schema.Schema.Type<typeof IdeTestRunSchema> {}

export const IdeRunReceiptSchema = Schema.Struct({
  receiptRef: IdeRunReceiptRefSchema,
  actor: IdeRunActorSchema,
  operation: Schema.Literals(["terminal_create", "terminal_close", "task_run", "task_cancel", "test_run", "test_cancel", "output_export", "dispose"]),
  subjectRef: nonEmptyText(192),
  binding: IdeRunBindingSchema,
  environmentManifestRef: IdeEnvironmentManifestRefSchema,
  outputChannelRef: IdeOutputChannelRefSchema,
  outcome: nonEmptyText(160),
  publicSafe: Schema.Literal(true),
  secretsIncluded: Schema.Literal(false),
  recordedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeRunReceipt" })
export interface IdeRunReceipt extends Schema.Schema.Type<typeof IdeRunReceiptSchema> {}

export const IdeRunSnapshotSchema = Schema.Struct({
  schemaVersion: IdeRunSchemaVersion,
  binding: IdeRunBindingSchema,
  taskDiscoveryGeneration: IdeTaskDiscoveryGenerationSchema,
  testDiscoveryGeneration: IdeTestDiscoveryGenerationSchema,
  profiles: Schema.Array(IdeTerminalProfileSchema),
  terminals: Schema.Array(IdeTerminalSessionSchema),
  taskDefinitions: Schema.Array(IdeTaskDefinitionSchema),
  taskRuns: Schema.Array(IdeTaskRunSchema),
  testControllers: Schema.Array(IdeTestControllerSchema),
  testRuns: Schema.Array(IdeTestRunSchema),
  outputChannels: Schema.Array(IdeOutputChannelSchema),
  receipts: Schema.Array(IdeRunReceiptSchema),
  stopped: Schema.Boolean,
}).annotate({ identifier: "IdeRunSnapshot" })
export interface IdeRunSnapshot extends Schema.Schema.Type<typeof IdeRunSnapshotSchema> {}

export const IdeRunCommandSchema = Schema.TaggedUnion({
  Refresh: {},
  Discover: {},
  RenameTerminal: { sessionRef: IdeTerminalSessionRefSchema, title: nonEmptyText(160) },
  SplitTerminal: { sessionRef: IdeTerminalSessionRefSchema, direction: Schema.Literals(["horizontal", "vertical"]) },
  StartTask: { definitionRef: IdeTaskDefinitionRefSchema, actor: IdeRunActorSchema },
  CancelTask: { runRef: IdeTaskRunRefSchema, actor: IdeRunActorSchema },
  RunTests: {
    controllerRef: IdeTestControllerRefSchema,
    itemRefs: Schema.Array(IdeTestItemRefSchema).check(Schema.isMaxLength(1_000)),
    profile: Schema.Literals(["run", "coverage"]),
    actor: IdeRunActorSchema,
    retryOf: Schema.NullOr(IdeTestRunRefSchema),
  },
  CancelTests: { runRef: IdeTestRunRefSchema, actor: IdeRunActorSchema },
  ClearOutput: { channelRef: IdeOutputChannelRefSchema },
  ExportOutput: { channelRef: IdeOutputChannelRefSchema, actor: IdeRunActorSchema },
  Stop: { reason: nonEmptyText(400) },
}).annotate({ identifier: "IdeRunCommand" })
export type IdeRunCommand = typeof IdeRunCommandSchema.Type

export const IdeRunEventSchema = Schema.TaggedUnion({
  Snapshot: { snapshot: IdeRunSnapshotSchema },
  Output: { chunk: IdeOutputChunkSchema },
}).annotate({ identifier: "IdeRunEvent" })
export type IdeRunEvent = typeof IdeRunEventSchema.Type

export const IdeRunCommandResultSchema = Schema.TaggedUnion({
  Succeeded: { snapshot: IdeRunSnapshotSchema },
  Refused: {
    reason: Schema.Literals(["invalid_input", "stale_generation", "not_found", "not_admitted", "already_running", "stopped", "unavailable"]),
    message: nonEmptyText(2_000),
    snapshot: IdeRunSnapshotSchema,
  },
}).annotate({ identifier: "IdeRunCommandResult" })
export type IdeRunCommandResult = typeof IdeRunCommandResultSchema.Type

export const IdeRunSnapshotChannel = "openagents-desktop/ide-run-snapshot" as const
export const IdeRunCommandChannel = "openagents-desktop/ide-run-command" as const
export const IdeRunEventChannel = "openagents-desktop/ide-run-event" as const

export const decodeIdeRunCommand = (value: unknown): IdeRunCommand | null => {
  const decoded = Schema.decodeUnknownExit(IdeRunCommandSchema, { onExcessProperty: "error" })(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const decodeIdeRunSnapshot = (value: unknown): IdeRunSnapshot | null => {
  const decoded = Schema.decodeUnknownExit(IdeRunSnapshotSchema, { onExcessProperty: "error" })(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const decodeIdeRunEvent = (value: unknown): IdeRunEvent | null => {
  const decoded = Schema.decodeUnknownExit(IdeRunEventSchema, { onExcessProperty: "error" })(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const decodeIdeRunCommandResult = (value: unknown): IdeRunCommandResult | null => {
  const decoded = Schema.decodeUnknownExit(IdeRunCommandResultSchema, { onExcessProperty: "error" })(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}
