import { SQL } from "@openagentsinc/postgres-runtime";
import {
  MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxPrivateIngressCapability,
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

const ingressCapability = (): ManagedSandboxPrivateIngressCapability => ({
  _tag: "Active",
  schema: MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION,
  capabilityRef: "capability.sbx10.ingress.store",
  sandboxRef: "sandbox.sbx10.ingress.store",
  resourceGeneration: 4,
  ownerRef,
  audienceRef: "audience.sbx10.owner-device",
  kind: "preview",
  issuedAt: observedAt(5),
  expiresAt: observedAt(10),
  ttlSeconds: 300,
  accessUrlDigest: digest("e"),
  accessUrlAtRest: "redacted",
  audiencePolicy: "owner_scoped_explicit_audience",
  publicAccess: false,
  permanentRoute: false,
  vnc: "unsupported",
  auditRefs: ["audit.sbx10.ingress.create"],
});

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

  test("stores only the ingress URL digest and atomically settles revoke cleanup", async () => {
    const active = ingressCapability();
    const create = {
      command: {
        _tag: "CreatePrivateIngress" as const,
        schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
        commandRef: "command.sbx10.ingress.store.create",
        idempotencyRef: "idempotency.sbx10.ingress.store.create",
        ownerRef,
        tenantRef,
        requestedAt: observedAt(5),
        sandboxRef: active.sandboxRef,
        resourceGeneration: active.resourceGeneration,
        audienceRef: active.audienceRef,
        kind: active.kind,
        ttlSeconds: active.ttlSeconds,
      },
      result: active,
    } satisfies ManagedSandboxPhase2StoredOperation;
    await store.settle({
      operation: create,
      checkpointMutation: { _tag: "PutIngress", capability: active },
    });
    expect(
      await store.readPrivateIngress({ ownerRef, tenantRef, capabilityRef: active.capabilityRef }),
    ).toEqual(active);
    expect(
      await store.readPrivateIngressForAudience({
        audienceRef: active.audienceRef,
        capabilityRef: active.capabilityRef,
      }),
    ).toEqual(active);
    expect(
      await store.readPrivateIngressForAudience({
        audienceRef: "audience.sbx10.other-device",
        capabilityRef: active.capabilityRef,
      }),
    ).toBeUndefined();

    const cleaned: ManagedSandboxPrivateIngressCapability = {
      ...active,
      _tag: "Cleaned",
      terminalState: "revoked",
      cleanedAt: observedAt(6),
      cleanupReceiptRef: "receipt.sbx10.ingress.store.cleanup",
      auditRefs: [...active.auditRefs, "audit.sbx10.ingress.revoke"],
    };
    const revoke = {
      command: {
        _tag: "RevokePrivateIngress" as const,
        schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
        commandRef: "command.sbx10.ingress.store.revoke",
        idempotencyRef: "idempotency.sbx10.ingress.store.revoke",
        ownerRef,
        tenantRef,
        requestedAt: observedAt(6),
        capabilityRef: active.capabilityRef,
        sandboxRef: active.sandboxRef,
        resourceGeneration: active.resourceGeneration,
      },
      result: cleaned,
    } satisfies ManagedSandboxPhase2StoredOperation;
    await store.settle({
      operation: revoke,
      checkpointMutation: { _tag: "PutIngress", capability: cleaned },
    });
    expect(
      await store.readPrivateIngress({ ownerRef, tenantRef, capabilityRef: active.capabilityRef }),
    ).toEqual(cleaned);
    expect(
      await store.readPrivateIngressForAudience({
        audienceRef: active.audienceRef,
        capabilityRef: active.capabilityRef,
      }),
    ).toEqual(cleaned);
    const rows = (await sql`
      SELECT capability_json, access_url_digest
      FROM khala_sync_managed_sandbox_private_ingress
      WHERE capability_ref = ${active.capabilityRef}
    `) as ReadonlyArray<{ capability_json: unknown; access_url_digest: string }>;
    expect(rows[0]?.access_url_digest).toBe(active.accessUrlDigest);
    expect(JSON.stringify(rows[0]?.capability_json)).not.toContain("https://");
  });
});
