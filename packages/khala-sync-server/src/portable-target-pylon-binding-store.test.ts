import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  PostgresPortableCommandPylonBindingResolver,
  PostgresPortableTargetPylonBindingStore,
} from "./portable-target-pylon-binding-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const now = "2026-07-20T13:00:00.000Z";
const ownerUserId = "owner.ide13.binding";
const ownerAgentUserId = "agent.ide13.binding";
const pylonRef = "pylon.ide13.binding";
const sessionRef = "session.ide13.binding";
const targetRef = "target.ide13.binding";
const digest = `sha256:${"a".repeat(64)}`;

describe.skipIf(!hasLocalPostgres())("IDE-13 target to Pylon binding authority", () => {
  let pg: LocalPostgres;
  let sql: SQL;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_target_pylon");
    await admin.end();
    const result = await runMigrations({ databaseUrl: pg.urlFor("khala_sync_ide13_target_pylon") });
    expect(result.applied).toContain("0087_portable_target_pylon_bindings.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_target_pylon"), max: 4 });
    await sql`
      INSERT INTO pylon_registrations
        (id,pylon_ref,owner_agent_user_id,owner_agent_credential_id,owner_agent_token_prefix,
         display_name,status,resource_mode,capability_refs_json,wallet_ready,latest_heartbeat_at,
         latest_heartbeat_status,latest_health_refs_json,latest_load_refs_json,
         latest_capacity_refs_json,provider_market_relay_refs_json,provider_nip90_lane_refs_json,
         public_projection_json,created_at,updated_at)
      VALUES ('registration.ide13.binding',${pylonRef},${ownerAgentUserId},'credential.ide13.binding',
        'oa_agent','IDE-13 binding','active','owner_local','[]',0,${now},'online','[]','[]','[]',
        '[]','[]','{}',${now},${now})`;
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref,owner_user_id,target_class,adapter_ref,compatibility_ref,isolation,data_posture,health)
      VALUES (${targetRef},${ownerUserId},'owner_local','adapter.ide13.binding',
        'compat.ide13.binding','owner_host_process','owner_device_only','ready')`;
    await sql`
      INSERT INTO khala_sync_portable_sessions
        (session_ref,owner_user_id,owner_scope_ref,work_context_ref,event_log_ref,
         current_projection_ref,command_scope_ref,root_agent_ref,state,
         current_attachment_ref,current_attachment_generation)
      VALUES (${sessionRef},${ownerUserId},${`scope.user.${ownerUserId}`} ,'work.ide13.binding',
        'eventlog.ide13.binding','projection.ide13.binding','commands.ide13.binding',
        'agent.ide13.root','active','attachment.ide13.binding',1)`;
    await sql`INSERT INTO khala_sync_portable_session_targets(session_ref,target_ref)
      VALUES (${sessionRef},${targetRef})`;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("admits, replays, renews with CAS, resolves live authority, and revokes", async () => {
    const store = new PostgresPortableTargetPylonBindingStore(
      sql as unknown as SyncSql,
      () => now,
      60_000,
    );
    const input = {
      idempotencyKeyHash: `sha256:${"1".repeat(64)}`,
      ownerUserId,
      ownerAgentUserId,
      sessionRef,
      targetRef,
      pylonRef,
      workerInstanceRef: "worker.ide13.binding",
      bindingDigest: digest,
      health: "ready" as const,
      evidenceRefs: ["evidence.ide13.binding"],
    };
    const admitted = await store.admit(input);
    expect(admitted).toMatchObject({ revision: 1, state: "active", pylonRef, targetRef });
    await expect(store.admit(input)).resolves.toEqual(admitted);
    await expect(
      store.admit({
        ...input,
        idempotencyKeyHash: `sha256:${"2".repeat(64)}`,
        workerInstanceRef: "worker.ide13.other",
        expectedRevision: 1,
      }),
    ).rejects.toMatchObject({ reason: "conflict" });

    const renewed = await store.admit({
      ...input,
      idempotencyKeyHash: `sha256:${"3".repeat(64)}`,
      expectedRevision: 1,
    });
    expect(renewed.revision).toBe(2);
    const resolver = new PostgresPortableCommandPylonBindingResolver(store, () => now);
    await expect(
      resolver.resolve({
        commandExecutionClaimRef: "claim.ide13.binding",
        ownerRef: ownerUserId,
        sessionRef,
        targetRef,
      }),
    ).resolves.toMatchObject({ pylonRef, targetRef });

    const revoked = await store.revoke({
      ...input,
      idempotencyKeyHash: `sha256:${"4".repeat(64)}`,
      expectedRevision: 2,
    });
    expect(revoked).toMatchObject({ revision: 3, state: "revoked", health: "revoked" });
    await expect(
      store.revoke({
        ...input,
        idempotencyKeyHash: `sha256:${"4".repeat(64)}`,
        expectedRevision: 2,
      }),
    ).resolves.toEqual(revoked);
    await expect(
      resolver.resolve({
        commandExecutionClaimRef: "claim.ide13.binding",
        ownerRef: ownerUserId,
        sessionRef,
        targetRef,
      }),
    ).rejects.toMatchObject({ reason: "not_found" });

    const events =
      await sql`SELECT event_kind,revision FROM khala_sync_portable_target_pylon_binding_events
      WHERE binding_ref=${admitted.bindingRef} ORDER BY revision`;
    expect(events).toEqual([
      { event_kind: "admitted", revision: "1" },
      { event_kind: "renewed", revision: "2" },
      { event_kind: "revoked", revision: "3" },
    ]);
  });
});
