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

export type DeleteInstanceResult = {
  deleted: boolean;
  stopped?: boolean;
};

export type RuntimeStatusData = {
  gateway: { status: 'running' | 'starting' | 'stopped' | 'error' };
  lastBackup: string | null;
  container: { instanceType: string };
  version: Record<string, string>;
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

export type PairingRequest = {
  id: string;
  code: string;
  createdAt: string;
  lastSeenAt: string;
  meta?: Record<string, string>;
};

export type PairingRequestsData = {
  channel: string;
  requests: Array<PairingRequest>;
};

export type BillingSummary = {
  user_id: string;
  balance_usd: number;
};

export type OpenclawApiConfig = {
  apiBase: string;
  internalKey: string;
  userId: string;
};

export const INTERNAL_KEY_HEADER = 'X-OA-Internal-Key';
export const USER_ID_HEADER = 'X-OA-User-Id';
export const SERVICE_TOKEN_HEADER = 'X-OpenAgents-Service-Token';

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

export const resolveInternalKey = (): string => {
  const key = process.env.OA_INTERNAL_KEY ?? process.env.OPENAGENTS_INTERNAL_KEY ?? '';
  if (!key.trim()) {
    throw new Error('OA_INTERNAL_KEY not configured');
  }
  return key.trim();
};

const resolveApiBaseEnv = (): string => {
  return (
    process.env.OPENCLAW_API_BASE ??
    process.env.OPENAGENTS_API_URL ??
    process.env.PUBLIC_API_URL ??
    ''
  );
};

const normalizeApiBase = (value: string): string => {
  const trimmed = value.trim().replace(/\/$/, '');
  if (!trimmed) return '';
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }
  if (/\/api(\/|$)/.test(trimmed)) {
    return `https://${trimmed}`;
  }
  return `https://${trimmed}/api`;
};

export const resolveApiBase = (): string => {
  const normalized = normalizeApiBase(resolveApiBaseEnv());
  if (!normalized) {
    throw new Error(
      'OpenClaw API base not configured. Set OPENCLAW_API_BASE, OPENAGENTS_API_URL, or PUBLIC_API_URL.',
    );
  }
  return normalized;
};

export async function openclawRequest<T>(
  config: OpenclawApiConfig,
  path: string,
  options: RequestInit = {},
): Promise<T | null> {
  const headers = new Headers(options.headers);
  headers.set('accept', 'application/json');
  const internalHeaders = buildInternalHeaders(config.internalKey, config.userId);
  Object.entries(internalHeaders).forEach(([key, value]) => headers.set(key, value));

  if (options.body && !headers.has('content-type')) {
    headers.set('content-type', 'application/json');
  }

  const response = await fetch(`${config.apiBase}${path}`, {
    ...options,
    headers,
  });

  const payload = (await response.json()) as ApiResponse<T>;
  if (!response.ok || !payload.ok) {
    throw new Error(payload.error ?? `Request failed (${response.status})`);
  }

  return payload.data ?? null;
}

export const getOpenclawInstance = (config: OpenclawApiConfig) =>
  openclawRequest<InstanceSummary | null>(config, '/openclaw/instance');

export const createOpenclawInstance = (config: OpenclawApiConfig) =>
  openclawRequest<InstanceSummary>(config, '/openclaw/instance', { method: 'POST' });

export const deleteOpenclawInstance = (config: OpenclawApiConfig) =>
  openclawRequest<DeleteInstanceResult>(config, '/openclaw/instance', { method: 'DELETE' });

export const getRuntimeStatus = (config: OpenclawApiConfig) =>
  openclawRequest<RuntimeStatusData>(config, '/openclaw/runtime/status');

export const getRuntimeDevices = (config: OpenclawApiConfig) =>
  openclawRequest<RuntimeDevicesData>(config, '/openclaw/runtime/devices');

export const approveRuntimeDevice = (config: OpenclawApiConfig, requestId: string) =>
  openclawRequest<{ approved: boolean; requestId: string }>(
    config,
    `/openclaw/runtime/devices/${encodeURIComponent(requestId)}/approve`,
    { method: 'POST' },
  );

export const listPairingRequests = (config: OpenclawApiConfig, channel: string) =>
  openclawRequest<PairingRequestsData>(
    config,
    `/openclaw/runtime/pairing/${encodeURIComponent(channel)}`,
  );

export const approvePairingRequest = (
  config: OpenclawApiConfig,
  args: { channel: string; code: string; notify?: boolean },
) =>
  openclawRequest<{ approved: boolean; channel: string; code: string }>(
    config,
    `/openclaw/runtime/pairing/${encodeURIComponent(args.channel)}/approve`,
    {
      method: 'POST',
      body: JSON.stringify({
        code: args.code,
        ...(typeof args.notify === 'boolean' ? { notify: args.notify } : {}),
      }),
    },
  );

export const backupRuntime = (config: OpenclawApiConfig) =>
  openclawRequest<{ lastBackup: string | null }>(config, '/openclaw/runtime/backup', {
    method: 'POST',
  });

export const restartRuntime = (config: OpenclawApiConfig) =>
  openclawRequest<{ message: string }>(config, '/openclaw/runtime/restart', { method: 'POST' });

export const getBillingSummary = (config: OpenclawApiConfig) =>
  openclawRequest<BillingSummary>(config, '/openclaw/billing/summary');
