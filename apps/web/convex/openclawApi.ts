/**
 * Convex actions that proxy OpenClaw instance get/create to the Rust API.
 * Used by Hatchery to avoid TanStack server-function path that can hit "Only HTML requests" error.
 */
import { action } from './_generated/server';
import { api } from './_generated/api';
import { v } from 'convex/values';

const DEFAULT_API_BASE = 'https://openagents.com/api';

type InstanceResult = {
  status: string;
  runtime_name?: string | null;
  created_at: number;
  updated_at: number;
  last_ready_at?: number | null;
};

function getApiBase(): string {
  const base =
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
    throw new Error('OA_INTERNAL_KEY not configured in Convex');
  }
  return key.trim();
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
    const res = await fetch(`${apiBase}/openclaw/instance`, {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        'X-OA-Internal-Key': internalKey,
        'X-OA-User-Id': userId,
      },
    });
    const text = await res.text();
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
    const res = await fetch(`${apiBase}/openclaw/instance`, {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        'X-OA-Internal-Key': internalKey,
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
      throw new Error(msg);
    }
    const data = body.data;
    if (!data) {
      throw new Error('No data in response');
    }
    return data;
  },
});
