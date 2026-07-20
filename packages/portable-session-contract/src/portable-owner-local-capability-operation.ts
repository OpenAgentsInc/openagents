import { Schema } from "effect";

export const PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION =
  "openagents.portable_owner_local_capability_operation.v1" as const;

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

export const PortableOwnerLocalCapabilityOperationActionSchema = Schema.Literals([
  "install",
  "wipe",
]).annotate({ identifier: "PortableOwnerLocalCapabilityOperationAction" });
export type PortableOwnerLocalCapabilityOperationAction =
  typeof PortableOwnerLocalCapabilityOperationActionSchema.Type;

export const PortableOwnerLocalCapabilityKindSchema = Schema.Literals([
  "provider",
  "scm_read",
  "scm_write",
  "tool",
  "api",
]).annotate({ identifier: "PortableOwnerLocalCapabilityKind" });
export type PortableOwnerLocalCapabilityKind =
  typeof PortableOwnerLocalCapabilityKindSchema.Type;

export const PortableOwnerLocalCapabilityOperationRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION),
  operationRef: PortableRef,
  action: PortableOwnerLocalCapabilityOperationActionSchema,
  capability: PortableOwnerLocalCapabilityKindSchema,
  commandExecutionClaimRef: PortableRef,
  ownerRef: PortableRef,
  pylonRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  targetRef: PortableRef,
  sourceLeaseRef: PortableRef,
  sourceGrantRef: PortableRef,
  destinationLeaseRef: PortableRef,
  destinationGrantRef: PortableRef,
  installationRef: Schema.NullOr(PortableRef),
  permissionRefs: PublicSafeRefs,
  permissionFingerprint: Sha256Digest,
  expiresAt: PortableTimestamp,
}).annotate({ identifier: "PortableOwnerLocalCapabilityOperationRequest" });
export type PortableOwnerLocalCapabilityOperationRequest =
  typeof PortableOwnerLocalCapabilityOperationRequestSchema.Type;

export const PortableOwnerLocalCapabilityOperationStateSchema = Schema.Literals([
  "pending",
  "claimed",
  "completed",
  "failed",
  "expired",
]).annotate({ identifier: "PortableOwnerLocalCapabilityOperationState" });
export type PortableOwnerLocalCapabilityOperationState =
  typeof PortableOwnerLocalCapabilityOperationStateSchema.Type;

export const PortableOwnerLocalCapabilityOperationClaimRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION),
  operationRef: PortableRef,
  claimRef: PortableRef,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  workerInstanceRef: PortableRef,
  leaseExpiresAt: PortableTimestamp,
}).annotate({ identifier: "PortableOwnerLocalCapabilityOperationClaimRequest" });
export type PortableOwnerLocalCapabilityOperationClaimRequest =
  typeof PortableOwnerLocalCapabilityOperationClaimRequestSchema.Type;

export const PortableOwnerLocalCapabilityOperationRenewRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION),
  claimRef: PortableRef,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  leaseExpiresAt: PortableTimestamp,
}).annotate({ identifier: "PortableOwnerLocalCapabilityOperationRenewRequest" });
export type PortableOwnerLocalCapabilityOperationRenewRequest =
  typeof PortableOwnerLocalCapabilityOperationRenewRequestSchema.Type;

export const PortableOwnerLocalCapabilityOperationResultRequestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION),
  claimRef: PortableRef,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  resultRef: PortableRef,
  resultStatus: Schema.Literals(["completed", "failed"]),
  receiptRef: Schema.NullOr(PortableRef),
  evidenceRefs: PublicSafeRefs,
  errorRef: Schema.NullOr(PortableRef),
  completedAt: PortableTimestamp,
}).annotate({ identifier: "PortableOwnerLocalCapabilityOperationResultRequest" });
export type PortableOwnerLocalCapabilityOperationResultRequest =
  typeof PortableOwnerLocalCapabilityOperationResultRequestSchema.Type;

export const PortableOwnerLocalCapabilityOperationRecordSchema = Schema.Struct({
  request: PortableOwnerLocalCapabilityOperationRequestSchema,
  requestFingerprint: Sha256Digest,
  state: PortableOwnerLocalCapabilityOperationStateSchema,
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
  receiptRef: Schema.NullOr(PortableRef),
  resultEvidenceRefs: PublicSafeRefs,
  errorRef: Schema.NullOr(PortableRef),
  completedAt: Schema.NullOr(PortableTimestamp),
  updatedAt: PortableTimestamp,
}).annotate({ identifier: "PortableOwnerLocalCapabilityOperationRecord" });
export type PortableOwnerLocalCapabilityOperationRecord =
  typeof PortableOwnerLocalCapabilityOperationRecordSchema.Type;
