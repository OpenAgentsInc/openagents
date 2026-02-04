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

export type SessionListParams = {
  limit?: number;
  activeMinutes?: number;
  messageLimit?: number;
  kinds?: Array<string>;
};

export type SessionHistoryParams = {
  sessionKey: string;
  limit?: number;
  includeTools?: boolean;
};

export type SessionSendParams = {
  sessionKey: string;
  message: string;
  timeoutSeconds?: number;
};

export type OpenclawInternalConfig = {
  apiBase: string;
  internalKey: string;
  userId: string;
  agentKey?: never;
};

export type OpenclawAgentConfig = {
  apiBase: string;
  agentKey: string;
  internalKey?: never;
  userId?: never;
};

export type OpenclawApiConfig = OpenclawInternalConfig | OpenclawAgentConfig;

export const INTERNAL_KEY_HEADER = 'X-OA-Internal-Key';
export const USER_ID_HEADER = 'X-OA-User-Id';
export const AGENT_KEY_HEADER = 'X-OA-Agent-Key';
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

const isAgentConfig = (config: OpenclawApiConfig): config is OpenclawAgentConfig =>
  'agentKey' in config && typeof config.agentKey === 'string';

const buildAuthHeaders = (config: OpenclawApiConfig): Record<string, string> => {
  if (isAgentConfig(config)) {
    const key = config.agentKey.trim();
    if (!key) {
      throw new Error('OpenClaw agent key not configured');
    }
    return { [AGENT_KEY_HEADER]: key };
  }
  const internalKey = config.internalKey.trim();
  const userId = config.userId.trim();
  if (!internalKey || !userId) {
    throw new Error('OpenClaw internal auth not configured');
  }
  return buildInternalHeaders(internalKey, userId);
};

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

const toQueryString = (params: URLSearchParams): string => {
  const query = params.toString();
  return query ? `?${query}` : '';
};

export async function openclawRequest<T>(
  config: OpenclawApiConfig,
  path: string,
  options: RequestInit = {},
): Promise<T | null> {
  const headers = new Headers(options.headers);
  headers.set('accept', 'application/json');
  const authHeaders = buildAuthHeaders(config);
  Object.entries(authHeaders).forEach(([key, value]) => headers.set(key, value));

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

export const listSessions = (config: OpenclawApiConfig, params?: SessionListParams) => {
  const search = new URLSearchParams();
  if (typeof params?.limit === 'number' && Number.isFinite(params.limit) && params.limit > 0) {
    search.set('limit', String(Math.floor(params.limit)));
  }
  if (
    typeof params?.activeMinutes === 'number' &&
    Number.isFinite(params.activeMinutes) &&
    params.activeMinutes > 0
  ) {
    search.set('activeMinutes', String(Math.floor(params.activeMinutes)));
  }
  if (
    typeof params?.messageLimit === 'number' &&
    Number.isFinite(params.messageLimit) &&
    params.messageLimit >= 0
  ) {
    search.set('messageLimit', String(Math.floor(params.messageLimit)));
  }
  if (Array.isArray(params?.kinds)) {
    params.kinds
      .map((kind) => (typeof kind === 'string' ? kind.trim() : ''))
      .filter(Boolean)
      .forEach((kind) => {
        search.append('kinds', kind);
      });
  }
  return openclawRequest<unknown>(config, `/openclaw/sessions${toQueryString(search)}`);
};

export const getSessionHistory = (config: OpenclawApiConfig, params: SessionHistoryParams) => {
  const search = new URLSearchParams();
  if (typeof params.limit === 'number' && Number.isFinite(params.limit) && params.limit > 0) {
    search.set('limit', String(Math.floor(params.limit)));
  }
  if (typeof params.includeTools === 'boolean') {
    search.set('includeTools', params.includeTools ? 'true' : 'false');
  }
  return openclawRequest<unknown>(
    config,
    `/openclaw/sessions/${encodeURIComponent(params.sessionKey)}/history${toQueryString(search)}`,
  );
};

export const sendSessionMessage = (config: OpenclawApiConfig, params: SessionSendParams) => {
  const payload: Record<string, unknown> = { message: params.message };
  if (
    typeof params.timeoutSeconds === 'number' &&
    Number.isFinite(params.timeoutSeconds) &&
    params.timeoutSeconds >= 0
  ) {
    payload.timeoutSeconds = Math.floor(params.timeoutSeconds);
  }
  return openclawRequest<unknown>(
    config,
    `/openclaw/sessions/${encodeURIComponent(params.sessionKey)}/send`,
    {
      method: 'POST',
      body: JSON.stringify(payload),
    },
  );
};
