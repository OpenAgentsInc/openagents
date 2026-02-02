import { createServerFn } from "@tanstack/react-start";
import { z } from "zod";
import { getApiBase } from "@/lib/api";

export type ApiResponse<T> = {
  ok: boolean;
  data?: T | null;
  error?: string | null;
};

export type InstanceSummary = {
  status: string;
  runtime_name?: string | null;
  created_at: number;
  updated_at: number;
  last_ready_at?: number | null;
};

export type RuntimeStatusData = {
  gateway: { status: "running" | "starting" | "stopped" | "error" };
  lastBackup: string | null;
  container: { instanceType: string };
  version: { clawdbot: string };
};

export type RuntimeDevicesData = {
  pending: Array<{
    requestId: string;
    client?: { platform?: string; mode?: string };
    requestedAt?: string;
  }>;
  paired: Array<{
    deviceId: string;
    client?: { platform?: string; mode?: string };
    pairedAt?: string;
  }>;
};

export type BillingSummary = {
  user_id: string;
  balance_usd: number;
};

export const INTERNAL_KEY_HEADER = "X-OA-Internal-Key";
export const USER_ID_HEADER = "X-OA-User-Id";
export const SERVICE_TOKEN_HEADER = "X-OpenAgents-Service-Token";

export const roundUsd = (value: number): number => {
  return Math.round((value + Number.EPSILON) * 100) / 100;
};

export const buildInternalHeaders = (internalKey: string, userId: string) => ({
  [INTERNAL_KEY_HEADER]: internalKey,
  [USER_ID_HEADER]: userId,
});

export const buildServiceTokenHeader = (token: string) => ({
  [SERVICE_TOKEN_HEADER]: token,
});

const resolveInternalKey = (): string => {
  const key =
    process.env.OA_INTERNAL_KEY ??
    process.env.OPENAGENTS_INTERNAL_KEY ??
    "";
  if (!key.trim()) {
    throw new Error("OA_INTERNAL_KEY not configured");
  }
  return key.trim();
};

const resolveUserId = (override?: string): string => {
  if (override && override.trim()) {
    return override.trim();
  }
  const userId =
    process.env.OA_INTERNAL_USER_ID ??
    process.env.OPENAGENTS_USER_ID ??
    "";
  if (!userId.trim()) {
    throw new Error("OA_INTERNAL_USER_ID not configured");
  }
  return userId.trim();
};

const resolveApiBase = (): string => {
  if (process.env.PUBLIC_API_URL) {
    return process.env.PUBLIC_API_URL.replace(/\/$/, "");
  }
  return getApiBase();
};

async function openclawRequest<T>(
  path: string,
  options: RequestInit = {},
  userId?: string,
): Promise<T | null> {
  const base = resolveApiBase();
  const internalKey = resolveInternalKey();
  const resolvedUserId = resolveUserId(userId);

  const headers = new Headers(options.headers);
  headers.set("accept", "application/json");
  const internalHeaders = buildInternalHeaders(internalKey, resolvedUserId);
  Object.entries(internalHeaders).forEach(([key, value]) => headers.set(key, value));

  if (options.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }

  const response = await fetch(`${base}${path}`, {
    ...options,
    headers,
  });

  const payload = (await response.json()) as ApiResponse<T>;
  if (!response.ok || !payload.ok) {
    throw new Error(payload.error ?? `Request failed (${response.status})`);
  }

  return payload.data ?? null;
}

export const getOpenclawInstance = createServerFn({ method: "GET" }).handler(async () => {
  return openclawRequest<InstanceSummary | null>("/openclaw/instance");
});

export const createOpenclawInstance = createServerFn({ method: "POST" }).handler(
  async () => {
    return openclawRequest<InstanceSummary>("/openclaw/instance", { method: "POST" });
  },
);

export const getRuntimeStatus = createServerFn({ method: "GET" }).handler(async () => {
  return openclawRequest<RuntimeStatusData>("/openclaw/runtime/status");
});

export const getRuntimeDevices = createServerFn({ method: "GET" }).handler(async () => {
  return openclawRequest<RuntimeDevicesData>("/openclaw/runtime/devices");
});

export const approveRuntimeDevice = createServerFn({ method: "POST" })
  .inputValidator(z.object({ requestId: z.string().min(1) }))
  .handler(async ({ data }) => {
    return openclawRequest<{ approved: boolean; requestId: string }>(
      `/openclaw/runtime/devices/${data.requestId}/approve`,
      { method: "POST" },
    );
  });

export const backupRuntime = createServerFn({ method: "POST" }).handler(async () => {
  return openclawRequest<{ lastBackup: string | null }>(
    "/openclaw/runtime/backup",
    { method: "POST" },
  );
});

export const restartRuntime = createServerFn({ method: "POST" }).handler(async () => {
  return openclawRequest<{ message: string }>(
    "/openclaw/runtime/restart",
    { method: "POST" },
  );
});

export const getBillingSummary = createServerFn({ method: "GET" }).handler(async () => {
  return openclawRequest<BillingSummary>("/openclaw/billing/summary");
});
