import { Context, Effect, Layer, SubscriptionRef } from 'effect';
import { AgentClient } from 'agents/client';
import { MessageType } from '@cloudflare/ai-chat/types';
import { AgentApiService } from './agentApi';
import { TelemetryService } from './telemetry';

import type { ChatStatus, UIMessage } from 'ai';

export type ChatSnapshot = {
  readonly messages: ReadonlyArray<UIMessage>;
  readonly status: ChatStatus;
  readonly errorText: string | null;
};

type ActiveStream = {
  readonly id: string;
  readonly messageId: string;
  parts: Array<any>;
};

type ChatSession = {
  readonly chatId: string;
  readonly agent: AgentClient | null;
  readonly agentUrlString: string | null;
  readonly state: SubscriptionRef.SubscriptionRef<ChatSnapshot>;
  readonly localRequestIds: Set<string>;
  messages: Array<UIMessage>;
  manualStatus: ChatStatus | null;
  manualErrorText: string | null;
  activeStream: ActiveStream | null;
  dispose: () => void;
};

export type ChatClient = {
  readonly open: (chatId: string) => Effect.Effect<SubscriptionRef.SubscriptionRef<ChatSnapshot>>;
  readonly send: (chatId: string, text: string) => Effect.Effect<void, Error>;
  readonly stop: (chatId: string) => Effect.Effect<void>;
  readonly clearHistory: (chatId: string) => Effect.Effect<void>;
  readonly setMessages: (chatId: string, messages: ReadonlyArray<UIMessage>) => Effect.Effect<void>;
};

export class ChatService extends Context.Tag('@openagents/web/ChatService')<ChatService, ChatClient>() {}

const initialSnapshot = (): ChatSnapshot => ({
  messages: [],
  status: 'ready',
  errorText: null,
});

function randomId(size = 8): string {
  let out = '';
  while (out.length < size) out += Math.random().toString(36).slice(2);
  return out.slice(0, size);
}

function safeJsonParse(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function cloneParts(parts: unknown): Array<any> {
  return Array.isArray(parts) ? [...parts] : [];
}

function applyMessageUpdated(prevMessages: ReadonlyArray<UIMessage>, updatedMessage: UIMessage): Array<UIMessage> {
  let idx = prevMessages.findIndex((m) => m.id === updatedMessage.id);

  if (idx < 0) {
    const updatedToolCallIds = new Set(
      cloneParts((updatedMessage as any).parts)
        .filter((p) => p && typeof p === 'object' && 'toolCallId' in p && p.toolCallId)
        .map((p) => String(p.toolCallId)),
    );
    if (updatedToolCallIds.size > 0) {
      idx = prevMessages.findIndex((m) =>
        cloneParts((m as any).parts).some(
          (p) => p && typeof p === 'object' && 'toolCallId' in p && updatedToolCallIds.has(String(p.toolCallId)),
        ),
      );
    }
  }

  if (idx < 0 && (updatedMessage as any).role === 'assistant') {
    // Last-resort reconciliation: when we locally stream a synthetic assistant message,
    // the server may later broadcast the canonical message with a different id.
    // Prefer replacing the latest assistant message over appending duplicates.
    for (let i = prevMessages.length - 1; i >= 0; i--) {
      const m = prevMessages[i] as any;
      if (m && m.role === 'assistant') {
        idx = i;
        break;
      }
    }
  }

  if (idx >= 0) {
    const updated = [...prevMessages];
    // Preserve the existing message id to avoid key churn when server updates a
    // message that was locally streamed with a synthetic id.
    updated[idx] = { ...updatedMessage, id: prevMessages[idx].id };
    return updated;
  }

  return [...prevMessages, updatedMessage];
}

function applyRemoteChunk(active: ActiveStream, chunkData: any): ActiveStream {
  switch (chunkData?.type) {
    case 'text-start': {
      active.parts.push({ type: 'text', text: '', state: 'streaming' });
      return active;
    }
    case 'text-delta': {
      const lastTextPart = [...active.parts].reverse().find((p) => p?.type === 'text');
      if (lastTextPart && lastTextPart.type === 'text') {
        lastTextPart.text += String(chunkData.delta ?? '');
      } else {
        active.parts.push({ type: 'text', text: String(chunkData.delta ?? '') });
      }
      return active;
    }
    case 'text-end': {
      const lastTextPart = [...active.parts].reverse().find((p) => p?.type === 'text');
      if (lastTextPart && 'state' in lastTextPart) lastTextPart.state = 'done';
      return active;
    }
    case 'reasoning-start': {
      active.parts.push({ type: 'reasoning', text: '', state: 'streaming' });
      return active;
    }
    case 'reasoning-delta': {
      const lastReasoningPart = [...active.parts].reverse().find((p) => p?.type === 'reasoning');
      if (lastReasoningPart && lastReasoningPart.type === 'reasoning') {
        lastReasoningPart.text += String(chunkData.delta ?? '');
      } else {
        active.parts.push({ type: 'reasoning', text: String(chunkData.delta ?? '') });
      }
      return active;
    }
    case 'reasoning-end': {
      const lastReasoningPart = [...active.parts].reverse().find((p) => p?.type === 'reasoning');
      if (lastReasoningPart && 'state' in lastReasoningPart) lastReasoningPart.state = 'done';
      return active;
    }
    case 'file': {
      active.parts.push({ type: 'file', mediaType: chunkData.mediaType, url: chunkData.url });
      return active;
    }
    case 'source-url': {
      active.parts.push({
        type: 'source-url',
        sourceId: chunkData.sourceId,
        url: chunkData.url,
        title: chunkData.title,
      });
      return active;
    }
    case 'source-document': {
      active.parts.push({
        type: 'source-document',
        sourceId: chunkData.sourceId,
        mediaType: chunkData.mediaType,
        title: chunkData.title,
        filename: chunkData.filename,
      });
      return active;
    }
    case 'tool-input-available': {
      active.parts.push({
        type: `tool-${String(chunkData.toolName ?? 'tool')}`,
        toolCallId: String(chunkData.toolCallId ?? ''),
        toolName: chunkData.toolName,
        state: 'input-available',
        input: chunkData.input,
      });
      return active;
    }
    case 'tool-output-available': {
      active.parts = active.parts.map((p) => {
        if (p && typeof p === 'object' && 'toolCallId' in p && p.toolCallId === chunkData.toolCallId && 'state' in p) {
          return { ...p, state: 'output-available', output: chunkData.output };
        }
        return p;
      });
      return active;
    }
    case 'step-start': {
      active.parts.push({ type: 'step-start' });
      return active;
    }
    default:
      return active;
  }
}

function toChatSnapshot(session: ChatSession): ChatSnapshot {
  const status: ChatStatus =
    session.manualStatus ?? (session.activeStream ? 'streaming' : 'ready');
  return {
    messages: session.messages,
    status,
    errorText: session.manualErrorText,
  };
}

function updateSnapshot(session: ChatSession): void {
  // SubscriptionRef is pure (no Env). Safe to run on the default runtime.
  Effect.runFork(SubscriptionRef.set(session.state, toChatSnapshot(session)));
}

export const ChatServiceLive = Layer.effect(
  ChatService,
  Effect.gen(function* () {
    const api = yield* AgentApiService;
    const telemetry = yield* TelemetryService;
    const sessions = new Map<string, ChatSession>();

    const open = Effect.fn('ChatService.open')(function* (chatId: string) {
      const existing = sessions.get(chatId);
      if (existing) return existing.state;

      // SSR safety: do not attempt to create WebSockets during server render.
      if (typeof window === 'undefined') {
        const state = yield* SubscriptionRef.make(initialSnapshot());
        sessions.set(chatId, {
          chatId,
          agent: null,
          agentUrlString: null,
          state,
          localRequestIds: new Set(),
          messages: [],
          manualStatus: null,
          manualErrorText: null,
          activeStream: null,
          dispose: () => {},
        });
        return state;
      }

      const initialMessages = yield* api.getMessages(chatId).pipe(Effect.catchAll(() => Effect.succeed([])));

      // Dev-only: Vite's HTTP proxy handles `/agents/**` fetches, but WebSocket proxying is unreliable
      // under TanStack Start + Cloudflare Vite plugin. Connect the AgentClient directly to the worker
      // port to prevent reconnect loops (seen as repeated 101s in the network panel).
      const isDev = Boolean((import.meta as any).env?.DEV);
      const shouldBypassViteWsProxy = isDev && window.location.port === '3000';
      const workerHost = shouldBypassViteWsProxy ? `${window.location.hostname}:8787` : window.location.host;
      const workerProtocol = shouldBypassViteWsProxy
        ? 'ws'
        : window.location.protocol === 'https:'
          ? 'wss'
          : 'ws';

      const agent = new AgentClient({
        agent: 'chat',
        name: chatId,
        host: workerHost,
        protocol: workerProtocol,
      });
      const agentOrigin = shouldBypassViteWsProxy ? `http://${workerHost}` : window.location.origin;
      const agentUrlString = new URL(`/agents/chat/${chatId}`, agentOrigin).toString();

      const state = yield* SubscriptionRef.make<ChatSnapshot>({
        messages: initialMessages,
        status: 'ready',
        errorText: null,
      });

      const session: ChatSession = {
        chatId,
        agent,
        agentUrlString,
        state,
        localRequestIds: new Set(),
        messages: [...initialMessages],
        manualStatus: null,
        manualErrorText: null,
        activeStream: null,
        dispose: () => {},
      };

      const onChatResponseChunk = (parsed: any, isLocal: boolean) => {
        const id = String(parsed.id ?? '');
        if (!id) return;

        const isContinuation = parsed.continuation === true;
        const done = Boolean(parsed.done);
        const errored = Boolean(parsed.error);

        // For local streams, the activeStream is always created by `send()`.
        // For remote/broadcast streams, we reconstruct state here.
        if (!session.activeStream || session.activeStream.id !== id) {
          let messageId = randomId(16);
          let existingParts: Array<any> = [];

          if (!isLocal && isContinuation) {
            for (let i = session.messages.length - 1; i >= 0; i--) {
              const m = session.messages[i] as any;
              if (m && m.role === 'assistant') {
                messageId = m.id;
                existingParts = cloneParts(m.parts);
                break;
              }
            }
          }

          session.activeStream = { id, messageId, parts: existingParts };
          session.manualStatus = 'streaming';
          session.manualErrorText = null;
        }

        const active = session.activeStream;

        if (String(parsed.body ?? '').trim()) {
          const chunkData = safeJsonParse(String(parsed.body ?? '')) as any;
          if (chunkData) {
            try {
              applyRemoteChunk(active, chunkData);

              const prev = session.messages;
              const existingIdx = prev.findIndex((m) => m.id === active.messageId);
              const partialMessage = {
                id: active.messageId,
                role: 'assistant',
                parts: [...active.parts],
              } as UIMessage;

              if (existingIdx >= 0) {
                const next = [...prev];
                next[existingIdx] = partialMessage;
                session.messages = next;
              } else {
                session.messages = [...prev, partialMessage];
              }
            } catch (err) {
              console.warn(
                '[ChatService] Failed to apply stream chunk:',
                err instanceof Error ? err.message : String(err),
                'body:',
                String(parsed.body ?? '').slice(0, 100),
              );
            }
          }
        }

        if (done || errored) {
          if (errored) {
            session.manualStatus = 'error';
            session.manualErrorText = typeof parsed.body === 'string' ? parsed.body : 'Chat stream failed.';
          } else {
            session.manualStatus = null;
            session.manualErrorText = null;
          }

          session.localRequestIds.delete(id);
          session.activeStream = null;
        }

        updateSnapshot(session);
      };

      const onAgentMessage = (event: MessageEvent) => {
        if (typeof (event as any).data !== 'string') return;
        const parsed = safeJsonParse((event as any).data) as any;
        if (!parsed || typeof parsed !== 'object') return;

        switch (parsed.type) {
          case MessageType.CF_AGENT_CHAT_CLEAR: {
            session.manualStatus = null;
            session.manualErrorText = null;
            session.activeStream = null;
            session.messages = [];
            updateSnapshot(session);
            return;
          }
          case MessageType.CF_AGENT_CHAT_MESSAGES: {
            const msgs = Array.isArray(parsed.messages) ? (parsed.messages as Array<UIMessage>) : [];
            session.manualStatus = null;
            session.manualErrorText = null;
            session.activeStream = null;
            session.messages = msgs;
            updateSnapshot(session);
            return;
          }
          case MessageType.CF_AGENT_MESSAGE_UPDATED: {
            const msg: unknown = parsed.message;
            if (!msg || typeof msg !== "object") return;
            session.messages = applyMessageUpdated(session.messages, msg as UIMessage);
            updateSnapshot(session);
            return;
          }
          case MessageType.CF_AGENT_STREAM_RESUMING: {
            const id = String(parsed.id ?? '');
            if (!id) return;

            session.activeStream = { id, messageId: randomId(16), parts: [] };
            session.manualStatus = 'streaming';
            updateSnapshot(session);

            try {
              agent.send(JSON.stringify({ type: MessageType.CF_AGENT_STREAM_RESUME_ACK, id }));
            } catch {
              // ignore
            }
            return;
          }
          case MessageType.CF_AGENT_USE_CHAT_RESPONSE: {
            const id = String(parsed.id ?? '');
            if (!id) return;

            const isLocal = session.localRequestIds.has(id);
            onChatResponseChunk(parsed, isLocal);
            return;
          }
          default:
            return;
        }
      };

      agent.addEventListener('message', onAgentMessage);

      session.dispose = () => {
        try {
          agent.removeEventListener('message', onAgentMessage);
        } catch {
          // ignore
        }
        try {
          agent.close();
        } catch {
          // ignore
        }
      };

      sessions.set(chatId, session);
      updateSnapshot(session);

      yield* telemetry.withNamespace('chat.service').event('chat.open', { chatId });

      return state;
    });

    const withSession = <TValue, TError>(
      chatId: string,
      f: (session: ChatSession) => Effect.Effect<TValue, TError>,
    ): Effect.Effect<TValue, TError> =>
      open(chatId).pipe(
        Effect.flatMap(() => {
          const session = sessions.get(chatId);
          if (!session || !session.agent || !session.agentUrlString) {
            return Effect.sync(() => {
              console.warn('[ChatService] Session missing after open()', { chatId });
              return undefined as unknown as TValue;
            });
          }
          return f(session);
        }),
      );

    const send = Effect.fn('ChatService.send')(function* (chatId: string, text: string) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          const userMsgId = randomId(16);
          const userMsg = {
            id: userMsgId,
            role: 'user',
            parts: [{ type: 'text', text }],
          } as UIMessage;

          // Request messages are the transcript up through the new user message.
          const requestMessages = [...session.messages, userMsg];
          session.messages = requestMessages;

          const requestId = randomId(16);
          session.localRequestIds.add(requestId);

          // Create a synthetic assistant message in the UI so the first paint
          // shows "streaming" immediately; server will later broadcast the
          // canonical message (we reconcile in applyMessageUpdated()).
          session.activeStream = {
            id: requestId,
            messageId: randomId(16),
            parts: [],
          };
          session.manualStatus = 'streaming';
          session.manualErrorText = null;

          updateSnapshot(session);

          try {
            session.agent!.send(
              JSON.stringify({
                id: requestId,
                type: MessageType.CF_AGENT_USE_CHAT_REQUEST,
                url: session.agentUrlString!,
                init: {
                  method: 'POST',
                  headers: { 'content-type': 'application/json' },
                  body: JSON.stringify({
                    id: chatId,
                    messages: requestMessages,
                    trigger: 'submit-message',
                    messageId: userMsgId,
                  }),
                },
              }),
            );
          } catch (err) {
            session.localRequestIds.delete(requestId);
            session.activeStream = null;
            session.manualStatus = 'error';
            session.manualErrorText = err instanceof Error ? err.message : String(err);
            updateSnapshot(session);
            throw err instanceof Error ? err : new Error(String(err));
          }
        }).pipe(
          Effect.tapError((err) =>
            telemetry.withNamespace('chat.service').log('error', 'chat.send_failed', { chatId, message: String(err) }),
          ),
        ),
      );
    });

    const stop = Effect.fn('ChatService.stop')(function* (chatId: string) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          const activeId = session.activeStream?.id;
          if (activeId) {
            try {
              session.agent!.send(JSON.stringify({ id: activeId, type: MessageType.CF_AGENT_CHAT_REQUEST_CANCEL }));
            } catch {
              // ignore
            }
          }
          session.localRequestIds.clear();
          session.activeStream = null;
          session.manualStatus = null;
          session.manualErrorText = null;
          updateSnapshot(session);
        }),
      );
    });

    const clearHistory = Effect.fn('ChatService.clearHistory')(function* (chatId: string) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          session.manualStatus = null;
          session.manualErrorText = null;
          session.activeStream = null;
          session.messages = [];
          try {
            session.agent!.send(JSON.stringify({ type: MessageType.CF_AGENT_CHAT_CLEAR }));
          } catch {
            // ignore
          }
          updateSnapshot(session);
        }),
      );
    });

    const setMessages = Effect.fn('ChatService.setMessages')(function* (chatId: string, messages: ReadonlyArray<UIMessage>) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          session.manualStatus = null;
          session.manualErrorText = null;
          session.activeStream = null;
          session.messages = [...messages];
          try {
            session.agent!.send(
              JSON.stringify({
                messages: Array.isArray(messages) ? messages : [],
                type: MessageType.CF_AGENT_CHAT_MESSAGES,
              }),
            );
          } catch {
            // ignore
          }
          updateSnapshot(session);
        }),
      );
    });

    return ChatService.of({ open, send, stop, clearHistory, setMessages });
  }),
);

