import {
  ManagedSandboxStoreError,
  PostgresManagedSandboxPhase2Store,
  type SyncSql,
} from "@openagentsinc/khala-sync-server";
import { runMigrations } from "@openagentsinc/khala-sync-server/migrate";
import {
  type LocalPostgres,
  hasLocalPostgres,
  startLocalPostgres,
} from "@openagentsinc/khala-sync-server/test/local-postgres";
import {
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
import postgresClient from "postgres";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { makeManagedSandboxPhase2PostgresStore } from "./managed-sandbox-phase2-postgres-store";
import {
  type ManagedSandboxPhase2Target,
  makeManagedSandboxPhase2Service,
} from "./managed-sandbox-phase2-service";

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
  checkpointRef: "checkpoint.sbx10.postgres-adapter",
  ownerRef: "owner.sbx10.postgres-adapter",
  tenantRef: "tenant.sbx10.postgres-adapter",
  sourceSandboxRef: "sandbox.sbx10.postgres-adapter",
  sourceResourceGeneration: 3,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.6299b3f",
  repositoryPostImageDigest: digest("c"),
  contentDigest: digest("d"),
  contentBytes: 8_192,
  formatRef: "format.sbx.content-tar.v1",
  state: "completed",
  completedAt: "2026-07-22T02:20:01.000Z",
  verifiedAt: "2026-07-22T02:20:02.000Z",
  retainedUntil: "2026-07-23T02:20:00.000Z",
  deleteOnExpiry: true,
  omissions,
  evidenceRefs: ["receipt.sbx10.postgres-adapter.verify"],
};

const command = {
  _tag: "CreateCheckpoint" as const,
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.postgres-adapter.create",
  idempotencyRef: "idempotency.sbx10.postgres-adapter.create",
  ownerRef: checkpoint.ownerRef,
  tenantRef: checkpoint.tenantRef,
  requestedAt: "2026-07-22T02:20:00.000Z",
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
};

const unused = () => Effect.die("unused Phase 2 target operation");

describe.skipIf(!hasLocalPostgres())("SBX-10 Phase 2 Effect Postgres adapter", () => {
  let localPostgres: LocalPostgres;
  let sql: ReturnType<typeof postgresClient>;

  beforeAll(async () => {
    localPostgres = await startLocalPostgres();
    await runMigrations({ databaseUrl: localPostgres.url });
    sql = postgresClient(localPostgres.url, { max: 4, prepare: false });
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (localPostgres !== undefined) await localPostgres.stop();
  });

  test("commits and replays exact checkpoint bytes through the Effect coordinator", () =>
    Effect.gen(function* () {
      let createCalls = 0;
      const target: ManagedSandboxPhase2Target = {
        createCheckpoint: () =>
          Effect.sync(() => {
            createCalls += 1;
            return checkpoint;
          }),
        archiveWithCheckpoint: unused,
        verifyCheckpoint: () => Effect.succeed(true),
        observeResourceGeneration: () => Effect.succeed(3),
        forkFromCheckpoint: unused,
        restoreCheckpoint: unused,
        deleteCheckpoint: unused,
      };
      const postgresStore = new PostgresManagedSandboxPhase2Store(sql as unknown as SyncSql);
      const store = makeManagedSandboxPhase2PostgresStore(postgresStore);
      const service = makeManagedSandboxPhase2Service({ store, target });

      const first = yield* service.execute(command);
      const replay = yield* service.execute(command);
      const storedCheckpoint = yield* store.readCheckpoint({
        ownerRef: command.ownerRef,
        tenantRef: command.tenantRef,
        checkpointRef: command.checkpointRef,
      });

      expect(first).toEqual(checkpoint);
      expect(replay).toEqual(first);
      expect(storedCheckpoint).toEqual(checkpoint);
      expect(createCalls).toBe(1);

      const rows = yield* Effect.promise(
        () => sql<[{ operation_count: number }]>`
            SELECT COUNT(*)::integer AS operation_count
            FROM khala_sync_managed_sandbox_phase2_operations
            WHERE command_ref = ${command.commandRef}
          `,
      );
      expect(rows[0]?.operation_count).toBe(1);
    }).pipe(Effect.runPromise));
});

test("redacts Postgres and integrity failures into closed Phase 2 errors", () =>
  Effect.gen(function* () {
    const conflictStore = makeManagedSandboxPhase2PostgresStore({
      lookupOperation: async () => {
        throw new ManagedSandboxStoreError("idempotency_conflict", "private row and SQL details");
      },
      readCheckpoint: async () => {
        throw new ManagedSandboxStoreError("corrupt_store", "private row bytes");
      },
      settle: async () => {
        throw new Error("/Users/private/database.sock");
      },
    });

    const conflict = yield* Effect.flip(
      conflictStore.lookupOperation({
        ownerRef: command.ownerRef,
        tenantRef: command.tenantRef,
        commandRef: command.commandRef,
        idempotencyRef: command.idempotencyRef,
      }),
    );
    const corrupt = yield* Effect.flip(
      conflictStore.readCheckpoint({
        ownerRef: command.ownerRef,
        tenantRef: command.tenantRef,
        checkpointRef: command.checkpointRef,
      }),
    );
    const unavailable = yield* Effect.flip(
      conflictStore.settle({
        operation: { command, result: checkpoint },
        checkpointMutation: { _tag: "Put", checkpoint },
      }),
    );

    expect(conflict).toMatchObject({
      _tag: "IdempotencyConflict",
      idempotencyRef: command.idempotencyRef,
      retryable: false,
    });
    expect(corrupt).toMatchObject({
      _tag: "CheckpointCorrupt",
      checkpointRef: command.checkpointRef,
      retryable: false,
    });
    expect(unavailable).toMatchObject({
      _tag: "InvalidRequest",
      message: "Phase 2 storage is unavailable",
      requestRef: command.commandRef,
      retryable: true,
    });
    expect(JSON.stringify([conflict, corrupt, unavailable])).not.toContain("private");
  }).pipe(Effect.runPromise));
