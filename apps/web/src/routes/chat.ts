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
          | {
              id?: unknown;
              messages?: Array<{
                id?: string;
                role: string;
                content: string;
              }>;
              [key: string]: unknown;
            }
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

        const normalizeRole = (
          role: string,
        ): 'user' | 'assistant' | 'system' =>
          role === 'user' || role === 'assistant' || role === 'system'
            ? role
            : 'assistant';

        const uiMessages = messages.map((message) => ({
          role: normalizeRole(message.role),
          parts: message.content
            ? [{ type: 'text' as const, text: message.content }]
            : [],
        }));

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          system: 'You are OpenAgents. Be concise.',
          messages: await convertToModelMessages(uiMessages),
        });

        const res = result.toUIMessageStreamResponse();
        const h = new Headers(res.headers);
        h.set('X-Chat-Source', 'local-fallback');
        return new Response(res.body, { status: res.status, headers: h });
      },
    },
  },
});
