import { SQL } from "@openagentsinc/postgres-runtime";
import {
  MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
} from "@openagentsinc/managed-sandbox-contract";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  type ManagedSandboxPhase2StoredOperation,
  PostgresManagedSandboxPhase2Store,
} from "./managed-sandbox-phase2-store.js";
import { ManagedSandboxStoreError } from "./managed-sandbox-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const ownerRef = "owner.sbx10";
const tenantRef = "tenant.sbx10";
const observedAt = (minute: number): string =>
  new Date(Date.UTC(2026, 6, 22, 1, minute, 0)).toISOString();
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

const createCommand = (suffix: string) => ({
  _tag: "CreateCheckpoint" as const,
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: `command.sbx10.${suffix}.create`,
  idempotencyRef: `idempotency.sbx10.${suffix}.create`,
  ownerRef,
  tenantRef,
  requestedAt: observedAt(0),
  checkpointRef: `checkpoint.sbx10.${suffix}`,
  sourceSandboxRef: `sandbox.sbx10.${suffix}`,
  sourceResourceGeneration: 4,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.a70fbf9",
  repositoryPostImageDigest: digest("c"),
  formatRef: "format.sbx.content-tar.v1",
  retainedUntil: observedAt(59),
});

const checkpoint = (suffix: string): ManagedSandboxContentCheckpoint => ({
  schema: MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: `checkpoint.sbx10.${suffix}`,
  ownerRef,
  tenantRef,
  sourceSandboxRef: `sandbox.sbx10.${suffix}`,
  sourceResourceGeneration: 4,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.a70fbf9",
  repositoryPostImageDigest: digest("c"),
  contentDigest: digest("d"),
  contentBytes: 8_192,
  formatRef: "format.sbx.content-tar.v1",
  state: "completed",
  completedAt: observedAt(1),
  verifiedAt: observedAt(2),
  retainedUntil: observedAt(59),
  deleteOnExpiry: true,
  omissions,
  evidenceRefs: [`receipt.sbx10.${suffix}.verify`],
});

const createOperation = (suffix: string) =>
  ({
    command: createCommand(suffix),
    result: checkpoint(suffix),
  }) satisfies ManagedSandboxPhase2StoredOperation;

const deleteOperation = (suffix: string) => {
  const source = checkpoint(suffix);
  return {
    command: {
      _tag: "DeleteCheckpoint",
      schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
      commandRef: `command.sbx10.${suffix}.delete`,
      idempotencyRef: `idempotency.sbx10.${suffix}.delete`,
      ownerRef,
      tenantRef,
      requestedAt: observedAt(3),
      checkpointRef: source.checkpointRef,
      reason: "owner_requested",
    },
    result: {
      schema: MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
      receiptRef: `receipt.sbx10.${suffix}.delete`,
      ownerRef,
      tenantRef,
      checkpointRef: source.checkpointRef,
      sourceSandboxRef: source.sourceSandboxRef,
      sourceResourceGeneration: source.sourceResourceGeneration,
      contentDigest: source.contentDigest,
      contentDeleted: true,
      outcome: "deleted",
      reason: "owner_requested",
      deletedAt: observedAt(4),
      evidenceRefs: [`receipt.sbx10.${suffix}.object-delete`],
    },
  } satisfies ManagedSandboxPhase2StoredOperation;
};

describe.skipIf(!hasLocalPostgres())("SBX-10 managed sandbox Phase 2 Postgres store", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let store: PostgresManagedSandboxPhase2Store;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_managed_sandbox_phase2");
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_managed_sandbox_phase2") });
    sql = SQL({ url: pg.urlFor("khala_sync_managed_sandbox_phase2"), max: 10 });
    store = new PostgresManagedSandboxPhase2Store(sql as unknown as SyncSql);
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("atomically stores checkpoint metadata and replays exact command bytes", async () => {
    const operation = createOperation("replay");
    const settled = await store.settle({
      operation,
      checkpointMutation: { _tag: "Put", checkpoint: checkpoint("replay") },
    });
    expect(settled).toEqual(operation);
    expect(
      await store.readCheckpoint({
        ownerRef,
        tenantRef,
        checkpointRef: checkpoint("replay").checkpointRef,
      }),
    ).toEqual(checkpoint("replay"));

    const replay = await store.lookupOperation({
      ownerRef,
      tenantRef,
      commandRef: operation.command.commandRef,
      idempotencyRef: operation.command.idempotencyRef,
    });
    expect(replay).toEqual(operation);
    expect(
      await store.lookupOperation({
        ownerRef,
        tenantRef,
        commandRef: "command.sbx10.replay.lookup",
        idempotencyRef: operation.command.idempotencyRef,
      }),
    ).toEqual(operation);
    expect(
      await store.settle({
        operation,
        checkpointMutation: { _tag: "Put", checkpoint: checkpoint("replay") },
      }),
    ).toEqual(operation);
  });

  test("refuses cross-owner reads and conflicting command or idempotency bytes", async () => {
    const operation = createOperation("conflict");
    await store.settle({
      operation,
      checkpointMutation: { _tag: "Put", checkpoint: checkpoint("conflict") },
    });

    await expect(
      store.readCheckpoint({
        ownerRef: "owner.sbx10.other",
        tenantRef,
        checkpointRef: checkpoint("conflict").checkpointRef,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });
    await expect(
      store.lookupOperation({
        ownerRef: "owner.sbx10.other",
        tenantRef,
        commandRef: operation.command.commandRef,
        idempotencyRef: operation.command.idempotencyRef,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });

    await expect(
      store.settle({
        operation: {
          ...operation,
          command: { ...operation.command, retainedUntil: observedAt(58) },
        },
        checkpointMutation: { _tag: "Put", checkpoint: checkpoint("conflict") },
      }),
    ).rejects.toMatchObject({ code: "idempotency_conflict" });

    await expect(
      store.settle({
        operation: {
          ...operation,
          command: { ...operation.command, commandRef: "command.sbx10.conflict.other" },
        },
        checkpointMutation: { _tag: "Put", checkpoint: checkpoint("conflict") },
      }),
    ).rejects.toMatchObject({ code: "idempotency_conflict" });
  });

  test("rolls back the replay row when its checkpoint mutation fails", async () => {
    const operation = deleteOperation("missing");
    await expect(
      store.settle({
        operation,
        checkpointMutation: {
          _tag: "Delete",
          checkpointRef: checkpoint("missing").checkpointRef,
        },
      }),
    ).rejects.toMatchObject({ code: "not_found" });

    expect(
      await store.lookupOperation({
        ownerRef,
        tenantRef,
        commandRef: operation.command.commandRef,
        idempotencyRef: operation.command.idempotencyRef,
      }),
    ).toBeUndefined();
  });

  test("deletes checkpoint metadata only with its exact content proof", async () => {
    const create = createOperation("delete");
    await store.settle({
      operation: create,
      checkpointMutation: { _tag: "Put", checkpoint: checkpoint("delete") },
    });
    const deletion = deleteOperation("delete");
    await store.settle({
      operation: deletion,
      checkpointMutation: { _tag: "Delete", checkpointRef: checkpoint("delete").checkpointRef },
    });

    expect(
      await store.readCheckpoint({
        ownerRef,
        tenantRef,
        checkpointRef: checkpoint("delete").checkpointRef,
      }),
    ).toBeUndefined();
    expect(
      await store.lookupOperation({
        ownerRef,
        tenantRef,
        commandRef: deletion.command.commandRef,
        idempotencyRef: deletion.command.idempotencyRef,
      }),
    ).toEqual(deletion);
  });

  test("rejects schema-invalid or private settlement bytes before persistence", async () => {
    const operation = createOperation("unsafe");
    await expect(
      store.settle({
        operation: {
          ...operation,
          command: {
            ...operation.command,
            rawCredential: "Bearer should-not-persist",
          } as unknown as typeof operation.command,
        },
        checkpointMutation: { _tag: "Put", checkpoint: checkpoint("unsafe") },
      }),
    ).rejects.toBeInstanceOf(ManagedSandboxStoreError);
  });
});
