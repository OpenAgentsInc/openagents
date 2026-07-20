import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type CapabilityTargetAdapter,
  type PortableCommandExecutionClaim,
  type PortableCommandExecutionClaimRequest,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { PortablePhaseTargetError } from "./portable-phase-target-adapter.js";
import { PostgresPortableSessionCommandQueue } from "./portable-session-command-queue.js";
import {
  PostgresPortableSessionCommandResolver,
  PostgresPortableSessionCommandRunner,
  type PostgresPortableSessionCommandResolverConfig,
} from "./portable-session-command-runner.js";
import {
  PostgresPortableSessionMoveRuntime,
  type PortableSessionMoveRuntimeBrokerConfig,
} from "./portable-session-move-runtime.js";
import type {
  PortableCheckpointBundle,
  PortableSessionMoveResult,
} from "./portable-session-move.js";
import type { PortableSessionAuthoritySnapshot } from "./portable-session-authority.js";
import type { SyncSql } from "./sql.js";

const now = "2026-07-20T12:00:00.000Z";
const leaseExpiresAt = "2026-07-20T12:10:00.000Z";
const ownerRef = "owner.ide13.runner";
const sessionRef = "session.ide13.runner";
const sourceTargetRef = "target.ide13.source";
const destinationTargetRef = "target.ide13.destination";
const sourceAttachmentRef = "attachment.ide13.source";
const commandRef = "command.ide13.runner";

const command = {
  schema: "openagents.portable_session_command.v1" as const,
  commandRef,
  idempotencyKey: "idempotency.ide13.runner",
  ownerRef,
  sessionRef,
  kind: "move" as const,
  expectedAttachmentRef: sourceAttachmentRef,
  expectedGeneration: 4,
  destinationTargetRef,
  checkpointRef: "checkpoint.ide13.runner",
  expiresAt: "2026-07-20T12:20:00.000Z",
};

const claim: PortableCommandExecutionClaim = {
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  claimRef: "claim.ide13.runner",
  commandRef,
  ownerRef,
  sessionRef,
  commandKind: "move",
  commandFingerprint: `sha256:${createHash("sha256").update(canonicalJson(command)).digest("hex")}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef,
  sourceGeneration: 4,
  destinationTargetRef,
  executorEnvironmentRef: sourceTargetRef,
  workerInstanceRef: "worker.ide13.runner",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: now,
  leaseExpiresAt,
  updatedAt: now,
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
};

const request: PortableCommandExecutionClaimRequest = {
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  commandRef: claim.commandRef,
  claimRef: claim.claimRef,
  executorEnvironmentRef: sourceTargetRef,
  workerInstanceRef: claim.workerInstanceRef,
  leaseExpiresAt,
};

const target = (
  targetRef: string,
  targetClass: PortableTargetDescriptor["targetClass"],
  adapterRef: string,
): Record<string, unknown> => ({
  target_ref: targetRef,
  target_class: targetClass,
  adapter_ref: adapterRef,
  compatibility_ref: "compatibility.ide13.runner",
  isolation: targetClass === "owner_local" ? "owner_host_process" : "dedicated_microvm",
  data_posture: targetClass === "owner_local" ? "owner_device_only" : "openagents_managed_region",
  health: "ready",
});

const snapshot = (): PortableSessionAuthoritySnapshot => ({
  session: {
    session_ref: sessionRef,
    owner_user_id: ownerRef,
    current_attachment_ref: sourceAttachmentRef,
    current_attachment_generation: 4,
  },
  executionBinding: {
    session_ref: sessionRef,
    owner_user_id: ownerRef,
    run_ref: "run.ide13.runner",
    repository_ref: "repository.ide13.runner",
    pinned_base_ref: "commit.ide13.runner",
  },
  targets: [
    target(sourceTargetRef, "owner_local", "adapter.ide13.local"),
    target(destinationTargetRef, "openagents_managed", "adapter.ide13.managed"),
  ],
  agents: [],
  attachments: [
    {
      attachment_ref: sourceAttachmentRef,
      target_ref: sourceTargetRef,
      generation: 4,
      state: "active",
      capability_lease_refs_json: ["lease.ide13.source"],
    },
  ],
  checkpoints: [],
  commands: [
    {
      command_ref: claim.commandRef,
      expected_attachment_ref: sourceAttachmentRef,
      expected_generation: 4,
      destination_target_ref: destinationTargetRef,
      command_json: command,
      status: "accepted",
    },
  ],
  current: [],
});

const adapter = (
  adapterRef: string,
  targetClass: PortableTargetDescriptor["targetClass"],
): CapabilityTargetAdapter => ({
  adapterRef,
  targetClass,
  redeem: async () => ({ installationRef: "installation.ide13.runner" }),
  wipe: async () => ({ wipeReceiptRef: "receipt.ide13.runner.wipe" }),
});

const broker = (): PortableSessionMoveRuntimeBrokerConfig => ({
  vault: {
    withSourceGrantMaterial: async () => {
      throw new Error("vault material must not be read in resolver tests");
    },
    revokeSourceGrant: async () => undefined,
  },
  targets: [
    {
      targetRef: sourceTargetRef,
      targetClass: "owner_local",
      adapterRef: "adapter.ide13.local",
      ready: true,
    },
    {
      targetRef: destinationTargetRef,
      targetClass: "openagents_managed",
      adapterRef: "adapter.ide13.managed",
      ready: true,
    },
  ],
  adapters: [
    adapter("adapter.ide13.local", "owner_local"),
    adapter("adapter.ide13.managed", "openagents_managed"),
  ],
});

const neverSql = Object.assign(
  async (): Promise<never> => {
    throw new Error("SQL must not run in this test");
  },
  {
    begin: async (): Promise<never> => {
      throw new Error("transaction must not run in this test");
    },
  },
) as SyncSql;

const config = (
  authority: PortableSessionAuthoritySnapshot = snapshot(),
): PostgresPortableSessionCommandResolverConfig => ({
  sql: neverSql,
  brokerFactory: { create: async () => broker() },
  pylonBindings: {
    resolve: async (scope) => ({ ...scope, pylonRef: `pylon.${scope.targetRef}` }),
  },
  capabilityGrantFacts: {
    resolve: async (scope) => ({
      facts: scope.sourceLeaseRefs.map((sourceLeaseRef) => ({
        sourceLeaseRef,
        destinationSourceGrantRef: `grant.${sourceLeaseRef}.destination`,
        expiresAt: "2026-07-20T12:09:00.000Z",
      })),
      bindings: [
        {
          sourceLeaseRef: "lease.ide13.source",
          grantRef: "grant.ide13.source",
          ownerUserId: ownerRef,
          kind: "provider",
          providerAccountRef: "provider-account.ide13",
        },
      ],
    }),
  },
  checkpointArtifacts: {
    resolve: async () => {
      throw new Error("artifact resolution is not expected");
    },
  },
  now: () => now,
  readAuthoritySnapshot: async () => authority,
});

afterEach(() => vi.restoreAllMocks());

describe("portable session command production resolver", () => {
  it("binds exact durable command, target, and capability facts", async () => {
    const resolverConfig = config();
    const create = vi.fn(resolverConfig.brokerFactory.create);
    const resolved = await new PostgresPortableSessionCommandResolver({
      ...resolverConfig,
      brokerFactory: { create },
    }).resolve(claim);

    expect(resolved.moveRef).toBe(claim.claimRef);
    expect(resolved.move.command).toEqual(command);
    expect(resolved.move.source.targetRef).toBe(sourceTargetRef);
    expect(resolved.move.destination.targetRef).toBe(destinationTargetRef);
    expect(resolved.move.destinationAttachmentRef).toMatch(/^attachment\.portable\.[a-f0-9]{64}$/u);
    expect(resolved.move.capabilityTransfers).toEqual([
      {
        sourceLeaseRef: "lease.ide13.source",
        destinationLeaseRef: expect.stringMatching(/^lease\.portable\.[a-f0-9]{64}$/u),
        destinationSourceGrantRef: "grant.lease.ide13.source.destination",
        expiresAt: "2026-07-20T12:09:00.000Z",
      },
    ]);
    expect(create).toHaveBeenCalledWith(
      expect.objectContaining({
        claim,
        grantBindings: [
          {
            sourceLeaseRef: "lease.ide13.source",
            grantRef: "grant.ide13.source",
            ownerUserId: ownerRef,
            kind: "provider",
            providerAccountRef: "provider-account.ide13",
          },
        ],
      }),
    );
  });

  it("rejects foreign owner authority before target effects", async () => {
    const authority = snapshot();
    authority.session.owner_user_id = "owner.ide13.foreign";
    await expect(
      new PostgresPortableSessionCommandResolver(config(authority)).resolve(claim),
    ).rejects.toMatchObject({ code: "authority_mismatch" });
  });

  it("rejects a stale current generation before target effects", async () => {
    const authority = snapshot();
    authority.session.current_attachment_generation = 5;
    await expect(
      new PostgresPortableSessionCommandResolver(config(authority)).resolve(claim),
    ).rejects.toMatchObject({ code: "authority_mismatch" });
  });

  it("rejects target drift and broker binding drift", async () => {
    const authority = snapshot();
    const destination = authority.targets[1];
    if (destination === undefined) throw new Error("destination fixture is absent");
    destination.adapter_ref = "adapter.ide13.drifted";
    await expect(
      new PostgresPortableSessionCommandResolver(config(authority)).resolve(claim),
    ).rejects.toMatchObject({ code: "target_mismatch" });
  });

  it("rejects a mismatched Pylon binding before phase persistence", async () => {
    const resolverConfig = config();
    const resolver = new PostgresPortableSessionCommandResolver({
      ...resolverConfig,
      pylonBindings: {
        resolve: async (scope) => ({
          ...scope,
          targetRef: destinationTargetRef,
          pylonRef: "pylon.ide13.wrong",
        }),
      },
    });
    const resolved = await resolver.resolve(claim);

    await expect(
      resolved.move.source.quiesceGraph({
        operationRef: "operation.ide13.quiesce",
        sessionRef,
        attachmentRef: sourceAttachmentRef,
        generation: 4,
        graph: {
          rootAgentRef: "agent.ide13.root",
          nodes: [
            {
              agentRef: "agent.ide13.root",
              threadRef: "thread.ide13.root",
              transcriptRef: "transcript.ide13.root",
              activityCursor: 1,
              lifecycle: "waiting",
              attachmentGeneration: 4,
            },
          ],
        },
        threadCursors: [],
      }),
    ).rejects.toMatchObject({
      code: "invalid",
      message: "portable phase Pylon binding is invalid",
    } satisfies Partial<PortablePhaseTargetError>);
  });

  it("rejects a missing source lease fact before target effects", async () => {
    const resolverConfig = config();
    await expect(
      new PostgresPortableSessionCommandResolver({
        ...resolverConfig,
        capabilityGrantFacts: { resolve: async () => ({ facts: [], bindings: [] }) },
      }).resolve(claim),
    ).rejects.toMatchObject({ code: "capability_mismatch" });
  });

  it("rejects an artifact digest mismatch before destination staging", async () => {
    const bundle = {
      checkpoint: {
        schema: "openagents.portable_checkpoint.v1",
        checkpointRef: command.checkpointRef,
        sessionRef,
        sourceAttachmentRef,
        sourceGeneration: 4,
        digest: `sha256:${"a".repeat(64)}`,
        repositoryRef: "repository.ide13.runner",
        repositoryRevisionRef: "commit.ide13.runner",
        repositoryPostImageDigest: `sha256:${"b".repeat(64)}`,
        diffDigest: `sha256:${"c".repeat(64)}`,
        eventLogCursor: 7,
        catalogGenerationRef: "catalog.ide13.runner",
        graphDigest: `sha256:${"d".repeat(64)}`,
        approvalRefs: [],
        artifactRefs: [],
        receiptRefs: [],
        secretMaterial: "excluded",
        processState: "excluded",
      },
      executionBinding: {
        schema: "openagents.portable_session_execution_binding.v1",
        sessionRef,
        ownerRef,
        runRef: "run.ide13.runner",
        repositoryRef: "repository.ide13.runner",
        pinnedBaseRef: "commit.ide13.runner",
      },
      graph: {
        rootAgentRef: "agent.ide13.root",
        nodes: [
          {
            agentRef: "agent.ide13.root",
            threadRef: "thread.ide13.root",
            transcriptRef: "transcript.ide13.root",
            activityCursor: 1,
            lifecycle: "waiting",
            attachmentGeneration: 4,
          },
        ],
      },
      threadCursors: [
        {
          threadRef: "thread.ide13.root",
          transcriptRef: "transcript.ide13.root",
          activityCursor: 1,
          eventCursor: 1,
        },
      ],
    } satisfies PortableCheckpointBundle;
    const phaseSql = Object.assign(
      async (strings: TemplateStringsArray): Promise<ReadonlyArray<Record<string, unknown>>> => {
        if (strings.join("?").includes("kind = 'checkpoint-create'")) {
          return [
            {
              request_json: {
                schema: "openagents.portable_phase_operation.v1",
                operationRef: "operation.ide13.checkpoint",
                commandRef: claim.commandRef,
                commandExecutionClaimRef: claim.claimRef,
                ownerRef,
                sessionRef,
                attachmentRef: sourceAttachmentRef,
                attachmentGeneration: 4,
                targetRef: sourceTargetRef,
                pylonRef: "pylon.ide13.source",
                kind: "checkpoint-create",
                checkpointRef: command.checkpointRef,
                checkpointObjectRef: null,
                checkpointDigest: null,
                evidenceRefs: [],
                expiresAt: leaseExpiresAt,
              },
              request_fingerprint: `sha256:${"1".repeat(64)}`,
              state: "completed",
              claim_ref: "phase-claim.ide13.runner",
              claim_fingerprint: `sha256:${"2".repeat(64)}`,
              worker_instance_ref: "worker.ide13.phase",
              claim_generation: 1,
              lease_revision: 2,
              claimed_at: now,
              lease_expires_at: leaseExpiresAt,
              result_ref: "result.ide13.checkpoint",
              result_fingerprint: `sha256:${"3".repeat(64)}`,
              result_status: "completed",
              result_checkpoint_ref: command.checkpointRef,
              result_checkpoint_object_ref: "artifact.ide13.checkpoint",
              result_checkpoint_digest: bundle.checkpoint.digest,
              result_checkpoint_manifest_digest: `sha256:${"e".repeat(64)}`,
              result_destination_activation_receipt_json: null,
              result_evidence_refs_json: ["evidence.ide13.checkpoint"],
              error_ref: null,
              completed_at: now,
              updated_at: now,
            },
          ];
        }
        throw new Error("unexpected SQL after artifact mismatch");
      },
      { begin: neverSql.begin },
    ) as SyncSql;
    const resolverConfig = config();
    const resolver = new PostgresPortableSessionCommandResolver({
      ...resolverConfig,
      sql: phaseSql,
      checkpointArtifacts: {
        resolve: async () => ({
          ...bundle,
          checkpoint: {
            ...bundle.checkpoint,
            digest: `sha256:${"f".repeat(64)}`,
          },
        }),
      },
    });
    const resolved = await resolver.resolve(claim);

    await expect(
      resolved.move.destination.stageCheckpoint({
        operationRef: "operation.ide13.stage",
        bundle,
        destinationAttachmentRef: resolved.move.destinationAttachmentRef,
        destinationGeneration: 5,
        capabilityLeaseRefs: resolved.move.capabilityTransfers.map(
          (transfer) => transfer.destinationLeaseRef,
        ),
      }),
    ).rejects.toMatchObject({ code: "artifact_mismatch" });
  });
});

describe("portable session command production runner", () => {
  it("replays the canonical runtime after a lost terminal ACK", async () => {
    const input = await new PostgresPortableSessionCommandResolver(config()).resolve(claim);
    const move: PortableSessionMoveResult = {
      schema: "openagents.portable_session_move.v1",
      status: "completed",
      commandRef: claim.commandRef,
      sessionRef,
      runRef: "run.ide13.runner",
      repositoryRef: "repository.ide13.runner",
      pinnedBaseRef: "commit.ide13.runner",
      sourceAttachmentRef,
      sourceGeneration: 4,
      destinationAttachmentRef: input.move.destinationAttachmentRef,
      destinationGeneration: 5,
      checkpointRef: command.checkpointRef,
      capabilityLeaseRefs: input.move.capabilityTransfers.map(
        (transfer) => transfer.destinationLeaseRef,
      ),
      acceptedWorkRefs: [],
      evidenceRefs: ["evidence.ide13.completed"],
    };
    vi.spyOn(PostgresPortableSessionCommandQueue.prototype, "claim")
      .mockResolvedValueOnce({ status: "claimed", claim })
      .mockResolvedValueOnce({ status: "replayed", claim });
    vi.spyOn(PostgresPortableSessionCommandResolver.prototype, "resolve").mockResolvedValue(input);
    vi.spyOn(PostgresPortableSessionMoveRuntime.prototype, "move")
      .mockResolvedValueOnce(move)
      .mockResolvedValueOnce({ ...move, status: "replayed" });
    const terminal = vi
      .spyOn(PostgresPortableSessionCommandQueue.prototype, "terminal")
      .mockRejectedValueOnce(new Error("terminal ACK lost"))
      .mockResolvedValue({
        status: "terminal",
        claim: {
          ...claim,
          state: "terminal",
          terminalStatus: "completed",
          leaseRevision: 2,
          outcomeRef: "outcome.ide13.completed",
        },
      });
    const runner = new PostgresPortableSessionCommandRunner({
      sql: neverSql,
      transaction: async () => {
        throw new Error("transaction must not run through the mocked canonical runtime");
      },
      brokerFactory: { create: async () => broker() },
      pylonBindings: config().pylonBindings,
      capabilityGrantFacts: config().capabilityGrantFacts,
      checkpointArtifacts: config().checkpointArtifacts,
      now: () => now,
    });

    await expect(runner.execute(request)).rejects.toThrow("terminal ACK lost");
    await expect(runner.execute(request)).resolves.toMatchObject({ status: "completed" });
    expect(PostgresPortableSessionMoveRuntime.prototype.move).toHaveBeenCalledTimes(2);
    expect(terminal).toHaveBeenCalledTimes(2);
  });
});
