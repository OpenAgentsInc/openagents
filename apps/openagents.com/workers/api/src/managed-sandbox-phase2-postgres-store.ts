import {
  ManagedSandboxStoreError,
  type PostgresManagedSandboxPhase2Store,
} from "@openagentsinc/khala-sync-server";
import type { ManagedSandboxPhase2Error } from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";

import type {
  ManagedSandboxPhase2CheckpointMutation,
  ManagedSandboxPhase2Operation,
  ManagedSandboxPhase2Store,
} from "./managed-sandbox-phase2-service";

type StoreClient = Pick<
  PostgresManagedSandboxPhase2Store,
  "lookupOperation" | "readCheckpoint" | "readPrivateIngress" | "settle"
>;

type ErrorContext = Readonly<{
  requestRef: string;
  idempotencyRef?: string;
  checkpointRef?: string;
}>;

const noEvidence: ReadonlyArray<string> = [];

const invalidRequest = (
  requestRef: string,
  message: string,
  retryable = false,
): ManagedSandboxPhase2Error => ({
  _tag: "InvalidRequest",
  requestRef,
  message,
  retryable,
  evidenceRefs: noEvidence,
});

const checkpointIncomplete = (checkpointRef: string): ManagedSandboxPhase2Error => ({
  _tag: "CheckpointIncomplete",
  checkpointRef,
  message: "the completed checkpoint does not exist",
  retryable: false,
  evidenceRefs: noEvidence,
});

const checkpointCorrupt = (checkpointRef: string): ManagedSandboxPhase2Error => ({
  _tag: "CheckpointCorrupt",
  checkpointRef,
  message: "the stored checkpoint integrity check failed",
  retryable: false,
  evidenceRefs: noEvidence,
});

const idempotencyConflict = (idempotencyRef: string): ManagedSandboxPhase2Error => ({
  _tag: "IdempotencyConflict",
  idempotencyRef,
  message: "the command or idempotency reference is bound to different request bytes",
  retryable: false,
  evidenceRefs: noEvidence,
});

const storeFailure = (error: unknown, context: ErrorContext): ManagedSandboxPhase2Error => {
  if (!(error instanceof ManagedSandboxStoreError)) {
    return invalidRequest(context.requestRef, "Phase 2 storage is unavailable", true);
  }

  switch (error.code) {
    case "idempotency_conflict":
    case "command_conflict":
      return context.idempotencyRef === undefined
        ? invalidRequest(context.requestRef, "Phase 2 operation bytes conflict")
        : idempotencyConflict(context.idempotencyRef);
    case "not_found":
      return context.checkpointRef === undefined
        ? invalidRequest(context.requestRef, "Phase 2 state does not exist")
        : checkpointIncomplete(context.checkpointRef);
    case "corrupt_store":
      return context.checkpointRef === undefined
        ? invalidRequest(context.requestRef, "Phase 2 storage integrity check failed")
        : checkpointCorrupt(context.checkpointRef);
    case "permission_denied":
      return invalidRequest(context.requestRef, "Phase 2 scope does not match");
    case "invalid":
    case "unsafe_value":
      return invalidRequest(context.requestRef, "Phase 2 persistence value is invalid");
    case "stale_version":
    case "stale_generation":
    case "invalid_transition":
    case "event_conflict":
    case "cursor_conflict":
      return invalidRequest(context.requestRef, "Phase 2 persisted state conflicts");
  }
};

/** Adapt durable Promise operations into the closed Effect coordinator store port. */
export const makeManagedSandboxPhase2PostgresStore = (
  client: StoreClient,
): ManagedSandboxPhase2Store => {
  const lookupOperation = Effect.fn("ManagedSandboxPhase2PostgresStore.lookupOperation")(
    (input: { ownerRef: string; tenantRef: string; commandRef: string; idempotencyRef: string }) =>
      Effect.tryPromise({
        try: () => client.lookupOperation(input),
        catch: (error) =>
          storeFailure(error, {
            requestRef: input.commandRef,
            idempotencyRef: input.idempotencyRef,
          }),
      }),
  );

  const readCheckpoint = Effect.fn("ManagedSandboxPhase2PostgresStore.readCheckpoint")(
    (input: { ownerRef: string; tenantRef: string; checkpointRef: string }) =>
      Effect.tryPromise({
        try: () => client.readCheckpoint(input),
        catch: (error) =>
          storeFailure(error, {
            requestRef: input.checkpointRef,
            checkpointRef: input.checkpointRef,
          }),
      }),
  );

  const settle = Effect.fn("ManagedSandboxPhase2PostgresStore.settle")(
    (input: {
      operation: ManagedSandboxPhase2Operation;
      checkpointMutation: ManagedSandboxPhase2CheckpointMutation;
    }) =>
      Effect.tryPromise({
        try: () => client.settle(input),
        catch: (error) => {
          const checkpointRef =
            input.checkpointMutation["_tag"] === "None"
              ? undefined
              : input.checkpointMutation["_tag"] === "PutIngress"
                ? undefined
              : input.checkpointMutation["_tag"] === "Put"
                ? input.checkpointMutation.checkpoint.checkpointRef
                : input.checkpointMutation.checkpointRef;
          return storeFailure(error, {
            requestRef: input.operation.command.commandRef,
            idempotencyRef: input.operation.command.idempotencyRef,
            ...(checkpointRef === undefined ? {} : { checkpointRef }),
          });
        },
      }).pipe(Effect.asVoid),
  );

  const readPrivateIngress = Effect.fn("ManagedSandboxPhase2PostgresStore.readPrivateIngress")(
    (input: { ownerRef: string; tenantRef: string; capabilityRef: string }) =>
      Effect.tryPromise({
        try: () => client.readPrivateIngress(input),
        catch: (error) => storeFailure(error, { requestRef: input.capabilityRef }),
      }),
  );

  return { lookupOperation, readCheckpoint, readPrivateIngress, settle };
};
