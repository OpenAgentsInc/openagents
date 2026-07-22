import { Schema } from "effect";

import { ExecutionEnvironmentRef, PortableRef, PortableTimestamp } from "./primitives.js";

const refs = (maximum: number) => Schema.Array(PortableRef).check(Schema.isMaxLength(maximum));

/**
 * Public-safe authority for one owner-managed execution environment.
 * The key reference identifies owner-held material. Key bytes never enter this
 * contract, Sync, or the OpenAgents checkpoint service.
 */
export const OwnerManagedEnvironmentEnrollmentSchema = Schema.Struct({
  schema: Schema.Literal("openagents.owner_managed_environment_enrollment.v1"),
  enrollmentRef: PortableRef,
  ownerRef: PortableRef,
  targetRef: ExecutionEnvironmentRef,
  pylonRef: PortableRef,
  workerInstanceRef: PortableRef,
  targetClass: Schema.Literal("owner_managed"),
  adapterRef: PortableRef,
  compatibilityRef: PortableRef,
  isolation: Schema.Literals(["owner_host_process", "owner_host_container"]),
  dataPosture: Schema.Literal("owner_managed_region"),
  custodyPolicy: Schema.Literal("owner_held_key"),
  checkpointKeyRef: PortableRef,
  regionRef: PortableRef,
  networkDestinationRefs: refs(32),
  dataDestinationRefs: refs(32),
  retentionSeconds: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
    Schema.isLessThanOrEqualTo(31_536_000),
  ),
  costPolicyRef: PortableRef,
  generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  revision: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  state: Schema.Literals(["active", "revoked"]),
  health: Schema.Literals(["ready", "draining", "offline", "revoked"]),
  evidenceRefs: refs(64),
  observedAt: PortableTimestamp,
  expiresAt: PortableTimestamp,
  revokedAt: Schema.NullOr(PortableTimestamp),
}).annotate({ identifier: "OwnerManagedEnvironmentEnrollment" });

export interface OwnerManagedEnvironmentEnrollment extends Schema.Schema.Type<
  typeof OwnerManagedEnvironmentEnrollmentSchema
> {}

export const OwnerManagedEnvironmentEnrollmentRequestSchema = Schema.Struct({
  schema: Schema.Literal("openagents.owner_managed_environment_enrollment.request.v1"),
  workerInstanceRef: OwnerManagedEnvironmentEnrollmentSchema.fields.workerInstanceRef,
  adapterRef: OwnerManagedEnvironmentEnrollmentSchema.fields.adapterRef,
  compatibilityRef: OwnerManagedEnvironmentEnrollmentSchema.fields.compatibilityRef,
  isolation: OwnerManagedEnvironmentEnrollmentSchema.fields.isolation,
  checkpointKeyRef: OwnerManagedEnvironmentEnrollmentSchema.fields.checkpointKeyRef,
  regionRef: OwnerManagedEnvironmentEnrollmentSchema.fields.regionRef,
  networkDestinationRefs: OwnerManagedEnvironmentEnrollmentSchema.fields.networkDestinationRefs,
  dataDestinationRefs: OwnerManagedEnvironmentEnrollmentSchema.fields.dataDestinationRefs,
  retentionSeconds: OwnerManagedEnvironmentEnrollmentSchema.fields.retentionSeconds,
  costPolicyRef: OwnerManagedEnvironmentEnrollmentSchema.fields.costPolicyRef,
  generation: OwnerManagedEnvironmentEnrollmentSchema.fields.generation,
  health: Schema.Literals(["ready", "draining"]),
  evidenceRefs: OwnerManagedEnvironmentEnrollmentSchema.fields.evidenceRefs,
  expectedRevision: Schema.optionalKey(
    Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  ),
}).annotate({ identifier: "OwnerManagedEnvironmentEnrollmentRequest" });

export interface OwnerManagedEnvironmentEnrollmentRequest extends Schema.Schema.Type<
  typeof OwnerManagedEnvironmentEnrollmentRequestSchema
> {}
