import { SQL } from "@openagentsinc/postgres-runtime";
import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  PORTABLE_COMMAND_SCHEMA_VERSION,
  PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
  type PortableCommandExecutionClaim,
  type PortablePhaseOperationKind,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { PostgresPortablePhaseOperationStore } from "./portable-phase-operation-store.js";
import {
  PortablePhaseTargetError,
  PostgresPortablePhaseTarget,
  type PostgresPortablePhaseTargetConfig,
} from "./portable-phase-target-adapter.js";
import type { PortableCheckpointBundle } from "./portable-session-move.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const ownerRef = "owner.ide13.adapter";
const sourceTargetRef = "target.ide13.adapter.source";
const destinationTargetRef = "target.ide13.adapter.destination";
const pylonRef = "pylon.ide13.adapter";
const now = "2026-07-20T12:00:00.000Z";
const operationExpiresAt = "2026-07-20T12:20:00.000Z";
const claimExpiresAt = "2026-07-20T12:30:00.000Z";
const digestA = `sha256:${"a".repeat(64)}`;
const digestB = `sha256:${"b".repeat(64)}`;
const digestC = `sha256:${"c".repeat(64)}`;
const digestD = `sha256:${"d".repeat(64)}`;

type Fixture = Readonly<{
  suffix: string;
  sessionRef: string;
  sourceAttachmentRef: string;
  destinationAttachmentRef: string;
  commandRef: string;
  claim: PortableCommandExecutionClaim;
  bundle: PortableCheckpointBundle;
}>;

describe.skipIf(!hasLocalPostgres())("IDE-13 portable phase target adapter", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let sequence = 0;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_ide13_phase_target");
    await admin.end();
    const migrated = await runMigrations({
      databaseUrl: pg.urlFor("khala_sync_ide13_phase_target"),
    });
    expect(migrated.applied).toContain("0084_portable_phase_operations.sql");
    sql = SQL({ url: pg.urlFor("khala_sync_ide13_phase_target"), max: 10 });
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref, owner_user_id, target_class, adapter_ref, compatibility_ref,
         isolation, data_posture, health)
      VALUES
        (${sourceTargetRef}, ${ownerRef}, 'owner_local', 'adapter.ide13.source',
         'compat.ide13.adapter', 'owner_host_process', 'owner_device_only', 'ready'),
        (${destinationTargetRef}, ${ownerRef}, 'owner_managed', 'adapter.ide13.destination',
         'compat.ide13.adapter', 'dedicated_microvm', 'owner_managed_region', 'ready')
    `;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const seed = async (): Promise<Fixture> => {
    sequence += 1;
    const suffix = String(sequence);
    const sessionRef = `session.ide13.adapter.${suffix}`;
    const sourceAttachmentRef = `attachment.ide13.adapter.source.${suffix}`;
    const destinationAttachmentRef = `attachment.ide13.adapter.destination.${suffix}`;
    const commandRef = `command.ide13.adapter.${suffix}`;
    const checkpointRef = `checkpoint.ide13.adapter.${suffix}`;
    const claimRef = `claim.ide13.adapter.command.${suffix}`;
    const command = {
      schema: PORTABLE_COMMAND_SCHEMA_VERSION,
      commandRef,
      idempotencyKey: `idempotency.ide13.adapter.${suffix}`,
      ownerRef,
      sessionRef,
      kind: "move" as const,
      expectedAttachmentRef: sourceAttachmentRef,
      expectedGeneration: 1,
      destinationTargetRef,
      checkpointRef,
      expiresAt: "2026-07-20T13:00:00.000Z",
    };
    const claim: PortableCommandExecutionClaim = {
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef,
      commandRef,
      ownerRef,
      sessionRef,
      commandKind: "move",
      commandFingerprint: digestA,
      claimFingerprint: digestB,
      sourceAttachmentRef,
      sourceGeneration: 1,
      destinationTargetRef,
      executorEnvironmentRef: sourceTargetRef,
      workerInstanceRef: `worker.ide13.adapter.command.${suffix}`,
      claimGeneration: 1,
      leaseRevision: 1,
      state: "claimed",
      claimedAt: now,
      leaseExpiresAt: claimExpiresAt,
      updatedAt: now,
      terminalStatus: null,
      pendingReconcileRef: null,
      outcomeRef: null,
      evidenceRefs: [],
    };
    const bundle: PortableCheckpointBundle = {
      checkpoint: {
        schema: "openagents.portable_checkpoint.v1",
        checkpointRef,
        sessionRef,
        sourceAttachmentRef,
        sourceGeneration: 1,
        digest: digestC,
        repositoryRef: `repository.ide13.adapter.${suffix}`,
        repositoryRevisionRef: `revision.ide13.adapter.${suffix}`,
        repositoryPostImageDigest: digestA,
        diffDigest: digestB,
        eventLogCursor: 9,
        catalogGenerationRef: `catalog.ide13.adapter.${suffix}`,
        graphDigest: digestD,
        approvalRefs: [],
        artifactRefs: [],
        receiptRefs: [`receipt.ide13.adapter.checkpoint.${suffix}`],
        secretMaterial: "excluded",
        processState: "excluded",
      },
      executionBinding: {
        schema: "openagents.portable_session_execution_binding.v1",
        sessionRef,
        ownerRef,
        runRef: `run.ide13.adapter.${suffix}`,
        repositoryRef: `repository.ide13.adapter.${suffix}`,
        pinnedBaseRef: `base.ide13.adapter.${suffix}`,
      },
      graph: {
        rootAgentRef: `agent.ide13.adapter.root.${suffix}`,
        nodes: [
          {
            agentRef: `agent.ide13.adapter.root.${suffix}`,
            threadRef: `thread.ide13.adapter.${suffix}`,
            transcriptRef: `transcript.ide13.adapter.${suffix}`,
            activityCursor: 8,
            lifecycle: "quiesced",
            attachmentGeneration: 1,
          },
        ],
      },
      threadCursors: [
        {
          threadRef: `thread.ide13.adapter.${suffix}`,
          transcriptRef: `transcript.ide13.adapter.${suffix}`,
          activityCursor: 8,
          eventCursor: 9,
        },
      ],
    };
    await sql.begin(async (tx) => {
      await tx`
        INSERT INTO khala_sync_portable_sessions
          (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
           event_log_ref, current_projection_ref, command_scope_ref, root_agent_ref,
           state, current_attachment_ref, current_attachment_generation)
        VALUES
          (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`}, ${`work.adapter.${suffix}`},
           ${`eventlog.adapter.${suffix}`}, ${`projection.adapter.${suffix}`},
           ${`commands.adapter.${suffix}`}, ${bundle.graph.rootAgentRef}, 'active',
           ${sourceAttachmentRef}, 1)
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
          (${sourceAttachmentRef}, ${sessionRef}, ${sourceTargetRef}, 1, 'active',
           ${JSON.stringify([bundle.graph.rootAgentRef])}::jsonb, '[]'::jsonb, '[]'::jsonb)
      `;
      await tx`
        INSERT INTO khala_sync_portable_commands
          (command_ref, idempotency_key, owner_user_id, session_ref, kind,
           expected_attachment_ref, expected_generation, destination_target_ref,
           checkpoint_ref, expires_at, command_json, status)
        VALUES
          (${commandRef}, ${command.idempotencyKey}, ${ownerRef}, ${sessionRef}, 'move',
           ${sourceAttachmentRef}, 1, ${destinationTargetRef}, ${checkpointRef},
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
          (${commandRef}, ${claimRef}, ${ownerRef}, ${sessionRef}, 'move', ${digestA},
           ${digestB}, ${sourceAttachmentRef}, 1, ${destinationTargetRef}, ${sourceTargetRef},
           ${claim.workerInstanceRef}, 1, 1, 'claimed', ${now}, ${claimExpiresAt}, ${now})
      `;
    });
    return {
      suffix,
      sessionRef,
      sourceAttachmentRef,
      destinationAttachmentRef,
      commandRef,
      claim,
      bundle,
    };
  };

  const descriptor = (targetRef: string): PortableTargetDescriptor => ({
    targetRef,
    targetClass: targetRef === sourceTargetRef ? "owner_local" : "owner_managed",
    adapterRef: `adapter.${targetRef}`,
    ownerRef,
    compatibilityRef: "compat.ide13.adapter",
    isolation: targetRef === sourceTargetRef ? "owner_host_process" : "dedicated_microvm",
    dataPosture: targetRef === sourceTargetRef ? "owner_device_only" : "owner_managed_region",
    health: "ready",
  });

  const completingHook =
    (
      fixture: Fixture,
      observed: PortablePhaseOperationKind[],
      resultStatus: "completed" | "failed" = "completed",
      artifactTransports?: Array<Readonly<{
        commandClaim: PortableCommandExecutionClaim;
        manifestDigest: string;
      }> | null>,
      omitStageReservation = false,
    ): NonNullable<PostgresPortablePhaseTargetConfig["onEnqueued"]> =>
    async (enqueued) => {
      observed.push(enqueued.operation.request.kind);
      artifactTransports?.push(enqueued.artifactTransport);
      if (enqueued.status === "replayed") return;
      const store = new PostgresPortablePhaseOperationStore(sql as unknown as SyncSql, () => now);
      const request = enqueued.operation.request;
      const workerInstanceRef = `worker.ide13.adapter.phase.${request.kind}.${fixture.suffix}`;
      const claimed = await store.claim({
        schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
        operationRef: request.operationRef,
        claimRef: `claim.ide13.adapter.phase.${request.kind}.${fixture.suffix}`,
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: request.attachmentGeneration,
        pylonRef: request.pylonRef,
        targetRef: request.targetRef,
        workerInstanceRef,
        leaseExpiresAt: "2026-07-20T12:10:00.000Z",
      });
      if (claimed.operation.claimRef === null) throw new Error("phase claim missing");
      const resultRef = `result.ide13.adapter.phase.${request.kind}.${fixture.suffix}`;
      const destinationActivationReceipt =
        request.kind === "destination-activate" && resultStatus === "completed"
          ? {
              schema: "openagents.ide_portable_destination_activation.v1" as const,
              receiptRef: `receipt.ide13.adapter.destination.${fixture.suffix}`,
              operationRef: request.operationRef,
              sessionRef: request.sessionRef,
              checkpointRef: fixture.bundle.checkpoint.checkpointRef,
              destinationTargetRef: request.targetRef,
              destinationAttachmentRef: request.attachmentRef,
              destinationRunnerSessionReservationRef:
                `runner-session-reservation.${fixture.suffix}`,
              destinationGeneration: request.attachmentGeneration,
              authentication: {
                state: "reauthenticated" as const,
                policyRef: "policy.portable.destination.owner_managed.v1",
                evidenceRef: `evidence.ide13.adapter.authentication.${fixture.suffix}`,
                observedAt: now,
                expiresAt: "2026-07-20T12:15:00.000Z",
              },
              helpersObservedAt: now,
              helpers: [
                {
                  kind: "pty" as const,
                  readiness: "ready" as const,
                  instanceRef: `instance.ide13.adapter.pty.${fixture.suffix}`,
                  versionRef: "version.ide13.adapter.pty.v1",
                  omissionRef: null,
                  evidenceRefs: [`evidence.ide13.adapter.pty.${fixture.suffix}`],
                },
                ...(["lsp", "dap", "watcher", "native"] as const).map((kind) => ({
                  kind,
                  readiness: "unsupported" as const,
                  instanceRef: null,
                  versionRef: null,
                  omissionRef: `omission.ide13.adapter.${kind}.${fixture.suffix}`,
                  evidenceRefs: [],
                })),
              ],
              activatedAgentRefs: fixture.bundle.graph.nodes.map((node) => node.agentRef),
              acceptedWorkRefs: [],
              evidenceRefs: [`evidence.ide13.adapter.activation.${fixture.suffix}`],
            }
          : null;
      await store.complete({
        schema: PORTABLE_PHASE_OPERATION_SCHEMA_VERSION,
        claimRef: claimed.operation.claimRef,
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: request.attachmentGeneration,
        pylonRef: request.pylonRef,
        targetRef: request.targetRef,
        workerInstanceRef,
        claimGeneration: 1,
        expectedLeaseRevision: 1,
        resultRef,
        resultStatus,
        checkpointRef:
          request.kind === "checkpoint-create" && resultStatus === "completed"
            ? fixture.bundle.checkpoint.checkpointRef
            : null,
        checkpointObjectRef:
          request.kind === "checkpoint-create" && resultStatus === "completed"
            ? `object.ide13.adapter.checkpoint.${fixture.suffix}`
            : null,
        checkpointDigest:
          request.kind === "checkpoint-create" && resultStatus === "completed"
            ? fixture.bundle.checkpoint.digest
            : null,
        checkpointManifestDigest:
          request.kind === "checkpoint-create" && resultStatus === "completed" ? digestC : null,
        destinationRunnerSessionReservationRef:
          request.kind === "checkpoint-stage" &&
          resultStatus === "completed" &&
          !omitStageReservation
            ? `runner-session-reservation.${fixture.suffix}`
            : null,
        destinationActivationReceipt,
        evidenceRefs: destinationActivationReceipt?.evidenceRefs ?? [
          `evidence.ide13.adapter.phase.${request.kind}.${fixture.suffix}`,
        ],
        errorRef: resultStatus === "failed" ? "error.ide13.adapter.target_failed" : null,
        completedAt: now,
      });
    };

  const target = (
    fixture: Fixture,
    targetRef: string,
    onEnqueued?: PostgresPortablePhaseTargetConfig["onEnqueued"],
    options: Partial<PostgresPortablePhaseTargetConfig> = {},
  ) =>
    new PostgresPortablePhaseTarget({
      sql: sql as unknown as SyncSql,
      commandExecutionClaim: fixture.claim,
      target: descriptor(targetRef),
      operationExpiresAt,
      resolvePylonRef: async () => pylonRef,
      resolveCheckpointBundle: async () => fixture.bundle,
      now: () => now,
      ...(onEnqueued === undefined ? {} : { onEnqueued }),
      ...options,
    });

  const quiesceInput = (fixture: Fixture) => ({
    operationRef: `operation.${fixture.commandRef}.source.quiesce`,
    sessionRef: fixture.sessionRef,
    attachmentRef: fixture.sourceAttachmentRef,
    generation: 1,
    graph: fixture.bundle.graph,
    threadCursors: fixture.bundle.threadCursors,
  });

  const checkpointInput = (fixture: Fixture) => ({
    operationRef: `operation.${fixture.commandRef}.checkpoint`,
    checkpointRef: fixture.bundle.checkpoint.checkpointRef,
    sessionRef: fixture.sessionRef,
    attachmentRef: fixture.sourceAttachmentRef,
    generation: 1,
    eventLogCursor: 9,
    executionBinding: fixture.bundle.executionBinding,
    graph: fixture.bundle.graph,
    threadCursors: fixture.bundle.threadCursors,
  });

  test("binds and completes the canonical phases in exact order, then replays bytes", async () => {
    const fixture = await seed();
    const observed: PortablePhaseOperationKind[] = [];
    const artifactTransports: Array<Readonly<{
      commandClaim: PortableCommandExecutionClaim;
      manifestDigest: string;
    }> | null> = [];
    const hook = completingHook(fixture, observed, "completed", artifactTransports);
    const source = target(fixture, sourceTargetRef, hook);
    const destination = target(fixture, destinationTargetRef, hook);

    await expect(source.createCheckpoint(checkpointInput(fixture))).rejects.toMatchObject({
      code: "conflict",
    });
    const quiesced = await source.quiesceGraph(quiesceInput(fixture));
    expect(quiesced.quiescedAgentRefs).toEqual([fixture.bundle.graph.rootAgentRef]);
    expect(await source.quiesceGraph(quiesceInput(fixture))).toEqual(quiesced);
    const bundle = await source.createCheckpoint(checkpointInput(fixture));
    expect(bundle).toEqual(fixture.bundle);
    const staged = await destination.stageCheckpoint({
      operationRef: `operation.${fixture.commandRef}.destination.stage`,
      bundle,
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [],
    });
    expect(staged.acceptingWork).toBe(false);
    const cleanup = await source.cleanupSource({
      operationRef: `operation.${fixture.commandRef}.source.cleanup`,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.sourceAttachmentRef,
      generation: 1,
      agentRefs: [fixture.bundle.graph.rootAgentRef],
    });
    expect(cleanup.processes).toBe("released");
    const activation = await destination.activate({
      operationRef: `operation.${fixture.commandRef}.destination.activate`,
      checkpointRef: fixture.bundle.checkpoint.checkpointRef,
      sessionRef: fixture.sessionRef,
      executionBinding: fixture.bundle.executionBinding,
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [],
    });
    expect(activation).toMatchObject({
      activatedAgentRefs: [fixture.bundle.graph.rootAgentRef],
      acceptedWorkRefs: [],
    });
    expect(activation.helpers[0]).toMatchObject({
      kind: "pty",
      readiness: "ready",
      instanceRef: `instance.ide13.adapter.pty.${fixture.suffix}`,
      versionRef: "version.ide13.adapter.pty.v1",
    });
    expect(observed).toEqual([
      "quiesce",
      "quiesce",
      "checkpoint-create",
      "checkpoint-stage",
      "source-cleanup",
      "destination-activate",
    ]);
    expect(artifactTransports).toEqual([
      null,
      null,
      null,
      { commandClaim: fixture.claim, manifestDigest: digestC },
      null,
      null,
    ]);
    const rows: Array<{ kind: string; request_json: unknown }> = await sql`
      SELECT kind, request_json
      FROM khala_sync_portable_phase_operations
      WHERE command_execution_claim_ref = ${fixture.claim.claimRef}
    `;
    expect(rows).toHaveLength(5);
    for (const row of rows) {
      const request =
        typeof row.request_json === "string" ? JSON.parse(row.request_json) : row.request_json;
      expect(request).toMatchObject({
        commandRef: fixture.commandRef,
        commandExecutionClaimRef: fixture.claim.claimRef,
        ownerRef,
        sessionRef: fixture.sessionRef,
        pylonRef,
      });
      expect(JSON.stringify(request)).not.toMatch(/checkpointBytes|\/Users\/|credential|token/i);
      expect(JSON.stringify(request)).not.toContain("manifestDigest");
    }
  });

  test("maps staged abort to its exact durable kind", async () => {
    const fixture = await seed();
    const observed: PortablePhaseOperationKind[] = [];
    const hook = completingHook(fixture, observed);
    const source = target(fixture, sourceTargetRef, hook);
    const destination = target(fixture, destinationTargetRef, hook);
    await source.quiesceGraph(quiesceInput(fixture));
    const bundle = await source.createCheckpoint(checkpointInput(fixture));
    await destination.stageCheckpoint({
      operationRef: `operation.${fixture.commandRef}.destination.stage`,
      bundle,
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [],
    });
    expect(
      await destination.abortStaged({
        operationRef: `operation.${fixture.commandRef}.destination.abort`,
        sessionRef: fixture.sessionRef,
        destinationAttachmentRef: fixture.destinationAttachmentRef,
        destinationGeneration: 2,
      }),
    ).toMatchObject({ evidenceRefs: [expect.stringContaining("staged-abort")] });
    expect(observed.at(-1)).toBe("staged-abort");
  });

  test("rejects caller and resolver checkpoint mismatches before activation", async () => {
    const stageFixture = await seed();
    const stageHook = completingHook(stageFixture, []);
    const stageSource = target(stageFixture, sourceTargetRef, stageHook);
    const stageDestination = target(stageFixture, destinationTargetRef, stageHook);
    await stageSource.quiesceGraph(quiesceInput(stageFixture));
    const stageBundle = await stageSource.createCheckpoint(checkpointInput(stageFixture));
    await expect(
      stageDestination.stageCheckpoint({
        operationRef: `operation.${stageFixture.commandRef}.destination.stage`,
        bundle: {
          ...stageBundle,
          checkpoint: {
            ...stageBundle.checkpoint,
            repositoryPostImageDigest: digestD,
          },
        },
        destinationAttachmentRef: stageFixture.destinationAttachmentRef,
        destinationGeneration: 2,
        capabilityLeaseRefs: [],
      }),
    ).rejects.toMatchObject({ code: "conflict" });

    const activateFixture = await seed();
    const activateHook = completingHook(activateFixture, []);
    const activateSource = target(activateFixture, sourceTargetRef, activateHook);
    const activateDestination = target(activateFixture, destinationTargetRef, activateHook);
    await activateSource.quiesceGraph(quiesceInput(activateFixture));
    const activateBundle = await activateSource.createCheckpoint(checkpointInput(activateFixture));
    await activateDestination.stageCheckpoint({
      operationRef: `operation.${activateFixture.commandRef}.destination.stage`,
      bundle: activateBundle,
      destinationAttachmentRef: activateFixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [],
    });
    await activateSource.cleanupSource({
      operationRef: `operation.${activateFixture.commandRef}.source.cleanup`,
      sessionRef: activateFixture.sessionRef,
      attachmentRef: activateFixture.sourceAttachmentRef,
      generation: 1,
      agentRefs: [activateFixture.bundle.graph.rootAgentRef],
    });
    await expect(
      activateDestination.activate({
        operationRef: `operation.${activateFixture.commandRef}.destination.activate.wrong`,
        checkpointRef: activateFixture.bundle.checkpoint.checkpointRef,
        sessionRef: activateFixture.sessionRef,
        executionBinding: activateFixture.bundle.executionBinding,
        destinationAttachmentRef: `${activateFixture.destinationAttachmentRef}.wrong`,
        destinationGeneration: 2,
        capabilityLeaseRefs: [],
      }),
    ).rejects.toMatchObject({ code: "conflict" });

    const mismatchedResolver = target(activateFixture, destinationTargetRef, activateHook, {
      resolveCheckpointBundle: async () => ({
        ...activateFixture.bundle,
        checkpoint: {
          ...activateFixture.bundle.checkpoint,
          digest: digestD,
        },
      }),
    });
    await expect(
      mismatchedResolver.activate({
        operationRef: `operation.${activateFixture.commandRef}.destination.activate`,
        checkpointRef: activateFixture.bundle.checkpoint.checkpointRef,
        sessionRef: activateFixture.sessionRef,
        executionBinding: activateFixture.bundle.executionBinding,
        destinationAttachmentRef: activateFixture.destinationAttachmentRef,
        destinationGeneration: 2,
        capabilityLeaseRefs: [],
      }),
    ).rejects.toMatchObject({ code: "conflict" });
  });

  test("rejects a completed destination stage with no runner-session reservation", async () => {
    const fixture = await seed();
    const hook = completingHook(fixture, [], "completed", undefined, true);
    const source = target(fixture, sourceTargetRef, hook);
    const destination = target(fixture, destinationTargetRef, hook);
    await source.quiesceGraph(quiesceInput(fixture));
    const bundle = await source.createCheckpoint(checkpointInput(fixture));
    await expect(destination.stageCheckpoint({
      operationRef: `operation.${fixture.commandRef}.destination.stage`,
      bundle,
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [],
    })).rejects.toMatchObject({ code: "invalid" });
  });

  test("surfaces failed, expired, canceled, and timed-out terminal waits", async () => {
    const failed = await seed();
    await expect(
      target(failed, sourceTargetRef, completingHook(failed, [], "failed")).quiesceGraph(
        quiesceInput(failed),
      ),
    ).rejects.toMatchObject({
      code: "failed",
      errorRef: "error.ide13.adapter.target_failed",
    });

    const expired = await seed();
    const expireHook: NonNullable<PostgresPortablePhaseTargetConfig["onEnqueued"]> = async () => {
      const store = new PostgresPortablePhaseOperationStore(
        sql as unknown as SyncSql,
        () => "2026-07-20T12:21:00.000Z",
      );
      await store.expire();
    };
    await expect(
      target(expired, sourceTargetRef, expireHook).quiesceGraph(quiesceInput(expired)),
    ).rejects.toMatchObject({ code: "expired" });

    const canceled = await seed();
    const controller = new AbortController();
    await expect(
      target(canceled, sourceTargetRef, async () => controller.abort(), {
        signal: controller.signal,
        timeout: "1 second",
      }).quiesceGraph(quiesceInput(canceled)),
    ).rejects.toMatchObject({ code: "canceled" });

    const timedOut = await seed();
    await expect(
      target(timedOut, sourceTargetRef, undefined, {
        pollInterval: "1 millis",
        timeout: "2 millis",
      }).quiesceGraph(quiesceInput(timedOut)),
    ).rejects.toMatchObject({ code: "timeout" });
  });

  test("refuses wrong target, generation, resolver material, and private checkpoint data", async () => {
    const fixture = await seed();
    const source = target(fixture, sourceTargetRef, completingHook(fixture, []));
    await expect(
      source.quiesceGraph({
        ...quiesceInput(fixture),
        generation: 2,
      }),
    ).rejects.toBeInstanceOf(PortablePhaseTargetError);
    const wrongTarget = target(fixture, destinationTargetRef, completingHook(fixture, []));
    await expect(wrongTarget.quiesceGraph(quiesceInput(fixture))).rejects.toMatchObject({
      code: "conflict",
    });

    const privateFixture = await seed();
    const privateObserved: PortablePhaseOperationKind[] = [];
    const privateSource = target(
      privateFixture,
      sourceTargetRef,
      completingHook(privateFixture, privateObserved),
      {
        resolveCheckpointBundle: async () => ({
          ...privateFixture.bundle,
          workspacePath: "/Users/private/workspace",
        }),
      },
    );
    await privateSource.quiesceGraph(quiesceInput(privateFixture));
    await expect(
      privateSource.createCheckpoint(checkpointInput(privateFixture)),
    ).rejects.toMatchObject({
      code: "unsafe_result",
    });

    const invalidPylon = await seed();
    await expect(
      target(invalidPylon, sourceTargetRef, undefined, {
        resolvePylonRef: async () => "/Users/private/pylon",
      }).quiesceGraph(quiesceInput(invalidPylon)),
    ).rejects.toMatchObject({ code: "invalid" });
  });
});
