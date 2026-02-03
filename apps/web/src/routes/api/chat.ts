import { createFileRoute } from '@tanstack/react-router';
import { openai } from '@ai-sdk/openai';
import { streamText, convertToModelMessages, type UIMessage } from 'ai';
import { z } from 'zod';
import { getAuth } from '@workos/authkit-tanstack-react-start';

/**
 * Chat endpoint for apps/web.
 * IMPORTANT: do NOT mount under `/api/*` because the Rust worker owns `/api/*` in production.
 */
export const Route = createFileRoute('/chat')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const { messages } = (await request.json()) as { messages: UIMessage[] };

        const auth = await getAuth().catch(() => null);
        const userId = auth?.user?.id;

        const internalKey = (process.env.OA_INTERNAL_KEY ?? '').trim();
        if (!internalKey) {
          // Fail fast: without this we can't call /api/openclaw/* in beta.
          return new Response(
            JSON.stringify({
              ok: false,
              error: 'OA_INTERNAL_KEY not configured on apps/web',
            }),
            { status: 500, headers: { 'content-type': 'application/json' } },
          );
        }
        if (!userId) {
          return new Response(
            JSON.stringify({ ok: false, error: 'not authenticated' }),
            { status: 401, headers: { 'content-type': 'application/json' } },
          );
        }
        // TS narrowing helper
        const userIdStr: string = userId;

        // Call the Rust API worker on the same apex domain.
        const origin = new URL(request.url).origin;
        const oaApiBase = `${origin}/api`;

        async function callOpenclaw<T>(
          path: string,
          init: RequestInit = {},
        ): Promise<T> {
          const headers = new Headers(init.headers);
          headers.set('accept', 'application/json');
          headers.set('X-OA-Internal-Key', internalKey);
          headers.set('X-OA-User-Id', userIdStr);
          if (init.body && !headers.has('content-type')) {
            headers.set('content-type', 'application/json');
          }

          const res = await fetch(`${oaApiBase}${path}`, {
            ...init,
            headers,
          });
          const json = (await res.json().catch(() => null)) as any;
          if (!res.ok || !json?.ok) {
            const msg = json?.error ?? `OpenClaw request failed (${res.status})`;
            throw new Error(msg);
          }
          return json.data as T;
        }

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          messages: await convertToModelMessages(messages),
          // Note: we keep tool typing loose here to avoid version-specific TS friction.
          // The runtime behavior is what matters; tighten types later.
          tools: {
            'openclaw.getInstance': {
              description: "Get the current user's OpenClaw instance summary (or null if none).",
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/instance', { method: 'GET' }),
            },
            'openclaw.provision': {
              description: 'Provision an OpenClaw instance for the current user.',
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/instance', { method: 'POST' }),
            },
            'openclaw.getStatus': {
              description: "Get runtime status for the current user's OpenClaw instance.",
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/runtime/status', { method: 'GET' }),
            },
            'openclaw.listDevices': {
              description: "List pending and paired devices for the current user's OpenClaw instance.",
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/runtime/devices', { method: 'GET' }),
            },
            'openclaw.approveDevice': {
              description: 'Approve a pending device by requestId.',
              parameters: z.object({ requestId: z.string().min(1) }),
              execute: async ({ requestId }: { requestId: string }) =>
                callOpenclaw(
                  `/openclaw/runtime/devices/${encodeURIComponent(requestId)}/approve`,
                  { method: 'POST' },
                ),
            },
            'openclaw.backupNow': {
              description: "Trigger a backup/sync for the current user's OpenClaw instance.",
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/runtime/backup', { method: 'POST' }),
            },
            'openclaw.restart': {
              description: 'Restart the OpenClaw gateway process.',
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/runtime/restart', { method: 'POST' }),
            },
            'openclaw.getBillingSummary': {
              description: 'Get current credit balance summary.',
              parameters: z.object({}),
              execute: async () => callOpenclaw('/openclaw/billing/summary', { method: 'GET' }),
            },
          } as any,
        });

        return result.toUIMessageStreamResponse();
      },
    },
  },
});
