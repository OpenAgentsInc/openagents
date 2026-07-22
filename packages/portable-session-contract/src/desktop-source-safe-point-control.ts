import { Schema } from "effect";

export const DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION =
  "openagents.desktop_source_safe_point_control.v1" as const;

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const PortableTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);
const PositiveInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));
const PublicSafeRefs = Schema.Array(PortableRef).check(Schema.isMaxLength(256));

/**
 * The private, owner-local discovery record. The bearer is process-scoped and
 * is never part of a portable operation request or durable server record.
 */
export const DesktopSourceSafePointRendezvousSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION),
  desktopInstanceRef: PortableRef,
  pylonRef: PortableRef,
  url: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512)),
  bearerToken: Schema.String.check(
    Schema.isMinLength(32),
    Schema.isMaxLength(256),
    Schema.isPattern(/^[a-f0-9]+$/),
  ),
  issuedAt: PortableTimestamp,
}).annotate({ identifier: "DesktopSourceSafePointRendezvous" });
export type DesktopSourceSafePointRendezvous = typeof DesktopSourceSafePointRendezvousSchema.Type;

/** Refs-only request from one exact claimed portable command execution. */
export const DesktopSourceSafePointControlRequestSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION),
  operationRef: PortableRef,
  commandRef: PortableRef,
  commandExecutionClaimRef: PortableRef,
  ownerRef: PortableRef,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  expiresAt: PortableTimestamp,
}).annotate({ identifier: "DesktopSourceSafePointControlRequest" });
export type DesktopSourceSafePointControlRequest =
  typeof DesktopSourceSafePointControlRequestSchema.Type;

export const DesktopSourceSafePointControlResponseSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION),
  desktopInstanceRef: PortableRef,
  operationRef: PortableRef,
  state: Schema.Literals(["quiescent", "not_quiescent", "refused"]),
  reasonRef: Schema.NullOr(PortableRef),
  evidenceRefs: PublicSafeRefs,
  remoteExecution: Schema.Literal("not_claimed"),
}).annotate({ identifier: "DesktopSourceSafePointControlResponse" });
export type DesktopSourceSafePointControlResponse =
  typeof DesktopSourceSafePointControlResponseSchema.Type;
