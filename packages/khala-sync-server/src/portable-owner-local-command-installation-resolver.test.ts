import { SQL } from "@openagentsinc/postgres-runtime";
import {
  PORTABLE_COMMAND_SCHEMA_VERSION,
  PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
  type PortableCapabilityLease,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { afterAll, beforeAll, describe, expect, test, vi } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  createPostgresOwnerLocalPortableCommandInstallationPortResolver,
  PortableOwnerLocalCommandInstallationResolverError,
} from "./portable-owner-local-command-installation-resolver.js";
import { PostgresPortableOwnerLocalCapabilityOperationStore } from "./portable-owner-local-capability-operation-store.js";
import type { PortableCommandTargetInstallationPortResolver } from "./portable-command-broker-factory.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const now = "2026-07-20T14:00:00.000Z";
const ownerRef = "owner.ide13.queued-installation";
const ownerAgentRef = "agent.ide13.queued-installation";
const pylonRef = "pylon.ide13.queued-installation";
const sourceTargetRef = "target.ide13.queued-installation.source";
const destinationTargetRef = "target.ide13.queued-installation.destination";
const digest = `sha256:${"a".repeat(64)}`;
const otherDigest = `sha256:${"b".repeat(64)}`;

const target = (targetRef: string, adapterRef: string): PortableTargetDescriptor => ({
  targetRef,
  targetClass: "owner_local",
  adapterRef,
  ownerRef,
  compatibilityRef: "compatibility.ide13.queued-installation",
  isolation: "owner_host_process",
  dataPosture: "owner_device_only",
  health: "ready",
});

const sourceTarget = target(sourceTargetRef, "adapter.ide13.queued-installation.source");
const destinationTarget = target(
  destinationTargetRef,
  "adapter.ide13.queued-installation.destination",
);

describe.skipIf(!hasLocalPostgres())("queued owner-local installation-port bridge", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let sequence = 0;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_queued_installation");
    await admin.end();
    const migrated = await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_ide13_queued_installation"),
    });
    expect(migrated.applied).toContain("0092_portable_owner_local_capability_results.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_queued_installation"), max: 10 });
    await sql`
      INSERT INTO pylon_registrations
        (id,pylon_ref,owner_agent_user_id,owner_agent_credential_id,owner_agent_token_prefix,
         display_name,status,resource_mode,capability_refs_json,wallet_ready,latest_heartbeat_at,
         latest_heartbeat_status,latest_health_refs_json,latest_load_refs_json,
         latest_capacity_refs_json,provider_market_relay_refs_json,provider_nip90_lane_refs_json,
         public_projection_json,created_at,updated_at)
      VALUES ('registration.ide13.queued-installation',${pylonRef},${ownerAgentRef},
        'credential.ide13.queued-installation','oa_agent','IDE-13 queued installation',
        'active','owner_local','[]',0,${now},'online','[]','[]','[]','[]','[]','{}',${now},${now})
    `;
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref,owner_user_id,target_class,adapter_ref,compatibility_ref,
         isolation,data_posture,health)
      VALUES
        (${sourceTargetRef},${ownerRef},'owner_local',${sourceTarget.adapterRef},
         ${sourceTarget.compatibilityRef},${sourceTarget.isolation},${sourceTarget.dataPosture},'ready'),
        (${destinationTargetRef},${ownerRef},'owner_local',${destinationTarget.adapterRef},
         ${destinationTarget.compatibilityRef},${destinationTarget.isolation},
         ${destinationTarget.dataPosture},'ready')
    `;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const seed = async () => {
    sequence += 1;
    const suffix = String(sequence);
    const sessionRef = `session.ide13.queued-installation.${suffix}`;
    const sourceAttachmentRef = `attachment.ide13.queued-installation.source.${suffix}`;
    const destinationAttachmentRef = `attachment.ide13.queued-installation.destination.${suffix}`;
    const commandRef = `command.ide13.queued-installation.${suffix}`;
    const commandExecutionClaimRef = `claim.ide13.queued-installation.${suffix}`;
    const command = {
      schema: PORTABLE_COMMAND_SCHEMA_VERSION,
      commandRef,
      idempotencyKey: `idempotency.ide13.queued-installation.${suffix}`,
      ownerRef,
      sessionRef,
      kind: "move" as const,
      expectedAttachmentRef: sourceAttachmentRef,
      expectedGeneration: 1,
      destinationTargetRef,
      checkpointRef: `checkpoint.ide13.queued-installation.${suffix}`,
      expiresAt: "2026-07-20T15:00:00.000Z",
    };
    await sql.begin(async (tx) => {
      await tx`
        INSERT INTO khala_sync_portable_sessions
          (session_ref,owner_user_id,owner_scope_ref,work_context_ref,event_log_ref,
           current_projection_ref,command_scope_ref,root_agent_ref,state,
           current_attachment_ref,current_attachment_generation)
        VALUES (${sessionRef},${ownerRef},${`scope.user.${ownerRef}`},
          ${`work.ide13.queued-installation.${suffix}`},
          ${`eventlog.ide13.queued-installation.${suffix}`},
          ${`projection.ide13.queued-installation.${suffix}`},
          ${`commands.ide13.queued-installation.${suffix}`},
          ${`agent.ide13.queued-installation.${suffix}`},'active',${sourceAttachmentRef},1)
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
          ${sourceTargetRef},${`worker.ide13.queued-installation.${suffix}`},1,1,'claimed',
          ${now},'2026-07-20T14:30:00.000Z',${now})
      `;
      await tx`
        INSERT INTO khala_sync_portable_target_pylon_bindings
          (binding_ref,owner_user_id,owner_agent_user_id,session_ref,target_ref,pylon_ref,
           worker_instance_ref,binding_digest,revision,state,health,evidence_refs_json,
           last_renewed_at,expires_at,created_at,updated_at)
        VALUES
          (${`binding.ide13.queued-installation.${suffix}.source`},${ownerRef},${ownerAgentRef},
           ${sessionRef},${sourceTargetRef},${pylonRef},
           ${`worker.ide13.queued-installation.${suffix}`},${digest},1,'active','ready','[]'::jsonb,
           ${now},'2026-07-20T14:25:00.000Z',${now},${now}),
          (${`binding.ide13.queued-installation.${suffix}.destination`},${ownerRef},${ownerAgentRef},
           ${sessionRef},${destinationTargetRef},${pylonRef},
           ${`worker.ide13.queued-installation.${suffix}`},${digest},1,'active','ready','[]'::jsonb,
           ${now},'2026-07-20T14:25:00.000Z',${now},${now})
      `;
    });
    const sourceLeaseRef = `lease.ide13.queued-installation.source.${suffix}`;
    const destinationLeaseRef = `lease.ide13.queued-installation.destination.${suffix}`;
    const sourceGrantRef = `grant.ide13.queued-installation.source.${suffix}`;
    const destinationGrantRef = `grant.ide13.queued-installation.destination.${suffix}`;
    const shared = {
      commandExecutionClaimRef,
      ownerRef,
      sessionRef,
      sourceAttachmentRef,
      sourceGeneration: 1,
      destinationAttachmentRef,
      destinationGeneration: 2,
      grantBindings: [
        {
          sourceLeaseRef,
          grantRef: sourceGrantRef,
          ownerUserId: ownerRef,
          kind: "provider" as const,
          providerAccountRef: `account.ide13.queued-installation.${suffix}`,
        },
      ],
      capabilityTransfers: [
        {
          sourceLeaseRef,
          destinationLeaseRef,
          destinationSourceGrantRef: destinationGrantRef,
          expiresAt: "2026-07-20T14:20:00.000Z",
        },
      ],
    };
    return {
      suffix,
      sourceLeaseRef,
      destinationLeaseRef,
      source: { ...shared, target: sourceTarget },
      destination: { ...shared, target: destinationTarget },
    };
  };

  const resolver = (timeoutMs = 1_000) =>
    createPostgresOwnerLocalPortableCommandInstallationPortResolver({
      sql: sql as unknown as SyncSql,
      now: () => now,
      pollIntervalMs: 1,
      timeoutMs,
    });

  const completeNext = async (
    fixture: Awaited<ReturnType<typeof seed>>,
    action: "install" | "wipe",
  ) => {
    const targetRef = action === "install" ? destinationTargetRef : sourceTargetRef;
    const store = new PostgresPortableOwnerLocalCapabilityOperationStore(
      sql as unknown as SyncSql,
      () => now,
    );
    let pending: Awaited<ReturnType<typeof store.pending>> = [];
    await vi.waitFor(async () => {
      pending = await store.pending(ownerRef, pylonRef, targetRef);
      expect(pending).toHaveLength(1);
    });
    const operation = pending[0];
    if (operation === undefined) throw new Error("pending operation is missing");
    expect(operation.request.action).toBe(action);
    const claim = await store.claim(ownerRef, {
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
      operationRef: operation.request.operationRef,
      claimRef: `claim.ide13.queued-installation.operation.${fixture.suffix}.${action}`,
      pylonRef,
      targetRef,
      sessionRef: operation.request.sessionRef,
      attachmentRef: operation.request.attachmentRef,
      attachmentGeneration: operation.request.attachmentGeneration,
      workerInstanceRef: `worker.ide13.queued-installation.operation.${action}`,
      leaseExpiresAt: "2026-07-20T14:00:00.500Z",
    });
    const claimed = claim.operation;
    if (
      claimed.claimRef === null ||
      claimed.workerInstanceRef === null ||
      claimed.claimGeneration === null ||
      claimed.leaseRevision === null
    ) {
      throw new Error("claimed operation authority is incomplete");
    }
    await store.complete(ownerRef, {
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_SCHEMA_VERSION,
      claimRef: claimed.claimRef,
      pylonRef,
      targetRef,
      sessionRef: claimed.request.sessionRef,
      attachmentRef: claimed.request.attachmentRef,
      attachmentGeneration: claimed.request.attachmentGeneration,
      workerInstanceRef: claimed.workerInstanceRef,
      claimGeneration: claimed.claimGeneration,
      expectedLeaseRevision: claimed.leaseRevision,
      resultRef: `result.ide13.queued-installation.${fixture.suffix}.${action}`,
      resultStatus: "completed",
      resultInstallationRef:
        action === "install" ? `installation.ide13.queued-installation.${fixture.suffix}` : null,
      receiptRef:
        action === "wipe"
          ? `receipt.ide13.queued-installation.${fixture.suffix}`
          : `receipt.ide13.queued-installation.install.${fixture.suffix}`,
      evidenceRefs:
        action === "install" ? [`evidence.ide13.queued-installation.${fixture.suffix}`] : [],
      errorRef: null,
      completedAt: now,
    });
  };

  test("installs and wipes through exact durable refs, including replay", async () => {
    const fixture = await seed();
    const destination = await resolver().resolve(fixture.destination);
    const source = await resolver().resolve(fixture.source);
    const destinationLease: PortableCapabilityLease = {
      leaseRef: fixture.destinationLeaseRef,
      ownerRef,
      sessionRef: fixture.destination.sessionRef,
      attachmentRef: fixture.destination.destinationAttachmentRef,
      attachmentGeneration: 2,
      targetRef: destinationTargetRef,
      capability: "provider",
      accountRef: `account.ide13.queued-installation.${fixture.suffix}`,
      expiresAt: "2026-07-20T14:20:00.000Z",
      state: "issued",
    };
    const installByReference = destination?.port.installByReference;
    if (installByReference === undefined) throw new Error("reference installation is missing");
    const install = installByReference({
      lease: destinationLease,
      permissions: ["permission.provider.use"],
    });
    await completeNext(fixture, "install");
    const installed = await install;
    expect(installed).toEqual({
      installationRef: `installation.ide13.queued-installation.${fixture.suffix}`,
      evidenceRef: `evidence.ide13.queued-installation.${fixture.suffix}`,
    });
    await expect(
      installByReference({
        lease: destinationLease,
        permissions: ["permission.provider.use"],
      }),
    ).resolves.toEqual(installed);
    const operationCount: Array<{ count: string }> = await sql`
      SELECT count(*)::text AS count
      FROM khala_sync_portable_owner_local_capability_operations
      WHERE command_execution_claim_ref=${fixture.destination.commandExecutionClaimRef}
        AND action='install'
    `;
    expect(operationCount[0]?.count).toBe("1");

    if (source === null) throw new Error("source installation port is missing");
    const wipe = source.port.wipe({
      leaseRef: fixture.sourceLeaseRef,
      targetRef: sourceTargetRef,
      attachmentRef: fixture.source.sourceAttachmentRef,
      attachmentGeneration: 1,
      installationRef: `installation.ide13.source-prior.${fixture.suffix}`,
    });
    await completeNext(fixture, "wipe");
    await expect(wipe).resolves.toEqual({
      wipeReceiptRef: `receipt.ide13.queued-installation.${fixture.suffix}`,
    });

    const serialized: Array<{ body: string }> = await sql`
      SELECT request_json::text AS body
      FROM khala_sync_portable_owner_local_capability_operations
      WHERE command_execution_claim_ref=${fixture.destination.commandExecutionClaimRef}
    `;
    expect(JSON.stringify(serialized)).not.toMatch(/material|bytes|base64|bearer|token/iu);
  });

  test("fails closed on timeout and exact binding drift", async () => {
    const timeoutFixture = await seed();
    const destination = await resolver(4).resolve(timeoutFixture.destination);
    const installByReference = destination?.port.installByReference;
    if (installByReference === undefined) throw new Error("reference installation is missing");
    const lease: PortableCapabilityLease = {
      leaseRef: timeoutFixture.destinationLeaseRef,
      ownerRef,
      sessionRef: timeoutFixture.destination.sessionRef,
      attachmentRef: timeoutFixture.destination.destinationAttachmentRef,
      attachmentGeneration: 2,
      targetRef: destinationTargetRef,
      capability: "provider",
      accountRef: `account.ide13.queued-installation.${timeoutFixture.suffix}`,
      expiresAt: "2026-07-20T14:20:00.000Z",
      state: "issued",
    };
    await expect(
      installByReference({
        lease,
        permissions: ["permission.provider.use"],
      }),
    ).rejects.toMatchObject({ code: "operation_timeout" });

    const driftFixture = await seed();
    const drifted = await resolver().resolve(driftFixture.destination);
    const driftInstallByReference = drifted?.port.installByReference;
    if (driftInstallByReference === undefined) {
      throw new Error("reference installation is missing");
    }
    await sql`
      UPDATE khala_sync_portable_target_pylon_bindings
      SET state='revoked',health='revoked',revoked_at=${now},updated_at=${now}
      WHERE session_ref=${driftFixture.destination.sessionRef}
        AND target_ref=${destinationTargetRef}
    `;
    await expect(
      driftInstallByReference({
        lease: {
          ...lease,
          leaseRef: driftFixture.destinationLeaseRef,
          sessionRef: driftFixture.destination.sessionRef,
          attachmentRef: driftFixture.destination.destinationAttachmentRef,
          accountRef: `account.ide13.queued-installation.${driftFixture.suffix}`,
        },
        permissions: ["permission.provider.use"],
      }),
    ).rejects.toBeInstanceOf(PortableOwnerLocalCommandInstallationResolverError);
  });

  test("returns null for non-owner-local targets without SQL", async () => {
    const resolve = vi.fn<PortableCommandTargetInstallationPortResolver["resolve"]>(
      resolver().resolve,
    );
    const fixture = await seed();
    await expect(
      resolve({
        ...fixture.destination,
        target: { ...destinationTarget, targetClass: "openagents_managed" },
      }),
    ).resolves.toBeNull();
  });
});
