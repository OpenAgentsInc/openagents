import { createOpenAI } from '@ai-sdk/openai';
import { convertToModelMessages, streamText, type UIMessage } from 'ai';
import type { AgentWorkerEnv } from './types';
import { err, ok } from './response';
import { isInternalKeyValid } from './auth/internalKey';

type ApprovalDecision = 'approved' | 'rejected';

type StoredApproval = {
  id: string;
  createdAtMs: number;
  status: 'pending' | ApprovalDecision;
  resolvedAtMs?: number;
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

function getLastUserText(messages: UIMessage[] | undefined): string | null {
  if (!messages || messages.length === 0) return null;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = messages[i];
    if (msg?.role === 'user') {
      const text = extractTextFromUiMessage(msg);
      if (text.length > 0) return text;
      return null;
    }
  }
  return null;
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
    };
  }

  private async saveState(next: ThreadStateV1): Promise<void> {
    await this.state.storage.put('state', next);
  }

  private async handleChat(request: Request, userId: string): Promise<Response> {
    const body = await readJson<ChatRequestBody>(request);
    if (!body) {
      return jsonError(400, 'bad_request', 'invalid json body');
    }

    const input = (body.input ?? '').trim();
    const userText = input.length > 0 ? input : getLastUserText(body.messages);
    if (!userText) {
      return jsonError(400, 'bad_request', 'missing user input');
    }

    const openai = createOpenAI({
      apiKey: this.env.OPENAI_API_KEY,
      baseURL: this.env.OPENAI_BASE_URL,
    });

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
        if (lastUserText !== userText) {
          state.messages.push({
            id: crypto.randomUUID(),
            role: 'user',
            parts: [{ type: 'text', text: userText }],
          });
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
      system:
        'You are OpenAgents. Be concise. If you need to take a sensitive action, ask for approval first.',
      messages: await convertToModelMessages(conversationMessages),
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
