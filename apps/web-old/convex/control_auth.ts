import { httpAction } from './_generated/server';
import { internal } from './_generated/api';

const CONTROL_HEADER = 'x-oa-control-key';

function unauthorized(message = 'Unauthorized'): Response {
  return new Response(message, { status: 401 });
}

function badRequest(message: string): Response {
  return new Response(message, { status: 400 });
}

function requireControlKey(request: Request): Response | null {
  const secret = process.env.OA_CONTROL_KEY;
  if (!secret) {
    return new Response('Control key not configured', { status: 500 });
  }
  const header = request.headers.get(CONTROL_HEADER);
  if (!header || header !== secret) {
    return unauthorized();
  }
  return null;
}

export const resolveToken = httpAction(async (ctx, request) => {
  if (request.method !== 'POST') {
    return new Response('Method Not Allowed', { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest('Invalid JSON');
  }

  const tokenValue = payload.token;
  if (typeof tokenValue !== 'string' || !tokenValue.trim()) {
    return badRequest('token is required');
  }

  const resolved = await ctx.runQuery(internal.apiTokens.resolveApiToken, {
    token: tokenValue.trim(),
  });

  if (!resolved) {
    return unauthorized('Invalid api token');
  }

  await ctx.runMutation(internal.apiTokens.updateApiTokenLastUsed, {
    tokenHash: resolved.tokenHash,
  });

  return new Response(
    JSON.stringify({
      ok: true,
      data: resolved,
    }),
    {
      status: 200,
      headers: { 'content-type': 'application/json' },
    },
  );
});

export const registerAgent = httpAction(async (ctx, request) => {
  if (request.method !== 'POST') {
    return new Response('Method Not Allowed', { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest('Invalid JSON');
  }

  const userIdRaw = payload.user_id ?? payload.userId ?? payload.subject;
  if (typeof userIdRaw !== 'string' || !userIdRaw.trim()) {
    return badRequest('user_id is required');
  }
  const user_id = userIdRaw.trim();

  const name = typeof payload.name === 'string' ? payload.name : undefined;
  const metadata = payload.metadata === null ? undefined : payload.metadata;
  const tokenNameRaw = payload.token_name ?? payload.tokenName ?? 'agent';
  const token_name = typeof tokenNameRaw === 'string' && tokenNameRaw.trim()
    ? tokenNameRaw
    : 'agent';

  const nostr = payload.nostr as { pubkey?: unknown } | undefined;
  const nostr_pubkey =
    typeof nostr?.pubkey === 'string' && nostr.pubkey.trim() ? nostr.pubkey.trim() : undefined;

  const user = await ctx.runMutation(internal.users.upsertAgentUser, {
    user_id,
    name,
    metadata,
    nostr_pubkey,
  });

  const token = await ctx.runMutation(internal.apiTokens.issueApiTokenForUser, {
    user_id,
    name: token_name,
  });

  return new Response(
    JSON.stringify({
      ok: true,
      data: {
        user_id: user.user_id,
        api_token: token.token,
        created: user.created,
      },
    }),
    {
      status: 200,
      headers: { 'content-type': 'application/json' },
    },
  );
});
