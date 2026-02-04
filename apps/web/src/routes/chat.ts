import { openai } from '@ai-sdk/openai';
import { jsonSchema } from '@ai-sdk/provider-utils';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { convertToModelMessages, stepCountIs, streamText } from 'ai';
import { z } from 'zod';
import type { UIMessage } from 'ai';
import type { OpenclawApiConfig } from '@/lib/openclawApi';
import { createApproval, getApproval } from '@/lib/approvalStore';
import {
  approvePairingRequest,
  approveRuntimeDevice,
  backupRuntime,
  createOpenclawInstance,
  getBillingSummary,
  getOpenclawInstance,
  getRuntimeDevices,
  getRuntimeStatus,
  listPairingRequests,
  resolveApiBase,
  resolveInternalKey,
  restartRuntime,
} from '@/lib/openclawApi';

type ApprovalGateResult = {
  status: 'approval_required' | 'approval_pending' | 'approval_rejected';
  approvalId: string;
  summary?: string;
  toolName?: string;
  toolInput?: unknown;
};

const defaultApprovalSummary = (toolName: string, toolInput: unknown): string => {
  if (toolName === 'openclaw_provision') return 'Provision a managed OpenClaw instance.';
  if (toolName === 'openclaw_restart') return 'Restart the OpenClaw gateway.';
  if (toolName === 'openclaw_approve_device') {
    if (toolInput && typeof toolInput === 'object' && 'requestId' in toolInput) {
      const requestId = (toolInput as { requestId?: unknown }).requestId;
      if (typeof requestId === 'string' && requestId.trim().length > 0) {
        return `Approve device pairing request ${requestId.trim()}.`;
      }
    }
    return 'Approve a pending device pairing request.';
  }
  if (toolName === 'openclaw_approve_pairing') {
    if (toolInput && typeof toolInput === 'object') {
      const channel = (toolInput as { channel?: unknown }).channel;
      const code = (toolInput as { code?: unknown }).code;
      if (typeof channel === 'string' && channel.trim().length > 0) {
        if (typeof code === 'string' && code.trim().length > 0) {
          return `Approve DM pairing ${channel.trim()} code ${code.trim()}.`;
        }
        return `Approve DM pairing request for ${channel.trim()}.`;
      }
    }
    return 'Approve a DM pairing request.';
  }
  return `Approve ${toolName}.`;
};

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
          const proxyResponse = await fetch(target, {
            method: 'POST',
            headers,
            body: JSON.stringify(body),
          });
          // If agent worker returns 401 (e.g. OA_INTERNAL_KEY mismatch), fall back to local chat so logged-in users still get a response.
          if (proxyResponse.status !== 401) {
            const h = new Headers(proxyResponse.headers);
            h.set('X-Chat-Source', 'agent-worker');
            return new Response(proxyResponse.body, { status: proxyResponse.status, headers: h });
          }
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

        const requireApproval = async <T,>({
          toolName,
          toolInput,
          approvalId,
          action,
        }: {
          toolName: string;
          toolInput: unknown;
          approvalId?: string;
          action: () => Promise<T>;
        }): Promise<T | ApprovalGateResult> => {
          const summary = defaultApprovalSummary(toolName, toolInput);
          if (approvalId) {
            const existing = getApproval(userIdStr, approvalId);
            if (existing) {
              if (existing.status === 'approved') return action();
              if (existing.status === 'rejected') {
                return {
                  status: 'approval_rejected',
                  approvalId: existing.id,
                  summary: existing.summary,
                  toolName,
                  toolInput,
                };
              }
              return {
                status: 'approval_pending',
                approvalId: existing.id,
                summary: existing.summary,
                toolName,
                toolInput,
              };
            }
          }

          const record = createApproval({
            userId: userIdStr,
            summary,
            toolName,
            toolInput,
          });
          return {
            status: 'approval_required',
            approvalId: record.id,
            summary: record.summary,
            toolName,
            toolInput,
          };
        };

        // OpenAI Responses API requires parameters to be JSON Schema with type: "object".
        // Explicit jsonSchema() ensures type: "object" (Zod empty object can emit type: "None").
        const emptySchema = jsonSchema({ type: 'object', properties: {} });
        const approveDeviceSchema = jsonSchema({
          type: 'object',
          properties: {
            requestId: { type: 'string', minLength: 1 },
            approvalId: { type: 'string', minLength: 1 },
          },
          required: ['requestId'],
        });

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          system: [
            'You are OpenAgents. Be concise.',
            'Sensitive actions (provisioning, pairing approvals, restarts) require explicit human approval.',
            'If a tool response includes status approval_required, ask the user to approve or reject.',
            'After approval, call the same tool again with the provided approvalId to continue.',
          ].join(' '),
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
              inputSchema: jsonSchema({
                type: 'object',
                properties: { approvalId: { type: 'string', minLength: 1 } },
              }),
              execute: async ({ approvalId }: { approvalId?: string }) =>
                requireApproval({
                  toolName: 'openclaw_provision',
                  toolInput: {},
                  approvalId,
                  action: () => createOpenclawInstance(apiConfig),
                }),
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
              execute: async ({ requestId, approvalId }: { requestId: string; approvalId?: string }) =>
                requireApproval({
                  toolName: 'openclaw_approve_device',
                  toolInput: { requestId },
                  approvalId,
                  action: () => approveRuntimeDevice(apiConfig, requestId),
                }),
            },
            openclaw_list_pairing_requests: {
              description: 'List pending DM pairing requests for a channel.',
              inputSchema: jsonSchema({
                type: 'object',
                properties: {
                  channel: { type: 'string', minLength: 1 },
                },
                required: ['channel'],
              }),
              execute: async ({ channel }: { channel: string }) => listPairingRequests(apiConfig, channel),
            },
            openclaw_approve_pairing: {
              description: 'Approve a DM pairing request by channel + code.',
              inputSchema: jsonSchema({
                type: 'object',
                properties: {
                  channel: { type: 'string', minLength: 1 },
                  code: { type: 'string', minLength: 1 },
                  notify: { type: 'boolean' },
                  approvalId: { type: 'string', minLength: 1 },
                },
                required: ['channel', 'code'],
              }),
              execute: async ({
                channel,
                code,
                notify,
                approvalId,
              }: {
                channel: string;
                code: string;
                notify?: boolean;
                approvalId?: string;
              }) =>
                requireApproval({
                  toolName: 'openclaw_approve_pairing',
                  toolInput: { channel, code, notify },
                  approvalId,
                  action: () => approvePairingRequest(apiConfig, { channel, code, notify }),
                }),
            },
            openclaw_backup_now: {
              description: "Trigger a backup/sync for the current user's OpenClaw instance.",
              inputSchema: emptySchema,
              execute: async () => backupRuntime(apiConfig),
            },
            openclaw_restart: {
              description: 'Restart the OpenClaw gateway process.',
              inputSchema: jsonSchema({
                type: 'object',
                properties: { approvalId: { type: 'string', minLength: 1 } },
              }),
              execute: async ({ approvalId }: { approvalId?: string }) =>
                requireApproval({
                  toolName: 'openclaw_restart',
                  toolInput: {},
                  approvalId,
                  action: () => restartRuntime(apiConfig),
                }),
            },
            openclaw_get_billing_summary: {
              description: 'Get current credit balance summary.',
              inputSchema: emptySchema,
              execute: async () => getBillingSummary(apiConfig),
            },
          } as any,
        });

        const res = result.toUIMessageStreamResponse({
          messageMetadata: ({ part }) =>
            part.type === 'start' ? { chatSource: 'local-fallback' } : undefined,
        });
        const h = new Headers(res.headers);
        h.set('X-Chat-Source', 'local-fallback');
        return new Response(res.body, { status: res.status, headers: h });
      },
    },
  },
});
