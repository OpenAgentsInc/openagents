import {
  MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  type ManagedSandboxPhase2Operation,
  type ManagedSandboxPhase2Store,
  type ManagedSandboxPhase2Target,
  makeManagedSandboxPhase2Service,
} from "./managed-sandbox-phase2-service";

const digest = (character: string) => `sha256:${character.repeat(64)}`;

const omissions = {
  credentials: "excluded" as const,
  accountSecrets: "excluded" as const,
  providerHiddenState: "excluded" as const,
  processMemory: "excluded" as const,
  processTable: "excluded" as const,
  ptyState: "excluded" as const,
  sockets: "excluded" as const,
  ports: "excluded" as const,
  networkIdentity: "excluded" as const,
};

const checkpoint = (): ManagedSandboxContentCheckpoint => ({
  schema: MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: "checkpoint.sbx10.1",
  ownerRef: "owner.test",
  tenantRef: "tenant.test",
  sourceSandboxRef: "sandbox.source.1",
  sourceResourceGeneration: 4,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.5c69496",
  repositoryPostImageDigest: digest("c"),
  contentDigest: digest("d"),
  contentBytes: 4_096,
  formatRef: "format.sbx.content-tar.v1",
  state: "completed",
  completedAt: "2026-07-22T00:00:00.000Z",
  verifiedAt: "2026-07-22T00:00:01.000Z",
  retainedUntil: "2026-07-23T00:00:01.000Z",
  deleteOnExpiry: true,
  omissions,
  evidenceRefs: ["receipt.checkpoint.verify.1"],
});

const baseCommand = {
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  ownerRef: "owner.test",
  tenantRef: "tenant.test",
  requestedAt: "2026-07-22T00:00:00.000Z",
};

const createCommand = () => ({
  ...baseCommand,
  _tag: "CreateCheckpoint" as const,
  commandRef: "command.checkpoint.create.1",
  idempotencyRef: "idempotency.checkpoint.create.1",
  checkpointRef: "checkpoint.sbx10.1",
  sourceSandboxRef: "sandbox.source.1",
  sourceResourceGeneration: 4,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.5c69496",
  repositoryPostImageDigest: digest("c"),
  formatRef: "format.sbx.content-tar.v1",
  retainedUntil: "2026-07-23T00:00:01.000Z",
});

const checkpointKey = (ownerRef: string, tenantRef: string, checkpointRef: string) =>
  `${ownerRef}:${tenantRef}:${checkpointRef}`;

const makeStore = (seed: ReadonlyArray<ManagedSandboxContentCheckpoint> = []) => {
  const operationsByCommand = new Map<string, ManagedSandboxPhase2Operation>();
  const operationsByIdempotency = new Map<string, ManagedSandboxPhase2Operation>();
  const checkpoints = new Map(
    seed.map((value) => [`${value.ownerRef}:${value.tenantRef}:${value.checkpointRef}`, value]),
  );
  const store: ManagedSandboxPhase2Store = {
    lookupOperation: ({ commandRef, idempotencyRef }) =>
      Effect.succeed(
        operationsByCommand.get(commandRef) ?? operationsByIdempotency.get(idempotencyRef),
      ),
    readCheckpoint: ({ ownerRef, tenantRef, checkpointRef }) =>
      Effect.succeed(checkpoints.get(checkpointKey(ownerRef, tenantRef, checkpointRef))),
    settle: ({ operation, checkpointMutation }) =>
      Effect.sync(() => {
        operationsByCommand.set(operation.command.commandRef, operation);
        operationsByIdempotency.set(operation.command.idempotencyRef, operation);
        if (checkpointMutation["_tag"] === "Put") {
          const value = checkpointMutation.checkpoint;
          checkpoints.set(
            checkpointKey(value.ownerRef, value.tenantRef, value.checkpointRef),
            value,
          );
        } else if (checkpointMutation["_tag"] === "Delete") {
          checkpoints.delete(
            checkpointKey(
              operation.command.ownerRef,
              operation.command.tenantRef,
              checkpointMutation.checkpointRef,
            ),
          );
        }
      }),
  };
  return { store, checkpoints, operationsByCommand };
};

const makeTarget = (overrides: Partial<ManagedSandboxPhase2Target> = {}) => {
  let createCalls = 0;
  let forkCalls = 0;
  const target: ManagedSandboxPhase2Target = {
    createCheckpoint: () =>
      Effect.sync(() => {
        createCalls += 1;
        return checkpoint();
      }),
    archiveWithCheckpoint: (command) =>
      Effect.succeed({
        _tag: "Archived",
        schema: MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
        stopRef: command.stopRef,
        sandboxRef: command.sourceSandboxRef,
        resourceGeneration: command.sourceResourceGeneration,
        checkpoint: checkpoint(),
        lifecycle: "stopped",
        archiveClaim: "allowed",
        observedAt: "2026-07-22T00:05:00.000Z",
        evidenceRefs: ["receipt.archive.1"],
      }),
    verifyCheckpoint: () => Effect.succeed(true),
    observeResourceGeneration: () => Effect.succeed(4),
    forkFromCheckpoint: (command, source) =>
      Effect.sync(() => {
        forkCalls += 1;
        return {
          schema: MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION,
          receiptRef: "receipt.fork.1",
          ownerRef: command.ownerRef,
          tenantRef: command.tenantRef,
          checkpointRef: source.checkpointRef,
          sourceSandboxRef: source.sourceSandboxRef,
          sourceResourceGeneration: source.sourceResourceGeneration,
          forkSandboxRef: "sandbox.fork.1",
          forkResourceGeneration: 0,
          sourceCapabilityRefs: command.sourceCapabilityRefs,
          forkCapabilityRefs: ["capability.fork.command"],
          grantPolicy: "mint_fresh",
          cleanupObligationRef: "cleanup.sandbox.fork.1",
          stateTransfer: omissions,
          processSessionContinuity: "none",
          outcome: "created",
          observedAt: "2026-07-22T00:06:00.000Z",
          evidenceRefs: ["receipt.fork.identity.1"],
        };
      }),
    restoreCheckpoint: (command, source) =>
      Effect.succeed({
        schema: MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION,
        receiptRef: "receipt.restore.1",
        ownerRef: command.ownerRef,
        tenantRef: command.tenantRef,
        checkpointRef: source.checkpointRef,
        sandboxRef: command.destinationSandboxRef,
        checkpointSourceGeneration: source.sourceResourceGeneration,
        restoredResourceGeneration: source.sourceResourceGeneration + 1,
        admittedServiceRefs: command.admittedServiceRefs,
        restartedServiceRefs: command.admittedServiceRefs,
        sourceCapabilityRefs: command.sourceCapabilityRefs,
        restoredCapabilityRefs: ["capability.restore.command"],
        grantPolicy: "mint_fresh",
        processSessionContinuity: "discontinuous",
        processMemoryRestored: false,
        ptyRestored: false,
        socketsRestored: false,
        outcome: "restored",
        observedAt: "2026-07-22T00:07:00.000Z",
        evidenceRefs: ["receipt.restore.1"],
      }),
    deleteCheckpoint: (command, source) =>
      Effect.succeed({
        schema: MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
        receiptRef: "receipt.checkpoint.delete.1",
        ownerRef: command.ownerRef,
        tenantRef: command.tenantRef,
        checkpointRef: source.checkpointRef,
        sourceSandboxRef: source.sourceSandboxRef,
        sourceResourceGeneration: source.sourceResourceGeneration,
        contentDigest: source.contentDigest,
        contentDeleted: true,
        outcome: "deleted",
        reason: command.reason,
        deletedAt: "2026-07-22T00:08:00.000Z",
        evidenceRefs: ["receipt.checkpoint.object-delete.1"],
      }),
    ...overrides,
  };
  return {
    target,
    createCalls: () => createCalls,
    forkCalls: () => forkCalls,
  };
};

describe("managed sandbox Phase 2 service", () => {
  it("verifies and commits a checkpoint once under exact replay", () =>
    Effect.gen(function* () {
      const state = makeStore();
      const target = makeTarget();
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
      });

      const first = yield* service.execute(createCommand());
      const replay = yield* service.execute(createCommand());

      expect(first).toEqual(checkpoint());
      expect(replay).toEqual(first);
      expect(target.createCalls()).toBe(1);
      expect(state.checkpoints.size).toBe(1);

      const conflict = yield* Effect.flip(
        service.execute({
          ...createCommand(),
          retainedUntil: "2026-07-24T00:00:01.000Z",
        }),
      );
      expect(conflict["_tag"]).toBe("IdempotencyConflict");
    }).pipe(Effect.runPromise));

  it("fails closed when checkpoint integrity verification fails", () =>
    Effect.gen(function* () {
      const state = makeStore();
      const target = makeTarget({ verifyCheckpoint: () => Effect.succeed(false) });
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
      });

      const failure = yield* Effect.flip(service.execute(createCommand()));

      expect(failure["_tag"]).toBe("CheckpointCorrupt");
      expect(state.checkpoints.size).toBe(0);
      expect(state.operationsByCommand.size).toBe(0);
    }).pipe(Effect.runPromise));

  it("refuses an expired or stale checkpoint before a fork effect", () =>
    Effect.gen(function* () {
      const expiredState = makeStore([checkpoint()]);
      const expiredTarget = makeTarget();
      const expiredService = makeManagedSandboxPhase2Service({
        store: expiredState.store,
        target: expiredTarget.target,
        now: () => new Date("2026-07-24T00:00:00.000Z"),
      });
      const forkCommand = {
        ...baseCommand,
        _tag: "ForkFromCheckpoint" as const,
        commandRef: "command.fork.1",
        idempotencyRef: "idempotency.fork.1",
        checkpointRef: "checkpoint.sbx10.1",
        expectedSourceSandboxRef: "sandbox.source.1",
        expectedSourceResourceGeneration: 4,
        sourceCapabilityRefs: ["capability.source.command"],
      };

      const expiredFailure = yield* Effect.flip(expiredService.execute(forkCommand));
      expect(expiredFailure["_tag"]).toBe("CheckpointExpired");
      expect(expiredTarget.forkCalls()).toBe(0);

      const staleState = makeStore([checkpoint()]);
      const staleTarget = makeTarget({ observeResourceGeneration: () => Effect.succeed(5) });
      const staleService = makeManagedSandboxPhase2Service({
        store: staleState.store,
        target: staleTarget.target,
        now: () => new Date("2026-07-22T00:10:00.000Z"),
      });
      const staleFailure = yield* Effect.flip(staleService.execute(forkCommand));
      expect(staleFailure["_tag"]).toBe("StaleSource");
      expect(staleTarget.forkCalls()).toBe(0);
    }).pipe(Effect.runPromise));

  it("creates a fork with a fresh identity and grants", () =>
    Effect.gen(function* () {
      const state = makeStore([checkpoint()]);
      const target = makeTarget();
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
        now: () => new Date("2026-07-22T00:10:00.000Z"),
      });
      const result = yield* service.execute({
        ...baseCommand,
        _tag: "ForkFromCheckpoint",
        commandRef: "command.fork.1",
        idempotencyRef: "idempotency.fork.1",
        checkpointRef: "checkpoint.sbx10.1",
        expectedSourceSandboxRef: "sandbox.source.1",
        expectedSourceResourceGeneration: 4,
        sourceCapabilityRefs: ["capability.source.command"],
      });

      expect("forkSandboxRef" in result && result.forkSandboxRef).toBe("sandbox.fork.1");
      expect("forkCapabilityRefs" in result && result.forkCapabilityRefs).toEqual([
        "capability.fork.command",
      ]);
    }).pipe(Effect.runPromise));

  it("restores admitted services with a new generation and no process continuity", () =>
    Effect.gen(function* () {
      const state = makeStore([checkpoint()]);
      const target = makeTarget();
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
        now: () => new Date("2026-07-22T00:10:00.000Z"),
      });
      const result = yield* service.execute({
        ...baseCommand,
        _tag: "RestoreCheckpoint",
        commandRef: "command.restore.1",
        idempotencyRef: "idempotency.restore.1",
        checkpointRef: "checkpoint.sbx10.1",
        destinationSandboxRef: "sandbox.destination.1",
        expectedSourceResourceGeneration: 4,
        admittedServiceRefs: ["service.agent-runtime"],
        sourceCapabilityRefs: ["capability.source.command"],
      });

      expect("restoredResourceGeneration" in result && result.restoredResourceGeneration).toBe(5);
      expect("restartedServiceRefs" in result && result.restartedServiceRefs).toEqual([
        "service.agent-runtime",
      ]);
      expect("processSessionContinuity" in result && result.processSessionContinuity).toBe(
        "discontinuous",
      );
    }).pipe(Effect.runPromise));

  it("records recovery-required truth when the required archive checkpoint fails", () =>
    Effect.gen(function* () {
      const state = makeStore();
      const target = makeTarget({
        archiveWithCheckpoint: (command) =>
          Effect.succeed({
            _tag: "CheckpointFailed",
            schema: MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
            stopRef: command.stopRef,
            sandboxRef: command.sourceSandboxRef,
            resourceGeneration: command.sourceResourceGeneration,
            attemptedCheckpointRef: command.checkpointRef,
            errorRef: "error.checkpoint.partial-upload",
            lifecycle: "recovery_required",
            archiveClaim: "forbidden",
            observedAt: "2026-07-22T00:05:00.000Z",
            evidenceRefs: ["receipt.checkpoint.failed.1"],
          }),
      });
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
      });
      const result = yield* service.execute({
        ...createCommand(),
        _tag: "ArchiveWithCheckpoint",
        commandRef: "command.archive.1",
        idempotencyRef: "idempotency.archive.1",
        stopRef: "stop.sbx10.1",
      });

      expect("archiveClaim" in result && result.archiveClaim).toBe("forbidden");
      expect(state.checkpoints.size).toBe(0);
    }).pipe(Effect.runPromise));

  it("deletes checkpoint metadata only after exact content-deletion proof", () =>
    Effect.gen(function* () {
      const state = makeStore([checkpoint()]);
      const target = makeTarget();
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
      });
      const result = yield* service.execute({
        ...baseCommand,
        _tag: "DeleteCheckpoint",
        commandRef: "command.checkpoint.delete.1",
        idempotencyRef: "idempotency.checkpoint.delete.1",
        checkpointRef: "checkpoint.sbx10.1",
        reason: "owner_requested",
      });

      expect("contentDeleted" in result && result.contentDeleted).toBe(true);
      expect(state.checkpoints.size).toBe(0);
    }).pipe(Effect.runPromise));

  it("keeps private ingress disabled before the separate security proof", () =>
    Effect.gen(function* () {
      const state = makeStore();
      const target = makeTarget();
      const service = makeManagedSandboxPhase2Service({
        store: state.store,
        target: target.target,
      });
      const failure = yield* Effect.flip(
        service.execute({
          ...baseCommand,
          _tag: "CreatePrivateIngress",
          commandRef: "command.ingress.1",
          idempotencyRef: "idempotency.ingress.1",
          sandboxRef: "sandbox.source.1",
          resourceGeneration: 4,
          audienceRef: "audience.owner-device.1",
          kind: "preview",
          ttlSeconds: 300,
        }),
      );

      expect(failure["_tag"]).toBe("PrivateIngressUnavailable");
      expect(failure).toMatchObject({ reasonRef: "security_proof_pending", retryable: false });
    }).pipe(Effect.runPromise));
});
