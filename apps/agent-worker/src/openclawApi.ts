import type { AgentWorkerEnv } from './types';

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

export type BillingSummary = {
  user_id: string;
  balance_usd: number;
};

export type OpenclawApiConfig = {
  apiBase: string;
  internalKey: string;
  userId: string;
};

export type SessionListArgs = {
  kinds?: string[];
  limit?: number;
  activeMinutes?: number;
  messageLimit?: number;
};

export type SessionHistoryArgs = {
  sessionKey: string;
  limit?: number;
  includeTools?: boolean;
};

export type SessionSendArgs = {
  sessionKey: string;
  message: string;
  timeoutSeconds?: number;
};

export const INTERNAL_KEY_HEADER = 'X-OA-Internal-Key';
export const USER_ID_HEADER = 'X-OA-User-Id';

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

export const resolveApiBase = (env: AgentWorkerEnv): string => {
  const raw =
    env.OPENCLAW_API_BASE ??
    env.OPENAGENTS_API_URL ??
    env.PUBLIC_API_URL ??
    '';
  const normalized = normalizeApiBase(raw);
  if (!normalized) {
    throw new Error(
      'OpenClaw API base not configured. Set OPENCLAW_API_BASE, OPENAGENTS_API_URL, or PUBLIC_API_URL.',
    );
  }
  return normalized;
};

export const buildApiConfig = (env: AgentWorkerEnv, userId: string): OpenclawApiConfig => {
  const internalKey = env.OA_INTERNAL_KEY?.trim();
  if (!internalKey) {
    throw new Error('OA_INTERNAL_KEY not configured');
  }
  return {
    apiBase: resolveApiBase(env),
    internalKey,
    userId,
  };
};

export async function openclawRequest<T>(
  config: OpenclawApiConfig,
  path: string,
  options: RequestInit = {},
): Promise<T | null> {
  const headers = new Headers(options.headers);
  headers.set('accept', 'application/json');
  headers.set(INTERNAL_KEY_HEADER, config.internalKey);
  headers.set(USER_ID_HEADER, config.userId);

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

export const backupRuntime = (config: OpenclawApiConfig) =>
  openclawRequest<{ lastBackup: string | null }>(config, '/openclaw/runtime/backup', {
    method: 'POST',
  });

export const restartRuntime = (config: OpenclawApiConfig) =>
  openclawRequest<{ message: string }>(config, '/openclaw/runtime/restart', { method: 'POST' });

export const getBillingSummary = (config: OpenclawApiConfig) =>
  openclawRequest<BillingSummary>(config, '/openclaw/billing/summary');

export const listSessions = (config: OpenclawApiConfig, args: SessionListArgs = {}) => {
  const params = new URLSearchParams();
  if (args.limit && Number.isFinite(args.limit)) params.set('limit', `${args.limit}`);
  if (args.activeMinutes && Number.isFinite(args.activeMinutes)) {
    params.set('activeMinutes', `${args.activeMinutes}`);
  }
  if (typeof args.messageLimit === 'number' && Number.isFinite(args.messageLimit)) {
    params.set('messageLimit', `${args.messageLimit}`);
  }
  if (Array.isArray(args.kinds)) {
    for (const kind of args.kinds) {
      if (kind) params.append('kinds', kind);
    }
  }
  const query = params.toString();
  const path = query ? `/openclaw/sessions?${query}` : '/openclaw/sessions';
  return openclawRequest<unknown>(config, path);
};

export const getSessionHistory = (config: OpenclawApiConfig, args: SessionHistoryArgs) => {
  const params = new URLSearchParams();
  if (args.limit && Number.isFinite(args.limit)) params.set('limit', `${args.limit}`);
  if (typeof args.includeTools === 'boolean') {
    params.set('includeTools', args.includeTools ? 'true' : 'false');
  }
  const query = params.toString();
  const path = query
    ? `/openclaw/sessions/${encodeURIComponent(args.sessionKey)}/history?${query}`
    : `/openclaw/sessions/${encodeURIComponent(args.sessionKey)}/history`;
  return openclawRequest<unknown>(config, path);
};

export const sendSessionMessage = (config: OpenclawApiConfig, args: SessionSendArgs) =>
  openclawRequest<unknown>(
    config,
    `/openclaw/sessions/${encodeURIComponent(args.sessionKey)}/send`,
    {
      method: 'POST',
      body: JSON.stringify({
        message: args.message,
        ...(typeof args.timeoutSeconds === 'number'
          ? { timeoutSeconds: args.timeoutSeconds }
          : {}),
      }),
    },
  );
