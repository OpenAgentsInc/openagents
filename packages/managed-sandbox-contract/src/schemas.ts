import { Schema as S } from "effect";

export const MANAGED_SANDBOX_SCHEMA_VERSION = "openagents.managed_sandbox.v1" as const;
export const MANAGED_SANDBOX_COMMAND_SCHEMA_VERSION =
  "openagents.managed_sandbox_command.v1" as const;
export const MANAGED_SANDBOX_EVENT_SCHEMA_VERSION = "openagents.managed_sandbox_event.v1" as const;
export const MANAGED_SANDBOX_RECEIPT_SCHEMA_VERSION =
  "openagents.managed_sandbox_receipt.v1" as const;

export const SandboxRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
export type SandboxRef = typeof SandboxRef.Type;

export const SandboxTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);

export const Sha256Digest = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/));

export const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

export const PositiveInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1));

export const SandboxLifecycle = S.Literals([
  "provisioning",
  "ready",
  "idle",
  "running",
  "stopping",
  "stopped",
  "resuming",
  "deleting",
  "deleted",
  "failed",
  "recovery_required",
]);
export type SandboxLifecycle = typeof SandboxLifecycle.Type;

export const SandboxLeaseState = S.Literals([
  "pending",
  "active",
  "expiring",
  "expired",
  "released",
]);

export const SandboxGuestState = S.Literals([
  "absent",
  "starting",
  "present",
  "stopping",
  "unknown",
]);

export const SandboxFilesystemState = S.Literals([
  "unallocated",
  "attached",
  "checkpointing",
  "durable",
  "deleted",
  "unknown",
]);

export const SandboxIngressState = S.Literals([
  "closed",
  "broker_only",
  "owner_tunnel",
  "revoked",
  "unknown",
]);

export const SandboxRuntimeState = S.Literals([
  "none",
  "starting",
  "running",
  "interrupting",
  "settled",
  "failed",
  "unknown",
]);

export const SandboxTargetDescriptorSchema = S.Struct({
  targetRef: SandboxRef,
  targetClass: S.Literal("openagents_managed"),
  provider: S.Literal("google_cloud"),
  adapterRef: SandboxRef,
  region: SandboxRef,
  isolation: S.Literals(["gce_vm", "firecracker_microvm"]),
  dataPosture: S.Literal("openagents_managed_region"),
});
export type SandboxTargetDescriptor = typeof SandboxTargetDescriptorSchema.Type;

export const SandboxLeaseSchema = S.Struct({
  leaseRef: SandboxRef,
  state: SandboxLeaseState,
  issuedAt: SandboxTimestamp,
  expiresAt: SandboxTimestamp,
  ttlSeconds: PositiveInt,
  renewable: S.Boolean,
});
export type SandboxLease = typeof SandboxLeaseSchema.Type;

export const SandboxBudgetSchema = S.Struct({
  currency: S.Literal("USD"),
  maxCostMicros: NonNegativeInt,
  maxCpuMillis: NonNegativeInt,
  maxNetworkBytes: NonNegativeInt,
  maxArtifactBytes: NonNegativeInt,
  maxLifetimeSeconds: PositiveInt,
});
export type SandboxBudget = typeof SandboxBudgetSchema.Type;

export const SandboxCapabilitySchema = S.Struct({
  capabilityRef: SandboxRef,
  kind: S.Literals([
    "agent_turn",
    "command",
    "file_read",
    "file_write",
    "artifact_read",
    "owner_tunnel",
  ]),
  state: S.Literals(["pending", "active", "revoked", "expired"]),
  expiresAt: SandboxTimestamp,
});
export type SandboxCapability = typeof SandboxCapabilitySchema.Type;

export const SandboxStateFactsSchema = S.Struct({
  lifecycle: SandboxLifecycle,
  leaseState: SandboxLeaseState,
  guestState: SandboxGuestState,
  filesystemState: SandboxFilesystemState,
  ingressState: SandboxIngressState,
  runtimeState: SandboxRuntimeState,
  acceptingWork: S.Boolean,
  cleanupComplete: S.Boolean,
});
export type SandboxStateFacts = typeof SandboxStateFactsSchema.Type;

export const ManagedSandboxResourceSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_SCHEMA_VERSION),
  sandboxRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  programRef: S.Literal("program.managed_agent_sandboxes"),
  workUnitRef: SandboxRef,
  attachmentRef: SandboxRef,
  attachmentGeneration: NonNegativeInt,
  resourceGeneration: NonNegativeInt,
  version: NonNegativeInt,
  lastEventSequence: NonNegativeInt,
  target: SandboxTargetDescriptorSchema,
  imageDigest: Sha256Digest,
  profileRef: SandboxRef,
  lease: SandboxLeaseSchema,
  budget: SandboxBudgetSchema,
  capabilities: S.Array(SandboxCapabilitySchema),
  facts: SandboxStateFactsSchema,
  createdAt: SandboxTimestamp,
  updatedAt: SandboxTimestamp,
});
export type ManagedSandboxResource = typeof ManagedSandboxResourceSchema.Type;

const CommandBase = {
  schema: S.Literal(MANAGED_SANDBOX_COMMAND_SCHEMA_VERSION),
  commandRef: SandboxRef,
  requestedByRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  idempotencyRef: SandboxRef,
  requestedAt: SandboxTimestamp,
};

export const ManagedSandboxCommandSchema = S.TaggedUnion({
  Create: {
    ...CommandBase,
    workUnitRef: SandboxRef,
    attachmentRef: SandboxRef,
    target: SandboxTargetDescriptorSchema,
    imageDigest: Sha256Digest,
    profileRef: SandboxRef,
    lease: SandboxLeaseSchema,
    budget: SandboxBudgetSchema,
    requestedCapabilities: S.Array(SandboxCapabilitySchema),
  },
  Inspect: {
    ...CommandBase,
    sandboxRef: SandboxRef,
  },
  Update: {
    ...CommandBase,
    sandboxRef: SandboxRef,
    expectedVersion: NonNegativeInt,
    lease: S.optionalKey(SandboxLeaseSchema),
    budget: S.optionalKey(SandboxBudgetSchema),
  },
  Stop: {
    ...CommandBase,
    sandboxRef: SandboxRef,
    expectedVersion: NonNegativeInt,
    reasonRef: SandboxRef,
  },
  Resume: {
    ...CommandBase,
    sandboxRef: SandboxRef,
    expectedVersion: NonNegativeInt,
  },
  Delete: {
    ...CommandBase,
    sandboxRef: SandboxRef,
    expectedVersion: NonNegativeInt,
    reasonRef: SandboxRef,
  },
  Dispatch: {
    ...CommandBase,
    sandboxRef: SandboxRef,
    expectedVersion: NonNegativeInt,
    turnRef: SandboxRef,
    capabilityRef: SandboxRef,
    promptDigest: Sha256Digest,
  },
  Interrupt: {
    ...CommandBase,
    sandboxRef: SandboxRef,
    expectedVersion: NonNegativeInt,
    turnRef: SandboxRef,
    reasonRef: SandboxRef,
  },
});
export type ManagedSandboxCommand = typeof ManagedSandboxCommandSchema.Type;

const EventBase = {
  schema: S.Literal(MANAGED_SANDBOX_EVENT_SCHEMA_VERSION),
  eventRef: SandboxRef,
  sandboxRef: SandboxRef,
  resourceGeneration: NonNegativeInt,
  sequence: PositiveInt,
  observedAt: SandboxTimestamp,
};

export const ManagedSandboxEventSchema = S.TaggedUnion({
  ProvisionRequested: {
    ...EventBase,
  },
  GuestReady: {
    ...EventBase,
  },
  RuntimeStarted: {
    ...EventBase,
    turnRef: SandboxRef,
  },
  RuntimeSettled: {
    ...EventBase,
    turnRef: SandboxRef,
  },
  RuntimeFailed: {
    ...EventBase,
    turnRef: SandboxRef,
    errorRef: SandboxRef,
  },
  StopRequested: {
    ...EventBase,
  },
  FilesystemCheckpointed: {
    ...EventBase,
    checkpointDigest: Sha256Digest,
  },
  FilesystemCheckpointFailed: {
    ...EventBase,
    errorRef: SandboxRef,
  },
  GuestStopped: {
    ...EventBase,
  },
  ResumeRequested: {
    ...EventBase,
  },
  DeleteRequested: {
    ...EventBase,
  },
  CleanupObserved: {
    ...EventBase,
  },
  OperationFailed: {
    ...EventBase,
    operationRef: SandboxRef,
    errorRef: SandboxRef,
  },
  RecoveryMarked: {
    ...EventBase,
    reasonRef: SandboxRef,
  },
});
export type ManagedSandboxEvent = typeof ManagedSandboxEventSchema.Type;

export const ManagedSandboxReceiptSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_RECEIPT_SCHEMA_VERSION),
  receiptRef: SandboxRef,
  commandRef: SandboxRef,
  sandboxRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  resourceGeneration: NonNegativeInt,
  version: NonNegativeInt,
  outcome: S.Literals(["accepted", "succeeded", "refused", "failed", "replayed"]),
  lifecycle: SandboxLifecycle,
  eventRefs: S.Array(SandboxRef),
  artifactRefs: S.Array(SandboxRef),
  errorCode: S.optionalKey(SandboxRef),
  observedAt: SandboxTimestamp,
});
export type ManagedSandboxReceipt = typeof ManagedSandboxReceiptSchema.Type;

const ErrorBase = {
  message: S.String,
  retryable: S.Boolean,
  evidenceRefs: S.Array(SandboxRef),
};

/** Closed public-safe failure vocabulary; provider defects stay behind refs. */
export const ManagedSandboxErrorSchema = S.TaggedUnion({
  InvalidRequest: { ...ErrorBase, fieldRef: S.optionalKey(SandboxRef) },
  AuthenticationRequired: ErrorBase,
  PermissionDenied: ErrorBase,
  ResourceNotFound: { ...ErrorBase, resourceRef: SandboxRef },
  Conflict: { ...ErrorBase, currentVersion: S.optionalKey(NonNegativeInt) },
  StaleGeneration: {
    ...ErrorBase,
    expectedGeneration: NonNegativeInt,
    receivedGeneration: NonNegativeInt,
  },
  LeaseExpired: { ...ErrorBase, leaseRef: SandboxRef },
  BudgetExceeded: { ...ErrorBase, budgetRef: SandboxRef },
  CapacityUnavailable: { ...ErrorBase, targetRef: SandboxRef },
  TargetUnavailable: { ...ErrorBase, targetRef: SandboxRef },
  CapabilityDenied: { ...ErrorBase, capabilityRef: SandboxRef },
  OperationFailed: { ...ErrorBase, operationRef: SandboxRef },
  RecoveryRequired: { ...ErrorBase, operationRef: SandboxRef },
  CapabilityNotImplemented: { ...ErrorBase, capabilityRef: SandboxRef },
});
export type ManagedSandboxError = typeof ManagedSandboxErrorSchema.Type;

export const BoxProjectionCursorSchema = S.Struct({
  translatorRef: SandboxRef,
  nativeEventSequence: NonNegativeInt,
  boxCursor: S.optionalKey(SandboxRef),
  omittedNativeKinds: S.Array(SandboxRef),
});
export type BoxProjectionCursor = typeof BoxProjectionCursorSchema.Type;
