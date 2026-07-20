import { Schema as S } from "effect"

export const PORTABLE_SESSION_SCHEMA_VERSION =
  "openagents.portable_session.v1" as const
export const PORTABLE_CHECKPOINT_SCHEMA_VERSION =
  "openagents.portable_checkpoint.v1" as const
export const PORTABLE_COMMAND_SCHEMA_VERSION =
  "openagents.portable_session_command.v1" as const

/** Canonical Khala Sync projection entity names for portable authority. */
export const PORTABLE_SESSION_ENTITY_TYPE = "portable_session" as const
export const PORTABLE_AGENT_GRAPH_ENTITY_TYPE = "portable_agent_graph" as const
export const PORTABLE_ATTACHMENT_ENTITY_TYPE = "portable_attachment" as const
export const PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE = "portable_target_directory" as const
export const PORTABLE_THREAD_CURRENT_ENTITY_TYPE = "portable_thread_current" as const
export const PORTABLE_COMMAND_ENTITY_TYPE = "portable_command" as const
export const PORTABLE_EXECUTION_BINDING_ENTITY_TYPE = "portable_execution_binding" as const

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

/**
 * ENV-1 vocabulary (docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md,
 * "Environment and endpoint vocabulary (ENV-1)"): the owner-scoped identity of
 * one ExecutionEnvironment — a local Pylon, an owner-managed remote
 * Pylon/oa-node, an OpenAgents Agent Computer, or an audited managed-provider
 * workspace. The identity binds to the owner scope that enrolled the
 * environment and to its enrollment/health receipts, never to a bare
 * hostname, address, or process. How a client currently reaches the
 * environment (an AccessEndpoint, possibly hinted by an AdvertisedEndpoint)
 * is a connection-layer fact that never enters this identity, and switching
 * AccessEndpoint or KnownEnvironment entry must never create, transfer, or
 * fence execution authority — only the attachment-generation contract does.
 * Wire shape is exactly `PortableRef`; this alias adds vocabulary, not a
 * serialization change.
 */
export const ExecutionEnvironmentRef = PortableRef
export type ExecutionEnvironmentRef = typeof ExecutionEnvironmentRef.Type

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

/**
 * Describes one ExecutionEnvironment (ENV-1): what the environment is and
 * what it may safely do — never how a client currently reaches it.
 * AccessEndpoint/AdvertisedEndpoint reachability facts stay at the connection
 * layer and out of this durable identity. The auth-bearing endpoint metadata
 * ENV-1 deferred is typed in `@openagentsinc/environment-auth` (ENV-2,
 * openagents #8780): scoped, DPoP-bound capability grants keyed to this
 * ExecutionEnvironment identity.
 */
export const PortableTargetDescriptorSchema = S.Struct({
  targetRef: ExecutionEnvironmentRef,
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
  targetRef: ExecutionEnvironmentRef,
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

/**
 * One per-thread transcript/activity cursor inside a portable checkpoint
 * bundle. Runtime-neutral wire shape shared by the Pylon operation ledger
 * (producer) and the Khala Sync server's managed provisioner (consumer);
 * moved here from `apps/pylon/src/portable-session-operation-ledger.ts` so
 * runtime-neutral consumers do not import Bun-typed Pylon modules.
 */
export const PylonPortableThreadCursorSchema = S.Struct({
  threadRef: S.String,
  transcriptRef: S.String,
  activityCursor: S.Number,
  eventCursor: S.Number,
})
export type PylonPortableThreadCursor =
  typeof PylonPortableThreadCursorSchema.Type

/**
 * The complete portable checkpoint bundle: the checkpoint record, its
 * execution binding, the agent graph, and per-thread cursors. Produced by
 * the Pylon operation ledger, consumed by portable-session destinations and
 * the managed agent-computer provisioner.
 */
export const PylonPortableCheckpointBundleSchema = S.Struct({
  checkpoint: PortableCheckpointSchema,
  executionBinding: PortableSessionExecutionBindingSchema,
  graph: PortableAgentGraphSchema,
  threadCursors: S.Array(PylonPortableThreadCursorSchema),
})
export type PylonPortableCheckpointBundle =
  typeof PylonPortableCheckpointBundleSchema.Type

/** Input to a checkpoint artifact resolver: the exact source binding whose post-image is exported. */
export type PortableCheckpointArtifactResolverInput = Readonly<{
  ownerRef: string
  targetRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  checkpointRef: string
  bundle: PylonPortableCheckpointBundle
}>

export type PortableCheckpointArtifact = Readonly<{
  artifactRef: string
  digest: `sha256:${string}`
  bytes: Uint8Array
}>

export type PortableCheckpointArtifactResolver = Readonly<{
  resolve: (
    input: PortableCheckpointArtifactResolverInput,
  ) => Promise<PortableCheckpointArtifact>
}>

export type PortableCheckpointArtifactStore =
  & PortableCheckpointArtifactResolver
  & Readonly<{
    registerArtifact: (input: Readonly<{
      bundle: PylonPortableCheckpointBundle
      artifact: PortableCheckpointArtifact
    }>) => Promise<void>
  }>

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
  targetRef: ExecutionEnvironmentRef,
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
  /** ENV-1: names the destination ExecutionEnvironment for move/attach/failback. */
  destinationTargetRef: S.optionalKey(ExecutionEnvironmentRef),
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

/** Confirmed target membership projected for one portable session. */
export const PortableTargetDirectoryProjectionSchema = S.Struct({
  sessionRef: PortableRef,
  targets: S.Array(PortableTargetDescriptorSchema),
})
export type PortableTargetDirectoryProjection =
  typeof PortableTargetDirectoryProjectionSchema.Type

/**
 * A command is first projected as accepted and later replaced by its durable
 * outcome. Clients must never synthesize the latter from a queued mutation.
 */
export const PortableCommandProjectionSchema = S.Union([
  S.Struct({
    command: PortableSessionCommandSchema,
    status: S.Literal("accepted"),
  }),
  S.Struct({
    command: PortableSessionCommandSchema,
    outcome: PortableSessionCommandOutcomeSchema,
  }),
])
export type PortableCommandProjection = typeof PortableCommandProjectionSchema.Type

export * from "./journeys.js"
export * from "./model.js"
export * from "./capability-broker.js"
export * from "./portable-command-execution.js"
export * from "./ide13-contract.js"
export * from "./ide13-model.js"
export * from "./portable-phase-operation.js"
export * from "./destination-readiness.js"
export * from "./desktop-source-safe-point-control.js"
export * from "./checkpoint-custody-transport.js"
