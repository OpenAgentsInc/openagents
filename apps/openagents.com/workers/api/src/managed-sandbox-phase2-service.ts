import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  type ManagedSandboxCheckpointDeleteReceipt,
  type ManagedSandboxCheckpointStopOutcome,
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxForkReceipt,
  type ManagedSandboxPhase2Command,
  type ManagedSandboxPhase2Error,
  type ManagedSandboxRestoreReceipt,
  decodeManagedSandboxCheckpointDeleteReceipt,
  decodeManagedSandboxCheckpointStopOutcome,
  decodeManagedSandboxContentCheckpoint,
  decodeManagedSandboxForkReceipt,
  decodeManagedSandboxPhase2Command,
  decodeManagedSandboxRestoreReceipt,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";

export type ManagedSandboxPhase2ExecutionResult =
  | ManagedSandboxContentCheckpoint
  | ManagedSandboxCheckpointStopOutcome
  | ManagedSandboxCheckpointDeleteReceipt
  | ManagedSandboxForkReceipt
  | ManagedSandboxRestoreReceipt;

export type ManagedSandboxPhase2Operation = Readonly<{
  command: ManagedSandboxPhase2Command;
  result: ManagedSandboxPhase2ExecutionResult;
}>;

export type ManagedSandboxPhase2CheckpointMutation =
  | Readonly<{ _tag: "Put"; checkpoint: ManagedSandboxContentCheckpoint }>
  | Readonly<{ _tag: "Delete"; checkpointRef: string }>
  | Readonly<{ _tag: "None" }>;

/** Durable Phase 2 state. `settle` commits the replay row and metadata mutation atomically. */
export type ManagedSandboxPhase2Store = Readonly<{
  lookupOperation: (input: {
    ownerRef: string;
    tenantRef: string;
    commandRef: string;
    idempotencyRef: string;
  }) => Effect.Effect<ManagedSandboxPhase2Operation | undefined, ManagedSandboxPhase2Error>;
  readCheckpoint: (input: {
    ownerRef: string;
    tenantRef: string;
    checkpointRef: string;
  }) => Effect.Effect<ManagedSandboxContentCheckpoint | undefined, ManagedSandboxPhase2Error>;
  settle: (input: {
    operation: ManagedSandboxPhase2Operation;
    checkpointMutation: ManagedSandboxPhase2CheckpointMutation;
  }) => Effect.Effect<void, ManagedSandboxPhase2Error>;
}>;

/** Provider effects are byte-idempotent under the command `idempotencyRef`. */
export type ManagedSandboxPhase2Target = Readonly<{
  createCheckpoint: (
    command: Extract<ManagedSandboxPhase2Command, { _tag: "CreateCheckpoint" }>,
  ) => Effect.Effect<unknown, ManagedSandboxPhase2Error>;
  archiveWithCheckpoint: (
    command: Extract<ManagedSandboxPhase2Command, { _tag: "ArchiveWithCheckpoint" }>,
  ) => Effect.Effect<unknown, ManagedSandboxPhase2Error>;
  verifyCheckpoint: (
    checkpoint: ManagedSandboxContentCheckpoint,
  ) => Effect.Effect<boolean, ManagedSandboxPhase2Error>;
  observeResourceGeneration: (input: {
    ownerRef: string;
    tenantRef: string;
    sandboxRef: string;
  }) => Effect.Effect<number, ManagedSandboxPhase2Error>;
  forkFromCheckpoint: (
    command: Extract<ManagedSandboxPhase2Command, { _tag: "ForkFromCheckpoint" }>,
    checkpoint: ManagedSandboxContentCheckpoint,
  ) => Effect.Effect<unknown, ManagedSandboxPhase2Error>;
  restoreCheckpoint: (
    command: Extract<ManagedSandboxPhase2Command, { _tag: "RestoreCheckpoint" }>,
    checkpoint: ManagedSandboxContentCheckpoint,
  ) => Effect.Effect<unknown, ManagedSandboxPhase2Error>;
  deleteCheckpoint: (
    command: Extract<ManagedSandboxPhase2Command, { _tag: "DeleteCheckpoint" }>,
    checkpoint: ManagedSandboxContentCheckpoint,
  ) => Effect.Effect<unknown, ManagedSandboxPhase2Error>;
}>;

const noEvidence: ReadonlyArray<string> = [];

const invalidRequest = (requestRef: string, message: string): ManagedSandboxPhase2Error => ({
  _tag: "InvalidRequest",
  requestRef,
  message,
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

const incomplete = (checkpointRef: string, message: string): ManagedSandboxPhase2Error => ({
  _tag: "CheckpointIncomplete",
  checkpointRef,
  message,
  retryable: false,
  evidenceRefs: noEvidence,
});

const corrupt = (checkpointRef: string, message: string): ManagedSandboxPhase2Error => ({
  _tag: "CheckpointCorrupt",
  checkpointRef,
  message,
  retryable: false,
  evidenceRefs: noEvidence,
});

const expired = (checkpointRef: string): ManagedSandboxPhase2Error => ({
  _tag: "CheckpointExpired",
  checkpointRef,
  message: "the checkpoint retention period ended",
  retryable: false,
  evidenceRefs: noEvidence,
});

const unavailableIngress = (): ManagedSandboxPhase2Error => ({
  _tag: "PrivateIngressUnavailable",
  reasonRef: "security_proof_pending",
  message: "private ingress is unavailable until its security proof passes",
  retryable: false,
  evidenceRefs: noEvidence,
});

const decodeCommand = (value: unknown) =>
  Effect.try({
    try: () => decodeManagedSandboxPhase2Command(value),
    catch: () => invalidRequest("request.phase2.invalid", "Phase 2 command validation failed"),
  });

const decodeTarget = <A>(
  decode: (value: unknown) => A,
  value: unknown,
  commandRef: string,
  message: string,
): Effect.Effect<A, ManagedSandboxPhase2Error> =>
  Effect.try({
    try: () => decode(value),
    catch: () => invalidRequest(commandRef, message),
  });

const sameRefs = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean =>
  left.length === right.length && left.every((value, index) => value === right[index]);

const checkpointMatchesCommand = (
  checkpoint: ManagedSandboxContentCheckpoint,
  command: Extract<
    ManagedSandboxPhase2Command,
    { _tag: "CreateCheckpoint" | "ArchiveWithCheckpoint" }
  >,
): boolean =>
  checkpoint.checkpointRef === command.checkpointRef &&
  checkpoint.ownerRef === command.ownerRef &&
  checkpoint.tenantRef === command.tenantRef &&
  checkpoint.sourceSandboxRef === command.sourceSandboxRef &&
  checkpoint.sourceResourceGeneration === command.sourceResourceGeneration &&
  checkpoint.sourceImageDigest === command.sourceImageDigest &&
  checkpoint.sourceToolchainDigest === command.sourceToolchainDigest &&
  checkpoint.repositoryRef === command.repositoryRef &&
  checkpoint.repositoryRevisionRef === command.repositoryRevisionRef &&
  checkpoint.repositoryPostImageDigest === command.repositoryPostImageDigest &&
  checkpoint.formatRef === command.formatRef &&
  checkpoint.retainedUntil === command.retainedUntil;

const assertCheckpointCommand = (
  checkpoint: ManagedSandboxContentCheckpoint,
  command: Extract<
    ManagedSandboxPhase2Command,
    { _tag: "CreateCheckpoint" | "ArchiveWithCheckpoint" }
  >,
): Effect.Effect<void, ManagedSandboxPhase2Error> =>
  checkpointMatchesCommand(checkpoint, command)
    ? Effect.void
    : Effect.fail(incomplete(command.checkpointRef, "checkpoint truth does not bind the command"));

const settle = (
  store: ManagedSandboxPhase2Store,
  command: ManagedSandboxPhase2Command,
  result: ManagedSandboxPhase2ExecutionResult,
  checkpointMutation: ManagedSandboxPhase2CheckpointMutation,
) =>
  store
    .settle({
      operation: { command, result },
      checkpointMutation,
    })
    .pipe(Effect.as(result));

const loadCurrentCheckpoint = (
  store: ManagedSandboxPhase2Store,
  command: Extract<
    ManagedSandboxPhase2Command,
    { _tag: "ForkFromCheckpoint" | "RestoreCheckpoint" | "DeleteCheckpoint" }
  >,
  now: Date,
  permitExpired: boolean,
): Effect.Effect<ManagedSandboxContentCheckpoint, ManagedSandboxPhase2Error> =>
  Effect.gen(function* () {
    const checkpoint = yield* store.readCheckpoint({
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      checkpointRef: command.checkpointRef,
    });
    if (checkpoint === undefined) {
      return yield* Effect.fail(
        incomplete(command.checkpointRef, "the completed checkpoint does not exist"),
      );
    }
    if (
      checkpoint.ownerRef !== command.ownerRef ||
      checkpoint.tenantRef !== command.tenantRef ||
      checkpoint.checkpointRef !== command.checkpointRef
    ) {
      return yield* Effect.fail(
        incomplete(command.checkpointRef, "the checkpoint is outside the owner scope"),
      );
    }
    if (!permitExpired && Date.parse(checkpoint.retainedUntil) <= now.getTime()) {
      return yield* Effect.fail(expired(command.checkpointRef));
    }
    return checkpoint;
  });

export const makeManagedSandboxPhase2Service = (input: {
  store: ManagedSandboxPhase2Store;
  target: ManagedSandboxPhase2Target;
  now?: () => Date;
}) => {
  const now = input.now ?? (() => new Date());

  const executeDecoded = Effect.fn("ManagedSandboxPhase2Service.executeDecoded")(function* (
    command: ManagedSandboxPhase2Command,
  ) {
    const replay = yield* input.store.lookupOperation({
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      commandRef: command.commandRef,
      idempotencyRef: command.idempotencyRef,
    });
    if (replay !== undefined) {
      if (canonicalJson(replay.command) !== canonicalJson(command)) {
        return yield* Effect.fail(idempotencyConflict(command.idempotencyRef));
      }
      return replay.result;
    }

    switch (command["_tag"]) {
      case "CreateCheckpoint": {
        if (Date.parse(command.retainedUntil) <= Date.parse(command.requestedAt)) {
          return yield* Effect.fail(
            invalidRequest(command.commandRef, "checkpoint retention must end after the request"),
          );
        }
        const raw = yield* input.target.createCheckpoint(command);
        const checkpoint = yield* decodeTarget(
          decodeManagedSandboxContentCheckpoint,
          raw,
          command.commandRef,
          "checkpoint target response failed validation",
        );
        yield* assertCheckpointCommand(checkpoint, command);
        if (!(yield* input.target.verifyCheckpoint(checkpoint))) {
          return yield* Effect.fail(
            corrupt(checkpoint.checkpointRef, "checkpoint integrity verification failed"),
          );
        }
        return yield* settle(input.store, command, checkpoint, {
          _tag: "Put",
          checkpoint,
        });
      }
      case "ArchiveWithCheckpoint": {
        if (Date.parse(command.retainedUntil) <= Date.parse(command.requestedAt)) {
          return yield* Effect.fail(
            invalidRequest(command.commandRef, "checkpoint retention must end after the request"),
          );
        }
        const raw = yield* input.target.archiveWithCheckpoint(command);
        const outcome = yield* decodeTarget(
          decodeManagedSandboxCheckpointStopOutcome,
          raw,
          command.commandRef,
          "checkpoint stop response failed validation",
        );
        if (
          outcome.stopRef !== command.stopRef ||
          outcome.sandboxRef !== command.sourceSandboxRef ||
          outcome.resourceGeneration !== command.sourceResourceGeneration
        ) {
          return yield* Effect.fail(
            invalidRequest(
              command.commandRef,
              "checkpoint stop response does not bind the command",
            ),
          );
        }
        if (outcome["_tag"] === "CheckpointFailed") {
          if (outcome.attemptedCheckpointRef !== command.checkpointRef) {
            return yield* Effect.fail(
              invalidRequest(
                command.commandRef,
                "checkpoint failure does not bind the attempted checkpoint",
              ),
            );
          }
          return yield* settle(input.store, command, outcome, { _tag: "None" });
        }
        yield* assertCheckpointCommand(outcome.checkpoint, command);
        if (!(yield* input.target.verifyCheckpoint(outcome.checkpoint))) {
          return yield* Effect.fail(
            corrupt(
              outcome.checkpoint.checkpointRef,
              "archived checkpoint integrity verification failed",
            ),
          );
        }
        return yield* settle(input.store, command, outcome, {
          _tag: "Put",
          checkpoint: outcome.checkpoint,
        });
      }
      case "ForkFromCheckpoint": {
        const checkpoint = yield* loadCurrentCheckpoint(input.store, command, now(), false);
        if (
          checkpoint.sourceSandboxRef !== command.expectedSourceSandboxRef ||
          checkpoint.sourceResourceGeneration !== command.expectedSourceResourceGeneration
        ) {
          return yield* Effect.fail(
            incomplete(
              checkpoint.checkpointRef,
              "checkpoint source does not match the requested fork source",
            ),
          );
        }
        if (!(yield* input.target.verifyCheckpoint(checkpoint))) {
          return yield* Effect.fail(
            corrupt(checkpoint.checkpointRef, "fork checkpoint integrity verification failed"),
          );
        }
        const observedGeneration = yield* input.target.observeResourceGeneration({
          ownerRef: command.ownerRef,
          tenantRef: command.tenantRef,
          sandboxRef: checkpoint.sourceSandboxRef,
        });
        if (observedGeneration !== checkpoint.sourceResourceGeneration) {
          return yield* Effect.fail({
            _tag: "StaleSource" as const,
            sourceSandboxRef: checkpoint.sourceSandboxRef,
            expectedGeneration: checkpoint.sourceResourceGeneration,
            receivedGeneration: observedGeneration,
            message: "the source generation changed after the checkpoint",
            retryable: false,
            evidenceRefs: noEvidence,
          });
        }
        const raw = yield* input.target.forkFromCheckpoint(command, checkpoint);
        const receipt = yield* decodeTarget(
          decodeManagedSandboxForkReceipt,
          raw,
          command.commandRef,
          "fork target response failed validation",
        );
        if (
          receipt.ownerRef !== command.ownerRef ||
          receipt.tenantRef !== command.tenantRef ||
          receipt.checkpointRef !== checkpoint.checkpointRef ||
          receipt.sourceSandboxRef !== checkpoint.sourceSandboxRef ||
          receipt.sourceResourceGeneration !== checkpoint.sourceResourceGeneration ||
          !sameRefs(receipt.sourceCapabilityRefs, command.sourceCapabilityRefs)
        ) {
          return yield* Effect.fail(
            invalidRequest(
              command.commandRef,
              "fork receipt does not bind the checkpoint and command",
            ),
          );
        }
        return yield* settle(input.store, command, receipt, { _tag: "None" });
      }
      case "RestoreCheckpoint": {
        const checkpoint = yield* loadCurrentCheckpoint(input.store, command, now(), false);
        if (checkpoint.sourceResourceGeneration !== command.expectedSourceResourceGeneration) {
          return yield* Effect.fail(
            incomplete(
              checkpoint.checkpointRef,
              "checkpoint generation does not match the restore command",
            ),
          );
        }
        if (!(yield* input.target.verifyCheckpoint(checkpoint))) {
          return yield* Effect.fail(
            corrupt(checkpoint.checkpointRef, "restore checkpoint integrity verification failed"),
          );
        }
        const raw = yield* input.target.restoreCheckpoint(command, checkpoint);
        const receipt = yield* decodeTarget(
          decodeManagedSandboxRestoreReceipt,
          raw,
          command.commandRef,
          "restore target response failed validation",
        );
        if (
          receipt.ownerRef !== command.ownerRef ||
          receipt.tenantRef !== command.tenantRef ||
          receipt.checkpointRef !== checkpoint.checkpointRef ||
          receipt.sandboxRef !== command.destinationSandboxRef ||
          receipt.checkpointSourceGeneration !== checkpoint.sourceResourceGeneration ||
          !sameRefs(receipt.admittedServiceRefs, command.admittedServiceRefs) ||
          !sameRefs(receipt.sourceCapabilityRefs, command.sourceCapabilityRefs)
        ) {
          return yield* Effect.fail(
            invalidRequest(
              command.commandRef,
              "restore receipt does not bind the checkpoint and command",
            ),
          );
        }
        return yield* settle(input.store, command, receipt, { _tag: "None" });
      }
      case "DeleteCheckpoint": {
        const checkpoint = yield* loadCurrentCheckpoint(input.store, command, now(), true);
        const raw = yield* input.target.deleteCheckpoint(command, checkpoint);
        const receipt = yield* decodeTarget(
          decodeManagedSandboxCheckpointDeleteReceipt,
          raw,
          command.commandRef,
          "checkpoint delete response failed validation",
        );
        if (
          receipt.ownerRef !== command.ownerRef ||
          receipt.tenantRef !== command.tenantRef ||
          receipt.checkpointRef !== checkpoint.checkpointRef ||
          receipt.sourceSandboxRef !== checkpoint.sourceSandboxRef ||
          receipt.sourceResourceGeneration !== checkpoint.sourceResourceGeneration ||
          receipt.contentDigest !== checkpoint.contentDigest ||
          receipt.reason !== command.reason
        ) {
          return yield* Effect.fail(
            invalidRequest(
              command.commandRef,
              "checkpoint delete receipt does not bind the checkpoint and command",
            ),
          );
        }
        return yield* settle(input.store, command, receipt, {
          _tag: "Delete",
          checkpointRef: checkpoint.checkpointRef,
        });
      }
      case "CreatePrivateIngress":
        return yield* Effect.fail(unavailableIngress());
    }
  });

  const execute = Effect.fn("ManagedSandboxPhase2Service.execute")((command: unknown) =>
    decodeCommand(command).pipe(Effect.flatMap(executeDecoded)),
  );

  return {
    execute,
  } as const;
};
