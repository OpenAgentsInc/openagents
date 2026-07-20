import { Schema } from "effect";

export const PORTABLE_PHASE_OPERATION_SCHEMA_VERSION =
  "openagents.portable_phase_operation.v1" as const;

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const PortableTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);
const Sha256Digest = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/));
const PositiveInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));
const PublicSafeRefs = Schema.Array(PortableRef).check(Schema.isMaxLength(256));

export const PortablePhaseOperationKindSchema = Schema.Literals([
  "quiesce",
  "checkpoint-create",
  "source-cleanup",
  "checkpoint-stage",
  "destination-activate",
  "staged-abort",
]).annotate({ identifier: "PortablePhaseOperationKind" });
export type PortablePhaseOperationKind = typeof PortablePhaseOperationKindSchema.Type;

export const PortablePhaseOperationRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_PHASE_OPERATION_SCHEMA_VERSION),
  operationRef: PortableRef,
  commandRef: PortableRef,
  commandExecutionClaimRef: PortableRef,
  ownerRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  targetRef: PortableRef,
  pylonRef: PortableRef,
  kind: PortablePhaseOperationKindSchema,
  checkpointRef: Schema.NullOr(PortableRef),
  checkpointObjectRef: Schema.NullOr(PortableRef),
  checkpointDigest: Schema.NullOr(Sha256Digest),
  evidenceRefs: PublicSafeRefs,
  expiresAt: PortableTimestamp,
}).annotate({ identifier: "PortablePhaseOperationRequest" });
export type PortablePhaseOperationRequest = typeof PortablePhaseOperationRequestSchema.Type;

export const PortablePhaseOperationStateSchema = Schema.Literals([
  "pending",
  "claimed",
  "completed",
  "failed",
  "expired",
]).annotate({ identifier: "PortablePhaseOperationState" });
export type PortablePhaseOperationState = typeof PortablePhaseOperationStateSchema.Type;

export const PortablePhaseOperationClaimRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_PHASE_OPERATION_SCHEMA_VERSION),
  operationRef: PortableRef,
  claimRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  workerInstanceRef: PortableRef,
  leaseExpiresAt: PortableTimestamp,
}).annotate({ identifier: "PortablePhaseOperationClaimRequest" });
export type PortablePhaseOperationClaimRequest =
  typeof PortablePhaseOperationClaimRequestSchema.Type;

export const PortablePhaseOperationRecordSchema = Schema.Struct({
  request: PortablePhaseOperationRequestSchema,
  requestFingerprint: Sha256Digest,
  state: PortablePhaseOperationStateSchema,
  claimRef: Schema.NullOr(PortableRef),
  claimFingerprint: Schema.NullOr(Sha256Digest),
  workerInstanceRef: Schema.NullOr(PortableRef),
  claimGeneration: Schema.NullOr(PositiveInt),
  leaseRevision: Schema.NullOr(PositiveInt),
  claimedAt: Schema.NullOr(PortableTimestamp),
  leaseExpiresAt: Schema.NullOr(PortableTimestamp),
  resultRef: Schema.NullOr(PortableRef),
  resultFingerprint: Schema.NullOr(Sha256Digest),
  resultStatus: Schema.NullOr(Schema.Literals(["completed", "failed", "expired"])),
  resultCheckpointRef: Schema.NullOr(PortableRef),
  resultCheckpointObjectRef: Schema.NullOr(PortableRef),
  resultCheckpointDigest: Schema.NullOr(Sha256Digest),
  resultEvidenceRefs: PublicSafeRefs,
  errorRef: Schema.NullOr(PortableRef),
  completedAt: Schema.NullOr(PortableTimestamp),
  updatedAt: PortableTimestamp,
}).annotate({ identifier: "PortablePhaseOperationRecord" });
export type PortablePhaseOperationRecord = typeof PortablePhaseOperationRecordSchema.Type;

export const PortablePhaseOperationRenewRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_PHASE_OPERATION_SCHEMA_VERSION),
  claimRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  leaseExpiresAt: PortableTimestamp,
}).annotate({ identifier: "PortablePhaseOperationRenewRequest" });
export type PortablePhaseOperationRenewRequest =
  typeof PortablePhaseOperationRenewRequestSchema.Type;

export const PortablePhaseOperationResultRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_PHASE_OPERATION_SCHEMA_VERSION),
  claimRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  resultRef: PortableRef,
  resultStatus: Schema.Literals(["completed", "failed"]),
  checkpointRef: Schema.NullOr(PortableRef),
  checkpointObjectRef: Schema.NullOr(PortableRef),
  checkpointDigest: Schema.NullOr(Sha256Digest),
  evidenceRefs: PublicSafeRefs,
  errorRef: Schema.NullOr(PortableRef),
  completedAt: PortableTimestamp,
}).annotate({ identifier: "PortablePhaseOperationResultRequest" });
export type PortablePhaseOperationResultRequest =
  typeof PortablePhaseOperationResultRequestSchema.Type;
