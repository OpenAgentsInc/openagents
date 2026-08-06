import { Effect } from "effect";

import { canonicalCommandJson, commandFingerprint } from "./canonical.ts";
import {
  CommandId,
  CommandReceipt,
  OperationName,
  OrderingKey,
  QueuedCommand,
  type EnqueueInput,
} from "./model.ts";
import { classifyClientOperation } from "./policy.ts";
import { ClientCommandTransport, ClientOutboxStore, type DeliveryGate } from "./services.ts";

export class CommandPolicyError extends Error {
  constructor(readonly operation: string, message: string) {
    super(message);
    this.name = "CommandPolicyError";
  }
}

export const buildQueuedCommand = (input: EnqueueInput): QueuedCommand => {
  const commandClass = classifyClientOperation(input.operation);
  if (commandClass !== "durable_intent" && commandClass !== "expiring_decision") {
    throw new CommandPolicyError(input.operation, "Only durable intents and expiring decisions may enter the outbox.");
  }
  if (input.commandId.trim() === "" || input.orderingKey.trim() === "") {
    throw new CommandPolicyError(input.operation, "Command and ordering identifiers are required.");
  }
  if (
    commandClass === "expiring_decision" &&
    (input.decisionRevision === undefined || input.expiresAtMs === undefined)
  ) {
    throw new CommandPolicyError(input.operation, "Expiring decisions require a revision and expiry.");
  }

  const payloadJson = canonicalCommandJson(input.payload);
  const fingerprint = commandFingerprint({
    commandId: input.commandId,
    operation: input.operation,
    orderingKey: input.orderingKey,
    payload: JSON.parse(payloadJson) as unknown,
    decisionRevision: input.decisionRevision ?? null,
    expiresAtMs: input.expiresAtMs ?? null,
  });

  return new QueuedCommand({
    version: "openagents.client_command_outbox.v1",
    commandId: CommandId.make(input.commandId),
    fingerprint,
    operation: OperationName.make(input.operation),
    commandClass,
    orderingKey: OrderingKey.make(input.orderingKey),
    payloadJson,
    createdAtMs: input.createdAtMs,
    attempt: 0,
    decisionRevision: input.decisionRevision ?? null,
    expiresAtMs: input.expiresAtMs ?? null,
  });
};

export const enqueueClientCommand = Effect.fn("ClientOutbox.enqueue")(function* (input: EnqueueInput) {
  const store = yield* ClientOutboxStore;
  const command = buildQueuedCommand(input);
  yield* store.put(command);
  return command;
});

export interface DrainSummary {
  readonly delivered: number;
  readonly terminal: number;
  readonly pending: number;
}

const terminalReceipt = (
  command: QueuedCommand,
  nowMs: number,
  status: "rejected" | "expired",
  code: string,
  detail: string,
  receiptRef: string,
): CommandReceipt =>
  new CommandReceipt({
    commandId: command.commandId,
    fingerprint: command.fingerprint,
    operation: command.operation,
    status,
    receiptRef,
    code,
    detail,
    recordedAtMs: nowMs,
  });

export const drainClientOutbox = Effect.fn("ClientOutbox.drain")(function* (
  gate: DeliveryGate,
  nowMs: number,
) {
  const store = yield* ClientOutboxStore;
  const transport = yield* ClientCommandTransport;
  const queued = [...(yield* store.list())].sort(
    (left, right) => left.createdAtMs - right.createdAtMs || left.commandId.localeCompare(right.commandId),
  );

  if (!gate.convexConnected || !gate.shellLive) {
    return { delivered: 0, terminal: 0, pending: queued.length } satisfies DrainSummary;
  }

  const blockedOrderingKeys = new Set<string>();
  let delivered = 0;
  let terminal = 0;

  for (const command of queued) {
    if (blockedOrderingKeys.has(command.orderingKey)) continue;

    if (command.commandClass === "expiring_decision") {
      const currentRevision = gate.decisionRevisions[command.operation];
      const expired = command.expiresAtMs === null || nowMs >= command.expiresAtMs;
      const stale = currentRevision === undefined || currentRevision !== command.decisionRevision;
      if (expired || stale) {
        yield* store.recordReceipt(
          terminalReceipt(
            command,
            nowMs,
            "expired",
            "fresh_decision_required",
            "The decision request expired or changed; request a fresh prompt.",
            `receipt.client.expired.${command.commandId}`,
          ),
        );
        yield* store.remove(command.commandId);
        terminal += 1;
        continue;
      }
    }

    const result = yield* Effect.result(transport.send(command));
    if (result._tag === "Failure") {
      if (result.failure.retryable) {
        blockedOrderingKeys.add(command.orderingKey);
        continue;
      }
      yield* store.recordReceipt(
        terminalReceipt(
          command,
          nowMs,
          "rejected",
          "transport_rejected",
          result.failure.detail,
          `receipt.client.rejected.${command.commandId}`,
        ),
      );
      yield* store.remove(command.commandId);
      terminal += 1;
      continue;
    }

    const response = result.success;
    if (response.status === "rejected") {
      yield* store.recordReceipt(
        terminalReceipt(command, nowMs, "rejected", response.code, response.detail, response.receiptRef),
      );
      yield* store.remove(command.commandId);
      terminal += 1;
      continue;
    }

    yield* store.recordReceipt(
      new CommandReceipt({
        commandId: command.commandId,
        fingerprint: command.fingerprint,
        operation: command.operation,
        status: response.status,
        receiptRef: response.receiptRef,
        code: "delivered",
        detail: response.status === "duplicate" ? "Server replayed the original receipt." : "Command accepted.",
        recordedAtMs: nowMs,
      }),
    );
    yield* store.remove(command.commandId);
    delivered += 1;
  }

  const pending = (yield* store.list()).length;
  return { delivered, terminal, pending } satisfies DrainSummary;
});
