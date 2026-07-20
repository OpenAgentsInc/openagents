import { Schema } from "effect"

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
)
const ExecutionEnvironmentRef = PortableRef
const PortableTargetClass = Schema.Literals(["owner_local", "owner_managed", "openagents_managed", "managed_provider"])
const PortableTimestamp = Schema.String.check(Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/))
const Sha256Digest = Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/))

const count = (maximum: number) => Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(maximum),
)
const text = (maximum: number) => Schema.String.check(Schema.isMaxLength(maximum))
const refs = (maximum: number) => Schema.Array(PortableRef).check(Schema.isMaxLength(maximum))

export const IdePortableCapabilityKindSchema = Schema.Literals([
  "files", "search", "language", "source_control", "terminal", "task", "test",
  "debug", "agent", "review", "artifact", "preview",
]).annotate({ identifier: "IdePortableCapabilityKind" })

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
}).annotate({ identifier: "IdePortableCapabilityFact" })

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
}).annotate({ identifier: "IdePortablePlacementFacts" })

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
}).annotate({ identifier: "IdePortableProjectRefs" })

export const IdePortableCheckpointPolicySchema = Schema.Struct({
  maximumBytes: count(1_073_741_824),
  maximumFiles: count(1_000_000),
  encryption: Schema.Literals(["owner_key", "target_envelope", "not_required_owner_device"]),
  encryptionKeyRef: Schema.NullOr(PortableRef),
  custody: Schema.Literals(["owner_device", "owner_managed", "openagents_managed"]),
  retentionSeconds: count(31_536_000),
  expiresAt: PortableTimestamp,
}).annotate({ identifier: "IdePortableCheckpointPolicy" })

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
}).annotate({ identifier: "IdePortableCheckpointManifest" })

export const IdePortablePlacementEventSchema = Schema.Struct({
  eventRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  placementRef: ExecutionEnvironmentRef,
  generation: count(1_000_000_000),
  sequence: count(9_007_199_254_740_991),
  previousSequence: Schema.NullOr(count(9_007_199_254_740_991)),
  kind: Schema.Literals(["quiescing", "checkpoint_verified", "source_revoked", "destination_staged", "capabilities_ready", "attached", "failed_back", "revoked", "stopped"]),
  occurredAt: PortableTimestamp,
  evidenceRefs: refs(128),
  publicSafe: Schema.Literal(true),
}).annotate({ identifier: "IdePortablePlacementEvent" })

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
}).annotate({ identifier: "IdePortableMoveReceipt" })

const failureFields = { operation: text(120), detailRef: PortableRef, retryable: Schema.Boolean }
export class IdePortableStaleWriter extends Schema.TaggedErrorClass<IdePortableStaleWriter>()("IdePortable.StaleWriter", failureFields) {}
export class IdePortableLeaseContention extends Schema.TaggedErrorClass<IdePortableLeaseContention>()("IdePortable.LeaseContention", failureFields) {}
export class IdePortableCheckpointFailure extends Schema.TaggedErrorClass<IdePortableCheckpointFailure>()("IdePortable.CheckpointFailure", failureFields) {}
export class IdePortableAuthorizationFailure extends Schema.TaggedErrorClass<IdePortableAuthorizationFailure>()("IdePortable.AuthorizationFailure", failureFields) {}
export class IdePortablePlacementFailure extends Schema.TaggedErrorClass<IdePortablePlacementFailure>()("IdePortable.PlacementFailure", failureFields) {}
export class IdePortableCancelled extends Schema.TaggedErrorClass<IdePortableCancelled>()("IdePortable.Cancelled", failureFields) {}
export class IdePortableTeardownFailure extends Schema.TaggedErrorClass<IdePortableTeardownFailure>()("IdePortable.TeardownFailure", failureFields) {}

export const IdePortableFailureSchema = Schema.Union([
  IdePortableStaleWriter,
  IdePortableLeaseContention,
  IdePortableCheckpointFailure,
  IdePortableAuthorizationFailure,
  IdePortablePlacementFailure,
  IdePortableCancelled,
  IdePortableTeardownFailure,
]).annotate({ identifier: "IdePortableFailure" })
