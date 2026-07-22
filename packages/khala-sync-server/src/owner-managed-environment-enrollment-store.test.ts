import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { PostgresOwnerManagedEnvironmentEnrollmentStore } from "./owner-managed-environment-enrollment-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const now = "2026-07-22T09:00:00.000Z";
const ownerUserId = "owner.ide13.owner-managed";
const ownerAgentUserId = "agent.ide13.owner-managed";
const pylonRef = "pylon.ide13.owner-managed";
const targetRef = "target.ide13.owner-managed";

describe.skipIf(!hasLocalPostgres())("IDE-13 owner-managed environment enrollment", () => {
  let pg: LocalPostgres;
  let sql: SQL;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_owner_managed");
    await admin.end();
    const result = await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_ide13_owner_managed"),
    });
    expect(result.applied).toContain("0093_owner_managed_environment_enrollments.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_owner_managed"), max: 4 });
    await sql`
      INSERT INTO pylon_registrations
        (id,pylon_ref,owner_agent_user_id,owner_agent_credential_id,owner_agent_token_prefix,
         display_name,status,resource_mode,capability_refs_json,wallet_ready,latest_heartbeat_at,
         latest_heartbeat_status,latest_health_refs_json,latest_load_refs_json,
         latest_capacity_refs_json,provider_market_relay_refs_json,provider_nip90_lane_refs_json,
         public_projection_json,created_at,updated_at)
      VALUES ('registration.ide13.owner-managed',${pylonRef},${ownerAgentUserId},
        'credential.ide13.owner-managed','oa_agent','IDE-13 owner-managed','active',
        'owner_managed','[]',0,${now},'online','[]','[]','[]','[]','[]','{}',${now},${now})`;
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref,owner_user_id,target_class,adapter_ref,compatibility_ref,isolation,data_posture,health)
      VALUES (${targetRef},${ownerUserId},'owner_managed','adapter.pylon.owner-managed.v1',
        'compatibility.portable-session.v1','owner_host_process','owner_managed_region','ready')`;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const input = {
    schema: "openagents.owner_managed_environment_enrollment.request.v1" as const,
    idempotencyKeyHash: `sha256:${"1".repeat(64)}`,
    ownerUserId,
    ownerAgentUserId,
    targetRef,
    pylonRef,
    workerInstanceRef: "worker.ide13.owner-managed.1",
    adapterRef: "adapter.pylon.owner-managed.v1",
    compatibilityRef: "compatibility.portable-session.v1",
    isolation: "owner_host_process" as const,
    checkpointKeyRef: "key.owner-managed.local.1",
    regionRef: "region.owner-managed.us-central1",
    networkDestinationRefs: ["network.openagents.sync"],
    dataDestinationRefs: ["data.owner-managed.checkpoint"],
    retentionSeconds: 3_600,
    costPolicyRef: "cost.owner-managed.owner-paid.v1",
    generation: 1,
    health: "ready" as const,
    evidenceRefs: ["evidence.ide13.owner-managed.enrollment.1"],
  };

  test("admits, replays, renews, resolves, and revokes refs-only authority", async () => {
    const store = new PostgresOwnerManagedEnvironmentEnrollmentStore(
      sql as unknown as SyncSql,
      () => now,
      60_000,
    );
    const admitted = await store.admit(input);
    expect(admitted).toMatchObject({
      targetClass: "owner_managed",
      custodyPolicy: "owner_held_key",
      revision: 1,
      generation: 1,
      state: "active",
    });
    await expect(store.admit(input)).resolves.toEqual(admitted);
    await expect(
      store.admit({ ...input, checkpointKeyRef: "key.private.bytes" }),
    ).rejects.toMatchObject({ reason: "conflict" });

    const renewed = await store.admit({
      ...input,
      idempotencyKeyHash: `sha256:${"2".repeat(64)}`,
      expectedRevision: 1,
    });
    expect(renewed.revision).toBe(2);
    await expect(store.resolveActive(ownerUserId, targetRef)).resolves.toEqual(renewed);
    await expect(
      store.admit({
        ...input,
        idempotencyKeyHash: `sha256:${"3".repeat(64)}`,
        generation: 0,
        expectedRevision: 2,
      }),
    ).rejects.toMatchObject({ reason: "stale_generation" });

    const revoked = await store.revoke({
      ...input,
      idempotencyKeyHash: `sha256:${"4".repeat(64)}`,
      expectedRevision: 2,
    });
    expect(revoked).toMatchObject({ revision: 3, state: "revoked", health: "revoked" });
    await expect(store.resolveActive(ownerUserId, targetRef)).resolves.toBeUndefined();

    const events = await sql`
      SELECT event_kind,revision
      FROM khala_sync_owner_managed_environment_enrollment_events
      WHERE enrollment_ref=${admitted.enrollmentRef}
      ORDER BY revision`;
    expect(events).toEqual([
      { event_kind: "admitted", revision: "1" },
      { event_kind: "renewed", revision: "2" },
      { event_kind: "revoked", revision: "3" },
    ]);
  });
});
