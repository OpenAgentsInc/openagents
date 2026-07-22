import { Deferred, Effect, Fiber, Layer } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdePortableCheckpointManifestSchema,
  IdePortableCoordinatorCommandSchema,
  IdePortableCoordinatorSnapshotSchema,
  IdePortableDestinationActivationReceiptSchema,
  IdePortablePlacementFailure,
} from "@openagentsinc/portable-session-contract";

import {
  IdePortableCoordinator,
  IdePortableCoordinatorError,
  makeIdePortableCoordinatorLayer,
  type IdePortableCoordinatorAdapter,
} from "./portable-coordinator-service.ts";

const digest = `sha256:${"a".repeat(64)}`;
const project = {
  projectRef: "project.alpha",
  projectRootRef: "root.alpha",
  worktreeRef: "worktree.alpha",
  selectedFileRef: null,
  documentSnapshotRef: null,
  proposalRef: null,
  diagnosticResultRef: null,
  testResultRef: null,
  artifactRef: null,
  evidenceRef: null,
} as const;

const seed = IdePortableCoordinatorSnapshotSchema.make({
  sessionRef: "session.alpha",
  project,
  phase: "attached",
  activePlacementRef: "placement.local",
  activeAttachmentRef: "attachment.local.1",
  activeGeneration: 1,
  pendingCommandRef: null,
  pendingDestinationPlacementRef: null,
  checkpointManifestRef: null,
  eventSequence: 1,
  stopped: false,
});

const manifest = IdePortableCheckpointManifestSchema.make({
  manifestRef: "manifest.alpha.1",
  checkpointRef: "checkpoint.alpha.1",
  sessionRef: seed.sessionRef,
  sourceAttachmentRef: seed.activeAttachmentRef,
  sourceGeneration: seed.activeGeneration,
  digest,
  byteSize: 1_024,
  fileCount: 2,
  repositoryPostImageDigest: digest,
  graphDigest: digest,
  project,
  includedCapabilityRefs: ["capability.files"],
  omittedCapabilityRefs: ["capability.pty"],
  historyRefs: [],
  proposalRefs: [],
  taskRefs: [],
  testRefs: [],
  deliveryEvidenceRefs: [],
  secretMaterial: "excluded",
  processState: "excluded",
  nativeState: "excluded",
  vimState: "destination_setting",
  themeState: "destination_setting",
  policy: {
    maximumBytes: 2_048,
    maximumFiles: 10,
    encryption: "owner_key",
    encryptionKeyRef: "key.alpha",
    custody: "owner_managed",
    retentionSeconds: 3_600,
    expiresAt: "2030-01-01T00:00:00.000Z",
  },
  integrityReceiptRef: "integrity.alpha.1",
});

const activation = IdePortableDestinationActivationReceiptSchema.make({
  schema: "openagents.ide_portable_destination_activation.v1",
  receiptRef: "receipt.activation.2",
  operationRef: "operation.activation.2",
  sessionRef: seed.sessionRef,
  checkpointRef: manifest.checkpointRef,
  destinationTargetRef: "placement.remote",
  destinationAttachmentRef: "attachment.remote.2",
  destinationRunnerSessionReservationRef: "reservation.runner.2",
  destinationGeneration: 2,
  authentication: {
    state: "reauthenticated",
    policyRef: "policy.destination.2",
    evidenceRef: "evidence.authentication.2",
    observedAt: "2029-01-01T00:00:00.000Z",
    expiresAt: "2030-01-01T00:00:00.000Z",
  },
  helpersObservedAt: "2029-01-01T00:00:00.000Z",
  helpers: [
    {
      kind: "pty",
      readiness: "ready",
      instanceRef: "instance.pty.2",
      versionRef: "version.pty.2",
      omissionRef: null,
      evidenceRefs: ["evidence.pty.2"],
    },
    {
      kind: "lsp",
      readiness: "ready",
      instanceRef: "instance.lsp.2",
      versionRef: "version.lsp.2",
      omissionRef: null,
      evidenceRefs: ["evidence.lsp.2"],
    },
    {
      kind: "dap",
      readiness: "unsupported",
      instanceRef: null,
      versionRef: null,
      omissionRef: "omission.dap.2",
      evidenceRefs: [],
    },
    {
      kind: "watcher",
      readiness: "ready",
      instanceRef: "instance.watcher.2",
      versionRef: "version.watcher.2",
      omissionRef: null,
      evidenceRefs: ["evidence.watcher.2"],
    },
    {
      kind: "native",
      readiness: "unsupported",
      instanceRef: null,
      versionRef: null,
      omissionRef: "omission.native.2",
      evidenceRefs: [],
    },
  ],
  activatedAgentRefs: ["agent.root.2"],
  acceptedWorkRefs: [],
  evidenceRefs: ["evidence.authentication.2", "evidence.pty.2", "evidence.lsp.2"],
});

const move = IdePortableCoordinatorCommandSchema.cases.Move.make({
  commandRef: "command.move.1",
  idempotencyKey: "idempotency.move.1",
  actorRef: "actor.owner",
  policyRef: "policy.portable",
  sessionRef: seed.sessionRef,
  project,
  expectedAttachmentRef: seed.activeAttachmentRef,
  expectedGeneration: seed.activeGeneration,
  deadlineAt: "2030-01-01T00:00:00.000Z",
  approvalRef: "approval.move.1",
  destinationPlacementRef: "placement.remote",
});

const adapter = (
  calls: string[],
  quiesce: IdePortableCoordinatorAdapter["quiesceAndCheckpoint"] = () => Effect.succeed(manifest),
  validate: IdePortableCoordinatorAdapter["validateCheckpoint"] = () => Effect.void,
  restart: IdePortableCoordinatorAdapter["restartFreshHelpers"] = () => Effect.succeed(activation),
  revoke: IdePortableCoordinatorAdapter["revokeSource"] = () => Effect.void,
): IdePortableCoordinatorAdapter => ({
  quiesceAndCheckpoint: (snapshot, command) =>
    Effect.sync(() => calls.push("quiesce")).pipe(Effect.andThen(quiesce(snapshot, command))),
  validateCheckpoint: (value, placement, stage) =>
    Effect.sync(() => {
      calls.push(`validate:${stage}`);
    }).pipe(Effect.andThen(validate(value, placement, stage))),
  stageDestination: () =>
    Effect.sync(() => {
      calls.push("stage");
      return { attachmentRef: "attachment.remote.2" };
    }),
  revokeSource: (snapshot, command) =>
    Effect.sync(() => {
      calls.push("revoke");
    }).pipe(Effect.andThen(revoke(snapshot, command))),
  attachDestination: () =>
    Effect.sync(() => {
      calls.push("attach");
    }),
  restartFreshHelpers: (placement, attachment, generation) =>
    Effect.sync(() => {
      calls.push("restart_helpers");
    }).pipe(Effect.andThen(restart(placement, attachment, generation))),
  rollbackDestination: () =>
    Effect.sync(() => {
      calls.push("rollback");
    }),
  resumeSource: () =>
    Effect.sync(() => {
      calls.push("resume_source");
    }),
  stop: () =>
    Effect.sync(() => {
      calls.push("stop");
    }),
});

const run = <A>(
  effect: Effect.Effect<A, IdePortableCoordinatorError, IdePortableCoordinator>,
  layer: Layer.Layer<IdePortableCoordinator>,
) => Effect.runPromise(effect.pipe(Effect.provide(layer)));

describe("IDE portable coordinator", () => {
  test("moves in the exclusive order, advances generation, fences stale writers, and replays idempotently", async () => {
    const calls: string[] = [];
    const layer = makeIdePortableCoordinatorLayer(seed, adapter(calls), {
      now: () => "2029-01-01T00:00:00.000Z",
      nextReceiptRef: () => "receipt.move.1",
    });
    const result = await run(
      Effect.gen(function* () {
        const service = yield* IdePortableCoordinator;
        const first = yield* service.execute(move);
        const replay = yield* service.execute(move);
        const collision = yield* service
          .execute({
            ...move,
            destinationPlacementRef: "placement.other",
          })
          .pipe(
            Effect.match({
              onFailure: (failure) => failure,
              onSuccess: () => null,
            }),
          );
        const stale = yield* service
          .authorizeMutation({
            sessionRef: seed.sessionRef,
            attachmentRef: seed.activeAttachmentRef,
            generation: 1,
          })
          .pipe(Effect.exit);
        yield* service.authorizeMutation({
          sessionRef: seed.sessionRef,
          attachmentRef: "attachment.remote.2",
          generation: 2,
        });
        return { first, replay, collision, stale };
      }),
      layer,
    );
    expect(calls).toEqual([
      "quiesce",
      "validate:source",
      "stage",
      "validate:destination",
      "revoke",
      "attach",
      "restart_helpers",
    ]);
    expect(result.first.snapshot.activeGeneration).toBe(2);
    expect(result.first.snapshot.activeAttachmentRef).toBe("attachment.remote.2");
    expect(result.first.receipt?.sourceGeneration).toBe(1);
    expect(result.first.receipt?.destinationGeneration).toBe(2);
    expect(result.replay.receipt?.receiptRef).toBe(result.first.receipt?.receiptRef);
    expect(result.first.receipt?.evidenceRefs).toContain(activation.receiptRef);
    expect(result.collision?.failure._tag).toBe("IdePortable.AuthorizationFailure");
    expect(result.collision?.failure.detailRef).toBe("portable.idempotency_key.collision");
    expect(calls).toHaveLength(7);
    expect(result.stale._tag).toBe("Failure");
  });

  test("cancels before source revocation, tears down the staged destination, and resumes the source", async () => {
    const calls: string[] = [];
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const entered = yield* Deferred.make<void>();
        const release = yield* Deferred.make<void>();
        const gate: IdePortableCoordinatorAdapter["validateCheckpoint"] = (
          _manifest,
          _placement,
          stage,
        ) =>
          stage === "destination"
            ? Deferred.succeed(entered, undefined).pipe(Effect.andThen(Deferred.await(release)))
            : Effect.void;
        const layer = makeIdePortableCoordinatorLayer(
          seed,
          adapter(calls, () => Effect.succeed(manifest), gate),
          {
            now: () => "2029-01-01T00:00:00.000Z",
          },
        );
        return yield* Effect.gen(function* () {
          const service = yield* IdePortableCoordinator;
          const fiber = yield* Effect.forkChild(service.execute(move));
          yield* Deferred.await(entered);
          const cancel = IdePortableCoordinatorCommandSchema.cases.Cancel.make({
            commandRef: "command.cancel.1",
            idempotencyKey: "idempotency.cancel.1",
            actorRef: "actor.owner",
            policyRef: "policy.portable",
            sessionRef: seed.sessionRef,
            project,
            expectedAttachmentRef: seed.activeAttachmentRef,
            expectedGeneration: 1,
            deadlineAt: "2030-01-01T00:00:00.000Z",
            approvalRef: "approval.cancel.1",
            targetCommandRef: move.commandRef,
          });
          yield* service.execute(cancel);
          yield* Deferred.succeed(release, undefined);
          const exit = yield* Fiber.await(fiber);
          const snapshot = yield* service.snapshot();
          return { exit, snapshot };
        }).pipe(Effect.provide(layer));
      }),
    );
    expect(result.exit._tag).toBe("Failure");
    expect(result.snapshot.phase).toBe("attached");
    expect(result.snapshot.activeGeneration).toBe(1);
    expect(calls).toContain("rollback");
    expect(calls).toContain("resume_source");
    expect(calls).not.toContain("revoke");
  });

  test("rolls back the staged destination and resumes the source when destination validation fails", async () => {
    const calls: string[] = [];
    const validationFailure = new IdePortableCoordinatorError({
      failure: new IdePortablePlacementFailure({
        operation: "validate_destination",
        detailRef: "portable.destination.validation_failed",
        retryable: true,
      }),
    });
    const layer = makeIdePortableCoordinatorLayer(
      seed,
      adapter(calls, undefined, (_manifest, _placement, stage) =>
        stage === "destination" ? Effect.fail(validationFailure) : Effect.void,
      ),
      { now: () => "2029-01-01T00:00:00.000Z" },
    );

    const result = await run(
      Effect.gen(function* () {
        const service = yield* IdePortableCoordinator;
        const exit = yield* service.execute(move).pipe(Effect.exit);
        const snapshot = yield* service.snapshot();
        return { exit, snapshot };
      }),
      layer,
    );

    expect(result.exit._tag).toBe("Failure");
    expect(result.snapshot.phase).toBe("attached");
    expect(result.snapshot.activeGeneration).toBe(1);
    expect(calls).toContain("rollback");
    expect(calls).toContain("resume_source");
    expect(calls).not.toContain("revoke");
  });

  test("fails closed in degraded state when activation evidence mismatches after source revocation", async () => {
    const calls: string[] = [];
    const layer = makeIdePortableCoordinatorLayer(
      seed,
      adapter(calls, undefined, undefined, () =>
        Effect.succeed({ ...activation, destinationTargetRef: "placement.other" }),
      ),
      { now: () => "2029-01-01T00:00:00.000Z" },
    );

    const result = await run(
      Effect.gen(function* () {
        const service = yield* IdePortableCoordinator;
        const failure = yield* service.execute(move).pipe(
          Effect.match({
            onFailure: (value) => value,
            onSuccess: () => null,
          }),
        );
        const snapshot = yield* service.snapshot();
        const stale = yield* service
          .authorizeMutation({
            sessionRef: seed.sessionRef,
            attachmentRef: seed.activeAttachmentRef,
            generation: seed.activeGeneration,
          })
          .pipe(
            Effect.match({
              onFailure: (value) => value,
              onSuccess: () => null,
            }),
          );
        return { failure, snapshot, stale };
      }),
      layer,
    );

    expect(result.failure?.failure._tag).toBe("IdePortable.PlacementFailure");
    expect(result.failure?.failure.detailRef).toBe("portable.destination.activation_mismatch");
    expect(result.snapshot.phase).toBe("degraded");
    expect(result.stale?.failure._tag).toBe("IdePortable.StaleWriter");
    expect(calls).toContain("revoke");
    expect(calls).toContain("attach");
    expect(calls).not.toContain("rollback");
    expect(calls).not.toContain("resume_source");
  });

  test("refuses cancellation after the atomic source-revocation cutover commits", async () => {
    const calls: string[] = [];
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const enteredRevoke = yield* Deferred.make<void>();
        const releaseRevoke = yield* Deferred.make<void>();
        const layer = makeIdePortableCoordinatorLayer(
          seed,
          adapter(calls, undefined, undefined, undefined, () =>
            Deferred.succeed(enteredRevoke, undefined).pipe(
              Effect.andThen(Deferred.await(releaseRevoke)),
            ),
          ),
          { now: () => "2029-01-01T00:00:00.000Z" },
        );

        return yield* Effect.gen(function* () {
          const service = yield* IdePortableCoordinator;
          const fiber = yield* Effect.forkChild(service.execute(move));
          yield* Deferred.await(enteredRevoke);
          const cancel = IdePortableCoordinatorCommandSchema.cases.Cancel.make({
            commandRef: "command.cancel.after-cutover",
            idempotencyKey: "idempotency.cancel.after-cutover",
            actorRef: "actor.owner",
            policyRef: "policy.portable",
            sessionRef: seed.sessionRef,
            project,
            expectedAttachmentRef: seed.activeAttachmentRef,
            expectedGeneration: seed.activeGeneration,
            deadlineAt: "2030-01-01T00:00:00.000Z",
            approvalRef: "approval.cancel.after-cutover",
            targetCommandRef: move.commandRef,
          });
          const cancellation = yield* service.execute(cancel).pipe(
            Effect.match({
              onFailure: (value) => value,
              onSuccess: () => null,
            }),
          );
          yield* Deferred.succeed(releaseRevoke, undefined);
          const completed = yield* Fiber.join(fiber);
          return { cancellation, completed };
        }).pipe(Effect.provide(layer));
      }),
    );

    expect(result.cancellation?.failure._tag).toBe("IdePortable.Cancelled");
    expect(result.cancellation?.failure.detailRef).toBe("portable.cancel.cutover_committed");
    expect(result.completed.snapshot.phase).toBe("attached");
    expect(result.completed.snapshot.activeGeneration).toBe(2);
    expect(calls).toContain("revoke");
    expect(calls).not.toContain("rollback");
  });
});
