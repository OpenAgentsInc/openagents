import { Schema as S } from "effect";

import {
  NonNegativeInt,
  PositiveInt,
  SandboxRef,
  SandboxTimestamp,
  Sha256Digest,
} from "./schemas.ts";

export const MANAGED_SANDBOX_GUEST_IO_SCHEMA_VERSION =
  "openagents.managed_sandbox_guest_io.v1" as const;
export const MANAGED_SANDBOX_GUEST_IO_RECEIPT_SCHEMA_VERSION =
  "openagents.managed_sandbox_guest_io_receipt.v1" as const;
export const MANAGED_SANDBOX_ARTIFACT_RECEIPT_SCHEMA_VERSION =
  "openagents.managed_sandbox_artifact_receipt.v1" as const;

export const ManagedSandboxGuestIoActionSchema = S.Literals([
  "read_file",
  "write_file",
  "execute_command",
  "read_artifact",
]);
export type ManagedSandboxGuestIoAction = typeof ManagedSandboxGuestIoActionSchema.Type;

export const ManagedSandboxGuestIoLimitsSchema = S.Struct({
  workspaceRootRef: SandboxRef,
  maxFileBytes: PositiveInt,
  maxArtifactBytes: PositiveInt,
  maxOutputBytes: PositiveInt,
  maxDurationMillis: PositiveInt,
  maxCpuMillis: PositiveInt,
  maxProcesses: PositiveInt,
  maxNetworkBytes: NonNegativeInt,
  networkPolicyRef: SandboxRef,
});
export type ManagedSandboxGuestIoLimits = typeof ManagedSandboxGuestIoLimitsSchema.Type;

const RequestBase = {
  schemaVersion: S.Literal(MANAGED_SANDBOX_GUEST_IO_SCHEMA_VERSION),
  operationRef: SandboxRef,
  idempotencyRef: SandboxRef,
  actorRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  programRef: SandboxRef,
  workUnitRef: SandboxRef,
  sandboxRef: SandboxRef,
  resourceGeneration: PositiveInt,
  capabilityRef: SandboxRef,
  capabilityState: S.Literal("active"),
  capabilityExpiresAt: SandboxTimestamp,
  requestedAt: SandboxTimestamp,
  limits: ManagedSandboxGuestIoLimitsSchema,
};

export const ManagedSandboxGuestIoRequestSchema = S.Union([
  S.Struct({
    ...RequestBase,
    action: S.Literal("read_file"),
    path: S.String.check(S.isMinLength(1), S.isMaxLength(1_024)),
    encoding: S.Literals(["utf8", "base64"]),
  }),
  S.Struct({
    ...RequestBase,
    action: S.Literal("write_file"),
    path: S.String.check(S.isMinLength(1), S.isMaxLength(1_024)),
    encoding: S.Literals(["utf8", "base64"]),
    content: S.String,
    contentDigest: Sha256Digest,
  }),
  S.Struct({
    ...RequestBase,
    action: S.Literal("execute_command"),
    command: S.String.check(S.isMinLength(1), S.isMaxLength(16_384)),
    commandDigest: Sha256Digest,
    cwd: S.String.check(S.isMinLength(1), S.isMaxLength(1_024)),
    timeoutMillis: PositiveInt,
  }),
  S.Struct({
    ...RequestBase,
    action: S.Literal("read_artifact"),
    path: S.String.check(S.isMinLength(1), S.isMaxLength(1_024)),
    retentionUntil: SandboxTimestamp,
  }),
]);
export type ManagedSandboxGuestIoRequest = typeof ManagedSandboxGuestIoRequestSchema.Type;

export const ManagedSandboxGuestIoReceiptSchema = S.Struct({
  schemaVersion: S.Literal(MANAGED_SANDBOX_GUEST_IO_RECEIPT_SCHEMA_VERSION),
  receiptRef: SandboxRef,
  operationRef: SandboxRef,
  sandboxRef: SandboxRef,
  resourceGeneration: PositiveInt,
  capabilityRef: SandboxRef,
  action: ManagedSandboxGuestIoActionSchema,
  outcome: S.Literals(["succeeded", "refused", "failed"]),
  pathDigest: Sha256Digest,
  startedAt: SandboxTimestamp,
  finishedAt: SandboxTimestamp,
  bytesRead: NonNegativeInt,
  bytesWritten: NonNegativeInt,
  cpuMillis: NonNegativeInt,
  networkBytes: NonNegativeInt,
  processRef: S.optionalKey(SandboxRef),
  processTerminated: S.Boolean,
  descendantsRemaining: NonNegativeInt,
  scratchCleaned: S.Boolean,
  ingressClosed: S.Boolean,
  egressDenied: S.Boolean,
  pathPolicy: S.Literal("resolved_beneath_workspace_root"),
  symlinkTraversal: S.Literal(false),
  secretScan: S.Literal("clean"),
  evidenceRefs: S.Array(SandboxRef),
});
export type ManagedSandboxGuestIoReceipt = typeof ManagedSandboxGuestIoReceiptSchema.Type;

export const ManagedSandboxArtifactReceiptSchema = S.Struct({
  schemaVersion: S.Literal(MANAGED_SANDBOX_ARTIFACT_RECEIPT_SCHEMA_VERSION),
  artifactRef: SandboxRef,
  contentDigest: Sha256Digest,
  byteLength: NonNegativeInt,
  sourceGeneration: PositiveInt,
  sourcePathDigest: Sha256Digest,
  retentionUntil: SandboxTimestamp,
  contentType: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  evidenceRefs: S.Array(SandboxRef),
});
export type ManagedSandboxArtifactReceipt = typeof ManagedSandboxArtifactReceiptSchema.Type;

const ResponseBase = {
  schemaVersion: S.Literal(MANAGED_SANDBOX_GUEST_IO_SCHEMA_VERSION),
  operationRef: SandboxRef,
  sandboxRef: SandboxRef,
  resourceGeneration: PositiveInt,
  receipt: ManagedSandboxGuestIoReceiptSchema,
};

export const ManagedSandboxGuestIoResponseSchema = S.Union([
  S.Struct({
    ...ResponseBase,
    action: S.Literal("read_file"),
    encoding: S.Literals(["utf8", "base64"]),
    content: S.String,
    contentDigest: Sha256Digest,
    byteLength: NonNegativeInt,
    binary: S.Boolean,
  }),
  S.Struct({
    ...ResponseBase,
    action: S.Literal("write_file"),
    contentDigest: Sha256Digest,
    byteLength: NonNegativeInt,
  }),
  S.Struct({
    ...ResponseBase,
    action: S.Literal("execute_command"),
    success: S.Boolean,
    exitCode: S.NullOr(S.Number),
    signal: S.NullOr(S.String),
    stdout: S.String,
    stderr: S.String,
    stdoutTruncated: S.Boolean,
    stderrTruncated: S.Boolean,
    timedOut: S.Boolean,
    cancelled: S.Boolean,
    durationMillis: NonNegativeInt,
    maxProcessesObserved: NonNegativeInt,
  }),
  S.Struct({
    ...ResponseBase,
    action: S.Literal("read_artifact"),
    contentBase64: S.String,
    artifact: ManagedSandboxArtifactReceiptSchema,
  }),
]);
export type ManagedSandboxGuestIoResponse = typeof ManagedSandboxGuestIoResponseSchema.Type;
