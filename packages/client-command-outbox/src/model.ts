import { Schema as S } from "effect";

export const CLIENT_COMMAND_OUTBOX_VERSION = "openagents.client_command_outbox.v1" as const;

export const CommandId = S.String.pipe(S.brand("ClientCommandId"));
export type CommandId = typeof CommandId.Type;

export const CommandFingerprint = S.String.pipe(
  S.check(S.isPattern(/^sha256:[a-f0-9]{64}$/u)),
  S.brand("ClientCommandFingerprint"),
);
export type CommandFingerprint = typeof CommandFingerprint.Type;

export const OperationName = S.String.pipe(S.brand("ClientOperationName"));
export type OperationName = typeof OperationName.Type;

export const OrderingKey = S.String.pipe(S.brand("ClientOrderingKey"));
export type OrderingKey = typeof OrderingKey.Type;

export const CommandClass = S.Literals([
  "durable_intent",
  "expiring_decision",
  "live_control",
  "destructive_git",
  "observation",
]);
export type CommandClass = typeof CommandClass.Type;

export const QueueableCommandClass = S.Literals(["durable_intent", "expiring_decision"]);
export type QueueableCommandClass = typeof QueueableCommandClass.Type;

export class QueuedCommand extends S.Class<QueuedCommand>("QueuedCommand")({
  version: S.Literal(CLIENT_COMMAND_OUTBOX_VERSION),
  commandId: CommandId,
  fingerprint: CommandFingerprint,
  operation: OperationName,
  commandClass: QueueableCommandClass,
  orderingKey: OrderingKey,
  payloadJson: S.String,
  createdAtMs: S.Int.pipe(S.check(S.isGreaterThanOrEqualTo(0))),
  attempt: S.Int.pipe(S.check(S.isGreaterThanOrEqualTo(0))),
  decisionRevision: S.NullOr(S.String),
  expiresAtMs: S.NullOr(S.Int.pipe(S.check(S.isGreaterThanOrEqualTo(0)))),
}) {}

export class CommandReceipt extends S.Class<CommandReceipt>("CommandReceipt")({
  commandId: CommandId,
  fingerprint: CommandFingerprint,
  operation: OperationName,
  status: S.Literals(["accepted", "duplicate", "rejected", "expired", "corrupt"]),
  receiptRef: S.String,
  code: S.String,
  detail: S.String,
  recordedAtMs: S.Int.pipe(S.check(S.isGreaterThanOrEqualTo(0))),
}) {}

export class QuarantinedCommand extends S.Class<QuarantinedCommand>("QuarantinedCommand")({
  quarantineRef: S.String,
  raw: S.String,
  reason: S.String,
  recordedAtMs: S.Int.pipe(S.check(S.isGreaterThanOrEqualTo(0))),
}) {}

export class ObservationCacheEntry extends S.Class<ObservationCacheEntry>("ObservationCacheEntry")({
  key: S.String,
  valueJson: S.String,
  observedAtMs: S.Int.pipe(S.check(S.isGreaterThanOrEqualTo(0))),
}) {}

export type ObservationPhase = "cached" | "synchronizing" | "live";

export interface ObservationProjection {
  readonly phase: ObservationPhase;
  readonly ageMs: number;
  readonly value: unknown;
}

export interface EnqueueInput {
  readonly commandId: string;
  readonly operation: string;
  readonly orderingKey: string;
  readonly payload: unknown;
  readonly createdAtMs: number;
  readonly decisionRevision?: string;
  readonly expiresAtMs?: number;
}

export interface DestructiveGitAuthorization {
  readonly preflightRef: string;
  readonly confirmationRef: string;
  readonly disclosureDigest: string;
}
