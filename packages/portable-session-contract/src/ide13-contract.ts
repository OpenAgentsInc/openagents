import { Schema } from "effect";

import {
  ExecutionEnvironmentRef,
  PortableRef,
  PortableTargetClass,
  PortableTimestamp,
  Sha256Digest,
} from "./primitives.js";

const count = (maximum: number) =>
  Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
    Schema.isLessThanOrEqualTo(maximum),
  );
const text = (maximum: number) => Schema.String.check(Schema.isMaxLength(maximum));
const refs = (maximum: number) => Schema.Array(PortableRef).check(Schema.isMaxLength(maximum));

export const IdePortableCapabilityKindSchema = Schema.Literals([
  "files",
  "search",
  "language",
  "source_control",
  "terminal",
  "task",
  "test",
  "debug",
  "agent",
  "review",
  "artifact",
  "preview",
]).annotate({ identifier: "IdePortableCapabilityKind" });
export type IdePortableCapabilityKind = typeof IdePortableCapabilityKindSchema.Type;

export const IdePortableCapabilityFactSchema = Schema.Struct({
  capabilityRef: PortableRef,
  kind: IdePortableCapabilityKindSchema,
  version: text(160),
  generation: count(1_000_000_000),
  readiness: Schema.Literals(["ready", "degraded", "unsupported", "stopped"]),
  freshness: Schema.Literals(["live", "cached", "stale", "unknown"]),
  startupLatencyMs: Schema.NullOr(count(86_400_000)),
  operationLatencyMs: Schema.NullOr(count(86_400_000)),
  omissionRefs: refs(128),
}).annotate({ identifier: "IdePortableCapabilityFact" });
export interface IdePortableCapabilityFact extends Schema.Schema.Type<
  typeof IdePortableCapabilityFactSchema
> {}

export const IdePortablePlacementFactsSchema = Schema.Struct({
  placementRef: ExecutionEnvironmentRef,
  targetClass: PortableTargetClass,
  providerRef: PortableRef,
  adapterRef: PortableRef,
  ownerRef: PortableRef,
  operatingSystem: Schema.Literals(["darwin", "windows", "linux", "ios", "android", "unknown"]),
  architecture: Schema.Literals(["x64", "arm64", "unknown"]),
  isolation: Schema.Literals(["owner_host_process", "owner_host_container", "dedicated_microvm"]),
  custody: Schema.Literals(["owner_device", "owner_managed", "openagents_managed"]),
  dataDestinations: Schema.Array(text(320)).check(Schema.isMaxLength(32)),
  networkDestinations: Schema.Array(text(320)).check(Schema.isMaxLength(32)),
  retentionSeconds: count(31_536_000),
  estimatedCostMicrounits: count(1_000_000_000_000),
  freshness: Schema.Literals(["live", "cached", "stale", "unknown"]),
  observedAt: PortableTimestamp,
  capabilities: Schema.Array(IdePortableCapabilityFactSchema).check(Schema.isMaxLength(64)),
  degradedReasonRefs: refs(64),
}).annotate({ identifier: "IdePortablePlacementFacts" });
export interface IdePortablePlacementFacts extends Schema.Schema.Type<
  typeof IdePortablePlacementFactsSchema
> {}

export const IdePortableProjectRefsSchema = Schema.Struct({
  projectRef: PortableRef,
  projectRootRef: PortableRef,
  worktreeRef: PortableRef,
  selectedFileRef: Schema.NullOr(PortableRef),
  documentSnapshotRef: Schema.NullOr(PortableRef),
  proposalRef: Schema.NullOr(PortableRef),
  diagnosticResultRef: Schema.NullOr(PortableRef),
  testResultRef: Schema.NullOr(PortableRef),
  artifactRef: Schema.NullOr(PortableRef),
  evidenceRef: Schema.NullOr(PortableRef),
}).annotate({ identifier: "IdePortableProjectRefs" });
export interface IdePortableProjectRefs extends Schema.Schema.Type<
  typeof IdePortableProjectRefsSchema
> {}

export const IdePortableCheckpointPolicySchema = Schema.Struct({
  maximumBytes: count(1_073_741_824),
  maximumFiles: count(1_000_000),
  encryption: Schema.Literals(["owner_key", "target_envelope", "not_required_owner_device"]),
  encryptionKeyRef: Schema.NullOr(PortableRef),
  custody: Schema.Literals(["owner_device", "owner_managed", "openagents_managed"]),
  retentionSeconds: count(31_536_000),
  expiresAt: PortableTimestamp,
}).annotate({ identifier: "IdePortableCheckpointPolicy" });
export interface IdePortableCheckpointPolicy extends Schema.Schema.Type<
  typeof IdePortableCheckpointPolicySchema
> {}

export const IdePortableCheckpointManifestSchema = Schema.Struct({
  manifestRef: PortableRef,
  checkpointRef: PortableRef,
  sessionRef: PortableRef,
  sourceAttachmentRef: PortableRef,
  sourceGeneration: count(1_000_000_000),
  digest: Sha256Digest,
  byteSize: count(1_073_741_824),
  fileCount: count(1_000_000),
  repositoryPostImageDigest: Sha256Digest,
  graphDigest: Sha256Digest,
  project: IdePortableProjectRefsSchema,
  includedCapabilityRefs: refs(64),
  omittedCapabilityRefs: refs(64),
  historyRefs: refs(1_000),
  proposalRefs: refs(1_000),
  taskRefs: refs(1_000),
  testRefs: refs(1_000),
  deliveryEvidenceRefs: refs(1_000),
  secretMaterial: Schema.Literal("excluded"),
  processState: Schema.Literal("excluded"),
  nativeState: Schema.Literal("excluded"),
  vimState: Schema.Literal("destination_setting"),
  themeState: Schema.Literal("destination_setting"),
  policy: IdePortableCheckpointPolicySchema,
  integrityReceiptRef: PortableRef,
}).annotate({ identifier: "IdePortableCheckpointManifest" });
export interface IdePortableCheckpointManifest extends Schema.Schema.Type<
  typeof IdePortableCheckpointManifestSchema
> {}

export const IdePortablePlacementEventSchema = Schema.Struct({
  eventRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  placementRef: ExecutionEnvironmentRef,
  generation: count(1_000_000_000),
  sequence: count(9_007_199_254_740_991),
  previousSequence: Schema.NullOr(count(9_007_199_254_740_991)),
  kind: Schema.Literals([
    "quiescing",
    "checkpoint_verified",
    "source_revoked",
    "destination_staged",
    "capabilities_ready",
    "attached",
    "failed_back",
    "revoked",
    "stopped",
  ]),
  occurredAt: PortableTimestamp,
  evidenceRefs: refs(128),
  publicSafe: Schema.Literal(true),
}).annotate({ identifier: "IdePortablePlacementEvent" });
export interface IdePortablePlacementEvent extends Schema.Schema.Type<
  typeof IdePortablePlacementEventSchema
> {}

export const IdePortableMoveReceiptSchema = Schema.Struct({
  receiptRef: PortableRef,
  commandRef: PortableRef,
  idempotencyKey: PortableRef,
  actorRef: PortableRef,
  policyRef: PortableRef,
  sessionRef: PortableRef,
  project: IdePortableProjectRefsSchema,
  sourcePlacementRef: ExecutionEnvironmentRef,
  destinationPlacementRef: ExecutionEnvironmentRef,
  sourceAttachmentRef: PortableRef,
  sourceGeneration: count(1_000_000_000),
  destinationAttachmentRef: PortableRef,
  destinationGeneration: count(1_000_000_000),
  checkpointManifestRef: PortableRef,
  transition: Schema.Literals(["move", "resume", "failback"]),
  status: Schema.Literals(["completed", "failed", "cancelled", "pending_reconcile"]),
  recoveryPointRef: PortableRef,
  omissionRefs: refs(128),
  evidenceRefs: refs(512),
  completedAt: PortableTimestamp,
}).annotate({ identifier: "IdePortableMoveReceipt" });
export interface IdePortableMoveReceipt extends Schema.Schema.Type<
  typeof IdePortableMoveReceiptSchema
> {}

export const IdePortableDestinationHelperKindSchema = Schema.Literals([
  "pty",
  "lsp",
  "dap",
  "watcher",
  "native",
]).annotate({ identifier: "IdePortableDestinationHelperKind" });
export type IdePortableDestinationHelperKind = typeof IdePortableDestinationHelperKindSchema.Type;

export const IdePortableDestinationHelperReadinessSchema = Schema.Struct({
  kind: IdePortableDestinationHelperKindSchema,
  readiness: Schema.Literals(["ready", "unsupported"]),
  instanceRef: Schema.NullOr(PortableRef),
  versionRef: Schema.NullOr(PortableRef),
  omissionRef: Schema.NullOr(PortableRef),
  evidenceRefs: refs(32),
}).annotate({ identifier: "IdePortableDestinationHelperReadiness" });
export interface IdePortableDestinationHelperReadiness extends Schema.Schema.Type<
  typeof IdePortableDestinationHelperReadinessSchema
> {}

export const IdePortableDestinationAuthenticationSchema = Schema.Struct({
  state: Schema.Literals(["reauthenticated", "expired", "revoked"]),
  policyRef: PortableRef,
  evidenceRef: PortableRef,
  observedAt: PortableTimestamp,
  expiresAt: Schema.NullOr(PortableTimestamp),
}).annotate({ identifier: "IdePortableDestinationAuthentication" });
export interface IdePortableDestinationAuthentication extends Schema.Schema.Type<
  typeof IdePortableDestinationAuthenticationSchema
> {}

/**
 * Public-safe destination admission evidence. It contains stable refs only.
 * It does not contain credential bytes, host paths, process IDs, or handles.
 */
export const IdePortableDestinationActivationReceiptSchema = Schema.Struct({
  schema: Schema.Literal("openagents.ide_portable_destination_activation.v1"),
  receiptRef: PortableRef,
  operationRef: PortableRef,
  sessionRef: PortableRef,
  checkpointRef: PortableRef,
  destinationTargetRef: ExecutionEnvironmentRef,
  destinationAttachmentRef: PortableRef,
  destinationRunnerSessionReservationRef: PortableRef,
  destinationGeneration: count(1_000_000_000),
  authentication: IdePortableDestinationAuthenticationSchema,
  helpersObservedAt: PortableTimestamp,
  helpers: Schema.Array(IdePortableDestinationHelperReadinessSchema).check(Schema.isMaxLength(5)),
  activatedAgentRefs: refs(10_000),
  acceptedWorkRefs: Schema.Array(
    Schema.Struct({
      agentRef: PortableRef,
      turnRef: PortableRef,
    }),
  ).check(Schema.isMaxLength(10_000)),
  evidenceRefs: refs(512),
}).annotate({ identifier: "IdePortableDestinationActivationReceipt" });
export interface IdePortableDestinationActivationReceipt extends Schema.Schema.Type<
  typeof IdePortableDestinationActivationReceiptSchema
> {}

const coordinatorCommandFields = {
  commandRef: PortableRef,
  idempotencyKey: PortableRef,
  actorRef: PortableRef,
  policyRef: PortableRef,
  sessionRef: PortableRef,
  project: IdePortableProjectRefsSchema,
  expectedAttachmentRef: PortableRef,
  expectedGeneration: count(1_000_000_000),
  deadlineAt: PortableTimestamp,
  approvalRef: Schema.NullOr(PortableRef),
};

export const IdePortableCoordinatorCommandSchema = Schema.TaggedUnion({
  Move: {
    ...coordinatorCommandFields,
    destinationPlacementRef: ExecutionEnvironmentRef,
  },
  Failback: {
    ...coordinatorCommandFields,
    destinationPlacementRef: ExecutionEnvironmentRef,
    recoveryPointRef: PortableRef,
  },
  Cancel: {
    ...coordinatorCommandFields,
    targetCommandRef: PortableRef,
  },
  Stop: {
    ...coordinatorCommandFields,
    reasonRef: PortableRef,
  },
}).annotate({ identifier: "IdePortableCoordinatorCommand" });
export type IdePortableCoordinatorCommand = typeof IdePortableCoordinatorCommandSchema.Type;

export const IdePortableCoordinatorSnapshotSchema = Schema.Struct({
  sessionRef: PortableRef,
  project: IdePortableProjectRefsSchema,
  phase: Schema.Literals([
    "attached",
    "quiescing",
    "checkpoint_verified",
    "destination_staged",
    "source_revoked",
    "attaching",
    "degraded",
    "stopped",
  ]),
  activePlacementRef: ExecutionEnvironmentRef,
  activeAttachmentRef: PortableRef,
  activeGeneration: count(1_000_000_000),
  pendingCommandRef: Schema.NullOr(PortableRef),
  pendingDestinationPlacementRef: Schema.NullOr(ExecutionEnvironmentRef),
  checkpointManifestRef: Schema.NullOr(PortableRef),
  eventSequence: count(9_007_199_254_740_991),
  stopped: Schema.Boolean,
}).annotate({ identifier: "IdePortableCoordinatorSnapshot" });
export interface IdePortableCoordinatorSnapshot extends Schema.Schema.Type<
  typeof IdePortableCoordinatorSnapshotSchema
> {}

const failureFields = { operation: text(120), detailRef: PortableRef, retryable: Schema.Boolean };
export class IdePortableStaleWriter extends Schema.TaggedErrorClass<IdePortableStaleWriter>()(
  "IdePortable.StaleWriter",
  failureFields,
) {}
export class IdePortableLeaseContention extends Schema.TaggedErrorClass<IdePortableLeaseContention>()(
  "IdePortable.LeaseContention",
  failureFields,
) {}
export class IdePortableCheckpointFailure extends Schema.TaggedErrorClass<IdePortableCheckpointFailure>()(
  "IdePortable.CheckpointFailure",
  failureFields,
) {}
export class IdePortableAuthorizationFailure extends Schema.TaggedErrorClass<IdePortableAuthorizationFailure>()(
  "IdePortable.AuthorizationFailure",
  failureFields,
) {}
export class IdePortablePlacementFailure extends Schema.TaggedErrorClass<IdePortablePlacementFailure>()(
  "IdePortable.PlacementFailure",
  failureFields,
) {}
export class IdePortableCancelled extends Schema.TaggedErrorClass<IdePortableCancelled>()(
  "IdePortable.Cancelled",
  failureFields,
) {}
export class IdePortableTeardownFailure extends Schema.TaggedErrorClass<IdePortableTeardownFailure>()(
  "IdePortable.TeardownFailure",
  failureFields,
) {}

export const IdePortableFailureSchema = Schema.Union([
  IdePortableStaleWriter,
  IdePortableLeaseContention,
  IdePortableCheckpointFailure,
  IdePortableAuthorizationFailure,
  IdePortablePlacementFailure,
  IdePortableCancelled,
  IdePortableTeardownFailure,
]).annotate({ identifier: "IdePortableFailure" });
export type IdePortableFailure = typeof IdePortableFailureSchema.Type;
