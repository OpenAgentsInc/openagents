import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCommandExecutionClaim,
  type PortableCommandExecutionClaimRequest,
} from "@openagentsinc/portable-session-contract";
import { Effect } from "effect";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import {
  PostgresPortableSessionCommandDispatch,
  type PortableSessionCommandDispatchConfig,
} from "./portable-session-command-dispatch.js";
import { PostgresPortableSessionCommandQueue } from "./portable-session-command-queue.js";
import { PostgresPortableSessionCommandRunner } from "./portable-session-command-runner.js";
import type { SyncTransactionWriter } from "./outbox-writer.js";
import type { SyncSql } from "./sql.js";

const now = "2026-07-20T12:00:00.000Z";
const leaseExpiresAt = "2026-07-20T12:05:00.000Z";

const neverSql = Object.assign(
  async (_strings: TemplateStringsArray, ..._values: ReadonlyArray<unknown>) => {
    throw new Error("SQL must not run through the mocked dispatch test");
  },
  {
    begin: async () => {
      throw new Error("transaction must not run through the mocked dispatch test");
    },
  },
) as SyncSql;

const request = (suffix: string): PortableCommandExecutionClaimRequest => ({
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  commandRef: `command.ide13.dispatch.${suffix}`,
  claimRef: `claim.ide13.dispatch.${suffix}`,
  executorEnvironmentRef: "target.ide13.dispatch.source",
  workerInstanceRef: "worker.ide13.dispatch.fixture",
  leaseExpiresAt,
});

const claim = (
  item: PortableCommandExecutionClaimRequest,
  state: PortableCommandExecutionClaim["state"] = "claimed",
): PortableCommandExecutionClaim => ({
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  claimRef: item.claimRef,
  commandRef: item.commandRef,
  ownerRef: "owner.ide13.dispatch",
  sessionRef: `session.${item.commandRef}`,
  commandKind: "move",
  commandFingerprint: `sha256:${"1".repeat(64)}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef: `attachment.${item.commandRef}`,
  sourceGeneration: 1,
  destinationTargetRef: "target.ide13.dispatch.destination",
  executorEnvironmentRef: item.executorEnvironmentRef,
  workerInstanceRef: item.workerInstanceRef,
  claimGeneration: 1,
  leaseRevision: state === "pending_reconcile" ? 2 : 1,
  state,
  claimedAt: now,
  leaseExpiresAt: item.leaseExpiresAt,
  updatedAt: now,
  terminalStatus: null,
  pendingReconcileRef: state === "pending_reconcile" ? `reconcile.${item.commandRef}` : null,
  outcomeRef: null,
  evidenceRefs: [],
});

const config = (): PortableSessionCommandDispatchConfig => ({
  sql: neverSql,
  transaction: async <A>(_run: (writer: SyncTransactionWriter) => Promise<A>) => {
    throw new Error("move transaction must not run through the mocked dispatch test");
  },
  brokerFactory: { create: async () => ({
    vault: {
      withSourceGrantMaterial: async () => {
        throw new Error("vault facts are intentionally absent from this unit test");
      },
      revokeSourceGrant: async () => undefined,
    },
    targets: [],
    adapters: [],
  }) },
  pylonBindings: {
    resolve: async () => {
      throw new Error("Pylon facts are intentionally absent from this unit test");
    },
  },
  capabilityGrantFacts: {
    resolve: async () => {
      throw new Error("capability facts are intentionally absent from this unit test");
    },
  },
  checkpointArtifacts: {
    resolve: async () => {
      throw new Error("artifact facts are intentionally absent from this unit test");
    },
  },
  dispatcherRef: "dispatcher.ide13.production-a",
  batchSize: 3,
  concurrency: 2,
  leaseDurationMs: 300_000,
  now: () => now,
});

afterEach(() => vi.restoreAllMocks());

describe("portable session accepted-command dispatch", () => {
  it("isolates one item failure and preserves pending reconciliation", async () => {
    const completed = request("completed");
    const failed = request("failed");
    const pending = request("pending");
    vi.spyOn(PostgresPortableSessionCommandQueue.prototype, "claimAcceptedBatch").mockResolvedValue(
      {
        claims: [completed, failed, pending].map((claimRequest) => ({
          commandRef: claimRequest.commandRef,
          claimRequest,
        })),
        skippedCommandRefs: ["command.ide13.dispatch.stale"],
      },
    );
    const execute = vi
      .spyOn(PostgresPortableSessionCommandRunner.prototype, "execute")
      .mockImplementation(async (item) => {
        if (item.commandRef === failed.commandRef) throw new Error("isolated failure");
        if (item.commandRef === pending.commandRef) {
          return { status: "pending_reconcile", claim: claim(item, "pending_reconcile") };
        }
        return { status: "completed", claim: claim(item) };
      });

    // This package's Vite Plus TestAPI does not expose an Effect test extension.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    const report = await Effect.runPromise(
      new PostgresPortableSessionCommandDispatch(config()).runTick(),
    );

    expect(report.discovered).toBe(3);
    expect(report.skippedCommandRefs).toEqual(["command.ide13.dispatch.stale"]);
    expect(report.items).toEqual([
      expect.objectContaining({ commandRef: completed.commandRef, status: "completed" }),
      expect.objectContaining({
        commandRef: failed.commandRef,
        status: "dispatch_failed",
        failureRef: expect.stringMatching(/^failure\.portable-command-dispatch\.[a-f0-9]{64}$/u),
      }),
      expect.objectContaining({ commandRef: pending.commandRef, status: "pending_reconcile" }),
    ]);
    expect(execute).toHaveBeenCalledTimes(3);
  });

  it("derives one stable worker ref from the exact dispatcher ref", async () => {
    const discovery = vi
      .spyOn(PostgresPortableSessionCommandQueue.prototype, "claimAcceptedBatch")
      .mockResolvedValue({ claims: [], skippedCommandRefs: [] });

    // This package's Vite Plus TestAPI does not expose an Effect test extension.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    const first = await Effect.runPromise(
      new PostgresPortableSessionCommandDispatch(config()).runTick(),
    );
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    const second = await Effect.runPromise(
      new PostgresPortableSessionCommandDispatch(config()).runTick(),
    );

    expect(first.workerInstanceRef).toMatch(/^worker\.portable-command\.[a-f0-9]{64}$/u);
    expect(second.workerInstanceRef).toBe(first.workerInstanceRef);
    expect(discovery).toHaveBeenNthCalledWith(1, {
      workerInstanceRef: first.workerInstanceRef,
      limit: 3,
      leaseDurationMs: 300_000,
    });
  });
});
