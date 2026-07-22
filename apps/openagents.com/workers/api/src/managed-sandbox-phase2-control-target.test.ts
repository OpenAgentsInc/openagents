import {
  MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxPhase2Command,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
import { describe, expect, it } from "vitest";

import {
  MANAGED_SANDBOX_PHASE2_TARGET_SCHEMA_VERSION,
  makeManagedSandboxPhase2ControlTarget,
} from "./managed-sandbox-phase2-control-target";

const digest = (character: string): `sha256:${string}` => `sha256:${character.repeat(64)}`;

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

const checkpoint: ManagedSandboxContentCheckpoint = {
  schema: MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: "checkpoint.sbx10.control-target",
  ownerRef: "owner.sbx10.control-target",
  tenantRef: "tenant.sbx10.control-target",
  sourceSandboxRef: "sandbox.sbx10.control-target",
  sourceResourceGeneration: 7,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.1e374ce",
  repositoryPostImageDigest: digest("c"),
  contentDigest: digest("d"),
  contentBytes: 16_384,
  formatRef: "format.sbx.content-tar.v1",
  state: "completed",
  completedAt: "2026-07-22T03:05:01.000Z",
  verifiedAt: "2026-07-22T03:05:02.000Z",
  retainedUntil: "2026-07-23T03:05:00.000Z",
  deleteOnExpiry: true,
  omissions,
  evidenceRefs: ["receipt.sbx10.control-target.verify"],
};

const createCommand = {
  _tag: "CreateCheckpoint",
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.control-target.create",
  idempotencyRef: "idempotency.sbx10.control-target.create",
  ownerRef: checkpoint.ownerRef,
  tenantRef: checkpoint.tenantRef,
  requestedAt: "2026-07-22T03:05:00.000Z",
  checkpointRef: checkpoint.checkpointRef,
  sourceSandboxRef: checkpoint.sourceSandboxRef,
  sourceResourceGeneration: checkpoint.sourceResourceGeneration,
  sourceImageDigest: checkpoint.sourceImageDigest,
  sourceToolchainDigest: checkpoint.sourceToolchainDigest,
  repositoryRef: checkpoint.repositoryRef,
  repositoryRevisionRef: checkpoint.repositoryRevisionRef,
  repositoryPostImageDigest: checkpoint.repositoryPostImageDigest,
  formatRef: checkpoint.formatRef,
  retainedUntil: checkpoint.retainedUntil,
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "CreateCheckpoint" }>;

const archiveCommand = {
  ...createCommand,
  _tag: "ArchiveWithCheckpoint",
  commandRef: "command.sbx10.control-target.archive",
  idempotencyRef: "idempotency.sbx10.control-target.archive",
  stopRef: "stop.sbx10.control-target",
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "ArchiveWithCheckpoint" }>;

const forkCommand = {
  _tag: "ForkFromCheckpoint",
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.control-target.fork",
  idempotencyRef: "idempotency.sbx10.control-target.fork",
  ownerRef: checkpoint.ownerRef,
  tenantRef: checkpoint.tenantRef,
  requestedAt: "2026-07-22T03:06:00.000Z",
  checkpointRef: checkpoint.checkpointRef,
  expectedSourceSandboxRef: checkpoint.sourceSandboxRef,
  expectedSourceResourceGeneration: checkpoint.sourceResourceGeneration,
  sourceCapabilityRefs: ["capability.source.control-target"],
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "ForkFromCheckpoint" }>;

const restoreCommand = {
  _tag: "RestoreCheckpoint",
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.control-target.restore",
  idempotencyRef: "idempotency.sbx10.control-target.restore",
  ownerRef: checkpoint.ownerRef,
  tenantRef: checkpoint.tenantRef,
  requestedAt: "2026-07-22T03:07:00.000Z",
  checkpointRef: checkpoint.checkpointRef,
  destinationSandboxRef: "sandbox.sbx10.control-target.restore",
  expectedSourceResourceGeneration: checkpoint.sourceResourceGeneration,
  admittedServiceRefs: ["service.agent-runtime"],
  sourceCapabilityRefs: ["capability.source.control-target"],
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "RestoreCheckpoint" }>;

const deleteCommand = {
  _tag: "DeleteCheckpoint",
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.control-target.delete",
  idempotencyRef: "idempotency.sbx10.control-target.delete",
  ownerRef: checkpoint.ownerRef,
  tenantRef: checkpoint.tenantRef,
  requestedAt: "2026-07-22T03:08:00.000Z",
  checkpointRef: checkpoint.checkpointRef,
  reason: "owner_requested",
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "DeleteCheckpoint" }>;

const results = {
  create_checkpoint: checkpoint,
  archive_with_checkpoint: {
    _tag: "CheckpointFailed",
    schema: MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
    stopRef: archiveCommand.stopRef,
    sandboxRef: archiveCommand.sourceSandboxRef,
    resourceGeneration: archiveCommand.sourceResourceGeneration,
    attemptedCheckpointRef: archiveCommand.checkpointRef,
    errorRef: "error.checkpoint.control-target.partial",
    lifecycle: "recovery_required",
    archiveClaim: "forbidden",
    observedAt: "2026-07-22T03:06:01.000Z",
    evidenceRefs: ["receipt.checkpoint.control-target.partial"],
  },
  verify_checkpoint: {
    verified: true,
    checkpointRef: checkpoint.checkpointRef,
    contentDigest: checkpoint.contentDigest,
    evidenceRefs: ["receipt.checkpoint.control-target.readback"],
  },
  observe_resource_generation: {
    ownerRef: checkpoint.ownerRef,
    tenantRef: checkpoint.tenantRef,
    sandboxRef: checkpoint.sourceSandboxRef,
    resourceGeneration: checkpoint.sourceResourceGeneration,
    evidenceRefs: ["receipt.sandbox.control-target.generation"],
  },
  fork_from_checkpoint: {
    schema: MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION,
    receiptRef: "receipt.sbx10.control-target.fork",
    ownerRef: checkpoint.ownerRef,
    tenantRef: checkpoint.tenantRef,
    checkpointRef: checkpoint.checkpointRef,
    sourceSandboxRef: checkpoint.sourceSandboxRef,
    sourceResourceGeneration: checkpoint.sourceResourceGeneration,
    forkSandboxRef: "sandbox.sbx10.control-target.fork",
    forkResourceGeneration: 1,
    sourceCapabilityRefs: forkCommand.sourceCapabilityRefs,
    forkCapabilityRefs: ["capability.fork.control-target"],
    grantPolicy: "mint_fresh",
    cleanupObligationRef: "cleanup.sandbox.control-target.fork",
    stateTransfer: omissions,
    processSessionContinuity: "none",
    outcome: "created",
    observedAt: "2026-07-22T03:06:01.000Z",
    evidenceRefs: ["receipt.sbx10.control-target.fork.identity"],
  },
  restore_checkpoint: {
    schema: MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION,
    receiptRef: "receipt.sbx10.control-target.restore",
    ownerRef: checkpoint.ownerRef,
    tenantRef: checkpoint.tenantRef,
    checkpointRef: checkpoint.checkpointRef,
    sandboxRef: restoreCommand.destinationSandboxRef,
    checkpointSourceGeneration: checkpoint.sourceResourceGeneration,
    restoredResourceGeneration: checkpoint.sourceResourceGeneration + 1,
    admittedServiceRefs: restoreCommand.admittedServiceRefs,
    restartedServiceRefs: restoreCommand.admittedServiceRefs,
    sourceCapabilityRefs: restoreCommand.sourceCapabilityRefs,
    restoredCapabilityRefs: ["capability.restore.control-target"],
    grantPolicy: "mint_fresh",
    processSessionContinuity: "discontinuous",
    processMemoryRestored: false,
    ptyRestored: false,
    socketsRestored: false,
    outcome: "restored",
    observedAt: "2026-07-22T03:07:01.000Z",
    evidenceRefs: ["receipt.sbx10.control-target.restore.service"],
  },
  delete_checkpoint: {
    schema: MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
    receiptRef: "receipt.sbx10.control-target.delete",
    ownerRef: checkpoint.ownerRef,
    tenantRef: checkpoint.tenantRef,
    checkpointRef: checkpoint.checkpointRef,
    sourceSandboxRef: checkpoint.sourceSandboxRef,
    sourceResourceGeneration: checkpoint.sourceResourceGeneration,
    contentDigest: checkpoint.contentDigest,
    contentDeleted: true,
    outcome: "deleted",
    reason: deleteCommand.reason,
    deletedAt: "2026-07-22T03:08:01.000Z",
    evidenceRefs: ["receipt.sbx10.control-target.delete.object"],
  },
} as const;

describe("managed sandbox Phase 2 Google Cloud control target", () => {
  it("binds every private target action to exact public-safe request bytes", () =>
    Effect.gen(function* () {
      const calls: Array<Readonly<Record<string, unknown>>> = [];
      const headers: Headers[] = [];
      const target = makeManagedSandboxPhase2ControlTarget({
        baseUrl: "https://control.example",
        bearerToken: "control-token-private",
        fetch: async (input, init) => {
          expect(String(input)).toBe(
            "https://control.example/v1/managed-sandbox/runtime/checkpoints",
          );
          const body = JSON.parse(String(init?.body)) as {
            action: keyof typeof results;
            requestRef: string;
          };
          calls.push(body);
          headers.push(new Headers(init?.headers));
          return Response.json({
            schemaVersion: MANAGED_SANDBOX_PHASE2_TARGET_SCHEMA_VERSION,
            action: body.action,
            requestRef: body.requestRef,
            result: results[body.action],
          });
        },
      });

      expect(yield* target.createCheckpoint(createCommand)).toEqual(checkpoint);
      expect(yield* target.archiveWithCheckpoint(archiveCommand)).toEqual(
        results.archive_with_checkpoint,
      );
      expect(yield* target.verifyCheckpoint(checkpoint)).toBe(true);
      expect(
        yield* target.observeResourceGeneration({
          ownerRef: checkpoint.ownerRef,
          tenantRef: checkpoint.tenantRef,
          sandboxRef: checkpoint.sourceSandboxRef,
        }),
      ).toBe(checkpoint.sourceResourceGeneration);
      expect(yield* target.forkFromCheckpoint(forkCommand, checkpoint)).toEqual(
        results.fork_from_checkpoint,
      );
      expect(yield* target.restoreCheckpoint(restoreCommand, checkpoint)).toEqual(
        results.restore_checkpoint,
      );
      expect(yield* target.deleteCheckpoint(deleteCommand, checkpoint)).toEqual(
        results.delete_checkpoint,
      );

      expect(calls.map((call) => call.action)).toEqual(Object.keys(results));
      expect(calls[0]).toMatchObject({ command: createCommand });
      expect(calls[4]).toMatchObject({ command: forkCommand, checkpoint });
      expect(calls[5]).toMatchObject({ command: restoreCommand, checkpoint });
      expect(calls[6]).toMatchObject({ command: deleteCommand, checkpoint });
      expect(JSON.stringify(calls)).not.toContain("control-token-private");
      expect(
        headers.every(
          (value) => value.get("x-openagents-managed-sandbox-token") === "control-token-private",
        ),
      ).toBe(true);
    }).pipe(Effect.runPromise));

  it("rejects a response that does not bind the exact request scope", () =>
    Effect.gen(function* () {
      const target = makeManagedSandboxPhase2ControlTarget({
        baseUrl: "https://control.example",
        bearerToken: "control-token-private",
        fetch: async () =>
          Response.json({
            schemaVersion: MANAGED_SANDBOX_PHASE2_TARGET_SCHEMA_VERSION,
            action: "create_checkpoint",
            requestRef: "command.sbx10.control-target.other",
            result: checkpoint,
          }),
      });

      const failure = yield* Effect.flip(target.createCheckpoint(createCommand));
      expect(failure).toMatchObject({
        _tag: "InvalidRequest",
        message: "the Phase 2 target response failed contract validation",
        retryable: false,
      });
    }).pipe(Effect.runPromise));

  it("redacts upstream bodies and transport failures", () =>
    Effect.gen(function* () {
      const refused = makeManagedSandboxPhase2ControlTarget({
        baseUrl: "https://control.example",
        bearerToken: "control-token-private",
        fetch: async () =>
          new Response('{"localPath":"/Users/private/database.sock"}', { status: 503 }),
      });
      const unavailable = yield* Effect.flip(refused.createCheckpoint(createCommand));
      expect(unavailable).toMatchObject({
        _tag: "InvalidRequest",
        message: "the Phase 2 target is unavailable",
        retryable: true,
      });
      expect(JSON.stringify(unavailable)).not.toContain("private");

      const conflicting = makeManagedSandboxPhase2ControlTarget({
        baseUrl: "https://control.example",
        bearerToken: "control-token-private",
        fetch: async () => new Response("row conflict details", { status: 409 }),
      });
      const conflict = yield* Effect.flip(conflicting.createCheckpoint(createCommand));
      expect(conflict).toMatchObject({
        _tag: "IdempotencyConflict",
        idempotencyRef: createCommand.idempotencyRef,
        retryable: false,
      });
      expect(JSON.stringify(conflict)).not.toContain("row conflict details");
    }).pipe(Effect.runPromise));
});
