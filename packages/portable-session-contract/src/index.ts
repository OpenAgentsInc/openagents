import { Schema as S } from "effect"

export const PORTABLE_SESSION_SCHEMA_VERSION =
  "openagents.portable_session.v1" as const
export const PORTABLE_CHECKPOINT_SCHEMA_VERSION =
  "openagents.portable_checkpoint.v1" as const
export const PORTABLE_COMMAND_SCHEMA_VERSION =
  "openagents.portable_session_command.v1" as const

export const PortableRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
)
export type PortableRef = typeof PortableRef.Type

export const Sha256Digest = S.String.check(
  S.isPattern(/^sha256:[a-f0-9]{64}$/),
)
export type Sha256Digest = typeof Sha256Digest.Type

export const PortableTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
)

const NonNegativeInt = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)

export const PortableTargetClass = S.Literals([
  "owner_local",
  "owner_managed",
  "openagents_managed",
  "managed_provider",
])
export type PortableTargetClass = typeof PortableTargetClass.Type

export const PortableIsolationClass = S.Literals([
  "owner_host_process",
  "owner_host_container",
  "dedicated_microvm",
])
export type PortableIsolationClass = typeof PortableIsolationClass.Type

export const PortableDataPosture = S.Literals([
  "owner_device_only",
  "owner_managed_region",
  "openagents_managed_region",
])
export type PortableDataPosture = typeof PortableDataPosture.Type

export const PortableTargetDescriptorSchema = S.Struct({
  targetRef: PortableRef,
  targetClass: PortableTargetClass,
  adapterRef: PortableRef,
  ownerRef: PortableRef,
  compatibilityRef: PortableRef,
  isolation: PortableIsolationClass,
  dataPosture: PortableDataPosture,
  health: S.Literals([
    "ready",
    "offline",
    "incompatible",
    "revoked",
    "upgrading",
    "draining",
    "unavailable",
  ]),
})
export type PortableTargetDescriptor =
  typeof PortableTargetDescriptorSchema.Type

export const PortableAgentLifecycle = S.Literals([
  "created",
  "running",
  "waiting",
  "quiescing",
  "quiesced",
  "completed",
  "failed",
  "canceled",
  "interrupted",
])
export type PortableAgentLifecycle = typeof PortableAgentLifecycle.Type

export const PortableAgentNodeSchema = S.Struct({
  agentRef: PortableRef,
  parentAgentRef: S.optionalKey(PortableRef),
  threadRef: PortableRef,
  transcriptRef: PortableRef,
  activityCursor: NonNegativeInt,
  lifecycle: PortableAgentLifecycle,
  attachmentGeneration: NonNegativeInt,
})
export type PortableAgentNode = typeof PortableAgentNodeSchema.Type

export const PortableAgentGraphSchema = S.Struct({
  rootAgentRef: PortableRef,
  nodes: S.Array(PortableAgentNodeSchema),
})
export type PortableAgentGraph = typeof PortableAgentGraphSchema.Type

export const PortableCodingSessionSchema = S.Struct({
  schema: S.Literal(PORTABLE_SESSION_SCHEMA_VERSION),
  sessionRef: PortableRef,
  ownerRef: PortableRef,
  identityBasis: S.Literal("owner_minted"),
  workContextRef: PortableRef,
  /** Authoritative append-only durable per-thread event plane. */
  eventLogRef: PortableRef,
  /** Repairable bounded projection; never outcome authority by itself. */
  currentProjectionRef: PortableRef,
  /** Ephemeral acceleration only; gaps repair from eventLogRef. */
  volatileStreamRef: S.optionalKey(PortableRef),
  commandScopeRef: PortableRef,
  graph: PortableAgentGraphSchema,
  adoptedFromLocalHistory: S.Boolean,
  adoptionReceiptRef: S.optionalKey(PortableRef),
})
export type PortableCodingSession = typeof PortableCodingSessionSchema.Type

/**
 * Additive PORT-03 binding for the execution identity that is applicable to a
 * bounded repository coding session. It is separate from the frozen v1
 * session envelope so pre-PORT-03 local-only rows remain readable, but no row
 * may move until this owner/session binding exists.
 */
export const PORTABLE_SESSION_EXECUTION_BINDING_SCHEMA_VERSION =
  "openagents.portable_session_execution_binding.v1" as const

export const PortableSessionExecutionBindingSchema = S.Struct({
  schema: S.Literal(PORTABLE_SESSION_EXECUTION_BINDING_SCHEMA_VERSION),
  sessionRef: PortableRef,
  ownerRef: PortableRef,
  runRef: PortableRef,
  repositoryRef: PortableRef,
  pinnedBaseRef: PortableRef,
})
export type PortableSessionExecutionBinding =
  typeof PortableSessionExecutionBindingSchema.Type

export const PortableAttachmentState = S.Literals([
  "preparing",
  "active",
  "quiescing",
  "quiesced",
  "detached",
  "failed",
  "reclaimed",
])
export type PortableAttachmentState = typeof PortableAttachmentState.Type

export const PortableAttachmentSchema = S.Struct({
  attachmentRef: PortableRef,
  sessionRef: PortableRef,
  targetRef: PortableRef,
  generation: NonNegativeInt,
  state: PortableAttachmentState,
  descendantAgentRefs: S.Array(PortableRef),
  capabilityLeaseRefs: S.Array(PortableRef),
  checkpointRef: S.optionalKey(PortableRef),
  evidenceRefs: S.Array(PortableRef),
})
export type PortableAttachment = typeof PortableAttachmentSchema.Type

export const PortableCheckpointSchema = S.Struct({
  schema: S.Literal(PORTABLE_CHECKPOINT_SCHEMA_VERSION),
  checkpointRef: PortableRef,
  sessionRef: PortableRef,
  sourceAttachmentRef: PortableRef,
  sourceGeneration: NonNegativeInt,
  digest: Sha256Digest,
  parentCheckpointRef: S.optionalKey(PortableRef),
  repositoryRef: PortableRef,
  repositoryRevisionRef: PortableRef,
  repositoryPostImageDigest: Sha256Digest,
  diffDigest: Sha256Digest,
  eventLogCursor: NonNegativeInt,
  catalogGenerationRef: PortableRef,
  graphDigest: Sha256Digest,
  approvalRefs: S.Array(PortableRef),
  artifactRefs: S.Array(PortableRef),
  receiptRefs: S.Array(PortableRef),
  secretMaterial: S.Literal("excluded"),
  processState: S.Literal("excluded"),
})
export type PortableCheckpoint = typeof PortableCheckpointSchema.Type

export const PortableCapabilityKind = S.Literals([
  "provider",
  "scm_read",
  "scm_write",
  "tool",
  "api",
])
export type PortableCapabilityKind = typeof PortableCapabilityKind.Type

export const PortableCapabilityLeaseSchema = S.Struct({
  leaseRef: PortableRef,
  ownerRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: NonNegativeInt,
  targetRef: PortableRef,
  capability: PortableCapabilityKind,
  accountRef: S.optionalKey(PortableRef),
  toolRef: S.optionalKey(PortableRef),
  expiresAt: PortableTimestamp,
  state: S.Literals(["issued", "redeemed", "revoked", "expired", "released"]),
})
export type PortableCapabilityLease =
  typeof PortableCapabilityLeaseSchema.Type

export const PortableSessionCommandKind = S.Literals([
  "stop",
  "checkpoint",
  "detach",
  "attach",
  "move",
  "abort_move",
  "resume",
  "failback",
])
export type PortableSessionCommandKind =
  typeof PortableSessionCommandKind.Type

export const PORTABLE_ACTION_INVOCATION_PATHS = [
  "click",
  "tap",
  "menu",
  "palette",
  "conflict_safe_key",
] as const

export const PortableActionInvocationPath = S.Literals(
  PORTABLE_ACTION_INVOCATION_PATHS,
)
export type PortableActionInvocationPath =
  typeof PortableActionInvocationPath.Type

export const PortableActionBindingSchema = S.Struct({
  actionRef: PortableRef,
  commandKind: PortableSessionCommandKind,
  invocationPaths: S.Array(PortableActionInvocationPath),
  commandSchema: S.Literal(PORTABLE_COMMAND_SCHEMA_VERSION),
})
export type PortableActionBinding = typeof PortableActionBindingSchema.Type

export const PortableSessionCommandSchema = S.Struct({
  schema: S.Literal(PORTABLE_COMMAND_SCHEMA_VERSION),
  commandRef: PortableRef,
  idempotencyKey: PortableRef,
  ownerRef: PortableRef,
  sessionRef: PortableRef,
  kind: PortableSessionCommandKind,
  expectedAttachmentRef: PortableRef,
  expectedGeneration: NonNegativeInt,
  destinationTargetRef: S.optionalKey(PortableRef),
  checkpointRef: S.optionalKey(PortableRef),
  expiresAt: PortableTimestamp,
})
export type PortableSessionCommand = typeof PortableSessionCommandSchema.Type

export const PortableSessionCommandOutcomeSchema = S.Struct({
  commandRef: PortableRef,
  sessionRef: PortableRef,
  status: S.Literals([
    "accepted",
    "rejected",
    "failed",
    "unknown_pending_reconcile",
    "completed",
    "expired",
  ]),
  sourceAttachmentRef: PortableRef,
  sourceGeneration: NonNegativeInt,
  destinationAttachmentRef: S.optionalKey(PortableRef),
  destinationGeneration: S.optionalKey(NonNegativeInt),
  checkpointRef: S.optionalKey(PortableRef),
  reasonRef: S.optionalKey(PortableRef),
  evidenceRefs: S.Array(PortableRef),
})
export type PortableSessionCommandOutcome =
  typeof PortableSessionCommandOutcomeSchema.Type

export * from "./journeys.js"
export * from "./model.js"
export * from "./capability-broker.js"
