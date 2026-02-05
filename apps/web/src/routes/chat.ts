import { openai } from '@ai-sdk/openai';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { convertToModelMessages, streamText } from 'ai';

/**
 * Chat endpoint for apps/web.
 * Does NOT mount under /api/* — the Rust worker owns /api/* in production.
 */
export const Route = createFileRoute('/chat')({
  beforeLoad: () => {
    throw redirect({ to: '/assistant' });
  },
  server: {
    handlers: {
      POST: async ({ request }) => {
        const body = (await request.json().catch(() => null)) as
          | { id?: unknown; messages?: Array<{ id?: string; role: string; content: string }>; [key: string]: unknown }
          | null;
        const messages = body?.messages;
        if (!body || !Array.isArray(messages)) {
          return new Response(
            JSON.stringify({ ok: false, error: 'invalid request body' }),
            { status: 400, headers: { 'content-type': 'application/json' } },
          );
        }

        const auth = await getAuth().catch(() => null);
        const userId = typeof auth?.user?.id === 'string' ? auth.user.id.trim() : '';
        if (!userId) {
          return new Response(
            JSON.stringify({ ok: false, error: 'not authenticated' }),
            { status: 401, headers: { 'content-type': 'application/json' } },
          );
        }

        const agentWorkerUrl = process.env.AGENT_WORKER_URL?.trim();
        const internalKey = process.env.OA_INTERNAL_KEY?.trim();
        const threadIdRaw = typeof body.id === 'string' ? body.id.trim() : '';
        const threadId = threadIdRaw ? `${userId}:${threadIdRaw}` : `user:${userId}`;

        if (agentWorkerUrl && internalKey) {
          const headers = new Headers();
          headers.set('content-type', 'application/json');
          headers.set('accept', request.headers.get('accept') ?? 'text/plain');
          headers.set('X-OA-Internal-Key', internalKey);
          headers.set('X-OA-User-Id', userId);
          headers.set('X-OA-Thread-Id', threadId);

          const proxyResponse = await fetch(`${agentWorkerUrl.replace(/\/$/, '')}/internal/chat`, {
            method: 'POST',
            headers,
            body: JSON.stringify(body),
          });
          if (proxyResponse.status !== 401) {
            const h = new Headers(proxyResponse.headers);
            h.set('X-Chat-Source', 'agent-worker');
            return new Response(proxyResponse.body, { status: proxyResponse.status, headers: h });
          }
        }

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          system: 'You are OpenAgents. Be concise.',
          messages: await convertToModelMessages(messages),
        });

        const res = result.toUIMessageStreamResponse();
        const h = new Headers(res.headers);
        h.set('X-Chat-Source', 'local-fallback');
        return new Response(res.body, { status: res.status, headers: h });
      },
    },
  },
});
