import { Schema as S } from "effect";

import {
  NonNegativeInt,
  PositiveInt,
  SandboxRef,
  SandboxTimestamp,
  Sha256Digest,
} from "./schemas.ts";

export const MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION =
  "openagents.managed_sandbox_content_checkpoint.v1" as const;
export const MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION =
  "openagents.managed_sandbox_phase2_command.v1" as const;
export const MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION =
  "openagents.managed_sandbox_checkpoint_stop.v1" as const;
export const MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION =
  "openagents.managed_sandbox_checkpoint_delete_receipt.v1" as const;
export const MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION =
  "openagents.managed_sandbox_fork_receipt.v1" as const;
export const MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION =
  "openagents.managed_sandbox_restore_receipt.v1" as const;
export const MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION =
  "openagents.managed_sandbox_private_ingress.v1" as const;

export const MANAGED_SANDBOX_PRIVATE_INGRESS_MAX_TTL_SECONDS = 900 as const;

const BoundedRefs = S.Array(SandboxRef).check(S.isMaxLength(64));
const ShortIngressTtlSeconds = PositiveInt.check(
  S.isLessThanOrEqualTo(MANAGED_SANDBOX_PRIVATE_INGRESS_MAX_TTL_SECONDS),
);

/** State classes that a content checkpoint can never capture or restore. */
export const ManagedSandboxCheckpointOmissionsSchema = S.Struct({
  credentials: S.Literal("excluded"),
  accountSecrets: S.Literal("excluded"),
  providerHiddenState: S.Literal("excluded"),
  processMemory: S.Literal("excluded"),
  processTable: S.Literal("excluded"),
  ptyState: S.Literal("excluded"),
  sockets: S.Literal("excluded"),
  ports: S.Literal("excluded"),
  networkIdentity: S.Literal("excluded"),
});
export type ManagedSandboxCheckpointOmissions = typeof ManagedSandboxCheckpointOmissionsSchema.Type;

const Phase2CommandBase = {
  schema: S.Literal(MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION),
  commandRef: SandboxRef,
  idempotencyRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  requestedAt: SandboxTimestamp,
};

const CheckpointSourceCommandFields = {
  checkpointRef: SandboxRef,
  sourceSandboxRef: SandboxRef,
  sourceResourceGeneration: NonNegativeInt,
  sourceImageDigest: Sha256Digest,
  sourceToolchainDigest: Sha256Digest,
  repositoryRef: SandboxRef,
  repositoryRevisionRef: SandboxRef,
  repositoryPostImageDigest: Sha256Digest,
  formatRef: SandboxRef,
  retainedUntil: SandboxTimestamp,
};

/** Host-authorized Phase 2 commands. They contain references and digests only. */
export const ManagedSandboxPhase2CommandSchema = S.TaggedUnion({
  CreateCheckpoint: {
    ...Phase2CommandBase,
    ...CheckpointSourceCommandFields,
  },
  ArchiveWithCheckpoint: {
    ...Phase2CommandBase,
    ...CheckpointSourceCommandFields,
    stopRef: SandboxRef,
  },
  ForkFromCheckpoint: {
    ...Phase2CommandBase,
    checkpointRef: SandboxRef,
    expectedSourceSandboxRef: SandboxRef,
    expectedSourceResourceGeneration: NonNegativeInt,
    sourceCapabilityRefs: BoundedRefs,
  },
  RestoreCheckpoint: {
    ...Phase2CommandBase,
    checkpointRef: SandboxRef,
    destinationSandboxRef: SandboxRef,
    expectedSourceResourceGeneration: NonNegativeInt,
    admittedServiceRefs: BoundedRefs,
    sourceCapabilityRefs: BoundedRefs,
  },
  DeleteCheckpoint: {
    ...Phase2CommandBase,
    checkpointRef: SandboxRef,
    reason: S.Literals(["owner_requested", "retention_expired", "sandbox_teardown"]),
  },
  CreatePrivateIngress: {
    ...Phase2CommandBase,
    sandboxRef: SandboxRef,
    resourceGeneration: NonNegativeInt,
    audienceRef: SandboxRef,
    kind: S.Literals(["desktop", "preview"]),
    ttlSeconds: ShortIngressTtlSeconds,
  },
});
export type ManagedSandboxPhase2Command = typeof ManagedSandboxPhase2CommandSchema.Type;

/** A completed and verified filesystem content checkpoint, not a VM image. */
export const ManagedSandboxContentCheckpointSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION),
  checkpointRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  sourceSandboxRef: SandboxRef,
  sourceResourceGeneration: NonNegativeInt,
  sourceImageDigest: Sha256Digest,
  sourceToolchainDigest: Sha256Digest,
  repositoryRef: SandboxRef,
  repositoryRevisionRef: SandboxRef,
  repositoryPostImageDigest: Sha256Digest,
  contentDigest: Sha256Digest,
  contentBytes: NonNegativeInt,
  formatRef: SandboxRef,
  state: S.Literal("completed"),
  completedAt: SandboxTimestamp,
  verifiedAt: SandboxTimestamp,
  retainedUntil: SandboxTimestamp,
  deleteOnExpiry: S.Literal(true),
  omissions: ManagedSandboxCheckpointOmissionsSchema,
  evidenceRefs: BoundedRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (checkpoint) =>
        Date.parse(checkpoint.verifiedAt) >= Date.parse(checkpoint.completedAt) &&
        Date.parse(checkpoint.retainedUntil) > Date.parse(checkpoint.verifiedAt),
      {
        message:
          "checkpoint verification must follow completion and retention must follow verification",
      },
    ),
  ),
);
export type ManagedSandboxContentCheckpoint = typeof ManagedSandboxContentCheckpointSchema.Type;

/** Proof that checkpoint bytes were deleted from the authoritative store. */
export const ManagedSandboxCheckpointDeleteReceiptSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION),
  receiptRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  checkpointRef: SandboxRef,
  sourceSandboxRef: SandboxRef,
  sourceResourceGeneration: NonNegativeInt,
  contentDigest: Sha256Digest,
  contentDeleted: S.Literal(true),
  outcome: S.Literal("deleted"),
  reason: S.Literals(["owner_requested", "retention_expired", "sandbox_teardown"]),
  deletedAt: SandboxTimestamp,
  evidenceRefs: BoundedRefs,
});
export type ManagedSandboxCheckpointDeleteReceipt =
  typeof ManagedSandboxCheckpointDeleteReceiptSchema.Type;

const CheckpointStopBase = {
  schema: S.Literal(MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION),
  stopRef: SandboxRef,
  sandboxRef: SandboxRef,
  resourceGeneration: NonNegativeInt,
  observedAt: SandboxTimestamp,
  evidenceRefs: BoundedRefs,
};

/** A failed required checkpoint can only produce recovery-required lifecycle truth. */
export const ManagedSandboxCheckpointStopOutcomeSchema = S.TaggedUnion({
  Archived: {
    ...CheckpointStopBase,
    checkpoint: ManagedSandboxContentCheckpointSchema,
    lifecycle: S.Literal("stopped"),
    archiveClaim: S.Literal("allowed"),
  },
  CheckpointFailed: {
    ...CheckpointStopBase,
    attemptedCheckpointRef: SandboxRef,
    errorRef: SandboxRef,
    lifecycle: S.Literal("recovery_required"),
    archiveClaim: S.Literal("forbidden"),
  },
}).pipe(
  S.check(
    S.makeFilter(
      (outcome) =>
        outcome["_tag"] === "CheckpointFailed" ||
        (outcome.sandboxRef === outcome.checkpoint.sourceSandboxRef &&
          outcome.resourceGeneration === outcome.checkpoint.sourceResourceGeneration),
      {
        message: "archived checkpoint must bind the stopped sandbox and resource generation",
      },
    ),
  ),
);
export type ManagedSandboxCheckpointStopOutcome =
  typeof ManagedSandboxCheckpointStopOutcomeSchema.Type;

/** A fork starts a new identity namespace and can use only newly minted grants. */
export const ManagedSandboxForkReceiptSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION),
  receiptRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  checkpointRef: SandboxRef,
  sourceSandboxRef: SandboxRef,
  sourceResourceGeneration: NonNegativeInt,
  forkSandboxRef: SandboxRef,
  forkResourceGeneration: PositiveInt,
  sourceCapabilityRefs: BoundedRefs,
  forkCapabilityRefs: BoundedRefs,
  grantPolicy: S.Literal("mint_fresh"),
  cleanupObligationRef: SandboxRef,
  stateTransfer: ManagedSandboxCheckpointOmissionsSchema,
  processSessionContinuity: S.Literal("none"),
  outcome: S.Literal("created"),
  observedAt: SandboxTimestamp,
  evidenceRefs: BoundedRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (receipt) => {
        const sourceCapabilityRefs = new Set(receipt.sourceCapabilityRefs);
        const forkCapabilityRefs = new Set(receipt.forkCapabilityRefs);
        return (
          receipt.sourceSandboxRef !== receipt.forkSandboxRef &&
          sourceCapabilityRefs.size === receipt.sourceCapabilityRefs.length &&
          forkCapabilityRefs.size === receipt.forkCapabilityRefs.length &&
          receipt.forkCapabilityRefs.every(
            (capabilityRef) => !sourceCapabilityRefs.has(capabilityRef),
          )
        );
      },
      {
        message:
          "fork identity must be new and fork capability refs must be unique and disjoint from source refs",
      },
    ),
  ),
);
export type ManagedSandboxForkReceipt = typeof ManagedSandboxForkReceiptSchema.Type;

/** Restore starts admitted services only and reports that process sessions do not continue. */
export const ManagedSandboxRestoreReceiptSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION),
  receiptRef: SandboxRef,
  ownerRef: SandboxRef,
  tenantRef: SandboxRef,
  checkpointRef: SandboxRef,
  sandboxRef: SandboxRef,
  checkpointSourceGeneration: NonNegativeInt,
  restoredResourceGeneration: PositiveInt,
  admittedServiceRefs: BoundedRefs,
  restartedServiceRefs: BoundedRefs,
  sourceCapabilityRefs: BoundedRefs,
  restoredCapabilityRefs: BoundedRefs,
  grantPolicy: S.Literal("mint_fresh"),
  processSessionContinuity: S.Literal("discontinuous"),
  processMemoryRestored: S.Literal(false),
  ptyRestored: S.Literal(false),
  socketsRestored: S.Literal(false),
  outcome: S.Literal("restored"),
  observedAt: SandboxTimestamp,
  evidenceRefs: BoundedRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (receipt) => {
        const admittedServiceRefs = new Set(receipt.admittedServiceRefs);
        const sourceCapabilityRefs = new Set(receipt.sourceCapabilityRefs);
        return (
          receipt.restoredResourceGeneration > receipt.checkpointSourceGeneration &&
          admittedServiceRefs.size === receipt.admittedServiceRefs.length &&
          new Set(receipt.restartedServiceRefs).size === receipt.restartedServiceRefs.length &&
          receipt.restartedServiceRefs.every((serviceRef) => admittedServiceRefs.has(serviceRef)) &&
          new Set(receipt.restoredCapabilityRefs).size === receipt.restoredCapabilityRefs.length &&
          receipt.restoredCapabilityRefs.every(
            (capabilityRef) => !sourceCapabilityRefs.has(capabilityRef),
          )
        );
      },
      {
        message:
          "restore must advance generation, restart admitted services only, and mint fresh capability refs",
      },
    ),
  ),
);
export type ManagedSandboxRestoreReceipt = typeof ManagedSandboxRestoreReceiptSchema.Type;

const PrivateIngressBase = {
  schema: S.Literal(MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION),
  capabilityRef: SandboxRef,
  sandboxRef: SandboxRef,
  resourceGeneration: NonNegativeInt,
  ownerRef: SandboxRef,
  audienceRef: SandboxRef,
  kind: S.Literals(["desktop", "preview"]),
  issuedAt: SandboxTimestamp,
  expiresAt: SandboxTimestamp,
  ttlSeconds: ShortIngressTtlSeconds,
  accessUrlDigest: Sha256Digest,
  accessUrlAtRest: S.Literal("redacted"),
  audiencePolicy: S.Literal("owner_scoped_explicit_audience"),
  publicAccess: S.Literal(false),
  permanentRoute: S.Literal(false),
  vnc: S.Literal("unsupported"),
  auditRefs: BoundedRefs,
};

/** Durable ingress state contains a digest, never a bearer URL. */
export const ManagedSandboxPrivateIngressCapabilitySchema = S.TaggedUnion({
  Active: PrivateIngressBase,
  Revoked: {
    ...PrivateIngressBase,
    revokedAt: SandboxTimestamp,
    revokeReceiptRef: SandboxRef,
  },
  Expired: {
    ...PrivateIngressBase,
    expiredAt: SandboxTimestamp,
    expiryReceiptRef: SandboxRef,
  },
  Cleaned: {
    ...PrivateIngressBase,
    terminalState: S.Literals(["revoked", "expired"]),
    cleanedAt: SandboxTimestamp,
    cleanupReceiptRef: SandboxRef,
  },
}).pipe(
  S.check(
    S.makeFilter(
      (capability) =>
        Date.parse(capability.expiresAt) - Date.parse(capability.issuedAt) ===
        capability.ttlSeconds * 1_000,
      { message: "private ingress timestamps must encode the exact bounded TTL" },
    ),
  ),
);
export type ManagedSandboxPrivateIngressCapability =
  typeof ManagedSandboxPrivateIngressCapabilitySchema.Type;

/** Phase 2 does not admit private ingress until the later security proof passes. */
export const ManagedSandboxPrivateIngressAdmissionSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION),
  available: S.Literal(false),
  reason: S.Literal("security_proof_pending"),
  publicVnc: S.Literal("unsupported"),
  ungatedPreview: S.Literal("unsupported"),
  permanentRoute: S.Literal("unsupported"),
});
export type ManagedSandboxPrivateIngressAdmission =
  typeof ManagedSandboxPrivateIngressAdmissionSchema.Type;

export const MANAGED_SANDBOX_PRIVATE_INGRESS_ADMISSION =
  ManagedSandboxPrivateIngressAdmissionSchema.make({
    schema: MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION,
    available: false,
    reason: "security_proof_pending",
    publicVnc: "unsupported",
    ungatedPreview: "unsupported",
    permanentRoute: "unsupported",
  });

const Phase2ErrorBase = {
  message: S.String,
  retryable: S.Boolean,
  evidenceRefs: BoundedRefs,
};

/** Closed public-safe fault vocabulary for deterministic Phase 2 refusal. */
export const ManagedSandboxPhase2ErrorSchema = S.TaggedUnion({
  InvalidRequest: { ...Phase2ErrorBase, requestRef: SandboxRef },
  IdempotencyConflict: { ...Phase2ErrorBase, idempotencyRef: SandboxRef },
  CheckpointIncomplete: { ...Phase2ErrorBase, checkpointRef: SandboxRef },
  CheckpointCorrupt: { ...Phase2ErrorBase, checkpointRef: SandboxRef },
  CheckpointExpired: { ...Phase2ErrorBase, checkpointRef: SandboxRef },
  StaleSource: {
    ...Phase2ErrorBase,
    sourceSandboxRef: SandboxRef,
    expectedGeneration: NonNegativeInt,
    receivedGeneration: NonNegativeInt,
  },
  DuplicateFork: { ...Phase2ErrorBase, idempotencyRef: SandboxRef },
  ResumeFailed: { ...Phase2ErrorBase, checkpointRef: SandboxRef },
  PrivateIngressUnavailable: { ...Phase2ErrorBase, reasonRef: SandboxRef },
  PrivateIngressRevoked: { ...Phase2ErrorBase, capabilityRef: SandboxRef },
  PrivateIngressExpired: { ...Phase2ErrorBase, capabilityRef: SandboxRef },
});
export type ManagedSandboxPhase2Error = typeof ManagedSandboxPhase2ErrorSchema.Type;

export const decodeManagedSandboxPhase2Command = S.decodeUnknownSync(
  ManagedSandboxPhase2CommandSchema,
  { onExcessProperty: "error" },
);

export const decodeManagedSandboxContentCheckpoint = S.decodeUnknownSync(
  ManagedSandboxContentCheckpointSchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxCheckpointDeleteReceipt = S.decodeUnknownSync(
  ManagedSandboxCheckpointDeleteReceiptSchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxCheckpointStopOutcome = S.decodeUnknownSync(
  ManagedSandboxCheckpointStopOutcomeSchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxForkReceipt = S.decodeUnknownSync(
  ManagedSandboxForkReceiptSchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxRestoreReceipt = S.decodeUnknownSync(
  ManagedSandboxRestoreReceiptSchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxPrivateIngressCapability = S.decodeUnknownSync(
  ManagedSandboxPrivateIngressCapabilitySchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxPrivateIngressAdmission = S.decodeUnknownSync(
  ManagedSandboxPrivateIngressAdmissionSchema,
  { onExcessProperty: "error" },
);
export const decodeManagedSandboxPhase2Error = S.decodeUnknownSync(
  ManagedSandboxPhase2ErrorSchema,
  { onExcessProperty: "error" },
);
