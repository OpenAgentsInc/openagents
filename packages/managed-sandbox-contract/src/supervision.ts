import { Schema as S } from "effect";

import {
  ManagedSandboxRuntimeIdentitySchema,
  ManagedSandboxTurnStatusSchema,
  NonNegativeInt,
  PositiveInt,
  SandboxLeaseState,
  SandboxLifecycle,
  SandboxRef,
  SandboxRuntimeState,
  SandboxTimestamp,
} from "./schemas.ts";

export const MANAGED_SANDBOX_SUPERVISION_SCHEMA_VERSION =
  "openagents.managed_sandbox_supervision.v1" as const;
export const MANAGED_SANDBOX_SUPERVISION_COMMAND_SCHEMA_VERSION =
  "openagents.managed_sandbox_supervision_command.v1" as const;
export const MANAGED_SANDBOX_SUPERVISION_OUTCOME_SCHEMA_VERSION =
  "openagents.managed_sandbox_supervision_outcome.v1" as const;

const BoundedRefs = S.Array(SandboxRef).check(S.isMaxLength(64));
const NullableTimestamp = S.NullOr(SandboxTimestamp);

export const ManagedSandboxSupervisionActorSchema = S.Literals([
  "principal.desktop",
  "principal.mobile",
  "principal.sarah",
  "principal.web",
  "principal.system",
  "principal.unknown",
]);

export const ManagedSandboxSupervisionProjectionSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_SUPERVISION_SCHEMA_VERSION),
  sandboxRef: SandboxRef,
  workUnitRef: SandboxRef,
  attachmentRef: SandboxRef,
  attachmentGeneration: NonNegativeInt,
  resourceGeneration: PositiveInt,
  version: NonNegativeInt,
  target: S.Struct({
    targetRef: SandboxRef,
    provider: S.Literal("google_cloud"),
    region: SandboxRef,
    isolation: S.Literals(["gce_vm", "firecracker_microvm"]),
    custody: S.Literal("openagents_managed_region"),
  }),
  state: S.Struct({
    lifecycle: SandboxLifecycle,
    runtime: SandboxRuntimeState,
    acceptingWork: S.Boolean,
  }),
  timing: S.Struct({
    createdAt: SandboxTimestamp,
    updatedAt: SandboxTimestamp,
    leaseExpiresAt: SandboxTimestamp,
    elapsedSeconds: NonNegativeInt,
    idleSeconds: NonNegativeInt,
    leaseState: SandboxLeaseState,
  }),
  budget: S.Struct({
    class: S.Literal("bounded"),
    currency: S.Literal("USD"),
    maxCostMicros: NonNegativeInt,
    observedCostMicros: S.NullOr(NonNegativeInt),
    state: S.Literals(["within_cap", "exhausted", "unreported"]),
  }),
  runtime: S.NullOr(
    S.Struct({
      turnRef: SandboxRef,
      status: ManagedSandboxTurnStatusSchema,
      identity: ManagedSandboxRuntimeIdentitySchema,
      actorRef: ManagedSandboxSupervisionActorSchema,
      startedAt: NullableTimestamp,
      settledAt: NullableTimestamp,
      terminalReasonRef: S.NullOr(SandboxRef),
    }),
  ),
  lastStructuralEvent: S.NullOr(
    S.Struct({
      eventRef: SandboxRef,
      kind: SandboxRef,
      sequence: PositiveInt,
      observedAt: SandboxTimestamp,
    }),
  ),
  attention: S.Struct({
    state: S.Literals(["none", "needs_action", "recovery_required"]),
    reasonRef: S.NullOr(SandboxRef),
  }),
  cleanup: S.Struct({
    state: S.Literals(["not_started", "in_progress", "complete", "recovery_required"]),
    receiptRef: S.NullOr(SandboxRef),
  }),
  outcomes: S.Struct({
    fileRefs: BoundedRefs,
    changeRefs: BoundedRefs,
    artifactRefs: BoundedRefs,
    evidenceRefs: BoundedRefs,
    receiptRefs: BoundedRefs,
  }),
});
export type ManagedSandboxSupervisionProjection =
  typeof ManagedSandboxSupervisionProjectionSchema.Type;

const SupervisionCommandBase = {
  schema: S.Literal(MANAGED_SANDBOX_SUPERVISION_COMMAND_SCHEMA_VERSION),
  commandRef: SandboxRef,
  idempotencyRef: SandboxRef,
  surface: S.Literals(["mobile", "web"]),
  sandboxRef: SandboxRef,
  expectedVersion: NonNegativeInt,
  expectedResourceGeneration: PositiveInt,
  issuedAt: SandboxTimestamp,
  expiresAt: SandboxTimestamp,
};

export const ManagedSandboxSupervisionCommandSchema = S.TaggedUnion({
  Interrupt: {
    ...SupervisionCommandBase,
    turnRef: SandboxRef,
    reasonRef: SandboxRef,
  },
  Stop: {
    ...SupervisionCommandBase,
    reasonRef: SandboxRef,
  },
  Resume: SupervisionCommandBase,
  Delete: {
    ...SupervisionCommandBase,
    reasonRef: SandboxRef,
  },
}).pipe(
  S.check(
    S.makeFilter((command) => Date.parse(command.expiresAt) > Date.parse(command.issuedAt), {
      message: "supervision command expiry must follow issue time",
    }),
  ),
);
export type ManagedSandboxSupervisionCommand = typeof ManagedSandboxSupervisionCommandSchema.Type;

export const ManagedSandboxSupervisionEnvelopeSchema = S.Struct({
  projections: S.Array(ManagedSandboxSupervisionProjectionSchema).check(S.isMaxLength(100)),
  observedAt: SandboxTimestamp,
});
export type ManagedSandboxSupervisionEnvelope = typeof ManagedSandboxSupervisionEnvelopeSchema.Type;

export const ManagedSandboxSupervisionOutcomeSchema = S.Struct({
  schema: S.Literal(MANAGED_SANDBOX_SUPERVISION_OUTCOME_SCHEMA_VERSION),
  commandRef: SandboxRef,
  idempotencyRef: SandboxRef,
  state: S.Literals(["applied", "refused", "failed", "pending"]),
  reasonRef: S.NullOr(SandboxRef),
  receiptRefs: BoundedRefs,
  projection: S.NullOr(ManagedSandboxSupervisionProjectionSchema),
  observedAt: SandboxTimestamp,
});
export type ManagedSandboxSupervisionOutcome = typeof ManagedSandboxSupervisionOutcomeSchema.Type;

export const decodeManagedSandboxSupervisionProjection = S.decodeUnknownSync(
  ManagedSandboxSupervisionProjectionSchema,
  {
    onExcessProperty: "error",
  },
);
export const decodeManagedSandboxSupervisionEnvelope = S.decodeUnknownSync(
  ManagedSandboxSupervisionEnvelopeSchema,
  {
    onExcessProperty: "error",
  },
);
export const decodeManagedSandboxSupervisionCommand = S.decodeUnknownSync(
  ManagedSandboxSupervisionCommandSchema,
  {
    onExcessProperty: "error",
  },
);
export const decodeManagedSandboxSupervisionOutcome = S.decodeUnknownSync(
  ManagedSandboxSupervisionOutcomeSchema,
  {
    onExcessProperty: "error",
  },
);
