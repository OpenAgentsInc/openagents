import {
  ManagedSandboxCommandSchema,
  ManagedSandboxEventSchema,
  ManagedSandboxReceiptSchema,
  ManagedSandboxResourceSchema,
  ManagedSandboxRuntimeIdentitySchema,
  ManagedSandboxTurnReceiptSchema,
  ManagedSandboxTurnSchema,
  ManagedSandboxTurnUsageSchema,
  SandboxBudgetSchema,
  SandboxCapabilitySchema,
  SandboxLeaseSchema,
  SandboxStateFactsSchema,
  SandboxTargetDescriptorSchema,
  Sha256Digest,
} from "@openagentsinc/managed-sandbox-contract"
import { Exit, Schema } from "effect"

import { IdeAgentAttachmentSchema, IdeAgentAttachmentRefSchema } from "./agent-code-contract.ts"
import {
  IdeAttachmentGenerationSchema,
  IdeCapabilitySnapshotSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"

const boundedRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
const boundedDetail = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800))
const nonNegativeInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))

export const IdeManagedSandboxSchemaVersion = Schema.Literal("openagents.desktop.ide-managed-sandbox.v1")

export const IdeManagedSandboxAdmissionSchema = Schema.TaggedUnion({
  Unavailable: {
    reason: boundedDetail,
    checkedAt: IdeTimestampSchema,
  },
  Available: {
    target: SandboxTargetDescriptorSchema,
    imageDigest: Sha256Digest,
    profileRef: boundedRef,
    lease: SandboxLeaseSchema,
    budget: SandboxBudgetSchema,
    requestedCapabilities: Schema.Array(SandboxCapabilitySchema).check(Schema.isMinLength(1), Schema.isMaxLength(16)),
    networkPosture: Schema.Literal("deny_all"),
    custody: Schema.Literal("openagents_managed_region"),
    retentionRef: boundedRef,
    checkedAt: IdeTimestampSchema,
  },
}).annotate({ identifier: "IdeManagedSandboxAdmission" })
export type IdeManagedSandboxAdmission = typeof IdeManagedSandboxAdmissionSchema.Type

export const IdeManagedSandboxGatewayResultSchema = Schema.Struct({
  command: ManagedSandboxCommandSchema,
  resource: ManagedSandboxResourceSchema,
  receipt: ManagedSandboxReceiptSchema,
  turn: Schema.NullOr(ManagedSandboxTurnSchema),
  turnReceipt: Schema.NullOr(ManagedSandboxTurnReceiptSchema),
  events: Schema.Array(ManagedSandboxEventSchema).check(Schema.isMaxLength(256)),
}).annotate({ identifier: "IdeManagedSandboxGatewayResult" })
export type IdeManagedSandboxGatewayResult = typeof IdeManagedSandboxGatewayResultSchema.Type

export const IdeManagedSandboxBindingSchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  sessionRef: IdeSessionRefSchema,
  agentAttachmentRef: IdeAgentAttachmentRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  placementGeneration: IdePlacementGenerationSchema,
  placementRef: IdePlacementRefSchema,
  workUnitRef: boundedRef,
  sandboxRef: boundedRef,
}).annotate({ identifier: "IdeManagedSandboxBinding" })
export type IdeManagedSandboxBinding = typeof IdeManagedSandboxBindingSchema.Type

export const IdeManagedSandboxResourceProjectionSchema = Schema.Struct({
  sandboxRef: boundedRef,
  workUnitRef: boundedRef,
  attachmentRef: boundedRef,
  attachmentGeneration: nonNegativeInteger,
  resourceGeneration: nonNegativeInteger,
  version: nonNegativeInteger,
  lastEventSequence: nonNegativeInteger,
  target: SandboxTargetDescriptorSchema,
  imageDigest: Sha256Digest,
  profileRef: boundedRef,
  lease: SandboxLeaseSchema,
  budget: SandboxBudgetSchema,
  capabilities: Schema.Array(SandboxCapabilitySchema).check(Schema.isMaxLength(16)),
  facts: SandboxStateFactsSchema,
  createdAt: IdeTimestampSchema,
  updatedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeManagedSandboxResourceProjection" })
export type IdeManagedSandboxResourceProjection = typeof IdeManagedSandboxResourceProjectionSchema.Type

export const IdeManagedSandboxTurnProjectionSchema = Schema.Struct({
  turnRef: boundedRef,
  commandRef: boundedRef,
  capabilityRef: boundedRef,
  turnSequence: nonNegativeInteger,
  lastEventSequence: nonNegativeInteger,
  runtime: ManagedSandboxRuntimeIdentitySchema,
  status: Schema.Literals(["pending", "running", "interrupting", "settled", "failed", "interrupted"]),
  usage: Schema.NullOr(ManagedSandboxTurnUsageSchema),
  createdAt: IdeTimestampSchema,
  startedAt: Schema.NullOr(IdeTimestampSchema),
  settledAt: Schema.NullOr(IdeTimestampSchema),
}).annotate({ identifier: "IdeManagedSandboxTurnProjection" })
export type IdeManagedSandboxTurnProjection = typeof IdeManagedSandboxTurnProjectionSchema.Type

export const IdeManagedSandboxReceiptProjectionSchema = Schema.Struct({
  receiptRef: boundedRef,
  commandRef: boundedRef,
  sandboxRef: boundedRef,
  resourceGeneration: nonNegativeInteger,
  version: nonNegativeInteger,
  outcome: Schema.Literals(["accepted", "succeeded", "refused", "failed", "replayed"]),
  lifecycle: Schema.Literals([
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
  ]),
  eventRefs: Schema.Array(boundedRef).check(Schema.isMaxLength(256)),
  artifactRefs: Schema.Array(boundedRef).check(Schema.isMaxLength(256)),
  errorCode: Schema.NullOr(boundedRef),
  observedAt: IdeTimestampSchema,
}).annotate({ identifier: "IdeManagedSandboxReceiptProjection" })
export type IdeManagedSandboxReceiptProjection = typeof IdeManagedSandboxReceiptProjectionSchema.Type

export const IdeManagedSandboxSnapshotSchema = Schema.Struct({
  schemaVersion: IdeManagedSandboxSchemaVersion,
  revision: nonNegativeInteger,
  admission: IdeManagedSandboxAdmissionSchema,
  binding: Schema.NullOr(IdeManagedSandboxBindingSchema),
  resource: Schema.NullOr(IdeManagedSandboxResourceProjectionSchema),
  projectCapability: Schema.NullOr(IdeCapabilitySnapshotSchema),
  turn: Schema.NullOr(IdeManagedSandboxTurnProjectionSchema),
  events: Schema.Array(ManagedSandboxEventSchema).check(Schema.isMaxLength(256)),
  receipts: Schema.Array(IdeManagedSandboxReceiptProjectionSchema).check(Schema.isMaxLength(64)),
  freshness: Schema.Literals(["live", "cached", "stale", "unavailable"]),
  latencyClass: Schema.Literals(["remote_interactive", "remote_background", "unavailable"]),
  lastError: Schema.NullOr(boundedDetail),
}).annotate({ identifier: "IdeManagedSandboxSnapshot" })
export type IdeManagedSandboxSnapshot = typeof IdeManagedSandboxSnapshotSchema.Type

const commandBase = {
  requestRef: boundedRef,
  idempotencyRef: boundedRef,
  requestedAt: IdeTimestampSchema,
}

const attachedCommandBase = {
  ...commandBase,
  expectedAttachment: IdeAgentAttachmentSchema,
}

export const IdeManagedSandboxCommandSchema = Schema.TaggedUnion({
  RefreshAdmission: commandBase,
  Create: {
    ...attachedCommandBase,
    workUnitRef: boundedRef,
  },
  Inspect: {
    ...attachedCommandBase,
    sandboxRef: boundedRef,
  },
  Dispatch: {
    ...attachedCommandBase,
    sandboxRef: boundedRef,
    turnRef: boundedRef,
    capabilityRef: boundedRef,
    prompt: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(100_000)),
    promptDigest: Sha256Digest,
    runtime: ManagedSandboxRuntimeIdentitySchema,
  },
  Interrupt: {
    ...attachedCommandBase,
    sandboxRef: boundedRef,
    turnRef: boundedRef,
    reasonRef: boundedRef,
  },
  Stop: {
    ...attachedCommandBase,
    sandboxRef: boundedRef,
    reasonRef: boundedRef,
  },
  Resume: {
    ...attachedCommandBase,
    sandboxRef: boundedRef,
  },
  Delete: {
    ...attachedCommandBase,
    sandboxRef: boundedRef,
    reasonRef: boundedRef,
  },
}).annotate({ identifier: "IdeManagedSandboxCommand" })
export type IdeManagedSandboxCommand = typeof IdeManagedSandboxCommandSchema.Type

export const IdeManagedSandboxCommandResultSchema = Schema.TaggedUnion({
  Succeeded: {
    snapshot: IdeManagedSandboxSnapshotSchema,
  },
  Refused: {
    reason: Schema.Literals([
      "invalid_input",
      "signed_out",
      "unattached",
      "stale_attachment",
      "not_configured",
      "wrong_sandbox",
      "stale_resource",
      "capability_denied",
      "gateway_unavailable",
      "invalid_response",
      "invariant_violation",
    ]),
    message: boundedDetail,
    snapshot: IdeManagedSandboxSnapshotSchema,
  },
}).annotate({ identifier: "IdeManagedSandboxCommandResult" })
export type IdeManagedSandboxCommandResult = typeof IdeManagedSandboxCommandResultSchema.Type

export const DesktopIdeManagedSandboxSnapshotChannel = "openagents-desktop/ide-managed-sandbox-snapshot" as const
export const DesktopIdeManagedSandboxCommandChannel = "openagents-desktop/ide-managed-sandbox-command" as const

export const emptyIdeManagedSandboxSnapshot = (checkedAt = "2026-07-19T00:00:00.000Z"): IdeManagedSandboxSnapshot =>
  IdeManagedSandboxSnapshotSchema.make({
    schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
    revision: 0,
    admission: {
      _tag: "Unavailable",
      reason: "OpenAgents-managed placement is not configured in this Desktop host.",
      checkedAt: IdeTimestampSchema.make(checkedAt),
    },
    binding: null,
    resource: null,
    projectCapability: null,
    turn: null,
    events: [],
    receipts: [],
    freshness: "unavailable",
    latencyClass: "unavailable",
    lastError: null,
  })

const decodeOrNull = <S extends Schema.ConstraintDecoder<unknown, never>>(
  schema: S,
  value: unknown,
): S["Type"] | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeIdeManagedSandboxSnapshot = (value: unknown): IdeManagedSandboxSnapshot | null =>
  decodeOrNull(IdeManagedSandboxSnapshotSchema, value)

export const decodeIdeManagedSandboxCommand = (value: unknown): IdeManagedSandboxCommand | null =>
  decodeOrNull(IdeManagedSandboxCommandSchema, value)

export const decodeIdeManagedSandboxCommandResult = (value: unknown): IdeManagedSandboxCommandResult | null =>
  decodeOrNull(IdeManagedSandboxCommandResultSchema, value)
