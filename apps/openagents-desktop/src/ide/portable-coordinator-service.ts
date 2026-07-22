import {
  Context,
  Effect,
  Layer,
  PubSub,
  Ref,
  Schema,
  Semaphore,
  Stream,
  SubscriptionRef,
} from "effect";

import {
  IdePortableAuthorizationFailure,
  IdePortableCancelled,
  IdePortableCheckpointFailure,
  IdePortableCoordinatorCommandSchema,
  IdePortableCoordinatorSnapshotSchema,
  IdePortableDestinationActivationReceiptSchema,
  IdePortableFailureSchema,
  IdePortableMoveReceiptSchema,
  IdePortablePlacementFailure,
  IdePortableStaleWriter,
  type IdePortableCheckpointManifest,
  type IdePortableCoordinatorCommand,
  type IdePortableCoordinatorSnapshot,
  type IdePortableDestinationActivationReceipt,
  type IdePortableDestinationHelperKind,
  type IdePortableFailure,
  type IdePortableMoveReceipt,
} from "@openagentsinc/portable-session-contract";

export class IdePortableCoordinatorError extends Schema.TaggedErrorClass<IdePortableCoordinatorError>()(
  "IdePortableCoordinatorError",
  { failure: IdePortableFailureSchema },
) {}

export type IdePortableCoordinatorEvent =
  | Readonly<{ _tag: "Snapshot"; snapshot: IdePortableCoordinatorSnapshot }>
  | Readonly<{ _tag: "Receipt"; receipt: IdePortableMoveReceipt }>
  | Readonly<{ _tag: "Failure"; failure: IdePortableFailure }>;

export interface IdePortableCoordinatorAdapter {
  readonly quiesceAndCheckpoint: (
    snapshot: IdePortableCoordinatorSnapshot,
    command: Extract<IdePortableCoordinatorCommand, { readonly _tag: "Move" | "Failback" }>,
  ) => Effect.Effect<IdePortableCheckpointManifest, IdePortableCoordinatorError>;
  readonly validateCheckpoint: (
    manifest: IdePortableCheckpointManifest,
    destinationPlacementRef: string,
    stage: "source" | "destination",
  ) => Effect.Effect<void, IdePortableCoordinatorError>;
  readonly stageDestination: (
    manifest: IdePortableCheckpointManifest,
    destinationPlacementRef: string,
    generation: number,
  ) => Effect.Effect<Readonly<{ attachmentRef: string }>, IdePortableCoordinatorError>;
  readonly revokeSource: (
    snapshot: IdePortableCoordinatorSnapshot,
    command: Extract<IdePortableCoordinatorCommand, { readonly _tag: "Move" | "Failback" }>,
  ) => Effect.Effect<void, IdePortableCoordinatorError>;
  readonly attachDestination: (
    manifest: IdePortableCheckpointManifest,
    destinationPlacementRef: string,
    attachmentRef: string,
    generation: number,
  ) => Effect.Effect<void, IdePortableCoordinatorError>;
  readonly restartFreshHelpers: (
    destinationPlacementRef: string,
    attachmentRef: string,
    generation: number,
  ) => Effect.Effect<IdePortableDestinationActivationReceipt, IdePortableCoordinatorError>;
  readonly rollbackDestination: (attachmentRef: string, reason: string) => Effect.Effect<void>;
  readonly resumeSource: (snapshot: IdePortableCoordinatorSnapshot) => Effect.Effect<void>;
  readonly stop: (reasonRef: string) => Effect.Effect<void, IdePortableCoordinatorError>;
}

export interface IdePortableCoordinatorShape {
  readonly snapshot: () => Effect.Effect<IdePortableCoordinatorSnapshot>;
  readonly execute: (
    command: IdePortableCoordinatorCommand,
  ) => Effect.Effect<
    Readonly<{ snapshot: IdePortableCoordinatorSnapshot; receipt: IdePortableMoveReceipt | null }>,
    IdePortableCoordinatorError
  >;
  readonly authorizeMutation: (
    binding: Readonly<{
      sessionRef: string;
      attachmentRef: string;
      generation: number;
    }>,
  ) => Effect.Effect<void, IdePortableCoordinatorError>;
  readonly receipts: () => Effect.Effect<ReadonlyArray<IdePortableMoveReceipt>>;
  readonly events: Stream.Stream<IdePortableCoordinatorEvent>;
}

export class IdePortableCoordinator extends Context.Service<
  IdePortableCoordinator,
  IdePortableCoordinatorShape
>()("@openagents/desktop/IdePortableCoordinator") {}

export interface IdePortableCoordinatorOptions {
  readonly now?: () => string;
  readonly nextReceiptRef?: () => string;
  readonly maximumReceipts?: number;
}

const error = (failure: IdePortableFailure): IdePortableCoordinatorError =>
  new IdePortableCoordinatorError({ failure });

const sameProject = (
  left: IdePortableCoordinatorSnapshot["project"],
  right: IdePortableCoordinatorSnapshot["project"],
): boolean =>
  left.projectRef === right.projectRef &&
  left.projectRootRef === right.projectRootRef &&
  left.worktreeRef === right.worktreeRef;

type MoveCommand = Extract<IdePortableCoordinatorCommand, { readonly _tag: "Move" | "Failback" }>;

interface CompletedMove {
  readonly command: MoveCommand;
  readonly receipt: IdePortableMoveReceipt;
}

type MoveCommandControl = "pending" | "cancelled" | "committed";

const sameMoveCommand = (left: MoveCommand, right: MoveCommand): boolean =>
  left._tag === right._tag &&
  left.commandRef === right.commandRef &&
  left.actorRef === right.actorRef &&
  left.policyRef === right.policyRef &&
  left.sessionRef === right.sessionRef &&
  sameProject(left.project, right.project) &&
  left.expectedAttachmentRef === right.expectedAttachmentRef &&
  left.expectedGeneration === right.expectedGeneration &&
  left.destinationPlacementRef === right.destinationPlacementRef &&
  (left._tag !== "Failback" ||
    (right._tag === "Failback" && left.recoveryPointRef === right.recoveryPointRef));

const requiredDestinationHelpers: ReadonlySet<IdePortableDestinationHelperKind> = new Set([
  "pty",
  "lsp",
  "dap",
  "watcher",
  "native",
]);

const activationMatchesMove = (
  activation: IdePortableDestinationActivationReceipt,
  manifest: IdePortableCheckpointManifest,
  destinationPlacementRef: string,
  destinationAttachmentRef: string,
  destinationGeneration: number,
): boolean => {
  const observedHelpers = new Set(activation.helpers.map(({ kind }) => kind));
  return (
    activation.sessionRef === manifest.sessionRef &&
    activation.checkpointRef === manifest.checkpointRef &&
    activation.destinationTargetRef === destinationPlacementRef &&
    activation.destinationAttachmentRef === destinationAttachmentRef &&
    activation.destinationGeneration === destinationGeneration &&
    activation.authentication.state === "reauthenticated" &&
    activation.acceptedWorkRefs.length === 0 &&
    observedHelpers.size === requiredDestinationHelpers.size &&
    [...requiredDestinationHelpers].every((kind) => observedHelpers.has(kind)) &&
    activation.helpers.every((helper) =>
      helper.readiness === "ready"
        ? helper.instanceRef !== null && helper.versionRef !== null
        : helper.omissionRef !== null,
    )
  );
};

export const makeIdePortableCoordinatorLayer = (
  seed: IdePortableCoordinatorSnapshot,
  adapter: IdePortableCoordinatorAdapter,
  options: IdePortableCoordinatorOptions = {},
): Layer.Layer<IdePortableCoordinator> =>
  Layer.effect(
    IdePortableCoordinator,
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make(IdePortableCoordinatorSnapshotSchema.make(seed));
      const receipts = yield* Ref.make<ReadonlyArray<IdePortableMoveReceipt>>([]);
      const completedByIdempotencyKey = yield* Ref.make<ReadonlyMap<string, CompletedMove>>(
        new Map(),
      );
      const commandControls = yield* Ref.make<ReadonlyMap<string, MoveCommandControl>>(new Map());
      const events = yield* PubSub.unbounded<IdePortableCoordinatorEvent>();
      const lock = yield* Semaphore.make(1);
      const now = options.now ?? (() => new Date().toISOString());
      const maximumReceipts = Math.max(1, options.maximumReceipts ?? 2_000);
      let receiptSequence = 0;
      const nextReceiptRef =
        options.nextReceiptRef ??
        (() => {
          receiptSequence += 1;
          return `ide.portable-receipt.local-${receiptSequence}`;
        });

      const publishSnapshot = Effect.fn("IdePortableCoordinator.publishSnapshot")(function* (
        snapshot: IdePortableCoordinatorSnapshot,
      ) {
        yield* SubscriptionRef.set(state, snapshot);
        yield* PubSub.publish(events, { _tag: "Snapshot", snapshot } as const);
        return snapshot;
      });

      const fail = Effect.fn("IdePortableCoordinator.fail")(function* (
        failure: IdePortableFailure,
      ) {
        yield* PubSub.publish(events, { _tag: "Failure", failure } as const);
        return yield* Effect.fail(error(failure));
      });

      const ensureCurrent = Effect.fn("IdePortableCoordinator.ensureCurrent")(function* (
        command: IdePortableCoordinatorCommand,
      ) {
        const current = yield* SubscriptionRef.get(state);
        if (current.stopped) {
          return yield* fail(
            new IdePortablePlacementFailure({
              operation: command._tag,
              detailRef: "portable.coordinator.stopped",
              retryable: false,
            }),
          );
        }
        if (
          command.sessionRef !== current.sessionRef ||
          !sameProject(command.project, current.project)
        ) {
          return yield* fail(
            new IdePortableAuthorizationFailure({
              operation: command._tag,
              detailRef: "portable.binding.mismatch",
              retryable: false,
            }),
          );
        }
        if (
          command.expectedAttachmentRef !== current.activeAttachmentRef ||
          command.expectedGeneration !== current.activeGeneration
        ) {
          return yield* fail(
            new IdePortableStaleWriter({
              operation: command._tag,
              detailRef: "portable.generation.stale",
              retryable: true,
            }),
          );
        }
        if (command.deadlineAt <= now()) {
          return yield* fail(
            new IdePortableCancelled({
              operation: command._tag,
              detailRef: "portable.deadline.expired",
              retryable: true,
            }),
          );
        }
        return current;
      });

      const ensureNotCancelled = Effect.fn("IdePortableCoordinator.ensureNotCancelled")(function* (
        commandRef: string,
      ) {
        const controls = yield* Ref.get(commandControls);
        if (controls.get(commandRef) === "cancelled") {
          return yield* fail(
            new IdePortableCancelled({
              operation: "move",
              detailRef: "portable.command.cancelled",
              retryable: true,
            }),
          );
        }
      });

      const removeCancellation = Effect.fn("IdePortableCoordinator.removeCancellation")(function* (
        commandRef: string,
      ) {
        yield* Ref.update(commandControls, (values) => {
          const next = new Map(values);
          next.delete(commandRef);
          return next;
        });
      });

      const commitCutover = Effect.fn("IdePortableCoordinator.commitCutover")(function* (
        commandRef: string,
      ) {
        return yield* Ref.modify(commandControls, (values) => {
          if (values.get(commandRef) !== "pending") return [false, values] as const;
          const next = new Map(values);
          next.set(commandRef, "committed");
          return [true, next] as const;
        });
      });

      const lookupCompleted = Effect.fn("IdePortableCoordinator.lookupCompleted")(function* (
        command: MoveCommand,
      ) {
        const completed = yield* Ref.get(completedByIdempotencyKey);
        const cached = completed.get(command.idempotencyKey);
        if (cached === undefined) return null;
        if (!sameMoveCommand(cached.command, command)) {
          return yield* fail(
            new IdePortableAuthorizationFailure({
              operation: command._tag,
              detailRef: "portable.idempotency_key.collision",
              retryable: false,
            }),
          );
        }
        return cached.receipt;
      });

      const transition = Effect.fn("IdePortableCoordinator.transition")(function* (
        current: IdePortableCoordinatorSnapshot,
        values: Partial<IdePortableCoordinatorSnapshot>,
      ) {
        return yield* publishSnapshot(
          IdePortableCoordinatorSnapshotSchema.make({
            ...current,
            ...values,
            eventSequence: current.eventSequence + 1,
          }),
        );
      });

      const move = Effect.fn("IdePortableCoordinator.move")(function* (
        command: MoveCommand,
        initial: IdePortableCoordinatorSnapshot,
      ) {
        const cached = yield* lookupCompleted(command);
        if (cached !== null) {
          return { snapshot: yield* SubscriptionRef.get(state), receipt: cached };
        }

        yield* Ref.update(commandControls, (values) => {
          const next = new Map(values);
          next.set(command.commandRef, "pending");
          return next;
        });

        let current = yield* transition(initial, {
          phase: "quiescing",
          pendingCommandRef: command.commandRef,
          pendingDestinationPlacementRef: command.destinationPlacementRef,
        });
        let stagedAttachmentRef: string | null = null;
        let sourceRevoked = false;

        const cleanup = Effect.fn("IdePortableCoordinator.cleanup")(function* (reason: string) {
          if (!sourceRevoked) {
            if (stagedAttachmentRef !== null) {
              yield* adapter.rollbackDestination(stagedAttachmentRef, reason);
            }
            yield* adapter.resumeSource(initial);
            yield* publishSnapshot(
              IdePortableCoordinatorSnapshotSchema.make({
                ...initial,
                eventSequence: current.eventSequence + 1,
              }),
            );
          } else if (sourceRevoked) {
            yield* publishSnapshot(
              IdePortableCoordinatorSnapshotSchema.make({
                ...current,
                phase: "degraded",
                eventSequence: current.eventSequence + 1,
              }),
            );
          }
        });

        const program = Effect.gen(function* () {
          const manifest = yield* adapter.quiesceAndCheckpoint(initial, command);
          yield* ensureNotCancelled(command.commandRef);
          if (
            manifest.sessionRef !== initial.sessionRef ||
            manifest.sourceAttachmentRef !== initial.activeAttachmentRef ||
            manifest.sourceGeneration !== initial.activeGeneration ||
            manifest.byteSize > manifest.policy.maximumBytes ||
            manifest.fileCount > manifest.policy.maximumFiles ||
            manifest.secretMaterial !== "excluded" ||
            manifest.processState !== "excluded" ||
            manifest.nativeState !== "excluded"
          ) {
            return yield* fail(
              new IdePortableCheckpointFailure({
                operation: command._tag,
                detailRef: "portable.checkpoint.policy_mismatch",
                retryable: false,
              }),
            );
          }
          yield* adapter.validateCheckpoint(manifest, command.destinationPlacementRef, "source");
          current = yield* transition(current, {
            phase: "checkpoint_verified",
            checkpointManifestRef: manifest.manifestRef,
          });
          yield* ensureNotCancelled(command.commandRef);
          const destinationGeneration = initial.activeGeneration + 1;
          const staged = yield* adapter.stageDestination(
            manifest,
            command.destinationPlacementRef,
            destinationGeneration,
          );
          stagedAttachmentRef = staged.attachmentRef;
          current = yield* transition(current, { phase: "destination_staged" });
          yield* adapter.validateCheckpoint(
            manifest,
            command.destinationPlacementRef,
            "destination",
          );
          yield* ensureNotCancelled(command.commandRef);
          const cutoverCommitted = yield* commitCutover(command.commandRef);
          if (!cutoverCommitted) {
            return yield* fail(
              new IdePortableCancelled({
                operation: command._tag,
                detailRef: "portable.command.cancelled",
                retryable: true,
              }),
            );
          }
          yield* adapter.revokeSource(initial, command);
          sourceRevoked = true;
          current = yield* transition(current, { phase: "source_revoked" });
          yield* adapter.attachDestination(
            manifest,
            command.destinationPlacementRef,
            staged.attachmentRef,
            destinationGeneration,
          );
          current = yield* transition(current, { phase: "attaching" });
          const rawActivation = yield* adapter.restartFreshHelpers(
            command.destinationPlacementRef,
            staged.attachmentRef,
            destinationGeneration,
          );
          const activation = yield* Schema.decodeUnknownEffect(
            IdePortableDestinationActivationReceiptSchema,
          )(rawActivation).pipe(
            Effect.mapError(() =>
              error(
                new IdePortablePlacementFailure({
                  operation: command._tag,
                  detailRef: "portable.destination.activation_invalid",
                  retryable: false,
                }),
              ),
            ),
          );
          if (
            !activationMatchesMove(
              activation,
              manifest,
              command.destinationPlacementRef,
              staged.attachmentRef,
              destinationGeneration,
            )
          ) {
            return yield* fail(
              new IdePortablePlacementFailure({
                operation: command._tag,
                detailRef: "portable.destination.activation_mismatch",
                retryable: false,
              }),
            );
          }
          const completedAt = now();
          const receipt = IdePortableMoveReceiptSchema.make({
            receiptRef: nextReceiptRef(),
            commandRef: command.commandRef,
            idempotencyKey: command.idempotencyKey,
            actorRef: command.actorRef,
            policyRef: command.policyRef,
            sessionRef: initial.sessionRef,
            project: initial.project,
            sourcePlacementRef: initial.activePlacementRef,
            destinationPlacementRef: command.destinationPlacementRef,
            sourceAttachmentRef: initial.activeAttachmentRef,
            sourceGeneration: initial.activeGeneration,
            destinationAttachmentRef: staged.attachmentRef,
            destinationGeneration,
            checkpointManifestRef: manifest.manifestRef,
            transition: command._tag === "Failback" ? "failback" : "move",
            status: "completed",
            recoveryPointRef:
              command._tag === "Failback" ? command.recoveryPointRef : manifest.checkpointRef,
            omissionRefs: manifest.omittedCapabilityRefs,
            evidenceRefs: [
              ...new Set([
                manifest.integrityReceiptRef,
                activation.receiptRef,
                ...activation.evidenceRefs,
              ]),
            ],
            completedAt,
          });
          current = yield* transition(current, {
            phase: "attached",
            activePlacementRef: command.destinationPlacementRef,
            activeAttachmentRef: staged.attachmentRef,
            activeGeneration: destinationGeneration,
            pendingCommandRef: null,
            pendingDestinationPlacementRef: null,
          });
          const retainedReceipts = yield* Ref.modify(receipts, (values) => {
            const next = [...values, receipt].slice(-maximumReceipts);
            return [next, next] as const;
          });
          yield* Ref.update(completedByIdempotencyKey, (values) => {
            const next = new Map(values);
            next.set(command.idempotencyKey, { command, receipt });
            const retainedKeys = new Set(
              retainedReceipts.map(({ idempotencyKey }) => idempotencyKey),
            );
            for (const key of next.keys()) {
              if (!retainedKeys.has(key)) next.delete(key);
            }
            return next;
          });
          yield* PubSub.publish(events, { _tag: "Receipt", receipt } as const);
          return { snapshot: current, receipt };
        });

        return yield* program.pipe(
          Effect.catch((cause: IdePortableCoordinatorError) =>
            cleanup(cause.failure._tag).pipe(Effect.andThen(Effect.fail(cause))),
          ),
          Effect.onInterrupt(() => cleanup("interrupted")),
          Effect.ensuring(removeCancellation(command.commandRef)),
        );
      });

      const cancel = Effect.fn("IdePortableCoordinator.cancel")(function* (
        command: Extract<IdePortableCoordinatorCommand, { readonly _tag: "Cancel" }>,
      ) {
        const current = yield* ensureCurrent(command);
        if (current.pendingCommandRef !== command.targetCommandRef) {
          return yield* fail(
            new IdePortableCancelled({
              operation: "cancel",
              detailRef: "portable.cancel.target_not_pending",
              retryable: false,
            }),
          );
        }
        const cancelled = yield* Ref.modify(commandControls, (values) => {
          if (values.get(command.targetCommandRef) !== "pending") {
            return [false, values] as const;
          }
          const next = new Map(values);
          next.set(command.targetCommandRef, "cancelled");
          return [true, next] as const;
        });
        if (!cancelled) {
          return yield* fail(
            new IdePortableCancelled({
              operation: "cancel",
              detailRef: "portable.cancel.cutover_committed",
              retryable: false,
            }),
          );
        }
        return { snapshot: current, receipt: null };
      });

      const stop = Effect.fn("IdePortableCoordinator.stop")(function* (
        command: Extract<IdePortableCoordinatorCommand, { readonly _tag: "Stop" }>,
      ) {
        const current = yield* ensureCurrent(command);
        yield* adapter.stop(command.reasonRef);
        const stopped = yield* transition(current, {
          phase: "stopped",
          stopped: true,
          pendingCommandRef: null,
          pendingDestinationPlacementRef: null,
        });
        return { snapshot: stopped, receipt: null };
      });

      const execute = Effect.fn("IdePortableCoordinator.execute")(function* (
        raw: IdePortableCoordinatorCommand,
      ) {
        const command = yield* Schema.decodeUnknownEffect(IdePortableCoordinatorCommandSchema)(
          raw,
        ).pipe(
          Effect.mapError(() =>
            error(
              new IdePortableAuthorizationFailure({
                operation: "decode",
                detailRef: "portable.command.invalid",
                retryable: false,
              }),
            ),
          ),
        );
        if (command._tag === "Cancel") return yield* cancel(command);
        if (command._tag === "Move" || command._tag === "Failback") {
          const cached = yield* lookupCompleted(command);
          if (cached !== null) {
            return { snapshot: yield* SubscriptionRef.get(state), receipt: cached };
          }
        }
        return yield* lock.withPermit(
          Effect.gen(function* () {
            const current = yield* ensureCurrent(command);
            if (command._tag === "Stop") return yield* stop(command);
            return yield* move(command, current);
          }),
        );
      });

      const authorizeMutation = Effect.fn("IdePortableCoordinator.authorizeMutation")(function* (
        binding: Readonly<{ sessionRef: string; attachmentRef: string; generation: number }>,
      ) {
        const current = yield* SubscriptionRef.get(state);
        if (
          current.stopped ||
          current.phase !== "attached" ||
          binding.sessionRef !== current.sessionRef ||
          binding.attachmentRef !== current.activeAttachmentRef ||
          binding.generation !== current.activeGeneration
        ) {
          return yield* fail(
            new IdePortableStaleWriter({
              operation: "authorize_mutation",
              detailRef: "portable.generation.stale",
              retryable: true,
            }),
          );
        }
      });

      return IdePortableCoordinator.of({
        snapshot: Effect.fn("IdePortableCoordinator.snapshot")(() => SubscriptionRef.get(state)),
        execute,
        authorizeMutation,
        receipts: Effect.fn("IdePortableCoordinator.receipts")(() => Ref.get(receipts)),
        events: Stream.fromPubSub(events),
      });
    }),
  );
