import { Context, Effect, Schema as S } from "effect";

import type { CommandReceipt, QuarantinedCommand, QueuedCommand } from "./model.ts";

export class OutboxStorageError extends S.TaggedErrorClass<OutboxStorageError>()("OutboxStorageError", {
  action: S.String,
  error: S.Defect(),
}) {}

export class OutboxTransportError extends S.TaggedErrorClass<OutboxTransportError>()("OutboxTransportError", {
  detail: S.String,
  retryable: S.Boolean,
}) {}

export type TransportResult =
  | Readonly<{ status: "accepted" | "duplicate"; receiptRef: string }>
  | Readonly<{ status: "rejected"; receiptRef: string; code: string; detail: string }>;

export interface DeliveryGate {
  readonly convexConnected: boolean;
  readonly shellLive: boolean;
  readonly decisionRevisions: Readonly<Record<string, string>>;
}

export class ClientOutboxStore extends Context.Service<
  ClientOutboxStore,
  {
    readonly put: (command: QueuedCommand) => Effect.Effect<void, OutboxStorageError>;
    readonly list: () => Effect.Effect<ReadonlyArray<QueuedCommand>, OutboxStorageError>;
    readonly remove: (commandId: string) => Effect.Effect<void, OutboxStorageError>;
    readonly recordReceipt: (receipt: CommandReceipt) => Effect.Effect<void, OutboxStorageError>;
    readonly quarantine: (entry: QuarantinedCommand) => Effect.Effect<void, OutboxStorageError>;
  }
>()("@openagentsinc/client-command-outbox/ClientOutboxStore") {}

export class ClientCommandTransport extends Context.Service<
  ClientCommandTransport,
  {
    readonly send: (command: QueuedCommand) => Effect.Effect<TransportResult, OutboxTransportError>;
  }
>()("@openagentsinc/client-command-outbox/ClientCommandTransport") {}
