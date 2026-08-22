import { SQL } from "@openagentsinc/postgres-runtime";
import { canonicalJson } from "@openagentsinc/khala-sync";
import { createHash } from "node:crypto";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  cloudComputerWorkspaceKeyAuthorizer,
  checkpointWorkspaceStateDigest,
  CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS,
} from "./cloud-computer-checkpoint.js";
import {
  CloudComputerCheckpointStoreError,
  PostgresCloudComputerCheckpointStore,
  type CommitCloudComputerCheckpointInput,
} from "./cloud-computer-checkpoint-store.js";
import { GoogleCloudStorageCheckpoint } from "./cloud-computer-checkpoint-gcs.js";
import { runMigrations } from "./migrate.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const at = (seconds: number): string =>
  new Date(Date.UTC(2026, 7, 22, 18, 0, seconds)).toISOString();
const digest = (character: string): string => `sha256:${character.repeat(64)}`;
const jsonDigest = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;
const admittedEntry = {
  path: "README.md",
  kind: "file" as const,
  classification: "workspace" as const,
  mode: 0o100644,
  byteCount: 48,
  contentDigest: digest("e") as `sha256:${string}`,
  linkTarget: null,
};
const materializedStateDigest = checkpointWorkspaceStateDigest([admittedEntry]);

describe.skipIf(!hasLocalPostgres())("cloud computer checkpoint Postgres authority", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let store: PostgresCloudComputerCheckpointStore;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const databaseName = `khala_sync_cloud_checkpoint_${process.pid}_${Date.now()}`;
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe(`CREATE DATABASE ${databaseName}`);
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor(databaseName) });
    sql = SQL({ url: pg.urlFor(databaseName), max: 12 });
    store = new PostgresCloudComputerCheckpointStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO khala_sync_cloud_computers
        (computer_ref, owner_ref, tenant_ref, conversation_ref, work_unit_ref,
         kind, runtime_class, generation, version, runtime_profile_ref,
         authority_snapshot_digest, budget_snapshot_digest, capability_refs,
         state, created_at, updated_at)
      VALUES
        ('computer.checkpoint.main', 'owner.checkpoint.main', 'tenant.checkpoint.main',
         'conversation.checkpoint.main', 'work.checkpoint.main', 'interactive_retained',
         'standard', 1, 1, 'profile.checkpoint.standard', ${digest("a")}, ${digest("b")},
         '["capability.checkpoint.execute"]'::jsonb, 'cold', ${at(0)}, ${at(0)})
    `;
    await store.initializeWorkspace({
      workspaceRef: "workspace.checkpoint.main",
      computerRef: "computer.checkpoint.main",
      runtimeGeneration: 1,
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      conversationRef: "conversation.checkpoint.main",
      baseImageDigest: digest("c"),
      baseImageSignatureRef: "signature.checkpoint.base",
      workspaceKeyRef: "key.checkpoint.workspace",
      workspaceKeyVersion: 1,
      createdAt: at(0),
    });
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const prepareAndVerify = async (
    suffix: string,
    expectedWorkspaceRevision: number,
    expectedParentCheckpointRef: string | null,
    contentOverrides: Readonly<Record<string, unknown>> = {},
  ) => {
    const requestDigest = digest(suffix === "one" ? "1" : suffix === "two" ? "2" : "3");
    const operationRef = `operation.checkpoint.${suffix}`;
    const objectRef = `object.checkpoint.${suffix}`;
    const checkpointRef = `checkpoint.checkpoint.${suffix}`;
    const objectGeneration = expectedWorkspaceRevision + 1;
    const deletedPaths = expectedParentCheckpointRef === null ? [] : ["removed.txt"];
    const contentManifest = {
      schema: "openagents.cloud_computer_checkpoint_content.v1",
      checkpointRef,
      operationRef,
      requestDigest,
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      workspaceRef: "workspace.checkpoint.main",
      computerRef: "computer.checkpoint.main",
      sourceRuntimeGeneration: 1,
      expectedWorkspaceRevision,
      parentCheckpointRef: expectedParentCheckpointRef,
      baseImageDigest: digest("c"),
      checkpointKind: expectedParentCheckpointRef === null ? "full" : "delta",
      workspaceKeyRef: "key.checkpoint.workspace",
      workspaceKeyVersion: 1,
      entries: [{ ...admittedEntry }],
      excludedPaths: [...CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS],
      deletedPaths,
      plaintextByteCount: 48,
      contentDigest: digest("e"),
      workspaceStateDigest: materializedStateDigest,
      retainUntil: at(50),
      ...contentOverrides,
    };
    const storageManifest = {
      schema: "openagents.cloud_computer_checkpoint_storage.v1",
      operationRef,
      requestDigest,
      workspaceRef: "workspace.checkpoint.main",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      computerRef: "computer.checkpoint.main",
      objectRef,
      objectGeneration,
      contentManifestDigest: jsonDigest(contentManifest),
      ciphertextDigest: digest("d"),
      encryptedByteCount: 64,
      workspaceKeyRef: "key.checkpoint.workspace",
      workspaceKeyVersion: 1,
      wrappedDekRef: `wrapped-dek.checkpoint.${suffix}`,
      retainUntil: at(50),
    };
    await store.prepare({
      operationRef,
      idempotencyRef: `idempotency.checkpoint.${suffix}`,
      requestDigest,
      workspaceRef: "workspace.checkpoint.main",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      computerRef: "computer.checkpoint.main",
      expectedRuntimeGeneration: 1,
      expectedWorkspaceRevision,
      expectedParentCheckpointRef,
      boundary: suffix === "two" ? "interval" : "explicit",
      createdAt: at(expectedWorkspaceRevision + 1),
    });
    await store.recordUploadProgress({
      operationRef,
      requestDigest,
      uploadSessionRef: `upload.checkpoint.${suffix}`,
      uploadedByteCount: 64,
      uncertain: true,
      observedAt: at(expectedWorkspaceRevision + 2),
    });
    await store.recordVerifiedObject({
      operationRef,
      requestDigest,
      objectRef,
      workspaceRef: "workspace.checkpoint.main",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      objectUri: `gcs/checkpoint/private/${suffix}`,
      objectGeneration,
      contentDigest: digest("e"),
      contentManifestDigest: jsonDigest(contentManifest),
      storageManifestDigest: jsonDigest(storageManifest),
      ciphertextDigest: digest("d"),
      crc32c: `crc32c-${suffix}`,
      workspaceKeyRef: "key.checkpoint.workspace",
      workspaceKeyVersion: 1,
      wrappedDekRef: `wrapped-dek.checkpoint.${suffix}`,
      encryptedByteCount: 64,
      createdAt: at(expectedWorkspaceRevision + 1),
      verifiedAt: at(expectedWorkspaceRevision + 3),
      retainUntil: at(50),
    });
    const commit: CommitCloudComputerCheckpointInput = {
      operationRef,
      requestDigest,
      checkpointRef,
      objectRef,
      contentDigest: digest("e"),
      workspaceStateDigest: materializedStateDigest,
      contentManifestDigest: jsonDigest(contentManifest),
      storageManifestDigest: jsonDigest(storageManifest),
      baseImageDigest: digest("c"),
      checkpointKind: expectedParentCheckpointRef === null ? "full" : "delta",
      deletedPaths,
      plaintextByteCount: 48,
      encryptedByteCount: 64,
      retentionPolicyRef: "retention.checkpoint.default",
      verifiedAt: at(expectedWorkspaceRevision + 3),
      committedAt: at(expectedWorkspaceRevision + 4),
      retainUntil: at(50),
      contentManifest,
      storageManifest,
    };
    return { commit, operationRef, requestDigest, objectRef };
  };

  test("persists resumable upload state and replays a lost commit acknowledgement", async () => {
    const fixture = await prepareAndVerify("one", 0, null);
    await expect(
      store.commit({ ...fixture.commit, contentDigest: digest("f") }),
    ).rejects.toMatchObject({ code: "integrity_failed" });
    const first = await store.commit(fixture.commit);
    const replay = await store.commit(fixture.commit);
    expect(first).toMatchObject({ workspaceRevision: 1, replayed: false });
    expect(replay).toMatchObject({ checkpointRef: first.checkpointRef, replayed: true });
    const operation = await store.prepare({
      operationRef: fixture.operationRef,
      idempotencyRef: "idempotency.checkpoint.one",
      requestDigest: fixture.requestDigest,
      workspaceRef: "workspace.checkpoint.main",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      computerRef: "computer.checkpoint.main",
      expectedRuntimeGeneration: 1,
      expectedWorkspaceRevision: 0,
      expectedParentCheckpointRef: null,
      boundary: "explicit",
      createdAt: at(1),
    });
    expect(operation).toMatchObject({ status: "committed", uploadedByteCount: 64 });
  });

  test("keeps the previous checkpoint reachable after an incremental commit", async () => {
    const unsafe = await prepareAndVerify("unsafe", 1, "checkpoint.checkpoint.one", {
      entries: [
        {
          path: "packages/app/.env.production",
          kind: "file",
          classification: "workspace",
          mode: 0o100600,
          byteCount: 48,
          contentDigest: digest("e"),
          linkTarget: null,
        },
      ],
    });
    await expect(store.commit(unsafe.commit)).rejects.toMatchObject({
      code: "integrity_failed",
    });
    await sql`DELETE FROM khala_sync_cloud_checkpoint_objects WHERE operation_ref = ${unsafe.operationRef}`;
    await sql`DELETE FROM khala_sync_cloud_checkpoint_operations WHERE operation_ref = ${unsafe.operationRef}`;
    const invalidMode = await prepareAndVerify("invalid-mode", 1, "checkpoint.checkpoint.one", {
      entries: [{ ...admittedEntry, mode: -1 }],
    });
    await expect(store.commit(invalidMode.commit)).rejects.toMatchObject({
      code: "integrity_failed",
    });
    await sql`DELETE FROM khala_sync_cloud_checkpoint_objects WHERE operation_ref = ${invalidMode.operationRef}`;
    await sql`DELETE FROM khala_sync_cloud_checkpoint_operations WHERE operation_ref = ${invalidMode.operationRef}`;
    const fixture = await prepareAndVerify("two", 1, "checkpoint.checkpoint.one");
    await store.commit(fixture.commit);
    const rows: ReadonlyArray<{
      readonly checkpoint_ref: string;
      readonly status: string;
      readonly reference_kind: string | null;
    }> = await sql`
      SELECT checkpoint.checkpoint_ref, checkpoint.status,
             reference.kind AS reference_kind
      FROM khala_sync_cloud_computer_checkpoints AS checkpoint
      LEFT JOIN khala_sync_cloud_checkpoint_references AS reference
        ON reference.source_checkpoint_ref = checkpoint.checkpoint_ref
       AND reference.kind = 'parent' AND reference.state = 'live'
      WHERE checkpoint.workspace_ref = 'workspace.checkpoint.main'
      ORDER BY checkpoint.workspace_revision
    `;
    expect(rows).toEqual([
      {
        checkpoint_ref: "checkpoint.checkpoint.one",
        status: "superseded",
        reference_kind: "parent",
      },
      { checkpoint_ref: "checkpoint.checkpoint.two", status: "committed", reference_kind: null },
    ]);
  });

  test("holds a source checkpoint while an owner-authorized fork is resealing", async () => {
    await sql`
      INSERT INTO khala_sync_cloud_computers
        (computer_ref, owner_ref, tenant_ref, conversation_ref, work_unit_ref,
         kind, runtime_class, generation, version, runtime_profile_ref,
         authority_snapshot_digest, budget_snapshot_digest, capability_refs,
         state, created_at, updated_at)
      VALUES
        ('computer.checkpoint.fork', 'owner.checkpoint.main', 'tenant.checkpoint.main',
         'conversation.checkpoint.fork', 'work.checkpoint.fork', 'interactive_retained',
         'standard', 1, 1, 'profile.checkpoint.standard', ${digest("a")}, ${digest("b")},
         '[]'::jsonb, 'cold', ${at(8)}, ${at(8)})
    `;
    const authorizer = cloudComputerWorkspaceKeyAuthorizer({ authorize: async () => true });
    const targetKeyAuthorization = await authorizer.authorize({
      operation: "fork",
      actorRef: "actor.checkpoint.owner",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      workspaceRef: "workspace.checkpoint.fork",
      keyRef: "key.checkpoint.fork",
      keyVersion: 1,
    });
    const forkInput = {
      forkRef: "fork.checkpoint.main",
      sourceWorkspaceRef: "workspace.checkpoint.main",
      sourceCheckpointRef: "checkpoint.checkpoint.two",
      targetWorkspace: {
        workspaceRef: "workspace.checkpoint.fork",
        computerRef: "computer.checkpoint.fork",
        runtimeGeneration: 1,
        ownerRef: "owner.checkpoint.main",
        tenantRef: "tenant.checkpoint.main",
        conversationRef: "conversation.checkpoint.fork",
        baseImageDigest: digest("c"),
        baseImageSignatureRef: "signature.checkpoint.base",
        workspaceKeyRef: "key.checkpoint.fork",
        workspaceKeyVersion: 1,
        createdAt: at(8),
      },
      targetKeyAuthorization,
      createdAt: at(8),
    };
    await expect(
      store.authorizeFork({
        ...forkInput,
        targetKeyAuthorization: { ...targetKeyAuthorization } as typeof targetKeyAuthorization,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });
    await store.authorizeFork(forkInput);
    await expect(
      store.beginDestroy({
        destroyRef: "destroy.checkpoint.blocked",
        workspaceRef: "workspace.checkpoint.main",
        ownerRef: "owner.checkpoint.main",
        tenantRef: "tenant.checkpoint.main",
        expectedRuntimeGeneration: 1,
        evidenceDigest: digest("9"),
        observedAt: at(9),
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    const references: ReadonlyArray<{ readonly state: string }> = await sql`
      SELECT state FROM khala_sync_cloud_checkpoint_references
      WHERE reference_ref = 'reference.fork-source.fork.checkpoint.main'
    `;
    expect(references).toEqual([{ state: "live" }]);
    await sql`
      INSERT INTO khala_sync_cloud_checkpoint_operations
        (operation_ref, idempotency_ref, request_digest, workspace_ref, owner_ref,
         tenant_ref, computer_ref, expected_runtime_generation,
         expected_workspace_revision, boundary, status, object_ref,
         object_generation, checkpoint_ref, result_digest, receipt_json,
         created_at, updated_at, completed_at)
      VALUES
        ('operation.checkpoint.fork', 'idempotency.checkpoint.fork', ${digest("8")},
         'workspace.checkpoint.fork', 'owner.checkpoint.main', 'tenant.checkpoint.main',
         'computer.checkpoint.fork', 1, 0, 'fork', 'committed',
         'object.checkpoint.fork', 1, 'checkpoint.checkpoint.fork', ${digest("8")},
         '{}'::jsonb, ${at(8)}, ${at(9)}, ${at(9)})
    `;
    await sql`
      INSERT INTO khala_sync_cloud_checkpoint_objects
        (object_ref, workspace_ref, operation_ref, owner_ref, tenant_ref, object_uri,
         object_generation, content_digest, content_manifest_digest,
         storage_manifest_digest, ciphertext_digest, crc32c, workspace_key_ref,
         workspace_key_version, wrapped_dek_ref, encrypted_byte_count, state,
         created_at, verified_at, retain_until)
      VALUES
        ('object.checkpoint.fork', 'workspace.checkpoint.fork', 'operation.checkpoint.fork',
         'owner.checkpoint.main', 'tenant.checkpoint.main', 'gcs/checkpoint/private/fork',
         1, ${digest("f")}, ${digest("1")}, ${digest("2")}, ${digest("3")}, 'crc32c-fork',
         'key.checkpoint.fork', 1, 'wrapped-dek.checkpoint.fork', 64, 'reachable',
         ${at(8)}, ${at(9)}, ${at(50)})
    `;
    await sql`
      INSERT INTO khala_sync_cloud_computer_checkpoints
        (checkpoint_ref, operation_ref, workspace_ref, owner_ref, tenant_ref,
         computer_ref, source_runtime_generation, workspace_revision,
         object_ref, content_digest, workspace_state_digest, content_manifest_digest,
         storage_manifest_digest, base_image_digest, checkpoint_kind,
         deleted_paths_json, plaintext_byte_count, encrypted_byte_count,
         retention_policy_ref, status, verified_at, committed_at, retain_until,
         content_manifest_json, storage_manifest_json)
      VALUES
        ('checkpoint.checkpoint.fork', 'operation.checkpoint.fork',
         'workspace.checkpoint.fork', 'owner.checkpoint.main', 'tenant.checkpoint.main',
         'computer.checkpoint.fork', 1, 1, 'object.checkpoint.fork', ${digest("f")},
         ${materializedStateDigest}, ${digest("1")}, ${digest("2")}, ${digest("c")}, 'full', '[]'::jsonb,
         48, 64, 'retention.checkpoint.default', 'committed', ${at(9)}, ${at(9)},
         ${at(50)}, '{}'::jsonb, '{}'::jsonb)
    `;
    await store.completeFork({
      forkRef: "fork.checkpoint.main",
      targetCheckpointRef: "checkpoint.checkpoint.fork",
      completedAt: at(10),
    });
    const completedForks: ReadonlyArray<{
      readonly state: string;
      readonly content_digest: string;
    }> = await sql`
        SELECT fork.state, checkpoint.content_digest
        FROM khala_sync_cloud_computer_workspace_forks AS fork
        JOIN khala_sync_cloud_computer_checkpoints AS checkpoint
          ON checkpoint.checkpoint_ref = fork.resealed_checkpoint_ref
        WHERE fork.fork_ref = 'fork.checkpoint.main'
      `;
    expect(completedForks).toEqual([{ state: "resealed", content_digest: digest("f") }]);
    await sql`DELETE FROM khala_sync_cloud_checkpoint_references WHERE workspace_ref = 'workspace.checkpoint.fork'`;
    await sql`DELETE FROM khala_sync_cloud_computer_workspace_forks WHERE fork_ref = 'fork.checkpoint.main'`;
    await sql`DELETE FROM khala_sync_cloud_computer_checkpoints WHERE workspace_ref = 'workspace.checkpoint.fork'`;
    await sql`DELETE FROM khala_sync_cloud_checkpoint_objects WHERE workspace_ref = 'workspace.checkpoint.fork'`;
    await sql`DELETE FROM khala_sync_cloud_checkpoint_operations WHERE workspace_ref = 'workspace.checkpoint.fork'`;
    await sql`DELETE FROM khala_sync_cloud_computer_workspaces WHERE workspace_ref = 'workspace.checkpoint.fork'`;
    await sql`DELETE FROM khala_sync_cloud_computers WHERE computer_ref = 'computer.checkpoint.fork'`;
  });

  test("rejects stale runtime commits while preserving the logical workspace", async () => {
    const staleFixture = await prepareAndVerify("three", 2, "checkpoint.checkpoint.two");
    await sql`
      UPDATE khala_sync_cloud_computers
      SET generation = 2, version = version + 1, updated_at = ${at(10)}
      WHERE computer_ref = 'computer.checkpoint.main' AND generation = 1
    `;
    await expect(store.commit(staleFixture.commit)).rejects.toMatchObject({
      code: "stale_generation",
    });
    expect(
      await store.reconcileStaleOperations({
        workspaceRef: "workspace.checkpoint.main",
        observedAt: at(10),
      }),
    ).toBe(1);
    const orphaned: ReadonlyArray<{
      readonly operation_status: string;
      readonly object_state: string;
    }> = await sql`
        SELECT operation.status AS operation_status, object.state AS object_state
        FROM khala_sync_cloud_checkpoint_operations AS operation
        JOIN khala_sync_cloud_checkpoint_objects AS object
          ON object.operation_ref = operation.operation_ref
        WHERE operation.operation_ref = ${staleFixture.operationRef}
      `;
    expect(orphaned).toEqual([{ operation_status: "orphaned", object_state: "tombstoned" }]);
    await store.advanceRuntimeGeneration({
      workspaceRef: "workspace.checkpoint.main",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      expectedRuntimeGeneration: 1,
      nextRuntimeGeneration: 2,
      observedAt: at(10),
    });
    await expect(
      store.prepare({
        operationRef: "operation.checkpoint.stale",
        idempotencyRef: "idempotency.checkpoint.stale",
        requestDigest: digest("4"),
        workspaceRef: "workspace.checkpoint.main",
        ownerRef: "owner.checkpoint.main",
        tenantRef: "tenant.checkpoint.main",
        computerRef: "computer.checkpoint.main",
        expectedRuntimeGeneration: 1,
        expectedWorkspaceRevision: 2,
        expectedParentCheckpointRef: "checkpoint.checkpoint.two",
        boundary: "stop",
        createdAt: at(11),
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    const plan = await store.restorePlan({
      workspaceRef: "workspace.checkpoint.main",
      ownerRef: "owner.checkpoint.main",
      tenantRef: "tenant.checkpoint.main",
      expectedRuntimeGeneration: 2,
    });
    expect(plan).toMatchObject({
      checkpointRef: "checkpoint.checkpoint.two",
      workspaceRevision: 2,
      checkpointKind: "delta",
      deletedPaths: ["removed.txt"],
    });
    expect(plan.layers.map((layer) => layer.checkpointRef)).toEqual([
      "checkpoint.checkpoint.one",
      "checkpoint.checkpoint.two",
    ]);
    await sql`
      UPDATE khala_sync_cloud_computers
      SET generation = 3, version = version + 1, updated_at = ${at(12)}
      WHERE computer_ref = 'computer.checkpoint.main' AND generation = 2
    `;
    await expect(
      store.restorePlan({
        workspaceRef: "workspace.checkpoint.main",
        ownerRef: "owner.checkpoint.main",
        tenantRef: "tenant.checkpoint.main",
        expectedRuntimeGeneration: 2,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });
    await sql`
      UPDATE khala_sync_cloud_computers
      SET generation = 2, version = version + 1, updated_at = ${at(13)}
      WHERE computer_ref = 'computer.checkpoint.main' AND generation = 3
    `;
  });

  test("fails cross-owner restore before exposing private object metadata", async () => {
    await expect(
      store.restorePlan({
        workspaceRef: "workspace.checkpoint.main",
        ownerRef: "owner.checkpoint.foreign",
        tenantRef: "tenant.checkpoint.main",
        expectedRuntimeGeneration: 2,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });
  });

  test("tombstones retained checkpoints and records generation-specific deletion evidence", async () => {
    expect(
      await store.beginDestroy({
        destroyRef: "destroy.checkpoint.main",
        workspaceRef: "workspace.checkpoint.main",
        ownerRef: "owner.checkpoint.main",
        tenantRef: "tenant.checkpoint.main",
        expectedRuntimeGeneration: 2,
        evidenceDigest: digest("5"),
        observedAt: at(40),
      }),
    ).toBe(2);
    expect(await store.gcCandidates(at(49))).toHaveLength(0);
    const candidates = await store.gcCandidates(at(51));
    expect(candidates).toHaveLength(3);
    await expect(
      store.confirmObjectDeleted({
        deletionRef: "delete.checkpoint.unverified",
        workspaceRef: candidates[0]!.workspaceRef,
        objectRef: candidates[0]!.objectRef,
        objectGeneration: candidates[0]!.objectGeneration,
        verification: {
          schema: "openagents.cloud_computer_gcs_deletion_verification.v1",
          objectRef: "object.checkpoint.wrong",
          objectGeneration: String(candidates[0]!.objectGeneration),
          generationPreconditionMet: true,
          allVersionsAbsent: true,
        },
        keyDisposition: "retained",
        observedAt: at(52),
      }),
    ).rejects.toMatchObject({ code: "integrity_failed" });
    for (const [index, candidate] of candidates.entries()) {
      const responses = [204, 404, 404, 200];
      const storage = new GoogleCloudStorageCheckpoint({
        bucket: "checkpoint-test",
        authorizationHeaders: async () => ({}),
        http: {
          request: async () => ({
            status: responses.shift()!,
            headers: {},
            body: responses.length === 0 ? {} : null,
          }),
        },
        resumableUploads: {
          load: async () => null,
          save: async () => undefined,
          remove: async () => undefined,
        },
      });
      // eslint-disable-next-line no-await-in-loop -- each deletion proof is generation-specific.
      const verification = await storage.deleteGeneration({
        objectRef: candidate.objectRef,
        generation: String(candidate.objectGeneration),
        byteCount: candidate.encryptedByteCount,
        ciphertextDigest: candidate.ciphertextDigest as `sha256:${string}`,
        state: "tombstoned",
      });
      // eslint-disable-next-line no-await-in-loop -- destruction settles each exact GCS generation.
      await store.confirmObjectDeleted({
        deletionRef: `delete.checkpoint.${index}`,
        workspaceRef: candidate.workspaceRef,
        objectRef: candidate.objectRef,
        objectGeneration: candidate.objectGeneration,
        verification,
        keyDisposition: index === candidates.length - 1 ? "destroyed" : "retained",
        observedAt: at(52 + index),
      });
    }
    const rows: ReadonlyArray<{ readonly state: string; readonly count: string | number }> =
      await sql`
        SELECT state, COUNT(*) AS count
        FROM khala_sync_cloud_checkpoint_objects
        GROUP BY state
      `;
    expect(rows.map((row) => ({ state: row.state, count: Number(row.count) }))).toEqual([
      { state: "deleted", count: 3 },
    ]);
    const workspace: ReadonlyArray<{ readonly state: string }> = await sql`
      SELECT state FROM khala_sync_cloud_computer_workspaces
      WHERE workspace_ref = 'workspace.checkpoint.main'
    `;
    expect(workspace[0]?.state).toBe("destroyed");
  });

  test("uses typed store errors", () => {
    expect(new CloudComputerCheckpointStoreError("conflict", "fixture")).toBeInstanceOf(Error);
  });
});
