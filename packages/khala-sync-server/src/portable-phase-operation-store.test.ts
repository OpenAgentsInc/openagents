import { SQL } from "@openagentsinc/postgres-runtime";
import {
  PORTABLE_COMMAND_SCHEMA_VERSION,
  PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
  type PortablePhaseOperationKind,
  type PortablePhaseOperationRequest,
} from "@openagentsinc/portable-session-contract";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  PortablePhaseOperationStoreError,
  PostgresPortablePhaseOperationStore,
} from "./portable-phase-operation-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const ownerRef = "owner.ide13.phase";
const sourceTargetRef = "target.ide13.phase.source";
const destinationTargetRef = "target.ide13.phase.destination";
const pylonRef = "pylon.ide13.phase.bound";
const now = "2026-07-20T12:00:00.000Z";
const digest = `sha256:${"a".repeat(64)}`;
const otherDigest = `sha256:${"b".repeat(64)}`;

describe.skipIf(!hasLocalPostgres())("IDE-13 durable portable phase exchange", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let sequence = 0;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_phase_exchange");
    await admin.end();
    const result = await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_ide13_phase_exchange"),
    });
    expect(result.applied).toContain("0086_portable_phase_operations.sql");
    expect(result.applied).toContain("0087_portable_phase_destination_activation_receipt.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_phase_exchange"), max: 10 });
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref, owner_user_id, target_class, adapter_ref, compatibility_ref,
         isolation, data_posture, health)
      VALUES
        (${sourceTargetRef}, ${ownerRef}, 'owner_local', 'adapter.ide13.phase.source',
         'compat.ide13.phase', 'owner_host_process', 'owner_device_only', 'ready'),
        (${destinationTargetRef}, ${ownerRef}, 'owner_managed', 'adapter.ide13.phase.destination',
         'compat.ide13.phase', 'dedicated_microvm', 'owner_managed_region', 'ready')
    `;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const seedExecution = async () => {
    sequence += 1;
    const suffix = String(sequence);
    const sessionRef = `session.ide13.phase.${suffix}`;
    const attachmentRef = `attachment.ide13.phase.source.${suffix}`;
    const commandRef = `command.ide13.phase.${suffix}`;
    const executionClaimRef = `claim.ide13.phase.command.${suffix}`;
    const command = {
      schema: PORTABLE_COMMAND_SCHEMA_VERSION,
      commandRef,
      idempotencyKey: `idempotency.ide13.phase.${suffix}`,
      ownerRef,
      sessionRef,
      kind: "move" as const,
      expectedAttachmentRef: attachmentRef,
      expectedGeneration: 1,
      destinationTargetRef,
      checkpointRef: `checkpoint.ide13.phase.input.${suffix}`,
      expiresAt: "2026-07-20T13:00:00.000Z",
    };
    await sql.begin(async (tx) => {
      await tx`
        INSERT INTO khala_sync_portable_sessions
          (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
           event_log_ref, current_projection_ref, command_scope_ref, root_agent_ref,
           state, current_attachment_ref, current_attachment_generation)
        VALUES
          (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`}, ${`work.phase.${suffix}`},
           ${`eventlog.phase.${suffix}`}, ${`projection.phase.${suffix}`},
           ${`commands.phase.${suffix}`}, ${`agent.phase.root.${suffix}`},
           'active', ${attachmentRef}, 1)
      `;
      await tx`
        INSERT INTO khala_sync_portable_session_targets (session_ref, target_ref)
        VALUES (${sessionRef}, ${sourceTargetRef}), (${sessionRef}, ${destinationTargetRef})
      `;
      await tx`
        INSERT INTO khala_sync_portable_attachments
          (attachment_ref, session_ref, target_ref, generation, state,
           descendant_agent_refs_json, capability_lease_refs_json, evidence_refs_json)
        VALUES
          (${attachmentRef}, ${sessionRef}, ${sourceTargetRef}, 1, 'active',
           '[]'::jsonb, '[]'::jsonb, '[]'::jsonb)
      `;
      await tx`
        INSERT INTO khala_sync_portable_commands
          (command_ref, idempotency_key, owner_user_id, session_ref, kind,
           expected_attachment_ref, expected_generation, destination_target_ref,
           checkpoint_ref, expires_at, command_json, status)
        VALUES
          (${commandRef}, ${command.idempotencyKey}, ${ownerRef}, ${sessionRef}, 'move',
           ${attachmentRef}, 1, ${destinationTargetRef}, ${command.checkpointRef},
           ${command.expiresAt}, ${JSON.stringify(command)}::jsonb, 'accepted')
      `;
      await tx`
        INSERT INTO khala_sync_portable_command_executions
          (command_ref, claim_ref, owner_user_id, session_ref, command_kind,
           command_fingerprint, claim_fingerprint, source_attachment_ref,
           source_generation, destination_target_ref, executor_environment_ref,
           worker_instance_ref, claim_generation, lease_revision, state,
           claimed_at, lease_expires_at, updated_at)
        VALUES
          (${commandRef}, ${executionClaimRef}, ${ownerRef}, ${sessionRef}, 'move',
           ${digest}, ${otherDigest}, ${attachmentRef}, 1, ${destinationTargetRef},
           ${sourceTargetRef}, ${`worker.ide13.command.${suffix}`}, 1, 1, 'claimed',
           ${now}, '2026-07-20T12:30:00.000Z', ${now})
      `;
    });
    return { suffix, sessionRef, attachmentRef, commandRef, executionClaimRef };
  };

  const operationRequest = (
    fixture: Awaited<ReturnType<typeof seedExecution>>,
    kind: PortablePhaseOperationKind = "quiesce",
  ) => {
    const destination = !["quiesce", "checkpoint-create", "source-cleanup"].includes(kind);
    const checkpointArtifact = kind === "checkpoint-stage" || kind === "destination-activate";
    const checkpointIdentity = checkpointArtifact || kind === "checkpoint-create";
    return {
      schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
      operationRef: `operation.ide13.phase.${fixture.suffix}.${kind}`,
      commandRef: fixture.commandRef,
      commandExecutionClaimRef: fixture.executionClaimRef,
      ownerRef,
      sessionRef: fixture.sessionRef,
      attachmentRef: destination
        ? `attachment.ide13.phase.destination.${fixture.suffix}`
        : fixture.attachmentRef,
      attachmentGeneration: destination ? 2 : 1,
      targetRef: destination ? destinationTargetRef : sourceTargetRef,
      pylonRef,
      kind,
      checkpointRef: checkpointIdentity ? `checkpoint.ide13.phase.${fixture.suffix}` : null,
      checkpointObjectRef: checkpointArtifact ? `object.ide13.phase.${fixture.suffix}` : null,
      checkpointDigest: checkpointArtifact ? digest : null,
      evidenceRefs: [`evidence.ide13.phase.request.${fixture.suffix}`],
      expiresAt: "2026-07-20T12:20:00.000Z",
    };
  };

  const claimRequest = (
    operation: Pick<
      PortablePhaseOperationRequest,
      "operationRef" | "sessionRef" | "attachmentRef" | "attachmentGeneration" | "targetRef"
    >,
    worker: string,
    claim = worker,
  ) => ({
    schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
    operationRef: operation.operationRef,
    claimRef: `claim.ide13.phase.operation.${claim}`,
    sessionRef: operation.sessionRef,
    attachmentRef: operation.attachmentRef,
    attachmentGeneration: operation.attachmentGeneration,
    pylonRef,
    targetRef: operation.targetRef,
    workerInstanceRef: `worker.ide13.phase.${worker}`,
    leaseExpiresAt: "2026-07-20T12:10:00.000Z",
  });

  const prepareDestinationActivation = async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const reservationRef = `runner-session-reservation.${fixture.suffix}`;
    const stage = await store.enqueue(operationRequest(fixture, "checkpoint-stage"));
    const stageClaim = await store.claim(
      claimRequest(stage.operation.request, `stage.${fixture.suffix}`),
    );
    if (stageClaim.operation.claimRef === null) throw new Error("stage claim ref is missing");
    await store.complete({
      schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
      claimRef: stageClaim.operation.claimRef,
      sessionRef: fixture.sessionRef,
      attachmentRef: stage.operation.request.attachmentRef,
      attachmentGeneration: 2,
      pylonRef,
      targetRef: destinationTargetRef,
      workerInstanceRef: `worker.ide13.phase.stage.${fixture.suffix}`,
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      resultRef: `result.ide13.phase.stage.${fixture.suffix}`,
      resultStatus: "completed" as const,
      checkpointRef: null,
      checkpointObjectRef: null,
      checkpointDigest: null,
      checkpointManifestDigest: null,
      destinationRunnerSessionReservationRef: reservationRef,
      destinationActivationReceipt: null,
      evidenceRefs: [`evidence.ide13.phase.stage.${fixture.suffix}`],
      errorRef: null,
      completedAt: now,
    });

    const operation = await store.enqueue(operationRequest(fixture, "destination-activate"));
    const claimed = await store.claim(
      claimRequest(operation.operation.request, `activation.${fixture.suffix}`),
    );
    if (claimed.operation.claimRef === null) throw new Error("phase claim ref is missing");
    const activationReceipt = {
      schema: "openagents.ide_portable_destination_activation.v1" as const,
      receiptRef: `receipt.ide13.phase.activation.${fixture.suffix}`,
      operationRef: operation.operation.request.operationRef,
      sessionRef: fixture.sessionRef,
      checkpointRef: operation.operation.request.checkpointRef,
      destinationTargetRef,
      destinationAttachmentRef: operation.operation.request.attachmentRef,
      destinationRunnerSessionReservationRef: reservationRef,
      destinationGeneration: 2,
      authentication: {
        state: "reauthenticated" as const,
        policyRef: "policy.portable.destination.owner_managed.v1",
        evidenceRef: `evidence.ide13.phase.auth.${fixture.suffix}`,
        observedAt: now,
        expiresAt: "2026-07-20T12:15:00.000Z",
      },
      helpersObservedAt: now,
      helpers: (["pty", "lsp", "dap", "watcher", "native"] as const).map((kind) => ({
        kind,
        readiness: "unsupported" as const,
        instanceRef: null,
        versionRef: null,
        omissionRef: `omission.ide13.phase.${kind}.${fixture.suffix}`,
        evidenceRefs: [],
      })),
      activatedAgentRefs: [`agent.ide13.phase.root.${fixture.suffix}`],
      acceptedWorkRefs: [],
      evidenceRefs: [`evidence.ide13.phase.activation.${fixture.suffix}`],
    };
    const completion = {
      schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
      claimRef: claimed.operation.claimRef,
      sessionRef: fixture.sessionRef,
      attachmentRef: operation.operation.request.attachmentRef,
      attachmentGeneration: 2,
      pylonRef,
      targetRef: destinationTargetRef,
      workerInstanceRef: `worker.ide13.phase.activation.${fixture.suffix}`,
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      resultRef: `result.ide13.phase.activation.${fixture.suffix}`,
      resultStatus: "completed" as const,
      checkpointRef: null,
      checkpointObjectRef: null,
      checkpointDigest: null,
      checkpointManifestDigest: null,
      destinationActivationReceipt: activationReceipt,
      evidenceRefs: activationReceipt.evidenceRefs,
      errorRef: null,
      completedAt: now,
    };
    return { activationReceipt, completion, fixture, operation, reservationRef, store };
  };

  test("enqueues byte-idempotently and exposes only the exact Pylon target queue", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const request = operationRequest(fixture);
    const first = await store.enqueue(request);
    expect(first).toMatchObject({ status: "enqueued", operation: { state: "pending", request } });
    expect(await store.enqueue(request)).toEqual({
      status: "replayed",
      operation: first.operation,
    });
    await expect(
      store.enqueue({ ...request, pylonRef: "pylon.ide13.phase.other" }),
    ).rejects.toMatchObject({
      code: "conflict",
    });
    await expect(
      store.enqueue({
        ...request,
        operationRef: `operation.ide13.phase.${fixture.suffix}.quiesce.other`,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    expect(await store.pending(pylonRef, sourceTargetRef)).toContainEqual(first.operation);
    expect(await store.read(pylonRef, sourceTargetRef, request.operationRef)).toEqual(
      first.operation,
    );
    await expect(
      store.read("pylon.ide13.phase.other", sourceTargetRef, request.operationRef),
    ).rejects.toMatchObject({ code: "not_found" });
    expect(await store.pending("pylon.ide13.phase.other", sourceTargetRef)).toEqual([]);
    await sql`
      UPDATE khala_sync_portable_phase_operations
      SET request_json = ${JSON.stringify({
        ...request,
        attachmentRef: "attachment.ide13.phase.tampered",
      })}::jsonb
      WHERE operation_ref = ${request.operationRef}
    `;
    await expect(store.pending(pylonRef, sourceTargetRef)).rejects.toMatchObject({
      code: "invalid",
    });
  });

  test("persists every refs-only remote phase kind", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const kinds: ReadonlyArray<PortablePhaseOperationKind> = [
      "quiesce",
      "checkpoint-create",
      "source-cleanup",
      "checkpoint-stage",
      "destination-activate",
      "staged-abort",
    ];
    const queued = await Promise.all(
      kinds.map((kind) => store.enqueue(operationRequest(fixture, kind))),
    );
    expect(queued.map((result) => result.operation.request.kind)).toEqual(kinds);
  });

  test("serializes concurrent byte-identical operation requests", async () => {
    const fixture = await seedExecution();
    const request = operationRequest(fixture);
    const first = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const second = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const results = await Promise.all([first.enqueue(request), second.enqueue(request)]);
    expect(results.filter((result) => result.status === "enqueued")).toHaveLength(1);
    expect(results.filter((result) => result.status === "replayed")).toHaveLength(1);
    expect(results[0]?.operation).toEqual(results[1]?.operation);
  });

  test("serializes two PostgreSQL claimers and refuses binding changes or takeover", async () => {
    const fixture = await seedExecution();
    const firstStore = new PostgresPortablePhaseOperationStore(
      sql as unknown as SyncSql,
      () => now,
    );
    const secondStore = new PostgresPortablePhaseOperationStore(
      sql as unknown as SyncSql,
      () => now,
    );
    const operation = await firstStore.enqueue(operationRequest(fixture));
    const results = await Promise.allSettled([
      firstStore.claim(claimRequest(operation.operation.request, "parallel-a")),
      secondStore.claim(claimRequest(operation.operation.request, "parallel-b")),
    ]);
    expect(results.filter((result) => result.status === "fulfilled")).toHaveLength(1);
    expect(results.find((result) => result.status === "rejected")).toMatchObject({
      reason: expect.objectContaining({ code: "conflict" }),
    });

    const claimed = results.find((result) => result.status === "fulfilled");
    if (claimed?.status !== "fulfilled") throw new Error("claim fixture missing");
    const workerRef = claimed.value.operation.workerInstanceRef;
    const claimRef = claimed.value.operation.claimRef;
    if (workerRef === null || claimRef === null) throw new Error("claimed binding is missing");
    const exactWorker = workerRef.slice("worker.ide13.phase.".length);
    const exactClaim = claimRef.slice("claim.ide13.phase.operation.".length);
    const exact = claimRequest(operation.operation.request, exactWorker, exactClaim);
    expect(await firstStore.claim(exact)).toEqual({
      status: "replayed",
      operation: claimed.value.operation,
    });
    await expect(
      firstStore.claim({ ...exact, pylonRef: "pylon.ide13.phase.other" }),
    ).rejects.toMatchObject({
      code: "conflict",
    });
    await expect(firstStore.claim({ ...exact, attachmentGeneration: 2 })).rejects.toMatchObject({
      code: "conflict",
    });
    expect(await firstStore.expire("2026-07-20T12:11:00.000Z")).toBeGreaterThan(0);
    await expect(
      firstStore.claim(claimRequest(operation.operation.request, "takeover")),
    ).rejects.toMatchObject({ code: "expired" });
  });

  test("renews with lease and generation CAS and completes checkpoint creation with refs only", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const operation = await store.enqueue(operationRequest(fixture, "checkpoint-create"));
    const claimed = await store.claim(claimRequest(operation.operation.request, "checkpoint"));
    const phaseClaimRef = claimed.operation.claimRef;
    if (phaseClaimRef === null) throw new Error("phase claim ref is missing");
    const renew = {
      schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
      claimRef: phaseClaimRef,
      sessionRef: operation.operation.request.sessionRef,
      attachmentRef: operation.operation.request.attachmentRef,
      attachmentGeneration: operation.operation.request.attachmentGeneration,
      pylonRef,
      targetRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.phase.checkpoint",
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      leaseExpiresAt: "2026-07-20T12:15:00.000Z",
    };
    const renewed = await store.renew(renew);
    expect(renewed).toMatchObject({ status: "renewed", operation: { leaseRevision: 2 } });
    expect(await store.renew(renew)).toEqual({ status: "replayed", operation: renewed.operation });
    await expect(store.renew({ ...renew, claimGeneration: 2 })).rejects.toMatchObject({
      code: "stale_generation",
    });

    const result = {
      schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
      claimRef: phaseClaimRef,
      sessionRef: operation.operation.request.sessionRef,
      attachmentRef: operation.operation.request.attachmentRef,
      attachmentGeneration: operation.operation.request.attachmentGeneration,
      pylonRef,
      targetRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.phase.checkpoint",
      claimGeneration: 1,
      expectedLeaseRevision: 2,
      resultRef: `result.ide13.phase.${fixture.suffix}`,
      resultStatus: "completed" as const,
      checkpointRef: `checkpoint.ide13.phase.result.${fixture.suffix}`,
      checkpointObjectRef: `object.ide13.phase.result.${fixture.suffix}`,
      checkpointDigest: otherDigest,
      checkpointManifestDigest: digest,
      destinationActivationReceipt: null,
      evidenceRefs: [`evidence.ide13.phase.result.${fixture.suffix}`],
      errorRef: null,
      completedAt: now,
    };
    const completed = await store.complete(result);
    expect(completed).toMatchObject({
      status: "completed",
      operation: {
        state: "completed",
        leaseRevision: 3,
        resultCheckpointObjectRef: result.checkpointObjectRef,
        resultCheckpointDigest: otherDigest,
        resultCheckpointManifestDigest: digest,
      },
    });
    expect(await store.complete(result)).toEqual({
      status: "replayed",
      operation: completed.operation,
    });
    await expect(
      store.complete({
        ...result,
        completedAt: "2026-07-20T12:00:00.001Z",
      }),
    ).rejects.toMatchObject({ code: "stale_revision" });
    await expect(
      store.complete({
        ...result,
        resultRef: `result.ide13.phase.conflict.${fixture.suffix}`,
      }),
    ).rejects.toMatchObject({ code: "stale_revision" });
  });

  test("persists a byte-idempotent failed result without checkpoint material", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const operation = await store.enqueue(operationRequest(fixture));
    const claimed = await store.claim(claimRequest(operation.operation.request, "failure"));
    if (claimed.operation.claimRef === null) throw new Error("phase claim ref is missing");
    const result = {
      schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
      claimRef: claimed.operation.claimRef,
      sessionRef: operation.operation.request.sessionRef,
      attachmentRef: operation.operation.request.attachmentRef,
      attachmentGeneration: operation.operation.request.attachmentGeneration,
      pylonRef,
      targetRef: sourceTargetRef,
      workerInstanceRef: "worker.ide13.phase.failure",
      claimGeneration: 1,
      expectedLeaseRevision: 1,
      resultRef: `result.ide13.phase.failed.${fixture.suffix}`,
      resultStatus: "failed" as const,
      checkpointRef: null,
      checkpointObjectRef: null,
      checkpointDigest: null,
      checkpointManifestDigest: null,
      destinationActivationReceipt: null,
      evidenceRefs: [`evidence.ide13.phase.failed.${fixture.suffix}`],
      errorRef: "error.ide13.phase.target_rejected",
      completedAt: now,
    };
    const failed = await store.complete(result);
    expect(failed).toMatchObject({
      status: "failed",
      operation: {
        state: "failed",
        resultStatus: "failed",
        resultFingerprint: expect.stringMatching(/^sha256:[0-9a-f]{64}$/),
        resultCheckpointObjectRef: null,
      },
    });
    expect(await store.complete(result)).toEqual({
      status: "replayed",
      operation: failed.operation,
    });
    await expect(
      store.complete({ ...result, errorRef: "error.ide13.phase.other" }),
    ).rejects.toMatchObject({ code: "stale_revision" });
  });

  test("accepts only an exact complete destination activation receipt", async () => {
    const { activationReceipt, completion, store } = await prepareDestinationActivation();
    await expect(
      store.complete({
        ...completion,
        destinationActivationReceipt: {
          ...activationReceipt,
          destinationGeneration: 1,
        },
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    await expect(
      store.complete({
        ...completion,
        destinationActivationReceipt: {
          ...activationReceipt,
          helpers: activationReceipt.helpers.slice(1),
        },
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    const completed = await store.complete(completion);
    expect(completed.operation.resultDestinationActivationReceipt).toEqual(activationReceipt);
    expect(await store.complete(completion)).toEqual({
      status: "replayed",
      operation: completed.operation,
    });
  });

  test("rejects a destination activation without a completed checkpoint stage", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    const activation = await store.enqueue(operationRequest(fixture, "destination-activate"));
    const claimed = await store.claim(
      claimRequest(activation.operation.request, `missing-stage.${fixture.suffix}`),
    );
    if (claimed.operation.claimRef === null) throw new Error("activation claim ref is missing");
    const prepared = await prepareDestinationActivation();
    await expect(
      store.complete({
        ...prepared.completion,
        claimRef: claimed.operation.claimRef,
        sessionRef: fixture.sessionRef,
        attachmentRef: activation.operation.request.attachmentRef,
        workerInstanceRef: `worker.ide13.phase.missing-stage.${fixture.suffix}`,
        resultRef: `result.ide13.phase.activation.missing-stage.${fixture.suffix}`,
        destinationActivationReceipt: {
          ...prepared.activationReceipt,
          receiptRef: `receipt.ide13.phase.activation.missing-stage.${fixture.suffix}`,
          operationRef: activation.operation.request.operationRef,
          sessionRef: fixture.sessionRef,
          checkpointRef: activation.operation.request.checkpointRef,
          destinationAttachmentRef: activation.operation.request.attachmentRef,
        },
      }),
    ).rejects.toMatchObject({ code: "conflict" });
  });

  test("rejects swapped, stale, and foreign checkpoint-stage bindings", async () => {
    const swapped = await prepareDestinationActivation();
    const other = await prepareDestinationActivation();
    await sql`
      UPDATE khala_sync_portable_phase_operations
      SET result_destination_runner_session_reservation_ref = ${other.reservationRef}
      WHERE command_execution_claim_ref = ${swapped.fixture.executionClaimRef}
        AND kind = 'checkpoint-stage'
    `;
    await expect(swapped.store.complete(swapped.completion)).rejects.toMatchObject({
      code: "invalid",
    });

    const stale = await prepareDestinationActivation();
    await sql`
      UPDATE khala_sync_portable_phase_operations
      SET attachment_generation = 1
      WHERE command_execution_claim_ref = ${stale.fixture.executionClaimRef}
        AND kind = 'checkpoint-stage'
    `;
    await expect(stale.store.complete(stale.completion)).rejects.toMatchObject({
      code: "conflict",
    });

    const foreign = await prepareDestinationActivation();
    await sql`
      UPDATE khala_sync_portable_phase_operations
      SET owner_user_id = 'owner.ide13.phase.foreign'
      WHERE command_execution_claim_ref = ${foreign.fixture.executionClaimRef}
        AND kind = 'checkpoint-stage'
    `;
    await expect(foreign.store.complete(foreign.completion)).rejects.toMatchObject({
      code: "conflict",
    });
  });

  test("rejects duplicate checkpoint-stage rows and expired command authority", async () => {
    const duplicate = await prepareDestinationActivation();
    const uniqueConstraints: Array<{ conname: string }> = await sql`
      SELECT conname
      FROM pg_constraint
      WHERE conrelid = 'khala_sync_portable_phase_operations'::regclass
        AND contype = 'u'
        AND pg_get_constraintdef(oid) LIKE '%command_execution_claim_ref, kind%'
    `;
    const uniqueConstraint = uniqueConstraints[0]?.conname;
    if (uniqueConstraint === undefined) throw new Error("phase uniqueness constraint is missing");
    const quotedUniqueConstraint = `"${uniqueConstraint.replaceAll('"', '""')}"`;
    await sql.unsafe(
      `ALTER TABLE khala_sync_portable_phase_operations DROP CONSTRAINT ${quotedUniqueConstraint}`,
    );
    try {
      await sql`
        INSERT INTO khala_sync_portable_phase_operations
        SELECT operation_ref || '.duplicate', request_fingerprint, command_ref,
               command_execution_claim_ref, owner_user_id, session_ref, attachment_ref,
               attachment_generation, target_ref, pylon_ref, kind, checkpoint_ref,
               checkpoint_object_ref, checkpoint_digest, request_evidence_refs_json,
               request_json, expires_at, state, claim_ref || '.duplicate', claim_fingerprint,
               worker_instance_ref, claim_generation, lease_revision, claimed_at,
               lease_expires_at, result_ref || '.duplicate', result_fingerprint, result_status,
               result_checkpoint_ref, result_checkpoint_object_ref, result_checkpoint_digest,
               result_evidence_refs_json, error_ref, completed_at, created_at, updated_at,
               result_destination_activation_receipt_json, result_checkpoint_manifest_digest,
               result_destination_runner_session_reservation_ref
        FROM khala_sync_portable_phase_operations
        WHERE command_execution_claim_ref = ${duplicate.fixture.executionClaimRef}
          AND kind = 'checkpoint-stage'
      `;
      await expect(duplicate.store.complete(duplicate.completion)).rejects.toMatchObject({
        code: "conflict",
      });
    } finally {
      await sql`
        DELETE FROM khala_sync_portable_phase_operations
        WHERE operation_ref = ${`operation.ide13.phase.${duplicate.fixture.suffix}.checkpoint-stage.duplicate`}
      `;
      await sql.unsafe(
        `ALTER TABLE khala_sync_portable_phase_operations ADD CONSTRAINT ${quotedUniqueConstraint} UNIQUE (command_execution_claim_ref, kind)`,
      );
    }

    const expired = await prepareDestinationActivation();
    await sql`
      UPDATE khala_sync_portable_command_executions
      SET lease_expires_at = '2026-07-20T12:05:00.000Z'
      WHERE claim_ref = ${expired.fixture.executionClaimRef}
    `;
    const laterStore = new PostgresPortablePhaseOperationStore(
      sql as unknown as SyncSql,
      () => "2026-07-20T12:06:00.000Z",
    );
    await expect(laterStore.complete(expired.completion)).rejects.toMatchObject({
      code: "expired",
    });
  });

  test("rejects stale generation, wrong destination binding, and private material", async () => {
    const fixture = await seedExecution();
    const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
    await expect(
      store.enqueue({
        ...operationRequest(fixture),
        attachmentGeneration: 2,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    await expect(
      store.enqueue({
        ...operationRequest(fixture, "checkpoint-stage"),
        targetRef: sourceTargetRef,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    const unsafeFields = [
      { checkpointBytes: "private-checkpoint" },
      { workspacePath: "/Users/private/workspace" },
      { providerCredential: "private-provider-credential" },
      { nativeHandle: "native-handle" },
    ];
    await Promise.all(
      unsafeFields.flatMap((unsafe) => [
        expect(store.enqueue({ ...operationRequest(fixture), ...unsafe })).rejects.toBeInstanceOf(
          PortablePhaseOperationStoreError,
        ),
        expect(store.enqueue({ ...operationRequest(fixture), ...unsafe })).rejects.toMatchObject({
          code: "unsafe_material",
        }),
      ]),
    );
  });
});
