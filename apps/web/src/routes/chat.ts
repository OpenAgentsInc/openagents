import { createFileRoute } from '@tanstack/react-router';
import { openai } from '@ai-sdk/openai';
import { convertToModelMessages, streamText } from 'ai';
import type { UIMessage } from 'ai';
import { z } from 'zod';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import {
  approveRuntimeDevice,
  backupRuntime,
  createOpenclawInstance,
  getBillingSummary,
  getOpenclawInstance,
  getRuntimeDevices,
  getRuntimeStatus,
  resolveApiBase,
  resolveInternalKey,
  restartRuntime,
} from '@/lib/openclawApi';
import type { OpenclawApiConfig } from '@/lib/openclawApi';

/**
 * Chat endpoint for apps/web.
 * IMPORTANT: do NOT mount under `/api/*` because the Rust worker owns `/api/*` in production.
 */
export const Route = createFileRoute('/chat')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const body = (await request.json().catch(() => null)) as
          | { id?: unknown; messages?: Array<UIMessage>; [key: string]: unknown }
          | null;
        const messages = body?.messages;
        if (!body || !Array.isArray(messages)) {
          return new Response(
            JSON.stringify({ ok: false, error: 'invalid request body' }),
            { status: 400, headers: { 'content-type': 'application/json' } },
          );
        }

        const auth = await getAuth().catch(() => null);
        const userId = auth?.user?.id;

        let internalKey = '';
        try {
          internalKey = resolveInternalKey();
        } catch (error) {
          // Fail fast: without this we can't call /api/openclaw/* in beta.
          return new Response(
            JSON.stringify({
              ok: false,
              error: error instanceof Error ? error.message : 'OA_INTERNAL_KEY not configured',
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

        const agentWorkerUrl = process.env.AGENT_WORKER_URL?.trim();
        if (agentWorkerUrl) {
          const threadIdRaw = typeof body.id === 'string' ? body.id.trim() : '';
          const threadId = threadIdRaw ? `${userIdStr}:${threadIdRaw}` : `user:${userIdStr}`;

          const headers = new Headers();
          headers.set('content-type', 'application/json');
          headers.set('accept', request.headers.get('accept') ?? 'text/plain');
          headers.set('X-OA-Internal-Key', internalKey);
          headers.set('X-OA-User-Id', userIdStr);
          headers.set('X-OA-Thread-Id', threadId);

          const target = `${agentWorkerUrl.replace(/\/$/, '')}/internal/chat`;
          return fetch(target, {
            method: 'POST',
            headers,
            body: JSON.stringify(body),
          });
        }

        // Call the Rust API worker on the same apex domain.
        const origin = new URL(request.url).origin;
        const apiBase = resolveApiBase(origin);
        const apiConfig: OpenclawApiConfig = {
          apiBase,
          internalKey,
          userId: userIdStr,
        };

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          messages: await convertToModelMessages(messages),
          // Note: we keep tool typing loose here to avoid version-specific TS friction.
          // The runtime behavior is what matters; tighten types later.
          tools: {
            'openclaw.getInstance': {
              description: "Get the current user's OpenClaw instance summary (or null if none).",
              parameters: z.object({}),
              execute: async () => getOpenclawInstance(apiConfig),
            },
            'openclaw.provision': {
              description: 'Provision an OpenClaw instance for the current user.',
              parameters: z.object({}),
              execute: async () => createOpenclawInstance(apiConfig),
            },
            'openclaw.getStatus': {
              description: "Get runtime status for the current user's OpenClaw instance.",
              parameters: z.object({}),
              execute: async () => getRuntimeStatus(apiConfig),
            },
            'openclaw.listDevices': {
              description: "List pending and paired devices for the current user's OpenClaw instance.",
              parameters: z.object({}),
              execute: async () => getRuntimeDevices(apiConfig),
            },
            'openclaw.approveDevice': {
              description: 'Approve a pending device by requestId.',
              parameters: z.object({ requestId: z.string().min(1) }),
              execute: async ({ requestId }: { requestId: string }) =>
                approveRuntimeDevice(apiConfig, requestId),
            },
            'openclaw.backupNow': {
              description: "Trigger a backup/sync for the current user's OpenClaw instance.",
              parameters: z.object({}),
              execute: async () => backupRuntime(apiConfig),
            },
            'openclaw.restart': {
              description: 'Restart the OpenClaw gateway process.',
              parameters: z.object({}),
              execute: async () => restartRuntime(apiConfig),
            },
            'openclaw.getBillingSummary': {
              description: 'Get current credit balance summary.',
              parameters: z.object({}),
              execute: async () => getBillingSummary(apiConfig),
            },
          } as any,
        });

        return result.toUIMessageStreamResponse();
      },
    },
  },
});
