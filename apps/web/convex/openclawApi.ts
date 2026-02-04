/**
 * Convex actions that proxy OpenClaw instance get/create to the Rust API.
 * Used by Hatchery to avoid TanStack server-function path that can hit "Only HTML requests" error.
 */
import { v } from 'convex/values';
import { api } from './_generated/api';
import { action } from './_generated/server';

const DEFAULT_API_BASE = 'https://openagents.com/api';

type InstanceResult = {
  status: string;
  runtime_name?: string | null;
  created_at: number;
  updated_at: number;
  last_ready_at?: number | null;
};

type RuntimeStatusResult = {
  gateway: { status: 'running' | 'starting' | 'stopped' | 'error' };
  lastBackup: string | null;
  container: { instanceType: string };
  version: Record<string, string>;
};

type RuntimeDevicesResult = {
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

type PairingRequestResult = {
  id: string;
  code: string;
  createdAt: string;
  lastSeenAt: string;
  meta?: Record<string, string>;
};

type PairingRequestsResult = {
  channel: string;
  requests: Array<PairingRequestResult>;
};

type ApproveDeviceResult = {
  approved: boolean;
  requestId: string;
};

type ApprovePairingResult = {
  approved: boolean;
  channel: string;
  code: string;
};

type BackupResult = {
  lastBackup: string | null;
};

type RestartResult = {
  message: string;
};

type DeleteInstanceResult = {
  deleted: boolean;
};

function getApiBase(): string {
  const base =
    process.env.OPENCLAW_API_BASE ??
    process.env.OPENCLAW_API_URL ??
    process.env.OPENAGENTS_API_URL ??
    process.env.PUBLIC_API_URL ??
    DEFAULT_API_BASE;
  const url = base.trim().replace(/\/$/, '');
  return url.startsWith('http') ? url : `https://${url}/api`;
}

function getInternalKey(): string {
  const key =
    process.env.OA_INTERNAL_KEY ?? process.env.OPENAGENTS_INTERNAL_KEY ?? '';
  if (!key.trim()) {
    throw new Error(
      'OA_INTERNAL_KEY not set in this Convex deployment. Convex actions do not read .env.local. Set it in Convex: Dashboard → Settings → Environment Variables, or run: npx convex env set OA_INTERNAL_KEY "<same value as API worker>"',
    );
  }
  return key.trim();
}

function buildOpenclawUrl(apiBase: string, path: string, internalKey: string): string {
  const url = new URL(`${apiBase}${path}`);
  url.searchParams.set('oa_internal_key', internalKey);
  // Bust any edge caches that might be serving stale 401s.
  url.searchParams.set('oa_cache_bust', Date.now().toString());
  return url.toString();
}

async function requestOpenclaw<T>(params: {
  apiBase: string;
  internalKey: string;
  userId: string;
  path: string;
  method?: 'GET' | 'POST' | 'DELETE';
  body?: string;
  label: string;
}): Promise<T> {
  const url = buildOpenclawUrl(params.apiBase, params.path, params.internalKey);
  const redactedUrl = new URL(url);
  if (redactedUrl.searchParams.has('oa_internal_key')) {
    redactedUrl.searchParams.set('oa_internal_key', 'REDACTED');
  }
  console.log(
    `[openclawApi ${params.label}] fetch key_len=${params.internalKey.length} url=${redactedUrl.toString()}`,
  );
  // Send key in both headers and query params: Convex's fetch may strip custom headers.
  const res = await fetch(url, {
    method: params.method ?? 'GET',
    redirect: 'manual',
    headers: {
      Accept: 'application/json',
      ...(params.body ? { 'Content-Type': 'application/json' } : {}),
      'X-OA-Internal-Key': params.internalKey,
      Authorization: `Bearer ${params.internalKey}`,
      'X-OA-User-Id': params.userId,
    },
    body: params.body,
  });
  const text = await res.text();
  if (res.status >= 300 && res.status < 400) {
    throw new Error(
      `OpenClaw API returned redirect (${res.status}). Set PUBLIC_API_URL to the final API base (e.g. https://openagents.com/api) with no trailing slash.`,
    );
  }
  if (res.status >= 500) {
    const detail = text.slice(0, 500);
    console.error(`[openclawApi ${params.label}] API 5xx:`, res.status, detail);
    throw new Error(`OpenClaw API error (${res.status}): ${detail || 'no body'}`);
  }
  const ct = res.headers.get('content-type') ?? '';
  if (!ct.includes('application/json')) {
    console.error(`[openclawApi ${params.label}] API non-JSON:`, res.status, ct, text.slice(0, 200));
    throw new Error(`OpenClaw API returned non-JSON (${res.status}): ${text.slice(0, 200) || 'empty'}`);
  }
  let body: { ok: boolean; data?: T | null; error?: string };
  try {
    body = JSON.parse(text) as { ok: boolean; data?: T | null; error?: string };
  } catch (parseErr) {
    console.error(`[openclawApi ${params.label}] API JSON parse failed:`, res.status, text.slice(0, 200), parseErr);
    throw new Error(`OpenClaw API invalid JSON (${res.status}): ${text.slice(0, 150)}`);
  }
  if (!res.ok || !body.ok) {
    const msg = body.error ?? `Request failed (${res.status})`;
    console.error(`[openclawApi ${params.label}] API error response:`, res.status, msg);
    const cacheStatus = res.headers.get('cf-cache-status') ?? 'unknown';
    const age = res.headers.get('age') ?? 'unknown';
    const workerVersion = res.headers.get('x-oa-worker-version') ?? 'unknown';
    const cfRay = res.headers.get('cf-ray') ?? 'unknown';
    console.error(
      `[openclawApi ${params.label}] cache status: cf-cache-status=${cacheStatus} age=${age}`,
    );
    console.error(
      `[openclawApi ${params.label}] worker: x-oa-worker-version=${workerVersion} cf-ray=${cfRay}`,
    );
    if (res.status === 401) {
      throw new Error(
        `${msg} Convex actions do not read .env.local. Set OA_INTERNAL_KEY and PUBLIC_API_URL in this Convex deployment (Dashboard → Environment Variables, or npx convex env set OA_INTERNAL_KEY "<same as API worker>" and npx convex env set PUBLIC_API_URL "https://openagents.com/api"). API worker must have the same OA_INTERNAL_KEY (npx wrangler secret put OA_INTERNAL_KEY in apps/api).`,
      );
    }
    throw new Error(msg);
  }
  if (body.data == null) {
    throw new Error('No data in response');
  }
  return body.data;
}

export const getInstance = action({
  args: {},
  returns: v.union(
    v.object({
      status: v.string(),
      runtime_name: v.optional(v.union(v.string(), v.null())),
      created_at: v.number(),
      updated_at: v.number(),
      last_ready_at: v.optional(v.union(v.number(), v.null())),
    }),
    v.null(),
  ),
  handler: async (ctx): Promise<{
    status: string;
    runtime_name?: string | null;
    created_at: number;
    updated_at: number;
    last_ready_at?: number | null;
  } | null> => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    const apiBase = getApiBase();
    const internalKey = getInternalKey();
    const userId = identity.subject;
    const url = buildOpenclawUrl(apiBase, '/openclaw/instance', internalKey);
    const res = await fetch(url, {
      method: 'GET',
      redirect: 'manual',
      headers: {
        Accept: 'application/json',
        'X-OA-Internal-Key': internalKey,
        Authorization: `Bearer ${internalKey}`,
        'X-OA-User-Id': userId,
      },
    });
    const text = await res.text();
    if (res.status >= 300 && res.status < 400) {
      console.error('[openclawApi getInstance] API redirect:', res.status);
      return null;
    }
    // 5xx or non-JSON: treat as unavailable, return null so UI can show "unavailable" / retry
    if (res.status >= 500) {
      console.error('[openclawApi getInstance] API 5xx:', res.status, text.slice(0, 500));
      return null;
    }
    const ct = res.headers.get('content-type') ?? '';
    if (!ct.includes('application/json')) {
      console.error('[openclawApi getInstance] API non-JSON:', res.status, ct, text.slice(0, 200));
      return null;
    }
    let body: { ok: boolean; data?: InstanceResult | null; error?: string };
    try {
      body = JSON.parse(text) as { ok: boolean; data?: InstanceResult | null; error?: string };
    } catch (parseErr) {
      console.error('[openclawApi getInstance] API JSON parse failed:', res.status, text.slice(0, 200), parseErr);
      return null;
    }
    if (!res.ok || !body.ok) {
      const msg = body.error ?? `Request failed (${res.status})`;
      if (res.status === 401) {
        throw new Error(
          `${msg} Convex actions do not read .env.local. Set OA_INTERNAL_KEY and PUBLIC_API_URL in this Convex deployment (Dashboard → Environment Variables, or npx convex env set ...). API worker must have the same OA_INTERNAL_KEY (npx wrangler secret put OA_INTERNAL_KEY in apps/api).`,
        );
      }
      throw new Error(msg);
    }
    return body.data ?? null;
  },
});

export const createInstance = action({
  args: {},
  returns: v.object({
    status: v.string(),
    runtime_name: v.optional(v.union(v.string(), v.null())),
    created_at: v.number(),
    updated_at: v.number(),
    last_ready_at: v.optional(v.union(v.number(), v.null())),
  }),
  handler: async (ctx): Promise<{
    status: string;
    runtime_name?: string | null;
    created_at: number;
    updated_at: number;
    last_ready_at?: number | null;
  }> => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    const apiBase = getApiBase();
    const internalKey = getInternalKey();
    const userId = identity.subject;
    const url = buildOpenclawUrl(apiBase, '/openclaw/instance', internalKey);
    const res = await fetch(url, {
      method: 'POST',
      redirect: 'manual',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        'X-OA-Internal-Key': internalKey,
        Authorization: `Bearer ${internalKey}`,
        'X-OA-User-Id': userId,
      },
      body: '{}',
    });
    const text = await res.text();
    if (res.status >= 500) {
      const detail = text.slice(0, 500);
      console.error('[openclawApi createInstance] API 5xx:', res.status, detail);
      throw new Error(`OpenClaw API error (${res.status}): ${detail || 'no body'}`);
    }
    const ct = res.headers.get('content-type') ?? '';
    if (!ct.includes('application/json')) {
      console.error('[openclawApi createInstance] API non-JSON:', res.status, ct, text.slice(0, 200));
      throw new Error(`OpenClaw API returned non-JSON (${res.status}): ${text.slice(0, 200) || 'empty'}`);
    }
    let body: { ok: boolean; data?: InstanceResult | null; error?: string };
    try {
      body = JSON.parse(text) as { ok: boolean; data?: InstanceResult | null; error?: string };
    } catch (parseErr) {
      console.error('[openclawApi createInstance] API JSON parse failed:', res.status, text.slice(0, 200), parseErr);
      throw new Error(`OpenClaw API invalid JSON (${res.status}): ${text.slice(0, 150)}`);
    }
    if (!res.ok || !body.ok) {
      const msg = body.error ?? `Request failed (${res.status})`;
      console.error('[openclawApi createInstance] API error response:', res.status, msg);
      if (res.status === 401) {
        throw new Error(
          `${msg} Convex actions do not read .env.local. Set OA_INTERNAL_KEY and PUBLIC_API_URL in this Convex deployment (Dashboard → Environment Variables, or npx convex env set ...). API worker must have the same OA_INTERNAL_KEY (npx wrangler secret put OA_INTERNAL_KEY in apps/api).`,
        );
      }
      throw new Error(msg);
    }
    const data = body.data;
    if (!data) {
      throw new Error('No data in response');
    }
    return data;
  },
});

export const deleteInstance = action({
  args: {},
  returns: v.object({
    deleted: v.boolean(),
  }),
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    return requestOpenclaw<DeleteInstanceResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: '/openclaw/instance',
      method: 'DELETE',
      label: 'deleteInstance',
    });
  },
});

export const getRuntimeStatus = action({
  args: {},
  returns: v.object({
    gateway: v.object({
      status: v.union(
        v.literal('running'),
        v.literal('starting'),
        v.literal('stopped'),
        v.literal('error'),
      ),
    }),
    lastBackup: v.union(v.string(), v.null()),
    container: v.object({ instanceType: v.string() }),
    version: v.record(v.string(), v.string()),
  }),
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    return requestOpenclaw<RuntimeStatusResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: '/openclaw/runtime/status',
      label: 'getRuntimeStatus',
    });
  },
});

export const getRuntimeDevices = action({
  args: {},
  returns: v.object({
    pending: v.array(
      v.object({
        requestId: v.string(),
        client: v.optional(
          v.object({
            platform: v.optional(v.string()),
            mode: v.optional(v.string()),
          }),
        ),
        requestedAt: v.optional(v.string()),
      }),
    ),
    paired: v.array(
      v.object({
        deviceId: v.string(),
        client: v.optional(
          v.object({
            platform: v.optional(v.string()),
            mode: v.optional(v.string()),
          }),
        ),
        pairedAt: v.optional(v.string()),
      }),
    ),
  }),
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    return requestOpenclaw<RuntimeDevicesResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: '/openclaw/runtime/devices',
      label: 'getRuntimeDevices',
    });
  },
});

export const approveRuntimeDevice = action({
  args: {
    requestId: v.string(),
  },
  returns: v.object({
    approved: v.boolean(),
    requestId: v.string(),
  }),
  handler: async (ctx, args) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    const encoded = encodeURIComponent(args.requestId);
    return requestOpenclaw<ApproveDeviceResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: `/openclaw/runtime/devices/${encoded}/approve`,
      method: 'POST',
      body: '{}',
      label: 'approveRuntimeDevice',
    });
  },
});

export const listPairingRequests = action({
  args: {
    channel: v.string(),
  },
  returns: v.object({
    channel: v.string(),
    requests: v.array(
      v.object({
        id: v.string(),
        code: v.string(),
        createdAt: v.string(),
        lastSeenAt: v.string(),
        meta: v.optional(v.record(v.string(), v.string())),
      }),
    ),
  }),
  handler: async (ctx, args) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    const encoded = encodeURIComponent(args.channel);
    return requestOpenclaw<PairingRequestsResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: `/openclaw/runtime/pairing/${encoded}`,
      label: 'listPairingRequests',
    });
  },
});

export const approvePairingRequest = action({
  args: {
    channel: v.string(),
    code: v.string(),
    notify: v.optional(v.boolean()),
  },
  returns: v.object({
    approved: v.boolean(),
    channel: v.string(),
    code: v.string(),
  }),
  handler: async (ctx, args) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    const encoded = encodeURIComponent(args.channel);
    return requestOpenclaw<ApprovePairingResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: `/openclaw/runtime/pairing/${encoded}/approve`,
      method: 'POST',
      body: JSON.stringify({
        code: args.code,
        ...(typeof args.notify === 'boolean' ? { notify: args.notify } : {}),
      }),
      label: 'approvePairingRequest',
    });
  },
});

export const backupRuntime = action({
  args: {},
  returns: v.object({
    lastBackup: v.union(v.string(), v.null()),
  }),
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    return requestOpenclaw<BackupResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: '/openclaw/runtime/backup',
      method: 'POST',
      body: '{}',
      label: 'backupRuntime',
    });
  },
});

export const restartRuntime = action({
  args: {},
  returns: v.object({
    message: v.string(),
  }),
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity?.subject) {
      throw new Error('not authenticated');
    }
    const access = await ctx.runQuery(api.access.getStatus, {});
    if (!access.allowed) {
      throw new Error('access denied');
    }
    return requestOpenclaw<RestartResult>({
      apiBase: getApiBase(),
      internalKey: getInternalKey(),
      userId: identity.subject,
      path: '/openclaw/runtime/restart',
      method: 'POST',
      body: '{}',
      label: 'restartRuntime',
    });
  },
});
