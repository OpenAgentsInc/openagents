import { SQL } from "@openagentsinc/postgres-runtime";
import {
  PORTABLE_COMMAND_SCHEMA_VERSION,
  PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
} from "@openagentsinc/portable-session-contract";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  PostgresPortableOwnerLocalCapabilityOperationStore,
  portableOwnerLocalCapabilityOperationRef,
  portableOwnerLocalCapabilityPermissionFingerprint,
} from "./portable-owner-local-capability-operation-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const now = "2026-07-20T14:00:00.000Z";
const ownerRef = "owner.ide13.capability";
const ownerAgentRef = "agent.ide13.capability";
const pylonRef = "pylon.ide13.capability";
const sourceTargetRef = "target.ide13.capability.source";
const destinationTargetRef = "target.ide13.capability.destination";
const digest = `sha256:${"a".repeat(64)}`;
const otherDigest = `sha256:${"b".repeat(64)}`;

describe.skipIf(!hasLocalPostgres())("IDE-13 owner-local capability operation exchange", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let sequence = 0;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_capability_operations");
    await admin.end();
    const result = await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_ide13_capability_operations"),
    });
    expect(result.applied).toContain("0089_portable_owner_local_capability_operations.sql");
    expect(result.applied).toContain("0090_portable_owner_local_capability_results.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_capability_operations"), max: 10 });
    await sql`
      INSERT INTO pylon_registrations
        (id,pylon_ref,owner_agent_user_id,owner_agent_credential_id,owner_agent_token_prefix,
         display_name,status,resource_mode,capability_refs_json,wallet_ready,latest_heartbeat_at,
         latest_heartbeat_status,latest_health_refs_json,latest_load_refs_json,
         latest_capacity_refs_json,provider_market_relay_refs_json,provider_nip90_lane_refs_json,
         public_projection_json,created_at,updated_at)
      VALUES ('registration.ide13.capability',${pylonRef},${ownerAgentRef},
        'credential.ide13.capability','oa_agent','IDE-13 capability','active','owner_local',
        '[]',0,${now},'online','[]','[]','[]','[]','[]','{}',${now},${now})
    `;
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref,owner_user_id,target_class,adapter_ref,compatibility_ref,
         isolation,data_posture,health)
      VALUES
        (${sourceTargetRef},${ownerRef},'owner_local','adapter.ide13.capability.source',
         'compat.ide13.capability','owner_host_process','owner_device_only','ready'),
        (${destinationTargetRef},${ownerRef},'owner_local','adapter.ide13.capability.destination',
         'compat.ide13.capability','owner_host_process','owner_device_only','ready')
    `;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const seedExecution = async () => {
    sequence += 1;
    const suffix = String(sequence);
    const sessionRef = `session.ide13.capability.${suffix}`;
    const sourceAttachmentRef = `attachment.ide13.capability.source.${suffix}`;
    const commandRef = `command.ide13.capability.${suffix}`;
    const commandExecutionClaimRef = `claim.ide13.capability.command.${suffix}`;
    const command = {
      schema: PORTABLE_COMMAND_SCHEMA_VERSION,
      commandRef,
      idempotencyKey: `idempotency.ide13.capability.${suffix}`,
      ownerRef,
      sessionRef,
      kind: "move" as const,
      expectedAttachmentRef: sourceAttachmentRef,
      expectedGeneration: 1,
      destinationTargetRef,
      checkpointRef: `checkpoint.ide13.capability.${suffix}`,
      expiresAt: "2026-07-20T15:00:00.000Z",
    };
    await sql.begin(async (tx) => {
      await tx`
        INSERT INTO khala_sync_portable_sessions
          (session_ref,owner_user_id,owner_scope_ref,work_context_ref,event_log_ref,
           current_projection_ref,command_scope_ref,root_agent_ref,state,
           current_attachment_ref,current_attachment_generation)
        VALUES (${sessionRef},${ownerRef},${`scope.user.${ownerRef}`},
          ${`work.ide13.capability.${suffix}`},${`eventlog.ide13.capability.${suffix}`},
          ${`projection.ide13.capability.${suffix}`},${`commands.ide13.capability.${suffix}`},
          ${`agent.ide13.capability.${suffix}`},'active',${sourceAttachmentRef},1)
      `;
      await tx`
        INSERT INTO khala_sync_portable_session_targets(session_ref,target_ref)
        VALUES (${sessionRef},${sourceTargetRef}),(${sessionRef},${destinationTargetRef})
      `;
      await tx`
        INSERT INTO khala_sync_portable_attachments
          (attachment_ref,session_ref,target_ref,generation,state,
           descendant_agent_refs_json,capability_lease_refs_json,evidence_refs_json)
        VALUES (${sourceAttachmentRef},${sessionRef},${sourceTargetRef},1,'active',
          '[]'::jsonb,'[]'::jsonb,'[]'::jsonb)
      `;
      await tx`
        INSERT INTO khala_sync_portable_commands
          (command_ref,idempotency_key,owner_user_id,session_ref,kind,
           expected_attachment_ref,expected_generation,destination_target_ref,
           checkpoint_ref,expires_at,command_json,status)
        VALUES (${commandRef},${command.idempotencyKey},${ownerRef},${sessionRef},'move',
          ${sourceAttachmentRef},1,${destinationTargetRef},${command.checkpointRef},
          ${command.expiresAt},${JSON.stringify(command)}::jsonb,'accepted')
      `;
      await tx`
        INSERT INTO khala_sync_portable_command_executions
          (command_ref,claim_ref,owner_user_id,session_ref,command_kind,
           command_fingerprint,claim_fingerprint,source_attachment_ref,
           source_generation,destination_target_ref,executor_environment_ref,
           worker_instance_ref,claim_generation,lease_revision,state,
           claimed_at,lease_expires_at,updated_at)
        VALUES (${commandRef},${commandExecutionClaimRef},${ownerRef},${sessionRef},'move',
          ${digest},${otherDigest},${sourceAttachmentRef},1,${destinationTargetRef},
          ${sourceTargetRef},${`worker.ide13.capability.command.${suffix}`},1,1,'claimed',
          ${now},'2026-07-20T14:30:00.000Z',${now})
      `;
      await tx`
        INSERT INTO khala_sync_portable_target_pylon_bindings
          (binding_ref,owner_user_id,owner_agent_user_id,session_ref,target_ref,pylon_ref,
           worker_instance_ref,binding_digest,revision,state,health,evidence_refs_json,
           last_renewed_at,expires_at,created_at,updated_at)
        VALUES
          (${`binding.ide13.capability.${suffix}.source`},${ownerRef},${ownerAgentRef},
           ${sessionRef},${sourceTargetRef},${pylonRef},
           ${`worker.ide13.capability.pylon.${suffix}`},${digest},1,'active','ready','[]'::jsonb,
           ${now},'2026-07-20T14:25:00.000Z',${now},${now}),
          (${`binding.ide13.capability.${suffix}.destination`},${ownerRef},${ownerAgentRef},
           ${sessionRef},${destinationTargetRef},${pylonRef},
           ${`worker.ide13.capability.pylon.${suffix}`},${digest},1,'active','ready','[]'::jsonb,
           ${now},'2026-07-20T14:25:00.000Z',${now},${now})
      `;
    });
    return { suffix, sessionRef, sourceAttachmentRef, commandExecutionClaimRef };
  };

  const operationRequest = (
    fixture: Awaited<ReturnType<typeof seedExecution>>,
    action: "install" | "wipe" = "install",
  ) => {
    const permissionRefs =
      action === "install" ? ["permission.github.write", "permission.provider.use"] : [];
    const identity = {
      action,
      capability: action === "install" ? ("scm_write" as const) : null,
      commandExecutionClaimRef: fixture.commandExecutionClaimRef,
      ownerRef,
      pylonRef,
      sessionRef: fixture.sessionRef,
      attachmentRef:
        action === "install"
          ? `attachment.ide13.capability.destination.${fixture.suffix}`
          : fixture.sourceAttachmentRef,
      attachmentGeneration: action === "install" ? 2 : 1,
      targetRef: action === "install" ? destinationTargetRef : sourceTargetRef,
      sourceLeaseRef: `lease.ide13.capability.source.${fixture.suffix}`,
      sourceGrantRef: `grant.ide13.capability.source.${fixture.suffix}`,
      destinationLeaseRef: `lease.ide13.capability.destination.${fixture.suffix}`,
      destinationGrantRef: `grant.ide13.capability.destination.${fixture.suffix}`,
      installationRef: action === "wipe" ? `installation.ide13.capability.${fixture.suffix}` : null,
      permissionRefs,
    };
    return {
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
      operationRef: portableOwnerLocalCapabilityOperationRef(identity),
      ...identity,
      permissionFingerprint: portableOwnerLocalCapabilityPermissionFingerprint(permissionRefs),
      expiresAt: "2026-07-20T14:20:00.000Z",
    };
  };

  test("applies the fresh refs-only migration and stores no secret transport columns", async () => {
    const columns: Array<{ column_name: string }> = await sql`
      SELECT column_name FROM information_schema.columns
      WHERE table_name = 'khala_sync_portable_owner_local_capability_operations'
      ORDER BY ordinal_position
    `;
    const names = columns.map((column) => column.column_name);
    expect(names).toContain("permission_fingerprint");
    expect(names).toContain("capability");
    expect(names).toContain("installation_ref");
    expect(names).toContain("result_installation_ref");
    expect(names).not.toEqual(
      expect.arrayContaining(["material", "bytes", "base64", "endpoint", "bearer"]),
    );
  });

  test("enqueues exact deterministic bytes and refuses replay or authority drift", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortableOwnerLocalCapabilityOperationStore(
      sql as unknown as SyncSql,
      () => now,
    );
    const request = operationRequest(fixture);
    const first = await store.enqueue(request);
    expect(first).toMatchObject({ status: "enqueued", operation: { state: "pending", request } });
    await expect(store.enqueue(request)).resolves.toEqual({
      status: "replayed",
      operation: first.operation,
    });
    await expect(
      store.enqueue({ ...request, expiresAt: "2026-07-20T14:19:00.000Z" }),
    ).rejects.toMatchObject({ code: "conflict" });
    await expect(
      store.enqueue({ ...request, operationRef: "operation.owner-local-capability.invalid" }),
    ).rejects.toMatchObject({ code: "invalid" });
    await expect(
      store.enqueue({ ...request, installationRef: "installation.ide13.unexpected" }),
    ).rejects.toMatchObject({ code: "invalid" });
    expect(await store.pending(ownerRef, pylonRef, destinationTargetRef)).toEqual([
      first.operation,
    ]);
    expect(await store.pending("owner.ide13.other", pylonRef, destinationTargetRef)).toEqual([]);
    await expect(
      store.read("owner.ide13.other", pylonRef, destinationTargetRef, request.operationRef),
    ).rejects.toMatchObject({ code: "not_found" });

    await sql`
      UPDATE khala_sync_portable_target_pylon_bindings SET state='revoked',health='revoked',
        revoked_at=${now},updated_at=${now}
      WHERE session_ref=${fixture.sessionRef} AND target_ref=${destinationTargetRef}
    `;
    expect(await store.pending(ownerRef, pylonRef, destinationTargetRef)).toEqual([]);
    await expect(
      store.read(ownerRef, pylonRef, destinationTargetRef, request.operationRef),
    ).rejects.toMatchObject({ code: "not_found" });
    await expect(
      store.claim(ownerRef, {
        schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
        operationRef: request.operationRef,
        claimRef: `claim.ide13.capability.operation.${fixture.suffix}`,
        pylonRef,
        targetRef: destinationTargetRef,
        sessionRef: fixture.sessionRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: 2,
        workerInstanceRef: `worker.ide13.capability.${fixture.suffix}`,
        leaseExpiresAt: "2026-07-20T14:10:00.000Z",
      }),
    ).rejects.toMatchObject({ code: "authority_unavailable" });
  });

  test("claims once with SKIP LOCKED and reconciles lost renew and completion acknowledgements", async () => {
    const fixture = await seedExecution();
    const request = operationRequest(fixture, "wipe");
    const firstStore = new PostgresPortableOwnerLocalCapabilityOperationStore(
      sql as unknown as SyncSql,
      () => now,
    );
    const secondStore = new PostgresPortableOwnerLocalCapabilityOperationStore(
      sql as unknown as SyncSql,
      () => now,
    );
    await firstStore.enqueue(request);
    await expect(firstStore.enqueue({ ...request, installationRef: null })).rejects.toMatchObject({
      code: "invalid",
    });
    const claim = (worker: string) => ({
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
      operationRef: request.operationRef,
      claimRef: `claim.ide13.capability.operation.${fixture.suffix}.${worker}`,
      pylonRef,
      targetRef: sourceTargetRef,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.sourceAttachmentRef,
      attachmentGeneration: 1,
      workerInstanceRef: `worker.ide13.capability.${worker}`,
      leaseExpiresAt: "2026-07-20T14:10:00.000Z",
    });
    await expect(firstStore.claim("owner.ide13.other", claim("other"))).rejects.toMatchObject({
      code: "conflict",
    });
    const claims = await Promise.allSettled([
      firstStore.claim(ownerRef, claim("one")),
      secondStore.claim(ownerRef, claim("two")),
    ]);
    expect(claims.filter((result) => result.status === "fulfilled")).toHaveLength(1);
    const fulfilled = claims.find((result) => result.status === "fulfilled");
    if (fulfilled?.status !== "fulfilled") throw new Error("claim result is missing");
    const claimed = fulfilled.value.operation;
    if (claimed.claimRef === null || claimed.workerInstanceRef === null) {
      throw new Error("claim binding is missing");
    }
    const renew = {
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
      claimRef: claimed.claimRef,
      pylonRef,
      targetRef: sourceTargetRef,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.sourceAttachmentRef,
      attachmentGeneration: 1,
      workerInstanceRef: claimed.workerInstanceRef,
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      leaseExpiresAt: "2026-07-20T14:15:00.000Z",
    };
    await expect(firstStore.renew("owner.ide13.other", renew)).rejects.toMatchObject({
      code: "conflict",
    });
    const renewed = await firstStore.renew(ownerRef, renew);
    await expect(firstStore.renew(ownerRef, renew)).resolves.toEqual({
      status: "replayed",
      operation: renewed.operation,
    });
    const result = {
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
      claimRef: claimed.claimRef,
      pylonRef,
      targetRef: sourceTargetRef,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.sourceAttachmentRef,
      attachmentGeneration: 1,
      workerInstanceRef: claimed.workerInstanceRef,
      claimGeneration: 1,
      expectedLeaseRevision: 2,
      resultRef: `result.ide13.capability.${fixture.suffix}`,
      resultStatus: "completed" as const,
      resultInstallationRef: null,
      receiptRef: `receipt.ide13.capability.${fixture.suffix}`,
      evidenceRefs: [],
      errorRef: null,
      completedAt: now,
    };
    await expect(firstStore.complete("owner.ide13.other", result)).rejects.toMatchObject({
      code: "conflict",
    });
    await expect(
      firstStore.complete(ownerRef, {
        ...result,
        resultInstallationRef: `installation.ide13.capability.unexpected.${fixture.suffix}`,
        evidenceRefs: [`evidence.ide13.capability.unexpected.${fixture.suffix}`],
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    const completed = await firstStore.complete(ownerRef, result);
    expect(completed).toMatchObject({ status: "completed", operation: { leaseRevision: 3 } });
    await expect(firstStore.complete(ownerRef, result)).resolves.toEqual({
      status: "replayed",
      operation: completed.operation,
    });
    await expect(
      firstStore.complete(ownerRef, {
        ...result,
        resultRef: `result.ide13.capability.other.${fixture.suffix}`,
      }),
    ).rejects.toMatchObject({ code: "stale_revision" });
    await expect(
      firstStore.read(ownerRef, pylonRef, sourceTargetRef, request.operationRef),
    ).resolves.toEqual(completed.operation);
  });
});
