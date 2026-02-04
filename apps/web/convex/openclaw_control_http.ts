/**
 * HTTP handlers for /control/openclaw/* used by the API worker (Rust).
 * Verifies x-oa-control-key and forwards to internal openclaw/billing functions.
 */
import { httpAction } from './_generated/server';
import { internal } from './_generated/api';

const CONTROL_HEADER = 'x-oa-control-key';

function requireControlKey(request: Request): Response | null {
  const secret = process.env.OA_CONTROL_KEY;
  if (!secret) {
    console.error('[openclaw control] OA_CONTROL_KEY not set in Convex env');
    return jsonResponse({ ok: false, error: 'Control key not configured (OA_CONTROL_KEY missing in Convex)' }, 500);
  }
  const header = request.headers.get(CONTROL_HEADER);
  if (!header || header !== secret) {
    console.error('[openclaw control] Unauthorized: missing or invalid x-oa-control-key header');
    return jsonResponse({ ok: false, error: 'Unauthorized (invalid or missing x-oa-control-key)' }, 401);
  }
  return null;
}

function jsonResponse(body: { ok: boolean; data?: unknown; error?: string }, status: number): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function instanceToApi(doc: {
  user_id: string;
  status: string;
  runtime_name?: string | null;
  created_at: number;
  updated_at: number;
  last_ready_at?: number | null;
  runtime_url?: string | null;
  cf_account_id?: string | null;
  cf_worker_name?: string | null;
  cf_worker_id?: string | null;
  cf_container_app_id?: string | null;
  cf_container_app_name?: string | null;
  r2_bucket_name?: string | null;
}): Record<string, unknown> {
  return {
    user_id: doc.user_id,
    status: doc.status,
    runtime_name: doc.runtime_name ?? null,
    runtime_url: doc.runtime_url ?? null,
    created_at: doc.created_at,
    updated_at: doc.updated_at,
    last_ready_at: doc.last_ready_at ?? null,
    cf_account_id: doc.cf_account_id ?? null,
    cf_worker_name: doc.cf_worker_name ?? null,
    cf_worker_id: doc.cf_worker_id ?? null,
    cf_container_app_id: doc.cf_container_app_id ?? null,
    cf_container_app_name: doc.cf_container_app_name ?? null,
    r2_bucket_name: doc.r2_bucket_name ?? null,
  };
}

export const handleInstanceGet = httpAction(async (ctx, request) => {
  if (request.method !== 'GET') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  const url = new URL(request.url);
  const user_id = url.searchParams.get('user_id')?.trim();
  if (!user_id) {
    return jsonResponse({ ok: false, error: 'user_id required' }, 400);
  }

  try {
    const instance = await ctx.runQuery(internal.openclaw.getInstanceForUser, { user_id });
    const data = instance ? instanceToApi(instance) : null;
    return jsonResponse({ ok: true, data }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[openclaw control GET /instance]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleInstancePost = httpAction(async (ctx, request) => {
  if (request.method !== 'POST') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  let body: Record<string, unknown> = {};
  try {
    const text = await request.text();
    if (text.trim()) {
      body = JSON.parse(text) as Record<string, unknown>;
    }
  } catch {
    return jsonResponse({ ok: false, error: 'Invalid JSON' }, 400);
  }

  const user_id = typeof body.user_id === 'string' ? body.user_id.trim() : '';
  if (!user_id) {
    return jsonResponse({ ok: false, error: 'user_id required' }, 400);
  }

  const args: Record<string, unknown> = { user_id };
  if (typeof body.status === 'string') args.status = body.status;
  if (typeof body.runtime_url === 'string') args.runtime_url = body.runtime_url;
  if (typeof body.runtime_name === 'string') args.runtime_name = body.runtime_name;
  if (typeof body.cf_account_id === 'string') args.cf_account_id = body.cf_account_id;
  if (typeof body.cf_worker_name === 'string') args.cf_worker_name = body.cf_worker_name;
  if (typeof body.cf_worker_id === 'string') args.cf_worker_id = body.cf_worker_id;
  if (typeof body.cf_container_app_id === 'string') args.cf_container_app_id = body.cf_container_app_id;
  if (typeof body.cf_container_app_name === 'string') args.cf_container_app_name = body.cf_container_app_name;
  if (typeof body.r2_bucket_name === 'string') args.r2_bucket_name = body.r2_bucket_name;

  try {
    const instance = await ctx.runMutation(internal.openclaw.upsertInstance, {
      user_id: args.user_id as string,
      status: args.status as string | undefined,
      runtime_url: args.runtime_url as string | undefined,
      runtime_name: args.runtime_name as string | undefined,
      cf_account_id: args.cf_account_id as string | undefined,
      cf_worker_name: args.cf_worker_name as string | undefined,
      cf_worker_id: args.cf_worker_id as string | undefined,
      cf_container_app_id: args.cf_container_app_id as string | undefined,
      cf_container_app_name: args.cf_container_app_name as string | undefined,
      r2_bucket_name: args.r2_bucket_name as string | undefined,
    });
    return jsonResponse({ ok: true, data: instanceToApi(instance) }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[openclaw control POST /instance]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleInstanceStatusPost = httpAction(async (ctx, request) => {
  if (request.method !== 'POST') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  let body: Record<string, unknown> = {};
  try {
    body = (await request.json()) as Record<string, unknown>;
  } catch {
    return jsonResponse({ ok: false, error: 'Invalid JSON' }, 400);
  }

  const user_id = typeof body.user_id === 'string' ? body.user_id.trim() : '';
  const status = typeof body.status === 'string' ? body.status.trim() : '';
  if (!user_id || !status) {
    return jsonResponse({ ok: false, error: 'user_id and status required' }, 400);
  }

  try {
    const instance = await ctx.runMutation(internal.openclaw.setInstanceStatus, { user_id, status });
    return jsonResponse({ ok: true, data: instanceToApi(instance) }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[openclaw control POST /instance/status]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleInstanceSecretPost = httpAction(async (ctx, request) => {
  if (request.method !== 'POST') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  let body: Record<string, unknown> = {};
  try {
    body = (await request.json()) as Record<string, unknown>;
  } catch {
    return jsonResponse({ ok: false, error: 'Invalid JSON' }, 400);
  }

  const user_id = typeof body.user_id === 'string' ? body.user_id.trim() : '';
  const key = typeof body.key === 'string' ? body.key.trim() : '';
  const value = typeof body.value === 'string' ? body.value : '';
  if (!user_id || !key) {
    return jsonResponse({ ok: false, error: 'user_id and key required' }, 400);
  }

  try {
    await ctx.runMutation(internal.openclaw.storeEncryptedSecret, { user_id, key, value });
    return jsonResponse({ ok: true, data: {} }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[openclaw control POST /instance/secret]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleInstanceSecretGet = httpAction(async (ctx, request) => {
  if (request.method !== 'GET') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  const url = new URL(request.url);
  const user_id = url.searchParams.get('user_id')?.trim();
  const key = url.searchParams.get('key')?.trim();
  if (!user_id || !key) {
    return jsonResponse({ ok: false, error: 'user_id and key required' }, 400);
  }

  try {
    const secret = await ctx.runQuery(internal.openclaw.getDecryptedSecret, { user_id, key });
    return jsonResponse({ ok: true, data: { secret: secret ?? undefined } }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[openclaw control GET /instance/secret]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleBillingSummaryGet = httpAction(async (ctx, request) => {
  if (request.method !== 'GET') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  const url = new URL(request.url);
  const user_id = url.searchParams.get('user_id')?.trim();
  if (!user_id) {
    return jsonResponse({ ok: false, error: 'user_id required' }, 400);
  }

  try {
    const summary = await ctx.runQuery(internal.billing.getCreditBalance, { user_id });
    return jsonResponse({ ok: true, data: summary }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[openclaw control GET /billing/summary]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});
