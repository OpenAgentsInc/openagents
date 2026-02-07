import { Context, Effect, Layer, SubscriptionRef } from 'effect';
import { AgentClient } from 'agents/client';
import { Chat } from '@ai-sdk/react';
import { DefaultChatTransport } from 'ai';
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
  readonly agent: AgentClient;
  readonly chat: Chat<UIMessage>;
  readonly state: SubscriptionRef.SubscriptionRef<ChatSnapshot>;
  readonly localRequestIds: Set<string>;
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

  if (idx >= 0) {
    const updated = [...prevMessages];
    // Preserve the existing message id to avoid React key churn when the server
    // updates a message that was locally streamed with a synthetic id.
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

function getRequestUrlString(request: RequestInfo | URL): string {
  if (request instanceof URL) return request.toString();
  if (typeof request === 'string') return request;
  if (request instanceof Request) return request.url;
  return String(request);
}

function toChatSnapshot(session: ChatSession): ChatSnapshot {
  return {
    messages: session.chat.messages,
    status: session.manualStatus ?? session.chat.status,
    errorText:
      session.manualErrorText ?? (session.chat.error ? session.chat.error.message : null),
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
        sessions.set(
          chatId,
          // Minimal stub session for SSR; nothing is connected.
          {
            chatId,
            agent: null as any,
            chat: null as any,
            state,
            localRequestIds: new Set(),
            manualStatus: null,
            manualErrorText: null,
            activeStream: null,
            dispose: () => {},
          },
        );
        return state;
      }

      const initialMessages = yield* api.getMessages(chatId).pipe(Effect.catchAll(() => Effect.succeed([])));

      // Dev-only: Vite's HTTP proxy handles `/agents/**` fetches, but WebSocket proxying is unreliable
      // under TanStack Start + Cloudflare Vite plugin. Connect the AgentClient directly to the worker
      // port to prevent reconnect loops (seen as repeated 101s in the network panel).
      const isDev = Boolean((import.meta as any).env?.DEV);
      const shouldBypassViteWsProxy = isDev && window.location.port === '3000';
      const workerHost = shouldBypassViteWsProxy
        ? `${window.location.hostname}:8787`
        : window.location.host;
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
      const agentOrigin = shouldBypassViteWsProxy
        ? `http://${workerHost}`
        : window.location.origin;
      const agentUrlString = new URL(`/agents/chat/${chatId}`, agentOrigin).toString();
      const localRequestIds = new Set<string>();
      const state = yield* SubscriptionRef.make<ChatSnapshot>({
        messages: initialMessages,
        status: 'ready',
        errorText: null,
      });

      const aiFetch = (request: RequestInfo | URL, init: RequestInit = {}): Promise<Response> => {
        const { method, keepalive, headers, body, redirect, integrity, signal, credentials, mode, referrer, referrerPolicy, window: win } =
          init;

        const id = randomId(8);
        const abortController = new AbortController();
        let controller: ReadableStreamDefaultController<Uint8Array> | undefined;

        localRequestIds.add(id);

        signal?.addEventListener('abort', () => {
          try {
            agent.send(JSON.stringify({ id, type: MessageType.CF_AGENT_CHAT_REQUEST_CANCEL }));
          } catch {
            // best-effort
          }
          abortController.abort();
          try {
            controller?.close();
          } catch {
            // ignore
          }
          localRequestIds.delete(id);
        });

        agent.addEventListener(
          'message',
          (event) => {
            if (typeof (event as any).data !== 'string') return;
            const parsed = safeJsonParse((event as any).data) as any;
            if (!parsed || parsed.type !== MessageType.CF_AGENT_USE_CHAT_RESPONSE) return;
            if (parsed.id !== id) return;

            if (parsed.error) {
              try {
                controller?.error(new Error(String(parsed.body ?? 'chat error')));
              } catch {
                // ignore
              }
              abortController.abort();
              localRequestIds.delete(id);
              return;
            }

            if (String(parsed.body ?? '').trim()) {
              controller?.enqueue(new TextEncoder().encode(`data: ${parsed.body}\n\n`));
            }

            if (parsed.done) {
              try {
                controller?.close();
              } catch {
                // ignore
              }
              abortController.abort();
              localRequestIds.delete(id);
            }
          },
          { signal: abortController.signal } as any,
        );

        const stream = new ReadableStream<Uint8Array>({
          start(c) {
            controller = c;
          },
          cancel(reason) {
            // Not fatal; this is mostly for debugging.
            console.warn('[ChatService] cancelling stream', id, reason ?? 'no reason');
          },
        });

        agent.send(
          JSON.stringify({
            id,
            init: {
              body,
              credentials,
              headers,
              integrity,
              keepalive,
              method,
              mode,
              redirect,
              referrer,
              referrerPolicy,
              window: win,
            },
            type: MessageType.CF_AGENT_USE_CHAT_REQUEST,
            url: getRequestUrlString(request),
          }),
        );

        return Promise.resolve(new Response(stream));
      };

      const transport = {
        sendMessages: (sendMessageOptions: Parameters<DefaultChatTransport<UIMessage>['sendMessages']>[0]) =>
          new DefaultChatTransport<UIMessage>({ api: agentUrlString, fetch: aiFetch as any }).sendMessages(sendMessageOptions),
        reconnectToStream: () => Promise.resolve(null),
      };

      const chat = new Chat<UIMessage>({
        messages: initialMessages,
        transport,
      });

      const session: ChatSession = {
        chatId,
        agent,
        chat,
        state,
        localRequestIds,
        manualStatus: null,
        manualErrorText: null,
        activeStream: null,
        dispose: () => {
          // Unregistering chat callbacks is handled below via closures.
        },
      };

      const unsubscribeMessages = chat['~registerMessagesCallback'](() => updateSnapshot(session));
      const unsubscribeStatus = chat['~registerStatusCallback'](() => updateSnapshot(session));
      const unsubscribeError = chat['~registerErrorCallback'](() => updateSnapshot(session));

      const onAgentMessage = (event: MessageEvent) => {
        if (typeof (event as any).data !== 'string') return;
        const parsed = safeJsonParse((event as any).data) as any;
        if (!parsed || typeof parsed !== 'object') return;

        switch (parsed.type) {
          case MessageType.CF_AGENT_CHAT_CLEAR: {
            session.manualStatus = null;
            session.manualErrorText = null;
            session.activeStream = null;
            chat.messages = [];
            updateSnapshot(session);
            return;
          }
          case MessageType.CF_AGENT_CHAT_MESSAGES: {
            const msgs = Array.isArray(parsed.messages) ? (parsed.messages as Array<UIMessage>) : [];
            session.manualStatus = null;
            session.manualErrorText = null;
            session.activeStream = null;
            chat.messages = msgs;
            updateSnapshot(session);
            return;
          }
          case MessageType.CF_AGENT_MESSAGE_UPDATED: {
            const msg: unknown = parsed.message;
            if (!msg || typeof msg !== 'object') return;
            chat.messages = applyMessageUpdated(chat.messages, msg as UIMessage);
            updateSnapshot(session);
            return;
          }
          case MessageType.CF_AGENT_STREAM_RESUMING: {
            const id = String(parsed.id ?? '');
            if (!id) return;

            session.activeStream = {
              id,
              messageId: randomId(16),
              parts: [],
            };
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
            if (session.localRequestIds.has(id)) return;

            // Remote/broadcast stream chunks (including resume).
            const isContinuation = parsed.continuation === true;
            if (!session.activeStream || session.activeStream.id !== id) {
              let messageId = randomId(16);
              let existingParts: Array<any> = [];
              if (isContinuation) {
                for (let i = chat.messages.length - 1; i >= 0; i--) {
                  const m = chat.messages[i];
                  if (m.role === 'assistant') {
                    messageId = m.id;
                    existingParts = cloneParts((m as any).parts);
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

                  const prev = chat.messages;
                  const existingIdx = prev.findIndex((m) => m.id === active.messageId);
                  const partialMessage = {
                    id: active.messageId,
                    role: 'assistant',
                    parts: [...active.parts],
                  } as UIMessage;

                  if (existingIdx >= 0) {
                    const next = [...prev];
                    next[existingIdx] = partialMessage;
                    chat.messages = next;
                  } else {
                    chat.messages = [...prev, partialMessage];
                  }
                } catch (err) {
                  console.warn(
                    '[ChatService] Failed to apply remote stream chunk:',
                    err instanceof Error ? err.message : String(err),
                    'body:',
                    String(parsed.body ?? '').slice(0, 100),
                  );
                }
              }
            }

            if (parsed.done || parsed.error) {
              if (parsed.error) {
                session.manualStatus = 'error';
                session.manualErrorText = typeof parsed.body === 'string' ? parsed.body : 'Chat stream failed.';
              } else {
                session.manualStatus = null;
                session.manualErrorText = null;
              }
              session.activeStream = null;
              updateSnapshot(session);
            }
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
          unsubscribeMessages();
        } catch {
          // ignore
        }
        try {
          unsubscribeStatus();
        } catch {
          // ignore
        }
        try {
          unsubscribeError();
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
          if (!session || !(session as any).agent || !(session as any).chat) {
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
        Effect.tryPromise({
          try: () => session.chat.sendMessage({ text }),
          catch: (err) => (err instanceof Error ? err : new Error(String(err))),
        }).pipe(
          Effect.tapError((err) =>
            telemetry.withNamespace('chat.service').log('error', 'chat.send_failed', {
              chatId,
              message: err.message,
            }),
          ),
        ),
      );
    });

    const stop = Effect.fn('ChatService.stop')(function* (chatId: string) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          // Stop local in-flight request (if any).
          session.chat.stop().catch(() => {});

          // Stop resumed/broadcast stream (if any).
          const activeId = session.activeStream?.id;
          if (activeId) {
            try {
              session.agent.send(JSON.stringify({ id: activeId, type: MessageType.CF_AGENT_CHAT_REQUEST_CANCEL }));
            } catch {
              // ignore
            }
            session.activeStream = null;
            session.manualStatus = null;
            session.manualErrorText = null;
            updateSnapshot(session);
          }
        }),
      );
    });

    const clearHistory = Effect.fn('ChatService.clearHistory')(function* (chatId: string) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          session.manualStatus = null;
          session.manualErrorText = null;
          session.activeStream = null;
          session.chat.messages = [];
          try {
            session.agent.send(JSON.stringify({ type: MessageType.CF_AGENT_CHAT_CLEAR }));
          } catch {
            // ignore
          }
          updateSnapshot(session);
        }),
      );
    });

    const setMessages = Effect.fn('ChatService.setMessages')(function* (
      chatId: string,
      messages: ReadonlyArray<UIMessage>,
    ) {
      yield* withSession(chatId, (session) =>
        Effect.sync(() => {
          session.manualStatus = null;
          session.manualErrorText = null;
          session.activeStream = null;
          session.chat.messages = [...messages];
          try {
            session.agent.send(
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

    return ChatService.of({
      open,
      send,
      stop,
      clearHistory,
      setMessages,
    });
  }),
);
