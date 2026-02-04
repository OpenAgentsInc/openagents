/**
 * HTTP handlers for /control/agent/* used by the API worker (Rust).
 * Verifies x-oa-control-key; agent signup and key lookup.
 */
import { httpAction } from './_generated/server';
import { internal } from './_generated/api';

const CONTROL_HEADER = 'x-oa-control-key';

function requireControlKey(request: Request): Response | null {
  const secret = process.env.OA_CONTROL_KEY;
  if (!secret) {
    console.error('[agent control] OA_CONTROL_KEY not set in Convex env');
    return jsonResponse(
      { ok: false, error: 'Control key not configured (OA_CONTROL_KEY missing in Convex)' },
      500,
    );
  }
  const header = request.headers.get(CONTROL_HEADER);
  if (!header || header !== secret) {
    console.error('[agent control] Unauthorized: missing or invalid x-oa-control-key header');
    return jsonResponse(
      { ok: false, error: 'Unauthorized (invalid or missing x-oa-control-key)' },
      401,
    );
  }
  return null;
}

function jsonResponse(
  body: { ok: boolean; data?: unknown; error?: string },
  status: number,
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

export const handleAgentSignup = httpAction(async (ctx, request) => {
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

  const handle = typeof body.handle === 'string' ? body.handle.trim() || undefined : undefined;
  const owner_workos_user_id =
    typeof body.owner_workos_user_id === 'string' ? body.owner_workos_user_id.trim() || undefined : undefined;
  const scopes = Array.isArray(body.scopes)
    ? (body.scopes as string[]).filter((s) => typeof s === 'string')
    : undefined;
  const description =
    typeof body.description === 'string' ? body.description.trim() || undefined : undefined;

  try {
    const result = await ctx.runMutation(internal.agentUsers.createAgentUserAndKey, {
      handle,
      owner_workos_user_id,
      scopes,
      description,
    });
    return jsonResponse(
      {
        ok: true,
        data: {
          agent_user_id: result.agentUserId,
          api_key: result.apiKey,
          key_id: result.keyId,
        },
      },
      200,
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[agent control POST /agent/signup]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleAgentTouchKey = httpAction(async (ctx, request) => {
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
  const key_hash = typeof body.key_hash === 'string' ? body.key_hash.trim() : '';
  if (!key_hash) {
    return jsonResponse({ ok: false, error: 'key_hash required' }, 400);
  }

  try {
    await ctx.runMutation(internal.agentUsers.touchAgentKeyLastUsed, { key_hash });
    return jsonResponse({ ok: true }, 200);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[agent control POST /agent/touch]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});

export const handleAgentByKeyHash = httpAction(async (ctx, request) => {
  if (request.method !== 'GET') {
    return new Response('Method Not Allowed', { status: 405 });
  }
  const authError = requireControlKey(request);
  if (authError) return authError;

  const url = new URL(request.url);
  const key_hash = url.searchParams.get('key_hash')?.trim();
  if (!key_hash) {
    return jsonResponse({ ok: false, error: 'key_hash required' }, 400);
  }

  try {
    const principal = await ctx.runQuery(internal.agentUsers.getAgentByKeyHash, { key_hash });
    if (!principal) {
      return jsonResponse({ ok: false, error: 'Invalid or revoked key' }, 404);
    }
    return jsonResponse(
      {
        ok: true,
        data: {
          agent_user_id: principal.agent_user_id,
          scopes: principal.scopes,
        },
      },
      200,
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.error('[agent control GET /agent/by-key-hash]', e);
    return jsonResponse({ ok: false, error: message }, 500);
  }
});
