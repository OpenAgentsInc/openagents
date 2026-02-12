import { describe, expect, it } from "@effect/vitest";

import {
  classifyFailure,
  deriveNodePaneModel,
  deriveTransactionHistoryModel,
  deriveWalletBalanceModel,
  latestExecutorFailure,
} from "../src/effect/paneModels";
import { initialDesktopRuntimeState, type ExecutorTask } from "../src/effect/model";

const now = 1_701_000_000_000;

const mkTask = (input: Partial<ExecutorTask> & Pick<ExecutorTask, "id" | "status">): ExecutorTask => ({
  id: input.id,
  ownerId: input.ownerId ?? "user_test",
  status: input.status,
  request: input.request ?? {
    url: "https://api.example.com/premium",
    method: "GET",
    maxSpendMsats: 2_500,
  },
  attemptCount: input.attemptCount ?? 0,
  createdAtMs: input.createdAtMs ?? now,
  updatedAtMs: input.updatedAtMs ?? now,
  ...(input.metadata !== undefined ? { metadata: input.metadata } : {}),
  ...(input.lastErrorCode ? { lastErrorCode: input.lastErrorCode } : {}),
  ...(input.lastErrorMessage ? { lastErrorMessage: input.lastErrorMessage } : {}),
  ...(input.failureReason ? { failureReason: input.failureReason } : {}),
});

describe("l402 pane models", () => {
  it("maps failure categories for policy/node/wallet/transport/endpoint", () => {
    expect(classifyFailure({ code: "DomainNotAllowedError" })).toBe("policy");
    expect(classifyFailure({ reason: "runtime_not_running" })).toBe("node");
    expect(classifyFailure({ message: "invalid_passphrase" })).toBe("wallet");
    expect(classifyFailure({ message: "fetch failed: ECONNREFUSED" })).toBe("transport");
    expect(classifyFailure({ reason: "invoice_expired" })).toBe("endpoint");
  });

  it("derives node sync transitions from runtime + wallet state", () => {
    const loading = deriveNodePaneModel({
      snapshot: initialDesktopRuntimeState(),
      loaded: false,
      uiError: null,
    });
    expect(loading.paneState).toBe("loading");
    expect(loading.syncStage).toBe("starting");

    const readySnapshot = {
      ...initialDesktopRuntimeState(),
      lnd: {
        ...initialDesktopRuntimeState().lnd,
        lifecycle: "running" as const,
        health: "healthy" as const,
        sync: {
          ...initialDesktopRuntimeState().lnd.sync,
          syncedToChain: true,
          walletSynced: true,
          syncedToGraph: true,
        },
      },
      wallet: {
        ...initialDesktopRuntimeState().wallet,
        walletState: "unlocked" as const,
      },
    };
    const ready = deriveNodePaneModel({
      snapshot: readySnapshot,
      loaded: true,
      uiError: null,
    });
    expect(ready.paneState).toBe("ready");
    expect(ready.syncStage).toBe("ready");

    const syncingSnapshot = {
      ...readySnapshot,
      lnd: {
        ...readySnapshot.lnd,
        sync: {
          ...readySnapshot.lnd.sync,
          syncedToChain: false as const,
          walletSynced: false as const,
        },
      },
    };
    const syncing = deriveNodePaneModel({
      snapshot: syncingSnapshot,
      loaded: true,
      uiError: null,
    });
    expect(syncing.syncStage).toBe("syncing");
    expect(syncing.syncLabel).toBe("Syncing chain data");
  });

  it("derives wallet balance summary from executed tasks", () => {
    const snapshot = {
      ...initialDesktopRuntimeState(),
      lnd: {
        ...initialDesktopRuntimeState().lnd,
        lifecycle: "running" as const,
        health: "healthy" as const,
        sync: {
          ...initialDesktopRuntimeState().lnd.sync,
          syncedToChain: true,
          walletSynced: true,
          syncedToGraph: true,
        },
      },
      wallet: {
        ...initialDesktopRuntimeState().wallet,
        walletState: "unlocked" as const,
      },
    };
    const tasks: ReadonlyArray<ExecutorTask> = [
      mkTask({
        id: "task_paid",
        status: "completed",
        metadata: {
          amountMsats: 1_250,
        },
      }),
      mkTask({
        id: "task_failed",
        status: "failed",
        failureReason: "invoice_expired",
      }),
      mkTask({
        id: "task_blocked",
        status: "blocked",
        failureReason: "host_blocked",
      }),
    ];

    const model = deriveWalletBalanceModel({
      snapshot,
      tasks,
      loaded: true,
      uiError: null,
    });
    expect(model.availability).toBe("ready");
    expect(model.estimatedSpendMsats).toBe(1_250);
    expect(model.settledCount).toBe(1);
    expect(model.failedCount).toBe(1);
    expect(model.blockedCount).toBe(1);
  });

  it("builds payment and invoice history models with empty/loading branches", () => {
    const loading = deriveTransactionHistoryModel({
      tasks: [],
      loaded: false,
      uiError: null,
    });
    expect(loading.paneState).toBe("loading");

    const tasks: ReadonlyArray<ExecutorTask> = [
      mkTask({
        id: "task_queued",
        status: "queued",
      }),
      mkTask({
        id: "task_completed",
        status: "completed",
        updatedAtMs: now + 2,
        metadata: { amountMsats: 2_000 },
      }),
      mkTask({
        id: "task_failed",
        status: "failed",
        updatedAtMs: now + 3,
        lastErrorCode: "PaymentFailedError",
        lastErrorMessage: "invoice_expired",
      }),
    ];
    const ready = deriveTransactionHistoryModel({
      tasks,
      loaded: true,
      uiError: null,
    });
    expect(ready.paneState).toBe("ready");
    expect(ready.payments.length).toBe(2);
    expect(ready.invoices.length).toBe(2);
    expect(ready.payments[0]?.id).toBe("task_failed");
    expect(ready.payments[0]?.category).toBe("endpoint");
  });

  it("returns latest executor failure with classification", () => {
    const latest = latestExecutorFailure([
      mkTask({
        id: "older",
        status: "failed",
        updatedAtMs: now + 10,
        lastErrorCode: "TransportError",
        lastErrorMessage: "connection reset",
      }),
      mkTask({
        id: "newer",
        status: "blocked",
        updatedAtMs: now + 20,
        lastErrorCode: "DomainNotAllowedError",
        failureReason: "host_blocked",
      }),
    ]);

    expect(latest?.id).toBe("newer");
    expect(latest?.category).toBe("policy");
    expect(latest?.reason).toBe("host_blocked");
  });
});
