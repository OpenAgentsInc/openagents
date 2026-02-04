import { createOpenAI } from '@ai-sdk/openai';
import { jsonSchema } from '@ai-sdk/provider-utils';
import { convertToModelMessages, stepCountIs, streamText, type UIMessage } from 'ai';
import type { AgentWorkerEnv } from './types';
import { err, ok } from './response';
import { isInternalKeyValid } from './auth/internalKey';
import type { OpenclawApiConfig } from './openclawApi';
import {
  approvePairingRequest,
  approveRuntimeDevice,
  backupRuntime,
  buildApiConfig,
  createOpenclawInstance,
  getBillingSummary,
  getOpenclawInstance,
  listPairingRequests,
  getRuntimeDevices,
  getRuntimeStatus,
  getSessionHistory,
  listSessions,
  restartRuntime,
  sendSessionMessage,
} from './openclawApi';

type ApprovalDecision = 'approved' | 'rejected';
type ApprovalStatus = 'pending' | ApprovalDecision;

type StoredApproval = {
  id: string;
  createdAtMs: number;
  status: ApprovalStatus;
  resolvedAtMs?: number;
  summary?: string;
  toolName?: string;
  toolInput?: unknown;
};

type ApprovalGateResult = {
  status: 'approval_required' | 'approval_rejected' | 'approval_pending';
  approvalId: string;
  summary?: string;
  toolName?: string;
  toolInput?: unknown;
};

type ThreadStateV1 = {
  version: 1;
  ownerUserId: string;
  createdAtMs: number;
  updatedAtMs: number;
  running: boolean;
  messages: UIMessage[];
  approvals: Record<string, StoredApproval>;
  lastUserMessageId?: string;
};

type ChatRequestBody = {
  threadId?: string;
  input?: string;
  messages?: UIMessage[];
};

type ApprovalRespondBody = {
  threadId?: string;
  approvalId: string;
  decision: ApprovalDecision;
};

const defaultApprovalSummary = (toolName: string, toolInput: unknown): string => {
  if (toolName === 'openclaw_provision') {
    return 'Provision a managed OpenClaw instance.';
  }
  if (toolName === 'openclaw_restart') {
    return 'Restart the OpenClaw gateway.';
  }
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

function jsonError(status: number, code: string, message: string, details?: Record<string, unknown> | null) {
  return new Response(JSON.stringify(err(code, message, details)), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function extractTextFromUiMessage(message: UIMessage): string {
  let text = '';
  for (const part of message.parts ?? []) {
    if (part && typeof part === 'object' && 'type' in part && (part as any).type === 'text') {
      const t = (part as any).text;
      if (typeof t === 'string') text += t;
    }
  }
  return text.trim();
}

function getLastUserMessage(messages: UIMessage[] | undefined): { id?: string; text?: string } {
  if (!messages || messages.length === 0) return {};
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = messages[i];
    if (msg?.role === 'user') {
      const text = extractTextFromUiMessage(msg);
      return {
        id: typeof msg.id === 'string' && msg.id.trim().length > 0 ? msg.id : undefined,
        text: text.length > 0 ? text : undefined,
      };
    }
  }
  return {};
}

function clampMessages(messages: UIMessage[], maxMessages: number): UIMessage[] {
  if (messages.length <= maxMessages) return messages;
  return messages.slice(messages.length - maxMessages);
}

function normalizeThreadId(threadId: string): string {
  return threadId.trim();
}

function normalizeUserId(userId: string): string {
  return userId.trim();
}

async function readJson<T>(request: Request): Promise<T | null> {
  try {
    return (await request.json()) as T;
  } catch {
    return null;
  }
}

export class ThreadAgent {
  private state: any;
  private env: AgentWorkerEnv;

  constructor(state: any, env: AgentWorkerEnv) {
    this.state = state;
    this.env = env;
  }

  async fetch(request: Request): Promise<Response> {
    if (!isInternalKeyValid(request.headers, this.env.OA_INTERNAL_KEY)) {
      return jsonError(401, 'unauthorized', 'unauthorized');
    }

    const userIdHeader = request.headers.get('x-oa-user-id');
    if (!userIdHeader) {
      return jsonError(400, 'bad_request', 'missing x-oa-user-id');
    }
    const userId = normalizeUserId(userIdHeader);
    if (!userId) {
      return jsonError(400, 'bad_request', 'invalid x-oa-user-id');
    }

    const url = new URL(request.url);
    if (url.pathname === '/internal/chat' && request.method === 'POST') {
      return this.handleChat(request, userId);
    }
    if (url.pathname === '/internal/approval/respond' && request.method === 'POST') {
      return this.handleApprovalRespond(request, userId);
    }

    return jsonError(404, 'not_found', 'not found');
  }

  private async loadState(userId: string): Promise<ThreadStateV1> {
    const existing = (await this.state.storage.get('state')) as ThreadStateV1 | undefined;
    if (existing) {
      if (existing.ownerUserId !== userId) {
        throw new Error('forbidden');
      }
      return existing;
    }

    const now = Date.now();
    return {
      version: 1,
      ownerUserId: userId,
      createdAtMs: now,
      updatedAtMs: now,
      running: false,
      messages: [],
      approvals: {},
      lastUserMessageId: undefined,
    };
  }

  private async saveState(next: ThreadStateV1): Promise<void> {
    await this.state.storage.put('state', next);
  }

  private async requireApproval<T>({
    userId,
    toolName,
    toolInput,
    approvalId,
    summary,
    action,
  }: {
    userId: string;
    toolName: string;
    toolInput: unknown;
    approvalId?: string;
    summary?: string;
    action: () => Promise<T>;
  }): Promise<T | ApprovalGateResult> {
    let decision: ApprovalStatus = 'pending';
    let resolvedId = '';
    let resolvedSummary = summary ?? defaultApprovalSummary(toolName, toolInput);
    let usedExisting = false;

    await this.state.blockConcurrencyWhile(async () => {
      const state = await this.loadState(userId);
      let approval: StoredApproval | undefined;

      if (approvalId) {
        approval = state.approvals[approvalId];
        usedExisting = Boolean(approval);
      }

      if (!approval) {
        const id = crypto.randomUUID();
        approval = {
          id,
          createdAtMs: Date.now(),
          status: 'pending',
          summary: resolvedSummary,
          toolName,
          toolInput,
        };
        state.approvals[id] = approval;
      }

      decision = approval.status;
      resolvedId = approval.id;
      resolvedSummary = approval.summary ?? resolvedSummary;
      state.updatedAtMs = Date.now();
      await this.saveState(state);
    });

    if ((decision as ApprovalDecision) === 'approved') {
      return action();
    }

    if ((decision as ApprovalDecision) === 'rejected') {
      return {
        status: 'approval_rejected',
        approvalId: resolvedId,
        summary: resolvedSummary,
        toolName,
        toolInput,
      };
    }

    return {
      status: usedExisting ? 'approval_pending' : 'approval_required',
      approvalId: resolvedId,
      summary: resolvedSummary,
      toolName,
      toolInput,
    };
  }

  private async handleChat(request: Request, userId: string): Promise<Response> {
    const body = await readJson<ChatRequestBody>(request);
    if (!body) {
      return jsonError(400, 'bad_request', 'invalid json body');
    }

    const input = (body.input ?? '').trim();
    const lastUser = getLastUserMessage(body.messages);
    const userText = input.length > 0 ? input : lastUser.text;
    if (!userText) {
      return jsonError(400, 'bad_request', 'missing user input');
    }
    const incomingMessageId = lastUser.id;

    const openai = createOpenAI({
      apiKey: this.env.OPENAI_API_KEY,
      baseURL: this.env.OPENAI_BASE_URL,
    });

    let apiConfig: OpenclawApiConfig;
    try {
      apiConfig = buildApiConfig(this.env, userId);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'OpenClaw API base not configured';
      return jsonError(500, 'config_error', message);
    }

    let conversationMessages: UIMessage[] = [];

    try {
      await this.state.blockConcurrencyWhile(async () => {
        const state = await this.loadState(userId);
        if (state.running) {
          throw new Error('already_running');
        }

        const now = Date.now();

        const last = state.messages[state.messages.length - 1];
        const lastUserText = last?.role === 'user' ? extractTextFromUiMessage(last) : null;
        const shouldAppend =
          incomingMessageId != null
            ? incomingMessageId !== state.lastUserMessageId
            : lastUserText !== userText;

        if (shouldAppend) {
          state.messages.push({
            id: crypto.randomUUID(),
            role: 'user',
            parts: [{ type: 'text', text: userText }],
          });
          if (incomingMessageId) {
            state.lastUserMessageId = incomingMessageId;
          }
        }

        state.running = true;
        state.updatedAtMs = now;
        state.messages = clampMessages(state.messages, 80);

        await this.saveState(state);
        conversationMessages = state.messages;
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'unknown error';
      if (message === 'forbidden') return jsonError(403, 'forbidden', 'forbidden');
      if (message === 'already_running') return jsonError(409, 'conflict', 'thread already running');
      return jsonError(500, 'internal_error', message);
    }

    const result = streamText({
      model: openai.responses('gpt-4o-mini'),
      system: [
        'You are OpenAgents. Be concise.',
        'Sensitive actions (provisioning, pairing approvals, restarts) require explicit human approval.',
        'If a tool response includes status approval_required, ask the user to approve or reject.',
        'After approval, call the same tool again with the provided approvalId to continue.',
      ].join(' '),
      messages: await convertToModelMessages(conversationMessages),
      stopWhen: stepCountIs(10),
      tools: {
        openclaw_get_instance: {
          description: "Get the current user's OpenClaw instance summary (or null if none).",
          inputSchema: jsonSchema({ type: 'object', properties: {} }),
          execute: async () => getOpenclawInstance(apiConfig),
        },
        openclaw_provision: {
          description: 'Provision an OpenClaw instance for the current user.',
          inputSchema: jsonSchema({
            type: 'object',
            properties: { approvalId: { type: 'string', minLength: 1 } },
          }),
          execute: async ({ approvalId }: { approvalId?: string }) =>
            this.requireApproval({
              userId,
              toolName: 'openclaw_provision',
              toolInput: {},
              approvalId,
              action: () => createOpenclawInstance(apiConfig),
            }),
        },
        openclaw_get_status: {
          description: "Get runtime status for the current user's OpenClaw instance.",
          inputSchema: jsonSchema({ type: 'object', properties: {} }),
          execute: async () => getRuntimeStatus(apiConfig),
        },
        openclaw_list_devices: {
          description: "List pending and paired devices for the current user's OpenClaw instance.",
          inputSchema: jsonSchema({ type: 'object', properties: {} }),
          execute: async () => getRuntimeDevices(apiConfig),
        },
        openclaw_approve_device: {
          description: 'Approve a pending device by requestId.',
          inputSchema: jsonSchema({
            type: 'object',
            properties: {
              requestId: { type: 'string', minLength: 1 },
              approvalId: { type: 'string', minLength: 1 },
            },
            required: ['requestId'],
          }),
          execute: async ({ requestId, approvalId }: { requestId: string; approvalId?: string }) =>
            this.requireApproval({
              userId,
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
            this.requireApproval({
              userId,
              toolName: 'openclaw_approve_pairing',
              toolInput: { channel, code, notify },
              approvalId,
              action: () => approvePairingRequest(apiConfig, { channel, code, notify }),
            }),
        },
        openclaw_backup_now: {
          description: "Trigger a backup/sync for the current user's OpenClaw instance.",
          inputSchema: jsonSchema({ type: 'object', properties: {} }),
          execute: async () => backupRuntime(apiConfig),
        },
        openclaw_restart: {
          description: 'Restart the OpenClaw gateway process.',
          inputSchema: jsonSchema({
            type: 'object',
            properties: { approvalId: { type: 'string', minLength: 1 } },
          }),
          execute: async ({ approvalId }: { approvalId?: string }) =>
            this.requireApproval({
              userId,
              toolName: 'openclaw_restart',
              toolInput: {},
              approvalId,
              action: () => restartRuntime(apiConfig),
            }),
        },
        openclaw_get_billing_summary: {
          description: 'Get current credit balance summary.',
          inputSchema: jsonSchema({ type: 'object', properties: {} }),
          execute: async () => getBillingSummary(apiConfig),
        },
        openclaw_list_sessions: {
          description: 'List OpenClaw sessions with optional filters.',
          inputSchema: jsonSchema({
            type: 'object',
            properties: {
              kinds: { type: 'array', items: { type: 'string' } },
              limit: { type: 'number', minimum: 1 },
              activeMinutes: { type: 'number', minimum: 1 },
              messageLimit: { type: 'number', minimum: 0 },
            },
          }),
          execute: async (args: {
            kinds?: string[];
            limit?: number;
            activeMinutes?: number;
            messageLimit?: number;
          }) => listSessions(apiConfig, args),
        },
        openclaw_get_session_history: {
          description: 'Fetch history for a specific OpenClaw session.',
          inputSchema: jsonSchema({
            type: 'object',
            properties: {
              sessionKey: { type: 'string', minLength: 1 },
              limit: { type: 'number', minimum: 1 },
              includeTools: { type: 'boolean' },
            },
            required: ['sessionKey'],
          }),
          execute: async (args: { sessionKey: string; limit?: number; includeTools?: boolean }) =>
            getSessionHistory(apiConfig, args),
        },
        openclaw_send_session_message: {
          description: 'Send a message into another OpenClaw session.',
          inputSchema: jsonSchema({
            type: 'object',
            properties: {
              sessionKey: { type: 'string', minLength: 1 },
              message: { type: 'string', minLength: 1 },
              timeoutSeconds: { type: 'number', minimum: 0 },
            },
            required: ['sessionKey', 'message'],
          }),
          execute: async (args: { sessionKey: string; message: string; timeoutSeconds?: number }) =>
            sendSessionMessage(apiConfig, args),
        },
      } as any,
      onFinish: ({ text }) => {
        this.state.waitUntil(
          (async () => {
            await this.state.blockConcurrencyWhile(async () => {
              const state = await this.loadState(userId);
              state.messages.push({
                id: crypto.randomUUID(),
                role: 'assistant',
                parts: [{ type: 'text', text }],
              });
              state.running = false;
              state.updatedAtMs = Date.now();
              state.messages = clampMessages(state.messages, 80);
              await this.saveState(state);
            });
          })(),
        );
      },
      onError: ({ error }) => {
        this.state.waitUntil(
          (async () => {
            console.log('streamText error:', error);
            await this.state.blockConcurrencyWhile(async () => {
              const state = await this.loadState(userId);
              state.running = false;
              state.updatedAtMs = Date.now();
              await this.saveState(state);
            });
          })(),
        );
      },
      onAbort: () => {
        this.state.waitUntil(
          (async () => {
            await this.state.blockConcurrencyWhile(async () => {
              const state = await this.loadState(userId);
              state.running = false;
              state.updatedAtMs = Date.now();
              await this.saveState(state);
            });
          })(),
        );
      },
    });

    return result.toUIMessageStreamResponse();
  }

  private async handleApprovalRespond(request: Request, userId: string): Promise<Response> {
    const body = await readJson<ApprovalRespondBody>(request);
    if (!body) {
      return jsonError(400, 'bad_request', 'invalid json body');
    }

    const approvalId = body.approvalId?.trim();
    if (!approvalId) {
      return jsonError(400, 'bad_request', 'missing approvalId');
    }
    if (body.decision !== 'approved' && body.decision !== 'rejected') {
      return jsonError(400, 'bad_request', 'invalid decision');
    }

    try {
      await this.state.blockConcurrencyWhile(async () => {
        const state = await this.loadState(userId);
        const approval = state.approvals[approvalId];
        if (!approval) {
          throw new Error('not_found');
        }
        approval.status = body.decision;
        approval.resolvedAtMs = Date.now();
        state.updatedAtMs = Date.now();
        await this.saveState(state);
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'unknown error';
      if (message === 'forbidden') return jsonError(403, 'forbidden', 'forbidden');
      if (message === 'not_found') return jsonError(404, 'not_found', 'approval not found');
      return jsonError(500, 'internal_error', message);
    }

    return new Response(JSON.stringify(ok({ approvalId, decision: body.decision })), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }
}
