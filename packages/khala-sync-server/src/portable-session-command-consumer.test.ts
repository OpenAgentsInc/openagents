import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCommandExecutionClaim,
  type PortableCommandExecutionClaimRequest,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, it, vi } from "vite-plus/test";

import {
  PortableSessionCommandConsumer,
  type PortableSessionCommandConsumerConfig,
} from "./portable-session-command-consumer.js";
import type { PortableSessionMoveRuntimeInput } from "./portable-session-move-runtime.js";
import type {
  PortableSessionExecutionTarget,
  PortableSessionMoveResult,
} from "./portable-session-move.js";

const now = "2026-07-20T12:00:00.000Z";

const request: PortableCommandExecutionClaimRequest = {
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  commandRef: "command.ide13.consumer",
  claimRef: "claim.ide13.consumer",
  executorEnvironmentRef: "target.ide13.source",
  workerInstanceRef: "worker.ide13.consumer",
  leaseExpiresAt: "2026-07-20T12:10:00.000Z",
};

const claim: PortableCommandExecutionClaim = {
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  claimRef: request.claimRef,
  commandRef: request.commandRef,
  ownerRef: "owner.ide13.consumer",
  sessionRef: "session.ide13.consumer",
  commandKind: "move",
  commandFingerprint: `sha256:${"1".repeat(64)}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef: "attachment.ide13.source",
  sourceGeneration: 4,
  destinationTargetRef: "target.ide13.destination",
  executorEnvironmentRef: request.executorEnvironmentRef,
  workerInstanceRef: request.workerInstanceRef,
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: now,
  leaseExpiresAt: request.leaseExpiresAt,
  updatedAt: now,
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
};

const unreachable = async (): Promise<never> => {
  throw new Error("target operation must not run in this consumer test");
};

const target = (targetRef: string): PortableSessionExecutionTarget => ({
  targetRef,
  targetClass: "owner_local",
  quiesceGraph: unreachable,
  createCheckpoint: unreachable,
  cleanupSource: unreachable,
  stageCheckpoint: unreachable,
  activate: unreachable,
  abortStaged: unreachable,
});

const runtimeInput = (): PortableSessionMoveRuntimeInput => ({
  moveRef: claim.claimRef,
  move: {
    command: {
      schema: "openagents.portable_session_command.v1",
      commandRef: claim.commandRef,
      idempotencyKey: "idempotency.ide13.consumer",
      ownerRef: claim.ownerRef,
      sessionRef: claim.sessionRef,
      kind: "move",
      expectedAttachmentRef: claim.sourceAttachmentRef,
      expectedGeneration: claim.sourceGeneration,
      destinationTargetRef: claim.destinationTargetRef,
      checkpointRef: "checkpoint.ide13.consumer",
      expiresAt: request.leaseExpiresAt,
    },
    destinationAttachmentRef: "attachment.ide13.destination",
    capabilityTransfers: [],
    source: target(claim.executorEnvironmentRef),
    destination: target(claim.destinationTargetRef),
  },
  broker: {} as PortableSessionMoveRuntimeInput["broker"],
});

const moveResult = (status: PortableSessionMoveResult["status"]): PortableSessionMoveResult => ({
  schema: "openagents.portable_session_move.v1",
  status,
  commandRef: claim.commandRef,
  sessionRef: claim.sessionRef,
  runRef: "run.ide13.consumer",
  repositoryRef: "repository.ide13.consumer",
  pinnedBaseRef: "commit.ide13.consumer",
  sourceAttachmentRef: claim.sourceAttachmentRef,
  sourceGeneration: claim.sourceGeneration,
  destinationAttachmentRef: "attachment.ide13.destination",
  destinationGeneration: 5,
  checkpointRef: "checkpoint.ide13.consumer",
  capabilityLeaseRefs: [],
  acceptedWorkRefs: [],
  evidenceRefs: [`evidence.ide13.${status}`],
});

const advancedClaim = (
  state: PortableCommandExecutionClaim["state"],
  terminalStatus: PortableCommandExecutionClaim["terminalStatus"] = null,
): PortableCommandExecutionClaim => ({
  ...claim,
  leaseRevision: 2,
  state,
  terminalStatus,
  pendingReconcileRef: state === "pending_reconcile" ? "reconcile.ide13.consumer" : null,
  outcomeRef: state === "terminal" ? "outcome.ide13.consumer" : null,
});

const queue = () => {
  const terminal = vi.fn(async () => ({
    status: "terminal" as const,
    claim: advancedClaim("terminal", "completed"),
  }));
  const markPendingReconcile = vi.fn(async () => ({
    status: "pending_reconcile" as const,
    claim: advancedClaim("pending_reconcile"),
  }));
  const value = {
    claim: vi.fn(async () => ({ status: "claimed" as const, claim })),
    terminal,
    markPendingReconcile,
  } satisfies PortableSessionCommandConsumerConfig["queue"];
  return value;
};

describe("portable session command consumer", () => {
  it("runs a matching claim through the canonical runtime and closes it", async () => {
    const durableQueue = queue();
    const move = vi.fn(async () => moveResult("completed"));
    const consumer = new PortableSessionCommandConsumer({
      queue: durableQueue,
      resolver: { resolve: async () => runtimeInput() },
      runtime: { move },
      now: () => now,
    });

    const result = await consumer.execute(request);

    expect(result.status).toBe("completed");
    expect(move).toHaveBeenCalledTimes(1);
    expect(durableQueue.terminal).toHaveBeenCalledWith(
      expect.objectContaining({
        terminalStatus: "completed",
        expectedLeaseRevision: 1,
      }),
    );
    expect(durableQueue.markPendingReconcile).not.toHaveBeenCalled();
  });

  it("rejects a resolver that changes the claimed destination before runtime entry", async () => {
    const durableQueue = queue();
    const move = vi.fn(async () => moveResult("completed"));
    const base = runtimeInput();
    const mismatched: PortableSessionMoveRuntimeInput = {
      ...base,
      move: {
        ...base.move,
        destination: target("target.ide13.foreign"),
      },
    };
    const consumer = new PortableSessionCommandConsumer({
      queue: durableQueue,
      resolver: { resolve: async () => mismatched },
      runtime: { move },
      now: () => now,
    });

    const result = await consumer.execute(request);

    expect(result.status).toBe("rejected");
    expect(move).not.toHaveBeenCalled();
    expect(durableQueue.terminal).toHaveBeenCalledWith(
      expect.objectContaining({
        terminalStatus: "rejected",
      }),
    );
  });

  it("retains an uncertain runtime exception for explicit reconciliation", async () => {
    const durableQueue = queue();
    const consumer = new PortableSessionCommandConsumer({
      queue: durableQueue,
      resolver: { resolve: async () => runtimeInput() },
      runtime: {
        move: async () => {
          throw new Error("ack lost");
        },
      },
      now: () => now,
    });

    const result = await consumer.execute(request);

    expect(result.status).toBe("pending_reconcile");
    expect(durableQueue.markPendingReconcile).toHaveBeenCalledTimes(1);
    expect(durableQueue.terminal).not.toHaveBeenCalled();
  });

  it("preserves the runtime pending state instead of reporting completion", async () => {
    const durableQueue = queue();
    const consumer = new PortableSessionCommandConsumer({
      queue: durableQueue,
      resolver: { resolve: async () => runtimeInput() },
      runtime: { move: async () => moveResult("activation_pending_reconcile") },
      now: () => now,
    });

    const result = await consumer.execute(request);

    expect(result.status).toBe("pending_reconcile");
    expect(result.move?.status).toBe("activation_pending_reconcile");
    expect(durableQueue.terminal).not.toHaveBeenCalled();
  });

  it("replays after a lost terminal acknowledgement without a second target effect", async () => {
    const durableQueue = queue();
    durableQueue.terminal
      .mockRejectedValueOnce(new Error("database acknowledgement lost"))
      .mockResolvedValueOnce({
        status: "terminal",
        claim: advancedClaim("terminal", "completed"),
      });
    let targetEffects = 0;
    let completed: PortableSessionMoveResult | undefined;
    const runtime = {
      move: vi.fn(async () => {
        if (completed === undefined) {
          targetEffects += 1;
          completed = moveResult("completed");
          return completed;
        }
        return { ...completed, status: "replayed" as const };
      }),
    };
    const consumer = new PortableSessionCommandConsumer({
      queue: durableQueue,
      resolver: { resolve: async () => runtimeInput() },
      runtime,
      now: () => now,
    });

    await expect(consumer.execute(request)).rejects.toThrow("database acknowledgement lost");
    const replay = await consumer.execute(request);

    expect(replay.status).toBe("completed");
    expect(runtime.move).toHaveBeenCalledTimes(2);
    expect(targetEffects).toBe(1);
  });
});
