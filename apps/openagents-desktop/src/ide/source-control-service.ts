import { Context, Data, Effect, Layer, PubSub, Schema, Semaphore, Stream, SubscriptionRef } from "effect";

import {
  IdeSourceControlCommandSchema,
  IdeSourceControlFailureSchema,
  IdeSourceControlReceiptSchema,
  IdeSourceControlSnapshotSchema,
  type IdeSourceControlCommand,
  type IdeSourceControlFailure,
  type IdeSourceControlReceipt,
  type IdeSourceControlReceiptRef,
  type IdeSourceControlSnapshot,
  type IdeSourceControlVersion,
} from "./source-control-contract.ts";

export class IdeSourceControlServiceError extends Data.TaggedError(
  "IdeSourceControlServiceError",
)<{ readonly failure: IdeSourceControlFailure }> {}

export const IdeSourceControlEventSchema = Schema.TaggedUnion({
  Snapshot: { snapshot: IdeSourceControlSnapshotSchema },
  Receipt: { receipt: IdeSourceControlReceiptSchema },
  Failure: { failure: IdeSourceControlFailureSchema },
}).annotate({ identifier: "IdeSourceControlEvent" });
export type IdeSourceControlEvent = typeof IdeSourceControlEventSchema.Type;

export interface IdeSourceControlAdapterResult {
  readonly snapshot: IdeSourceControlSnapshot;
  readonly changedPaths: ReadonlyArray<string>;
  readonly conflictPaths: ReadonlyArray<string>;
  readonly omittedFacts: ReadonlyArray<string>;
  readonly recoveryRef: IdeSourceControlReceipt["recoveryRef"];
}

export interface IdeSourceControlAdapter {
  readonly refresh: (
    command: Extract<IdeSourceControlCommand, { readonly _tag: "Refresh" }>,
  ) => Effect.Effect<IdeSourceControlSnapshot, IdeSourceControlServiceError>;
  readonly execute: (
    command: Exclude<IdeSourceControlCommand, { readonly _tag: "Refresh" }>,
    current: IdeSourceControlSnapshot,
  ) => Effect.Effect<IdeSourceControlAdapterResult, IdeSourceControlServiceError>;
  readonly stop: (reason: string) => Effect.Effect<void>;
}

export interface IdeSourceControlServiceShape {
  readonly snapshot: () => Effect.Effect<IdeSourceControlSnapshot>;
  readonly execute: (
    command: IdeSourceControlCommand,
  ) => Effect.Effect<
    Readonly<{ snapshot: IdeSourceControlSnapshot; receipt: IdeSourceControlReceipt | null }>,
    IdeSourceControlServiceError
  >;
  readonly receipts: () => Effect.Effect<ReadonlyArray<IdeSourceControlReceipt>>;
  readonly events: Stream.Stream<IdeSourceControlEvent>;
  readonly stop: (reason: string) => Effect.Effect<IdeSourceControlSnapshot>;
}

export class IdeSourceControlService extends Context.Service<
  IdeSourceControlService,
  IdeSourceControlServiceShape
>()("@openagents/desktop/IdeSourceControlService") {}

export interface IdeSourceControlServiceOptions {
  readonly now?: () => string;
  readonly nextReceiptRef?: () => IdeSourceControlReceiptRef;
  readonly maximumReceipts?: number;
}

const sameBinding = (
  left: IdeSourceControlSnapshot["binding"],
  right: IdeSourceControlSnapshot["binding"],
): boolean =>
  left.projectRef === right.projectRef &&
  left.rootRef === right.rootRef &&
  left.worktreeRef === right.worktreeRef &&
  left.attachmentGeneration === right.attachmentGeneration &&
  left.repositoryRef === right.repositoryRef;

export const sameSourceControlVersion = (
  left: IdeSourceControlVersion,
  right: IdeSourceControlVersion,
): boolean =>
  left.repositoryGeneration === right.repositoryGeneration &&
  left.statusRef === right.statusRef &&
  left.headOid === right.headOid &&
  left.indexOid === right.indexOid &&
  left.worktreeOid === right.worktreeOid &&
  left.refGeneration === right.refGeneration &&
  left.configGeneration === right.configGeneration &&
  left.remoteGeneration === right.remoteGeneration &&
  left.credentialHelperGeneration === right.credentialHelperGeneration;

const failure = (
  code: IdeSourceControlFailure["code"],
  message: string,
  current: IdeSourceControlSnapshot | null,
  operationRef: IdeSourceControlFailure["operationRef"] = null,
  retryable = false,
): IdeSourceControlServiceError =>
  new IdeSourceControlServiceError({
    failure: IdeSourceControlFailureSchema.make({
      schemaVersion: "openagents.desktop.ide-source-control.v1",
      operationRef,
      code,
      message,
      currentVersion: current?.version ?? null,
      conflictPaths: current?.paths
        .filter((entry) => entry.worktreeState === "conflicted" || entry.indexState === "conflicted")
        .map((entry) => entry.path) ?? [],
      recoveryRef: null,
      retryable,
    }),
  });

const isReadOnly = (
  command: Exclude<IdeSourceControlCommand, { readonly _tag: "Refresh" }>,
): command is Extract<IdeSourceControlCommand, { readonly _tag: "History" | "Blame" | "ProviderRefresh" }> =>
  command._tag === "History" || command._tag === "Blame" || command._tag === "ProviderRefresh";

export const makeIdeSourceControlServiceLayer = (
  seed: IdeSourceControlSnapshot,
  adapter: IdeSourceControlAdapter,
  options: IdeSourceControlServiceOptions = {},
): Layer.Layer<IdeSourceControlService> =>
  Layer.effect(
    IdeSourceControlService,
    Effect.gen(function* () {
      const initial = IdeSourceControlSnapshotSchema.make(seed);
      const state = yield* SubscriptionRef.make(initial);
      const receiptState = yield* SubscriptionRef.make<ReadonlyArray<IdeSourceControlReceipt>>([]);
      const events = yield* PubSub.unbounded<IdeSourceControlEvent>();
      const operationLock = yield* Semaphore.make(1);
      const now = options.now ?? (() => new Date().toISOString());
      const maximumReceipts = Math.max(1, options.maximumReceipts ?? 2_000);
      let receiptSequence = 0;
      const nextReceiptRef = options.nextReceiptRef ?? (() => {
        receiptSequence += 1;
        return `ide.scm-receipt.local-${receiptSequence}` as IdeSourceControlReceiptRef;
      });

      const publish = Effect.fn("IdeSourceControl.publish")((event: IdeSourceControlEvent) =>
        PubSub.publish(events, event));

      const emitFailure = Effect.fn("IdeSourceControl.emitFailure")(function* (
        error: IdeSourceControlServiceError,
      ) {
        yield* publish(IdeSourceControlEventSchema.cases.Failure.make({ failure: error.failure }));
        return yield* Effect.fail(error);
      });

      const ensureActive = Effect.fn("IdeSourceControl.ensureActive")(function* (
        command: IdeSourceControlCommand,
      ) {
        const current = yield* SubscriptionRef.get(state);
        if (current.stopped) {
          return yield* emitFailure(failure(
            "stopped",
            "The source-control graph is stopped.",
            current,
            command._tag === "Refresh" ? null : command.operationRef,
          ));
        }
        return current;
      });

      const validateMutation = Effect.fn("IdeSourceControl.validateMutation")(function* (
        command: Exclude<IdeSourceControlCommand, { readonly _tag: "Refresh" }>,
        current: IdeSourceControlSnapshot,
      ) {
        if (!sameBinding(command.binding, current.binding)) {
          return yield* emitFailure(failure(
            "stale_version",
            "The command binding does not match the active project and worktree.",
            current,
            command.operationRef,
            true,
          ));
        }
        if (!isReadOnly(command) && !sameSourceControlVersion(command.expected, current.version)) {
          return yield* emitFailure(failure(
            "stale_version",
            "Repository state changed. Refresh and preview the operation again.",
            current,
            command.operationRef,
            true,
          ));
        }
        const destructiveForAgent = command.actor._tag === "Agent" && [
          "Discard", "Switch", "Merge", "Rebase", "CherryPick", "Revert",
          "Continue", "Abort", "Push", "WorktreeRemove",
        ].includes(command._tag);
        if (destructiveForAgent && command.approvalRef === null) {
          return yield* emitFailure(failure(
            "approval_required",
            "This agent operation requires an explicit approval reference.",
            current,
            command.operationRef,
          ));
        }
      });

      const executeUnlocked = Effect.fn("IdeSourceControl.executeUnlocked")(function* (
        raw: IdeSourceControlCommand,
      ) {
        const command = yield* Schema.decodeUnknownEffect(IdeSourceControlCommandSchema)(raw).pipe(
          Effect.mapError(() => failure(
            "invalid_command",
            "The source-control command is invalid.",
            null,
          )),
        );
        const current = yield* ensureActive(command);
        if (command._tag === "Refresh") {
          if (!sameBinding(command.binding, current.binding)) {
            return yield* emitFailure(failure(
              "stale_version",
              "The refresh binding does not match the active project and worktree.",
              current,
              null,
              true,
            ));
          }
          const snapshot = yield* adapter.refresh(command).pipe(
            Effect.tapError((error) => publish(
              IdeSourceControlEventSchema.cases.Failure.make({ failure: error.failure }),
            )),
          );
          const decoded = IdeSourceControlSnapshotSchema.make(snapshot);
          yield* SubscriptionRef.set(state, decoded);
          yield* publish(IdeSourceControlEventSchema.cases.Snapshot.make({ snapshot: decoded }));
          return { snapshot: decoded, receipt: null };
        }

        yield* validateMutation(command, current);
        const result = yield* adapter.execute(command, current).pipe(
          Effect.tapError((error) => publish(
            IdeSourceControlEventSchema.cases.Failure.make({ failure: error.failure }),
          )),
        );
        const postImage = IdeSourceControlSnapshotSchema.make(result.snapshot);
        if (!sameBinding(postImage.binding, current.binding)) {
          return yield* emitFailure(failure(
            "operation_failed",
            "The adapter returned a post-image for a different project or worktree.",
            current,
            command.operationRef,
          ));
        }
        if (postImage.version.repositoryGeneration <= current.version.repositoryGeneration) {
          return yield* emitFailure(failure(
            "operation_failed",
            "The adapter did not advance the repository generation.",
            current,
            command.operationRef,
          ));
        }

        const receipt = IdeSourceControlReceiptSchema.make({
          schemaVersion: "openagents.desktop.ide-source-control.v1",
          receiptRef: nextReceiptRef(),
          operationRef: command.operationRef,
          command: command._tag,
          binding: current.binding,
          preVersion: current.version,
          postVersion: postImage.version,
          postImage,
          changedPaths: [...result.changedPaths],
          conflictPaths: [...result.conflictPaths],
          omittedFacts: [...result.omittedFacts],
          recoveryRef: result.recoveryRef,
          deliveryFacts: postImage.delivery,
          actor: command.actor,
          approvalRef: command.approvalRef,
          completedAt: now(),
        });
        yield* SubscriptionRef.set(state, postImage);
        yield* SubscriptionRef.update(receiptState, (receipts) =>
          [...receipts, receipt].slice(-maximumReceipts));
        yield* publish(IdeSourceControlEventSchema.cases.Snapshot.make({ snapshot: postImage }));
        yield* publish(IdeSourceControlEventSchema.cases.Receipt.make({ receipt }));
        return { snapshot: postImage, receipt };
      });

      const execute = Effect.fn("IdeSourceControl.execute")((raw: IdeSourceControlCommand) =>
        operationLock.withPermit(executeUnlocked(raw)));

      const stop = Effect.fn("IdeSourceControl.stop")(function* (reason: string) {
        yield* adapter.stop(reason);
        const stopped = yield* SubscriptionRef.modify(state, (current) => {
          const next = IdeSourceControlSnapshotSchema.make({ ...current, stopped: true });
          return [next, next] as const;
        });
        yield* publish(IdeSourceControlEventSchema.cases.Snapshot.make({ snapshot: stopped }));
        yield* PubSub.shutdown(events);
        return stopped;
      });

      return IdeSourceControlService.of({
        snapshot: Effect.fn("IdeSourceControl.snapshot")(() => SubscriptionRef.get(state)),
        execute,
        receipts: Effect.fn("IdeSourceControl.receipts")(() => SubscriptionRef.get(receiptState)),
        events: Stream.fromPubSub(events),
        stop,
      });
    }),
  );
