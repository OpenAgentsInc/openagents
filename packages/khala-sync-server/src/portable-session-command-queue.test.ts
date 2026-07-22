import { SQL } from "@openagentsinc/postgres-runtime";
import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  PORTABLE_COMMAND_SCHEMA_VERSION,
} from "@openagentsinc/portable-session-contract";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { PortableSessionCommandConsumer } from "./portable-session-command-consumer.js";
import {
  PortableSessionCommandQueueError,
  PostgresPortableSessionCommandQueue,
} from "./portable-session-command-queue.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const ownerRef = "owner.ide13.queue";
const sourceTargetRef = "target.ide13.queue.source";
const destinationTargetRef = "target.ide13.queue.destination";
const baseNow = "2026-07-20T12:00:00.000Z";

describe.skipIf(!hasLocalPostgres())("IDE-13 portable command execution queue", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let sequence = 0;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_command_queue");
    await admin.end();
    const result = await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_ide13_command_queue"),
    });
    expect(result.applied).toContain("0083_portable_command_execution.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_command_queue"), max: 10 });
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref, owner_user_id, target_class, adapter_ref, compatibility_ref,
         isolation, data_posture, health)
      VALUES
        (${sourceTargetRef}, ${ownerRef}, 'owner_local', 'adapter.ide13.source',
         'compat.ide13.v1', 'owner_host_process', 'owner_device_only', 'ready'),
        (${destinationTargetRef}, ${ownerRef}, 'openagents_managed', 'adapter.ide13.destination',
         'compat.ide13.v1', 'dedicated_microvm', 'openagents_managed_region', 'ready')
    `;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const seedCommand = async (expiresAt = "2026-07-20T13:00:00.000Z") => {
    sequence += 1;
    const suffix = String(sequence);
    const sessionRef = `session.ide13.queue.${suffix}`;
    const attachmentRef = `attachment.ide13.queue.${suffix}`;
    const checkpointRef = `checkpoint.ide13.queue.${suffix}`;
    const commandRef = `command.ide13.queue.${suffix}`;
    const command = {
      schema: PORTABLE_COMMAND_SCHEMA_VERSION,
      commandRef,
      idempotencyKey: `idempotency.ide13.queue.${suffix}`,
      ownerRef,
      sessionRef,
      kind: "move" as const,
      expectedAttachmentRef: attachmentRef,
      expectedGeneration: 1,
      destinationTargetRef,
      checkpointRef,
      expiresAt,
    };
    await sql.begin(async (tx) => {
      await tx`
        INSERT INTO khala_sync_portable_sessions
          (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
           event_log_ref, current_projection_ref, command_scope_ref, root_agent_ref,
           state, current_attachment_ref, current_attachment_generation)
        VALUES
          (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`},
           ${`work.ide13.${suffix}`}, ${`eventlog.ide13.${suffix}`},
           ${`projection.ide13.${suffix}`}, ${`commands.ide13.${suffix}`},
           ${`agent.ide13.root.${suffix}`}, 'active', ${attachmentRef}, 1)
      `;
      await tx`
        INSERT INTO khala_sync_portable_session_targets (session_ref, target_ref)
        VALUES (${sessionRef}, ${sourceTargetRef}), (${sessionRef}, ${destinationTargetRef})
      `;
      await tx`
        INSERT INTO khala_sync_portable_attachments
          (attachment_ref, session_ref, target_ref, generation, state,
           descendant_agent_refs_json, capability_lease_refs_json,
           checkpoint_ref, evidence_refs_json)
        VALUES
          (${attachmentRef}, ${sessionRef}, ${sourceTargetRef}, 1, 'active',
           '[]'::jsonb, '[]'::jsonb, ${checkpointRef}, '[]'::jsonb)
      `;
      await tx`
        INSERT INTO khala_sync_portable_commands
          (command_ref, idempotency_key, owner_user_id, session_ref, kind,
           expected_attachment_ref, expected_generation, destination_target_ref,
           checkpoint_ref, expires_at, command_json, status)
        VALUES
          (${commandRef}, ${command.idempotencyKey}, ${ownerRef}, ${sessionRef},
           'move', ${attachmentRef}, 1, ${destinationTargetRef}, ${checkpointRef},
           ${command.expiresAt}, ${JSON.stringify(command)}::jsonb, 'accepted')
      `;
    });
    return { commandRef, sessionRef };
  };

  const claimInput = (commandRef: string, worker: string, claim = worker) => ({
    schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
    commandRef,
    claimRef: `claim.ide13.${claim}`,
    executorEnvironmentRef: sourceTargetRef,
    workerInstanceRef: `worker.ide13.${worker}`,
    leaseExpiresAt: "2026-07-20T12:05:00.000Z",
  });

  test("serializes two workers on the command row in real PostgreSQL", async () => {
    const { commandRef } = await seedCommand();
    const first = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const second = new PostgresPortableSessionCommandQueue(
      sql as unknown as SyncSql,
      () => baseNow,
    );
    const results = await Promise.allSettled([
      first.claim(claimInput(commandRef, "parallel-a")),
      second.claim(claimInput(commandRef, "parallel-b")),
    ]);
    expect(results.filter((result) => result.status === "fulfilled")).toHaveLength(1);
    const rejected = results.find((result) => result.status === "rejected");
    expect(rejected).toMatchObject({
      reason: expect.objectContaining({ code: "claim_conflict" }),
    });
  });

  test("discovers and claims each accepted command once across concurrent ticks", async () => {
    const commands = await Promise.all([seedCommand(), seedCommand(), seedCommand()]);
    const first = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const second = new PostgresPortableSessionCommandQueue(
      sql as unknown as SyncSql,
      () => baseNow,
    );
    const workerInstanceRef = "worker.ide13.dispatch.stable";
    const [left, right] = await Promise.all([
      first.claimAcceptedBatch({ workerInstanceRef, limit: 2, leaseDurationMs: 300_000 }),
      second.claimAcceptedBatch({ workerInstanceRef, limit: 2, leaseDurationMs: 300_000 }),
    ]);
    const claims = [...left.claims, ...right.claims];
    expect(claims).toHaveLength(3);
    expect(new Set(claims.map((claim) => claim.commandRef))).toEqual(
      new Set(commands.map((command) => command.commandRef)),
    );
    expect(new Set(claims.map((claim) => claim.claimRequest.claimRef)).size).toBe(3);
    expect(
      claims.every(
        (claim) =>
          claim.claimRequest.workerInstanceRef === workerInstanceRef &&
          claim.claimRequest.leaseExpiresAt === "2026-07-20T12:05:00.000Z",
      ),
    ).toBe(true);
    expect(
      await first.claimAcceptedBatch({ workerInstanceRef, limit: 3, leaseDurationMs: 300_000 }),
    ).toEqual({ claims: [], skippedCommandRefs: [] });
  });

  test("clamps a discovered execution lease to the command expiry", async () => {
    const command = await seedCommand("2026-07-20T12:02:00.000Z");
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const batch = await queue.claimAcceptedBatch({
      workerInstanceRef: "worker.ide13.dispatch.expiry-bound",
      limit: 1,
      leaseDurationMs: 300_000,
    });
    expect(batch.claims).toHaveLength(1);
    expect(batch.claims[0]).toMatchObject({
      commandRef: command.commandRef,
      claimRequest: { leaseExpiresAt: "2026-07-20T12:02:00.000Z" },
    });
  });

  test("skips expired, stale, unbound, and pending-reconcile commands", async () => {
    const expired = await seedCommand();
    const stale = await seedCommand();
    const unbound = await seedCommand();
    const pending = await seedCommand();
    await sql`
      UPDATE khala_sync_portable_commands
      SET expires_at = '2026-07-20T11:59:59.000Z'
      WHERE command_ref = ${expired.commandRef}
    `;
    await sql`
      UPDATE khala_sync_portable_sessions
      SET current_attachment_generation = 2
      WHERE session_ref = ${stale.sessionRef}
    `;
    await sql`
      DELETE FROM khala_sync_portable_session_targets
      WHERE session_ref = ${unbound.sessionRef} AND target_ref = ${destinationTargetRef}
    `;
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const pendingClaim = await queue.claim(claimInput(pending.commandRef, "pending-discovery"));
    await queue.markPendingReconcile({
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef: pendingClaim.claim.claimRef,
      executorEnvironmentRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.pending-discovery",
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      pendingReconcileRef: "reconcile.ide13.pending-discovery",
      evidenceRefs: ["evidence.ide13.pending-discovery"],
      observedAt: baseNow,
    });
    expect(
      await queue.claimAcceptedBatch({
        workerInstanceRef: "worker.ide13.dispatch.filter",
        limit: 10,
        leaseDurationMs: 300_000,
      }),
    ).toEqual({ claims: [], skippedCommandRefs: [] });
  });

  test("isolates malformed accepted command bytes from another discovery item", async () => {
    const malformed = await seedCommand();
    const eligible = await seedCommand();
    await sql`
      UPDATE khala_sync_portable_commands
      SET owner_user_id = 'owner.ide13.foreign'
      WHERE command_ref = ${malformed.commandRef}
    `;
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const batch = await queue.claimAcceptedBatch({
      workerInstanceRef: "worker.ide13.dispatch.isolation",
      limit: 10,
      leaseDurationMs: 300_000,
    });
    expect(batch.skippedCommandRefs).toEqual([malformed.commandRef]);
    expect(batch.claims.map((claim) => claim.commandRef)).toEqual([eligible.commandRef]);
  });

  test("replays the same claim bytes and refuses a conflicting executor", async () => {
    const { commandRef, sessionRef } = await seedCommand();
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const input = claimInput(commandRef, "restart");
    const claimed = await queue.claim(input);
    const replayed = await new PostgresPortableSessionCommandQueue(
      sql as unknown as SyncSql,
      () => baseNow,
    ).claim(input);
    expect(claimed.status).toBe("claimed");
    expect(replayed).toEqual({ status: "replayed", claim: claimed.claim });
    await sql`
      UPDATE khala_sync_portable_sessions
      SET current_attachment_generation = 2 WHERE session_ref = ${sessionRef}
    `;
    expect(await queue.claim(input)).toEqual({ status: "replayed", claim: claimed.claim });
    await expect(queue.claim(claimInput(commandRef, "other", "other"))).rejects.toMatchObject({
      code: "claim_conflict",
    });
  });

  test("renews with revision and generation fencing, then applies terminal CAS", async () => {
    const { commandRef } = await seedCommand();
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const claimed = await queue.claim(claimInput(commandRef, "lifecycle"));
    const renew = {
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef: claimed.claim.claimRef,
      executorEnvironmentRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.lifecycle",
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      leaseExpiresAt: "2026-07-20T12:10:00.000Z",
    };
    const renewed = await queue.renew(renew);
    expect(renewed).toMatchObject({ status: "renewed", claim: { leaseRevision: 2 } });
    expect(await queue.renew(renew)).toEqual({ status: "replayed", claim: renewed.claim });
    await expect(queue.renew({ ...renew, claimGeneration: 2 })).rejects.toMatchObject({
      code: "stale_generation",
    });

    const terminal = {
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef: claimed.claim.claimRef,
      executorEnvironmentRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.lifecycle",
      claimGeneration: 1,
      expectedLeaseRevision: 2,
      terminalStatus: "completed" as const,
      outcomeRef: "outcome.ide13.lifecycle.completed",
      evidenceRefs: ["evidence.ide13.lifecycle.completed"],
      completedAt: baseNow,
    };
    const completed = await queue.terminal(terminal);
    expect(completed).toMatchObject({
      status: "terminal",
      claim: { leaseRevision: 3, state: "terminal", terminalStatus: "completed" },
    });
    expect(await queue.terminal(terminal)).toEqual({ status: "replayed", claim: completed.claim });
    expect(await queue.claim(claimInput(commandRef, "lifecycle"))).toEqual({
      status: "replayed",
      claim: completed.claim,
    });
    const replayConsumer = new PortableSessionCommandConsumer({
      queue,
      resolver: {
        resolve: async () => {
          throw new Error("terminal replay must not resolve");
        },
      },
      runtime: {
        move: async () => {
          throw new Error("terminal replay must not run");
        },
      },
      now: () => baseNow,
    });
    expect(await replayConsumer.execute(claimInput(commandRef, "lifecycle"))).toEqual({
      status: "completed",
      claim: completed.claim,
    });
    await expect(
      queue.terminal({
        ...terminal,
        outcomeRef: "outcome.ide13.lifecycle.conflict",
      }),
    ).rejects.toMatchObject({ code: "stale_revision" });
  });

  test("persists pending reconcile with idempotent claimed-state CAS", async () => {
    const { commandRef } = await seedCommand();
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const claimed = await queue.claim(claimInput(commandRef, "reconcile"));
    const pending = {
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef: claimed.claim.claimRef,
      executorEnvironmentRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.reconcile",
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      pendingReconcileRef: "reconcile.ide13.authority-pending",
      evidenceRefs: ["evidence.ide13.authority-pending"],
      observedAt: baseNow,
    };
    const marked = await queue.markPendingReconcile(pending);
    expect(marked).toMatchObject({
      status: "pending_reconcile",
      claim: {
        state: "pending_reconcile",
        leaseRevision: 2,
        pendingReconcileRef: pending.pendingReconcileRef,
      },
    });
    expect(await queue.markPendingReconcile(pending)).toEqual({
      status: "replayed",
      claim: marked.claim,
    });
    await expect(
      queue.markPendingReconcile({
        ...pending,
        pendingReconcileRef: "reconcile.ide13.conflict",
      }),
    ).rejects.toMatchObject({ code: "stale_revision" });
  });

  test("expires without unsafe takeover and preserves claim generation", async () => {
    const { commandRef } = await seedCommand();
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    const claimed = await queue.claim(claimInput(commandRef, "expiry"));
    expect(await queue.expire("2026-07-20T12:06:00.000Z")).toBeGreaterThan(0);
    await expect(
      queue.claim({
        ...claimInput(commandRef, "takeover", "takeover"),
        leaseExpiresAt: "2026-07-20T12:15:00.000Z",
      }),
    ).rejects.toMatchObject({ code: "claim_expired" });
    const rows: Array<{ claim_ref: string; claim_generation: string | number; state: string }> =
      await sql`
      SELECT claim_ref, claim_generation, state
      FROM khala_sync_portable_command_executions WHERE command_ref = ${commandRef}
    `;
    expect(rows).toEqual([
      {
        claim_ref: claimed.claim.claimRef,
        claim_generation: expect.toSatisfy((value: unknown) => Number(value) === 1),
        state: "expired",
      },
    ]);
  });

  test("refuses stale session authority and forbidden private material", async () => {
    const stale = await seedCommand();
    await sql`
      UPDATE khala_sync_portable_sessions
      SET current_attachment_generation = 2 WHERE session_ref = ${stale.sessionRef}
    `;
    const queue = new PostgresPortableSessionCommandQueue(sql as unknown as SyncSql, () => baseNow);
    await expect(queue.claim(claimInput(stale.commandRef, "stale"))).rejects.toMatchObject({
      code: "stale_generation",
    });

    const unsafe = await seedCommand();
    await expect(
      queue.claim({
        ...claimInput(unsafe.commandRef, "unsafe"),
        workerInstanceRef: "/Users/person/private-worker",
      }),
    ).rejects.toBeInstanceOf(PortableSessionCommandQueueError);
    await expect(
      queue.claim({
        ...claimInput(unsafe.commandRef, "unsafe"),
        workerInstanceRef: "/Users/person/private-worker",
      }),
    ).rejects.toMatchObject({ code: "unsafe_material" });
  });
});
