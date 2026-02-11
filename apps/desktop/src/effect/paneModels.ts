import type { DesktopRuntimeState, ExecutorTask } from "./model";

export type PaneLoadState = "loading" | "ready" | "empty" | "error";

export type FailureCategory = "policy" | "node" | "wallet" | "transport" | "endpoint" | "unknown";

export type NodeSyncStage = "offline" | "starting" | "syncing" | "ready" | "error";

export type NodePaneModel = Readonly<{
  readonly paneState: PaneLoadState;
  readonly syncStage: NodeSyncStage;
  readonly syncLabel: string;
  readonly diagnostic: string | null;
}>;

export type WalletBalanceModel = Readonly<{
  readonly paneState: PaneLoadState;
  readonly availability: "uninitialized" | "locked" | "degraded" | "ready";
  readonly estimatedSpendMsats: number;
  readonly settledCount: number;
  readonly failedCount: number;
  readonly blockedCount: number;
  readonly cacheHits: number;
}>;

export type PaymentHistoryEntry = Readonly<{
  readonly id: string;
  readonly method: string;
  readonly urlLabel: string;
  readonly status: ExecutorTask["status"];
  readonly updatedAtMs: number;
  readonly amountMsats: number | null;
  readonly category: FailureCategory | null;
  readonly reason: string | null;
}>;

export type InvoiceHistoryEntry = Readonly<{
  readonly id: string;
  readonly urlLabel: string;
  readonly state: "settled" | "failed" | "pending";
  readonly updatedAtMs: number;
  readonly reason: string | null;
}>;

export type TransactionHistoryModel = Readonly<{
  readonly paneState: PaneLoadState;
  readonly payments: ReadonlyArray<PaymentHistoryEntry>;
  readonly invoices: ReadonlyArray<InvoiceHistoryEntry>;
}>;

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;

const asNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

type FailureInput = Readonly<{
  readonly code?: string | undefined;
  readonly message?: string | undefined;
  readonly reason?: string | undefined;
}>;

const textParts = (input: FailureInput): string => [input.code ?? "", input.message ?? "", input.reason ?? ""].join(" ").toLowerCase();

const includesAny = (text: string, words: ReadonlyArray<string>): boolean =>
  words.some((word) => text.includes(word));

const amountFromTask = (task: ExecutorTask): number | null => {
  const metadata = asRecord(task.metadata);
  if (!metadata) return null;
  const amount = asNumber(metadata.amountMsats);
  if (amount === null) return null;
  return Math.max(0, Math.floor(amount));
};

export const toSafeUrlLabel = (url: string): string => {
  try {
    const parsed = new URL(url);
    const suffix = parsed.search ? "?..." : "";
    return `${parsed.host}${parsed.pathname}${suffix}`;
  } catch {
    return url.length > 96 ? `${url.slice(0, 96)}...` : url;
  }
};

export const classifyFailure = (input: FailureInput): FailureCategory => {
  const text = textParts(input);
  if (text.length === 0) return "unknown";

  if (
    includesAny(text, [
      "domainnotallowed",
      "host_blocked",
      "policy",
      "deny",
      "blocked_scope",
      "blocked",
      "budget_exceeded",
      "max_spend",
    ])
  ) {
    return "policy";
  }

  if (
    includesAny(text, [
      "wallet",
      "passphrase",
      "seed",
      "mnemonic",
      "wallet_not_initialized",
      "invalid_passphrase",
      "restore_not_prepared",
    ])
  ) {
    return "wallet";
  }

  if (
    includesAny(text, [
      "runtime_not_running",
      "lnd",
      "neutrino",
      "process_not_alive",
      "sync",
      "node",
      "grpc",
      "macaroon",
      "tls.cert",
    ])
  ) {
    return "node";
  }

  if (
    includesAny(text, [
      "timeout",
      "network",
      "dns",
      "econnrefused",
      "econnreset",
      "fetch",
      "transport",
      "socket",
      "connection",
      "tls handshake",
    ])
  ) {
    return "transport";
  }

  if (includesAny(text, ["http ", "http_", "status", "invoice_expired", "endpoint", "unauthorized"])) {
    return "endpoint";
  }

  return "unknown";
};

export const deriveNodePaneModel = (input: {
  readonly snapshot: DesktopRuntimeState;
  readonly loaded: boolean;
  readonly uiError: string | null;
}): NodePaneModel => {
  if (!input.loaded) {
    return {
      paneState: "loading",
      syncStage: "starting",
      syncLabel: "Bootstrapping local node state",
      diagnostic: null,
    };
  }

  if (input.uiError) {
    return {
      paneState: "error",
      syncStage: "error",
      syncLabel: "Desktop state refresh failed",
      diagnostic: input.uiError,
    };
  }

  const runtime = input.snapshot.lnd;

  if (runtime.lifecycle === "failed") {
    return {
      paneState: "error",
      syncStage: "error",
      syncLabel: "Node failed",
      diagnostic: runtime.lastError,
    };
  }

  if (runtime.lifecycle === "unavailable" || runtime.lifecycle === "stopped") {
    return {
      paneState: "empty",
      syncStage: "offline",
      syncLabel: "Node offline",
      diagnostic: runtime.lastError,
    };
  }

  if (runtime.lifecycle === "starting" || runtime.lifecycle === "backoff") {
    return {
      paneState: "ready",
      syncStage: "starting",
      syncLabel: "Node starting",
      diagnostic: runtime.lastError,
    };
  }

  if (runtime.health === "unhealthy") {
    return {
      paneState: "error",
      syncStage: "error",
      syncLabel: "Node unhealthy",
      diagnostic: runtime.lastError ?? "health probe failed",
    };
  }

  if (runtime.health === "healthy" && input.snapshot.wallet.walletState === "unlocked") {
    return {
      paneState: "ready",
      syncStage: "ready",
      syncLabel: "Ready for payment execution",
      diagnostic: null,
    };
  }

  return {
    paneState: "ready",
    syncStage: "syncing",
    syncLabel: "Running, waiting for wallet readiness",
    diagnostic: runtime.lastError,
  };
};

export const deriveWalletBalanceModel = (input: {
  readonly snapshot: DesktopRuntimeState;
  readonly tasks: ReadonlyArray<ExecutorTask>;
  readonly loaded: boolean;
  readonly uiError: string | null;
}): WalletBalanceModel => {
  if (!input.loaded) {
    return {
      paneState: "loading",
      availability: "uninitialized",
      estimatedSpendMsats: 0,
      settledCount: 0,
      failedCount: 0,
      blockedCount: 0,
      cacheHits: 0,
    };
  }

  if (input.uiError) {
    return {
      paneState: "error",
      availability: "degraded",
      estimatedSpendMsats: 0,
      settledCount: 0,
      failedCount: 0,
      blockedCount: 0,
      cacheHits: 0,
    };
  }

  const settledTasks = input.tasks.filter((task) => task.status === "paid" || task.status === "cached" || task.status === "completed");
  const failedCount = input.tasks.filter((task) => task.status === "failed").length;
  const blockedCount = input.tasks.filter((task) => task.status === "blocked").length;
  const cacheHits = input.tasks.filter((task) => task.status === "cached").length;
  const estimatedSpendMsats = settledTasks.reduce((sum, task) => sum + (amountFromTask(task) ?? 0), 0);

  const walletState = input.snapshot.wallet.walletState;
  if (walletState === "uninitialized") {
    return {
      paneState: "empty",
      availability: "uninitialized",
      estimatedSpendMsats,
      settledCount: settledTasks.length,
      failedCount,
      blockedCount,
      cacheHits,
    };
  }

  if (walletState !== "unlocked") {
    return {
      paneState: "ready",
      availability: "locked",
      estimatedSpendMsats,
      settledCount: settledTasks.length,
      failedCount,
      blockedCount,
      cacheHits,
    };
  }

  const nodeHealthy = input.snapshot.lnd.lifecycle === "running" && input.snapshot.lnd.health === "healthy";
  if (!nodeHealthy) {
    return {
      paneState: "ready",
      availability: "degraded",
      estimatedSpendMsats,
      settledCount: settledTasks.length,
      failedCount,
      blockedCount,
      cacheHits,
    };
  }

  return {
    paneState: "ready",
    availability: "ready",
    estimatedSpendMsats,
    settledCount: settledTasks.length,
    failedCount,
    blockedCount,
    cacheHits,
  };
};

const toPaymentEntry = (task: ExecutorTask): PaymentHistoryEntry => {
  const reason = task.failureReason ?? task.lastErrorMessage ?? null;
  const category = reason || task.lastErrorCode
    ? classifyFailure({
        code: task.lastErrorCode,
        message: task.lastErrorMessage,
        reason: task.failureReason,
      })
    : null;
  return {
    id: task.id,
    method: task.request.method ?? "GET",
    urlLabel: toSafeUrlLabel(task.request.url),
    status: task.status,
    updatedAtMs: task.updatedAtMs,
    amountMsats: amountFromTask(task),
    category,
    reason,
  };
};

const toInvoiceEntry = (task: ExecutorTask): InvoiceHistoryEntry => {
  const reason = task.failureReason ?? task.lastErrorMessage ?? null;
  const state =
    task.status === "paid" || task.status === "cached" || task.status === "completed"
      ? "settled"
      : task.status === "failed" || task.status === "blocked"
        ? "failed"
        : "pending";
  return {
    id: task.id,
    urlLabel: toSafeUrlLabel(task.request.url),
    state,
    updatedAtMs: task.updatedAtMs,
    reason,
  };
};

export const deriveTransactionHistoryModel = (input: {
  readonly tasks: ReadonlyArray<ExecutorTask>;
  readonly loaded: boolean;
  readonly uiError: string | null;
  readonly limit?: number;
}): TransactionHistoryModel => {
  if (!input.loaded) {
    return {
      paneState: "loading",
      payments: [],
      invoices: [],
    };
  }

  if (input.uiError) {
    return {
      paneState: "error",
      payments: [],
      invoices: [],
    };
  }

  const limit = Math.max(1, Math.min(200, input.limit ?? 30));
  const sorted = [...input.tasks].sort((a, b) => b.updatedAtMs - a.updatedAtMs).slice(0, limit);
  const historyRows = sorted.filter((task) => task.status !== "queued" && task.status !== "approved");

  if (historyRows.length === 0) {
    return {
      paneState: "empty",
      payments: [],
      invoices: [],
    };
  }

  return {
    paneState: "ready",
    payments: historyRows.map(toPaymentEntry),
    invoices: historyRows.map(toInvoiceEntry),
  };
};

export const latestExecutorFailure = (
  tasks: ReadonlyArray<ExecutorTask>,
): (PaymentHistoryEntry & { readonly category: FailureCategory }) | null => {
  const failed = [...tasks]
    .filter((task) => task.status === "failed" || task.status === "blocked")
    .sort((a, b) => b.updatedAtMs - a.updatedAtMs);
  const latest = failed[0];
  if (!latest) return null;
  const base = toPaymentEntry(latest);
  return {
    ...base,
    category:
      base.category ??
      classifyFailure({
        code: latest.lastErrorCode,
        message: latest.lastErrorMessage,
        reason: latest.failureReason,
      }),
  };
};
