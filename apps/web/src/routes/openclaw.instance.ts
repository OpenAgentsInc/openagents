import { createFileRoute } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { ConvexHttpClient } from 'convex/browser';
import { api } from '../../convex/_generated/api';
import {
  createOpenclawInstance,
  getOpenclawInstance,
  resolveApiBase,
  resolveInternalKey,
  type InstanceSummary,
  type OpenclawApiConfig,
} from '@/lib/openclawApi';

type ApiEnvelope<T> = {
  ok: boolean;
  data?: T | null;
  error?: string;
};

const JSON_HEADERS = { 'content-type': 'application/json' };

function json<T>(status: number, payload: ApiEnvelope<T>) {
  return new Response(JSON.stringify(payload), { status, headers: JSON_HEADERS });
}

function resolveConvexUrl(): string {
  const url = (import.meta as any).env?.VITE_CONVEX_URL ?? process.env.VITE_CONVEX_URL;
  if (!url) {
    throw new Error('VITE_CONVEX_URL not configured');
  }
  return url;
}

async function requireAccess(origin: string): Promise<OpenclawApiConfig> {
  const auth = await getAuth().catch(() => null);
  const user = auth?.user;
  if (!user) {
    throw new Error('not authenticated');
  }

  const token = auth?.accessToken ?? null;
  if (!token) {
    throw new Error('missing auth token');
  }

  const internalKey = resolveInternalKey();
  const apiBase = resolveApiBase(origin);

  const convex = new ConvexHttpClient(resolveConvexUrl());
  convex.setAuth(token);
  const access = await convex.query(api.access.getStatus, {});
  if (!access.allowed) {
    throw new Error('access denied');
  }

  return {
    apiBase,
    internalKey,
    userId: user.id,
  };
}

async function requireAccessFromRequest(request: Request): Promise<{
  config: OpenclawApiConfig;
} | Response> {
  try {
    const origin = new URL(request.url).origin;
    const config = await requireAccess(origin);
    return { config };
  } catch (err) {
    const message = err instanceof Error ? err.message : 'unauthorized';
    const status = message === 'not authenticated' || message === 'missing auth token' ? 401 : message === 'access denied' ? 403 : 500;
    return json(status, { ok: false, error: message });
  }
}

/** Server function: bypasses router HTML check so Hatchery can load instance data. */
export const getOpenclawInstanceServer = createServerFn({ method: 'POST' })
  .inputValidator((data: { origin: string }) => data)
  .handler(async (opts): Promise<{ ok: true; data: InstanceSummary | null }> => {
    const config = await requireAccess(opts.data.origin);
    const instance = await getOpenclawInstance(config);
    return { ok: true, data: instance };
  });

/** Server function: bypasses router HTML check so Hatchery can provision instance. */
export const createOpenclawInstanceServer = createServerFn({ method: 'POST' })
  .inputValidator((data: { origin: string }) => data)
  .handler(async (opts): Promise<{ ok: true; data: InstanceSummary }> => {
    const config = await requireAccess(opts.data.origin);
    const instance = await createOpenclawInstance(config);
    if (!instance) {
      throw new Error('No instance returned from API');
    }
    return { ok: true, data: instance };
  });

export const Route = createFileRoute('/openclaw/instance')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        try {
          const gate = await requireAccessFromRequest(request);
          if (gate instanceof Response) return gate;
          const data = await getOpenclawInstance(gate.config);
          return json(200, { ok: true, data });
        } catch (error) {
          return json(500, {
            ok: false,
            error: error instanceof Error ? error.message : 'failed to load instance',
          });
        }
      },
      POST: async ({ request }) => {
        try {
          const gate = await requireAccessFromRequest(request);
          if (gate instanceof Response) return gate;
          const data = await createOpenclawInstance(gate.config);
          return json(200, { ok: true, data });
        } catch (error) {
          return json(500, {
            ok: false,
            error: error instanceof Error ? error.message : 'failed to provision instance',
          });
        }
      },
    },
  },
});
