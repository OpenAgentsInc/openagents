import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test, vi } from "vite-plus/test";

import {
  PostgresCloudComputerCheckpointStore,
  type CloudComputerCheckpointRestorePlan,
} from "./cloud-computer-checkpoint-store.js";
import {
  CloudComputerLifecycle,
  PostgresCloudComputerLifecycleStateStore,
  type CloudComputerLifecycleStateStore,
} from "./cloud-computer-lifecycle.js";
import { runMigrations } from "./migrate.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const digest = (character: string): string => `sha256:${character.repeat(64)}`;
const scope = {
  computerRef: "computer.lifecycle.one",
  workspaceRef: "workspace.lifecycle.one",
  ownerRef: "owner.lifecycle.one",
  tenantRef: "tenant.lifecycle.one",
} as const;
const plan: CloudComputerCheckpointRestorePlan = {
  schema: "openagents.cloud_computer_restore_plan.v1",
  workspaceRef: scope.workspaceRef,
  checkpointRef: "checkpoint.lifecycle.two",
  workspaceRevision: 2,
  objectRef: "object.lifecycle.two",
  objectGeneration: 2,
  contentDigest: digest("1"),
  contentManifestDigest: digest("2"),
  storageManifestDigest: digest("3"),
  ciphertextDigest: digest("4"),
  workspaceKeyRef: "key.lifecycle.one",
  workspaceKeyVersion: 1,
  baseImageDigest: digest("5"),
  checkpointKind: "delta",
  deletedPaths: ["removed.txt"],
  encryptedByteCount: 10,
  contentManifest: {},
  storageManifest: {},
  layers: [],
};

const fixture = () => {
  const events: Array<string> = [];
  const state: CloudComputerLifecycleStateStore = {
    beginStop: vi.fn(async () => {
      events.push("stopping");
    }),
    finishStop: vi.fn(async (input) => {
      events.push(input.outcome);
    }),
    beginRestore: vi.fn(async () => {
      events.push("computer-generation");
    }),
    finishRestore: vi.fn(async (input) => {
      events.push(input.outcome);
    }),
    recordHostLoss: vi.fn(async () => {
      events.push("host-loss-evidence");
    }),
  };
  const checkpoints = {
    advanceRuntimeGeneration: vi.fn(async () => {
      events.push("workspace-generation");
    }),
    restorePlan: vi.fn(async () => {
      events.push("full-chain-plan");
      return plan;
    }),
  } as unknown as PostgresCloudComputerCheckpointStore;
  return { events, state, checkpoints, lifecycle: new CloudComputerLifecycle(state, checkpoints) };
};

describe("cloud computer lifecycle orchestration", () => {
  test("moves stopping to cold only after the stop checkpoint commits", async () => {
    const { lifecycle, events, state } = fixture();
    const result = await lifecycle.stop(
      { ...scope, generation: 4, observedAt: "2026-08-22T20:00:00.000Z" },
      async () => {
        events.push("checkpoint-committed");
        return { checkpointRef: "checkpoint.lifecycle.stop" };
      },
    );

    expect(events).toEqual(["stopping", "checkpoint-committed", "cold"]);
    expect(result).toEqual({ state: "cold", checkpointRef: "checkpoint.lifecycle.stop" });
    expect(state.finishStop).toHaveBeenCalledWith(
      expect.objectContaining({ outcome: "cold", checkpointRef: "checkpoint.lifecycle.stop" }),
    );
  });

  test("moves stopping to failed when the stop checkpoint fails", async () => {
    const { lifecycle, events, state } = fixture();
    await expect(
      lifecycle.stop(
        { ...scope, generation: 4, observedAt: "2026-08-22T20:00:00.000Z" },
        async () => {
          events.push("checkpoint-failed");
          throw new Error("upload failed");
        },
      ),
    ).rejects.toThrow("upload failed");
    expect(events).toEqual(["stopping", "checkpoint-failed", "failed"]);
    expect(state.finishStop).toHaveBeenCalledWith(
      expect.objectContaining({ outcome: "failed", checkpointRef: null }),
    );
  });

  test("advances both generations before full-chain restore and activating the lease", async () => {
    const { lifecycle, events, checkpoints, state } = fixture();
    const result = await lifecycle.resume(
      {
        ...scope,
        expectedGeneration: 4,
        nextGeneration: 5,
        providerLeaseRef: "lease.lifecycle.five",
        observedAt: "2026-08-22T20:01:00.000Z",
      },
      async ({ plan: restorePlan, expectedBaseImageDigest }) => {
        events.push("restored");
        expect(restorePlan.layers).toBe(plan.layers);
        expect(expectedBaseImageDigest).toBe(digest("5"));
      },
    );

    expect(events).toEqual([
      "computer-generation",
      "workspace-generation",
      "full-chain-plan",
      "restored",
      "active",
    ]);
    expect(checkpoints.restorePlan).toHaveBeenCalledWith(
      expect.objectContaining({ expectedRuntimeGeneration: 5 }),
    );
    expect(state.finishRestore).toHaveBeenCalledWith(
      expect.objectContaining({ generation: 5, providerLeaseRef: "lease.lifecycle.five" }),
    );
    expect(result.state).toBe("active");
  });

  test("records host-loss evidence before advancing and restoring a replacement", async () => {
    const { lifecycle, events, state } = fixture();
    await lifecycle.replaceLostHost(
      {
        ...scope,
        evidenceRef: "evidence.lifecycle.host-loss",
        evidenceDigest: digest("6"),
        expectedGeneration: 4,
        nextGeneration: 5,
        lostProviderLeaseRef: "lease.lifecycle.lost",
        replacementProviderLeaseRef: "lease.lifecycle.replacement",
        observedAt: "2026-08-22T20:02:00.000Z",
      },
      async () => {
        events.push("restored");
      },
    );

    expect(events).toEqual([
      "host-loss-evidence",
      "computer-generation",
      "workspace-generation",
      "full-chain-plan",
      "restored",
      "active",
    ]);
    expect(state.recordHostLoss).toHaveBeenCalledWith(
      expect.objectContaining({
        generation: 4,
        providerLeaseRef: "lease.lifecycle.lost",
        evidenceDigest: digest("6"),
      }),
    );
  });

  test("marks a new generation failed when restore does not complete", async () => {
    const { lifecycle, events, state } = fixture();
    await expect(
      lifecycle.resume(
        {
          ...scope,
          expectedGeneration: 4,
          nextGeneration: 5,
          providerLeaseRef: "lease.lifecycle.five",
          observedAt: "2026-08-22T20:03:00.000Z",
        },
        async () => {
          events.push("restore-failed");
          throw new Error("corrupt checkpoint");
        },
      ),
    ).rejects.toThrow("corrupt checkpoint");
    expect(events.at(-1)).toBe("failed");
    expect(state.finishRestore).toHaveBeenLastCalledWith(
      expect.objectContaining({ generation: 5, providerLeaseRef: null, outcome: "failed" }),
    );
  });
});

describe.skipIf(!hasLocalPostgres())("cloud computer lifecycle Postgres state", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let state: PostgresCloudComputerLifecycleStateStore;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const databaseName = `khala_sync_cloud_lifecycle_${process.pid}_${Date.now()}`;
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe(`CREATE DATABASE ${databaseName}`);
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor(databaseName) });
    sql = SQL({ url: pg.urlFor(databaseName), max: 4 });
    const checkpointStore = new PostgresCloudComputerCheckpointStore(sql as unknown as SyncSql);
    state = new PostgresCloudComputerLifecycleStateStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO khala_sync_cloud_computers
        (computer_ref, owner_ref, tenant_ref, conversation_ref, work_unit_ref,
         kind, runtime_class, generation, version, runtime_profile_ref,
         authority_snapshot_digest, budget_snapshot_digest, capability_refs,
         state, active_lease_ref, created_at, updated_at)
      VALUES
        (${scope.computerRef}, ${scope.ownerRef}, ${scope.tenantRef},
         'conversation.lifecycle.one', 'work.lifecycle.one', 'interactive_retained',
         'standard', 1, 1, 'profile.lifecycle.standard', ${digest("a")}, ${digest("b")},
         '[]'::jsonb, 'active', 'lease.lifecycle.one',
         '2026-08-22T20:00:00.000Z', '2026-08-22T20:00:00.000Z')
    `;
    await checkpointStore.initializeWorkspace({
      ...scope,
      runtimeGeneration: 1,
      conversationRef: "conversation.lifecycle.one",
      baseImageDigest: digest("5"),
      baseImageSignatureRef: "signature.lifecycle.base",
      workspaceKeyRef: "key.lifecycle.one",
      workspaceKeyVersion: 1,
      createdAt: "2026-08-22T20:00:00.000Z",
    });
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("persists stop, generation, activation, and host-loss transitions", async () => {
    await state.beginStop({ ...scope, generation: 1, observedAt: "2026-08-22T20:01:00.000Z" });
    await sql`
      UPDATE khala_sync_cloud_computers
      SET latest_checkpoint_ref = 'checkpoint.lifecycle.stop'
      WHERE computer_ref = ${scope.computerRef}
    `;
    await state.finishStop({
      ...scope,
      generation: 1,
      checkpointRef: "checkpoint.lifecycle.stop",
      outcome: "cold",
      observedAt: "2026-08-22T20:02:00.000Z",
    });
    await state.beginRestore({
      ...scope,
      expectedGeneration: 1,
      nextGeneration: 2,
      observedAt: "2026-08-22T20:03:00.000Z",
    });
    await state.finishRestore({
      ...scope,
      generation: 2,
      providerLeaseRef: "lease.lifecycle.two",
      outcome: "active",
      observedAt: "2026-08-22T20:04:00.000Z",
    });
    await state.recordHostLoss({
      ...scope,
      evidenceRef: "evidence.lifecycle.loss",
      generation: 2,
      providerLeaseRef: "lease.lifecycle.two",
      evidenceDigest: digest("6"),
      observedAt: "2026-08-22T20:05:00.000Z",
    });
    await state.recordHostLoss({
      ...scope,
      evidenceRef: "evidence.lifecycle.loss",
      generation: 2,
      providerLeaseRef: "lease.lifecycle.two",
      evidenceDigest: digest("6"),
      observedAt: "2026-08-22T20:05:00.000Z",
    });

    const computers: ReadonlyArray<{
      state: string;
      generation: string | number;
      active_lease_ref: string | null;
    }> = await sql`
      SELECT state, generation, active_lease_ref
      FROM khala_sync_cloud_computers WHERE computer_ref = ${scope.computerRef}
    `;
    expect(computers[0]).toMatchObject({ state: "failed", active_lease_ref: null });
    expect(Number(computers[0]?.generation)).toBe(2);
    const evidence: ReadonlyArray<{ evidence_digest: string }> = await sql`
      SELECT evidence_digest FROM khala_sync_cloud_computer_host_loss_evidence
      WHERE evidence_ref = 'evidence.lifecycle.loss'
    `;
    expect(evidence).toEqual([{ evidence_digest: digest("6") }]);
  });
});
