import { Schema } from "effect";

export const PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION =
  "openagents.portable_command_execution.v1" as const;

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const ExecutionEnvironmentRef = PortableRef;
const PortableTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);
const Sha256Digest = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/));

const PositiveInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));

const PublicSafeRefs = Schema.Array(PortableRef).check(Schema.isMaxLength(256));
const ExecutableCommandKind = Schema.Literals(["attach", "move", "failback"]);

export const PortableCommandExecutionStateSchema = Schema.Literals([
  "claimed",
  "pending_reconcile",
  "terminal",
  "expired",
]).annotate({ identifier: "PortableCommandExecutionState" });
export type PortableCommandExecutionState = typeof PortableCommandExecutionStateSchema.Type;

export const PortableCommandExecutionClaimRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION),
  commandRef: PortableRef,
  claimRef: PortableRef,
  executorEnvironmentRef: ExecutionEnvironmentRef,
  workerInstanceRef: PortableRef,
  leaseExpiresAt: PortableTimestamp,
}).annotate({ identifier: "PortableCommandExecutionClaimRequest" });
export type PortableCommandExecutionClaimRequest =
  typeof PortableCommandExecutionClaimRequestSchema.Type;

export const PortableCommandExecutionClaimSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION),
  claimRef: PortableRef,
  commandRef: PortableRef,
  ownerRef: PortableRef,
  sessionRef: PortableRef,
  commandKind: ExecutableCommandKind,
  commandFingerprint: Sha256Digest,
  claimFingerprint: Sha256Digest,
  sourceAttachmentRef: PortableRef,
  sourceGeneration: PositiveInt,
  destinationTargetRef: ExecutionEnvironmentRef,
  executorEnvironmentRef: ExecutionEnvironmentRef,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  leaseRevision: PositiveInt,
  state: PortableCommandExecutionStateSchema,
  claimedAt: PortableTimestamp,
  leaseExpiresAt: PortableTimestamp,
  updatedAt: PortableTimestamp,
  terminalStatus: Schema.NullOr(Schema.Literals(["completed", "failed", "rejected", "expired"])),
  pendingReconcileRef: Schema.NullOr(PortableRef),
  outcomeRef: Schema.NullOr(PortableRef),
  evidenceRefs: PublicSafeRefs,
}).annotate({ identifier: "PortableCommandExecutionClaim" });
export type PortableCommandExecutionClaim = typeof PortableCommandExecutionClaimSchema.Type;

export const PortableCommandExecutionClaimResultSchema = Schema.Struct({
  status: Schema.Literals(["claimed", "replayed"]),
  claim: PortableCommandExecutionClaimSchema,
}).annotate({ identifier: "PortableCommandExecutionClaimResult" });
export type PortableCommandExecutionClaimResult =
  typeof PortableCommandExecutionClaimResultSchema.Type;

export const PortableCommandExecutionRenewRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION),
  claimRef: PortableRef,
  executorEnvironmentRef: ExecutionEnvironmentRef,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  leaseExpiresAt: PortableTimestamp,
}).annotate({ identifier: "PortableCommandExecutionRenewRequest" });
export type PortableCommandExecutionRenewRequest =
  typeof PortableCommandExecutionRenewRequestSchema.Type;

export const PortableCommandExecutionRenewResultSchema = Schema.Struct({
  status: Schema.Literals(["renewed", "replayed"]),
  claim: PortableCommandExecutionClaimSchema,
}).annotate({ identifier: "PortableCommandExecutionRenewResult" });
export type PortableCommandExecutionRenewResult =
  typeof PortableCommandExecutionRenewResultSchema.Type;

export const PortableCommandExecutionPendingReconcileRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION),
  claimRef: PortableRef,
  executorEnvironmentRef: ExecutionEnvironmentRef,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  pendingReconcileRef: PortableRef,
  evidenceRefs: PublicSafeRefs,
  observedAt: PortableTimestamp,
}).annotate({ identifier: "PortableCommandExecutionPendingReconcileRequest" });
export type PortableCommandExecutionPendingReconcileRequest =
  typeof PortableCommandExecutionPendingReconcileRequestSchema.Type;

export const PortableCommandExecutionPendingReconcileResultSchema = Schema.Struct({
  status: Schema.Literals(["pending_reconcile", "replayed"]),
  claim: PortableCommandExecutionClaimSchema,
}).annotate({ identifier: "PortableCommandExecutionPendingReconcileResult" });
export type PortableCommandExecutionPendingReconcileResult =
  typeof PortableCommandExecutionPendingReconcileResultSchema.Type;

export const PortableCommandExecutionTerminalRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION),
  claimRef: PortableRef,
  executorEnvironmentRef: ExecutionEnvironmentRef,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  terminalStatus: Schema.Literals(["completed", "failed", "rejected"]),
  outcomeRef: PortableRef,
  evidenceRefs: PublicSafeRefs,
  completedAt: PortableTimestamp,
}).annotate({ identifier: "PortableCommandExecutionTerminalRequest" });
export type PortableCommandExecutionTerminalRequest =
  typeof PortableCommandExecutionTerminalRequestSchema.Type;

export const PortableCommandExecutionTerminalResultSchema = Schema.Struct({
  status: Schema.Literals(["terminal", "replayed"]),
  claim: PortableCommandExecutionClaimSchema,
}).annotate({ identifier: "PortableCommandExecutionTerminalResult" });
export type PortableCommandExecutionTerminalResult =
  typeof PortableCommandExecutionTerminalResultSchema.Type;
