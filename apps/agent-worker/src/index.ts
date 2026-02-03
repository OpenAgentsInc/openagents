import type { AgentWorkerEnv } from './types';
import { err } from './response';
import { isInternalKeyValid } from './auth/internalKey';

export { ThreadAgent } from './threadAgent';

async function resolveThreadId(request: Request): Promise<string | null> {
  const header = request.headers.get('x-oa-thread-id');
  if (header && header.trim().length > 0) return header.trim();

  const url = new URL(request.url);
  const qp = url.searchParams.get('threadId');
  if (qp && qp.trim().length > 0) return qp.trim();

  if (request.method === 'POST') {
    const cloned = request.clone();
    try {
      const body = (await cloned.json()) as { threadId?: unknown } | null;
      if (body && typeof body.threadId === 'string' && body.threadId.trim().length > 0) {
        return body.threadId.trim();
      }
    } catch {
      // ignore
    }
  }

  return null;
}

function jsonError(status: number, code: string, message: string, details?: Record<string, unknown> | null) {
  return new Response(JSON.stringify(err(code, message, details)), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function requireUserId(request: Request): string | null {
  const header = request.headers.get('x-oa-user-id');
  if (!header) return null;
  const userId = header.trim();
  return userId.length > 0 ? userId : null;
}

function requireInternalKey(request: Request, expected: string | undefined): Response | null {
  if (!isInternalKeyValid(request.headers, expected)) {
    return jsonError(401, 'unauthorized', 'unauthorized');
  }
  return null;
}

async function routeInternal(request: Request, env: AgentWorkerEnv): Promise<Response> {
  const url = new URL(request.url);

  if (url.pathname === '/internal/chat' && request.method === 'POST') {
    const userId = requireUserId(request);
    if (!userId) return jsonError(400, 'bad_request', 'missing x-oa-user-id');

    const threadId = await resolveThreadId(request);
    if (!threadId) return jsonError(400, 'bad_request', 'missing threadId');

    const id = env.ThreadAgent.idFromName(threadId);
    const stub = env.ThreadAgent.get(id);
    return stub.fetch(request);
  }

  if (url.pathname === '/internal/approval/respond' && request.method === 'POST') {
    const userId = requireUserId(request);
    if (!userId) return jsonError(400, 'bad_request', 'missing x-oa-user-id');

    const threadId = await resolveThreadId(request);
    if (!threadId) return jsonError(400, 'bad_request', 'missing threadId');

    const id = env.ThreadAgent.idFromName(threadId);
    const stub = env.ThreadAgent.get(id);
    return stub.fetch(request);
  }

  return jsonError(404, 'not_found', 'not found');
}

export default {
  async fetch(request: Request, env: AgentWorkerEnv): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname.startsWith('/internal/')) {
      const maybeUnauthorized = requireInternalKey(request, env.OA_INTERNAL_KEY);
      if (maybeUnauthorized) return maybeUnauthorized;
      return routeInternal(request, env);
    }

    return jsonError(404, 'not_found', 'not found');
  },
};
