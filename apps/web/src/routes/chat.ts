import { createFileRoute, redirect } from '@tanstack/react-router';
import { openai } from '@ai-sdk/openai';
import { jsonSchema } from '@ai-sdk/provider-utils';
import { convertToModelMessages, streamText, stepCountIs } from 'ai';
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
  beforeLoad: () => {
    // GET /chat has no UI; send users to the assistant.
    throw redirect({ to: '/assistant' });
  },
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

        // OpenClaw API base must be explicit so server-side tool calls never hit the TanStack app.
        let apiBase = '';
        try {
          apiBase = resolveApiBase();
        } catch (error) {
          return new Response(
            JSON.stringify({
              ok: false,
              error:
                error instanceof Error
                  ? error.message
                  : 'OpenClaw API base not configured',
            }),
            { status: 500, headers: { 'content-type': 'application/json' } },
          );
        }
        const apiConfig: OpenclawApiConfig = {
          apiBase,
          internalKey,
          userId: userIdStr,
        };

        // OpenAI Responses API requires parameters to be JSON Schema with type: "object".
        // Explicit jsonSchema() ensures type: "object" (Zod empty object can emit type: "None").
        const emptySchema = jsonSchema({ type: 'object', properties: {} });
        const approveDeviceSchema = jsonSchema({
          type: 'object',
          properties: { requestId: { type: 'string', minLength: 1 } },
          required: ['requestId'],
        });

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          messages: await convertToModelMessages(messages),
          // Allow multiple tool-call rounds (default is 1) so model can e.g. get instance → provision → list devices → approve.
          stopWhen: stepCountIs(10),
          // Tool names must match OpenAI pattern: ^[a-zA-Z0-9_-]+$ (no dots).
          tools: {
            openclaw_get_instance: {
              description: "Get the current user's OpenClaw instance summary (or null if none).",
              inputSchema: emptySchema,
              execute: async () => getOpenclawInstance(apiConfig),
            },
            openclaw_provision: {
              description: 'Provision an OpenClaw instance for the current user.',
              inputSchema: emptySchema,
              execute: async () => createOpenclawInstance(apiConfig),
            },
            openclaw_get_status: {
              description: "Get runtime status for the current user's OpenClaw instance.",
              inputSchema: emptySchema,
              execute: async () => getRuntimeStatus(apiConfig),
            },
            openclaw_list_devices: {
              description: "List pending and paired devices for the current user's OpenClaw instance.",
              inputSchema: emptySchema,
              execute: async () => getRuntimeDevices(apiConfig),
            },
            openclaw_approve_device: {
              description: 'Approve a pending device by requestId.',
              inputSchema: approveDeviceSchema,
              execute: async ({ requestId }: { requestId: string }) =>
                approveRuntimeDevice(apiConfig, requestId),
            },
            openclaw_backup_now: {
              description: "Trigger a backup/sync for the current user's OpenClaw instance.",
              inputSchema: emptySchema,
              execute: async () => backupRuntime(apiConfig),
            },
            openclaw_restart: {
              description: 'Restart the OpenClaw gateway process.',
              inputSchema: emptySchema,
              execute: async () => restartRuntime(apiConfig),
            },
            openclaw_get_billing_summary: {
              description: 'Get current credit balance summary.',
              inputSchema: emptySchema,
              execute: async () => getBillingSummary(apiConfig),
            },
          } as any,
        });

        return result.toUIMessageStreamResponse();
      },
    },
  },
});
