import { useEffect, useMemo, useRef } from 'react';
import { useChat, type UIMessage } from '@ai-sdk/react';
import type { ChatTransport } from 'ai';
import { useConvex } from 'convex/react';
import {
  AssistantRuntime,
  useAuiState,
  unstable_RemoteThreadListAdapter,
  unstable_useRemoteThreadListRuntime,
  type ThreadMessage,
} from '@assistant-ui/react';
import {
  AssistantChatTransport,
  useAISDKRuntime,
  type UseChatRuntimeOptions,
} from '@assistant-ui/react-ai-sdk';
import { createAssistantStream } from 'assistant-stream';
import { api } from '../../../convex/_generated/api';
import type { Id } from '../../../convex/_generated/dataModel';

type ThreadId = Id<'threads'>;

const DEFAULT_NEW_TITLE = 'New Chat';
const TITLE_LIMIT = 60;

const trimTitle = (value: string): string => {
  const trimmed = value.trim().replace(/\s+/g, ' ');
  if (!trimmed) return '';
  return trimmed.length > TITLE_LIMIT ? `${trimmed.slice(0, TITLE_LIMIT - 1)}…` : trimmed;
};

const deriveTitleFromMessages = (messages: readonly ThreadMessage[]): string => {
  for (const message of messages) {
    if (message.role !== 'user') continue;
    const text = message.content
      .filter((part) => part.type === 'text')
      .map((part) => part.text)
      .join(' ')
      .trim();
    if (text) return trimTitle(text);
  }
  return '';
};

const useDynamicChatTransport = <UI_MESSAGE extends UIMessage = UIMessage>(
  transport: ChatTransport<UI_MESSAGE>,
): ChatTransport<UI_MESSAGE> => {
  const transportRef = useRef<ChatTransport<UI_MESSAGE>>(transport);
  useEffect(() => {
    transportRef.current = transport;
  }, [transport]);
  return useMemo(
    () =>
      new Proxy(transportRef.current, {
        get(_, prop) {
          const res = transportRef.current[prop as keyof ChatTransport<UI_MESSAGE>];
          return typeof res === 'function' ? res.bind(transportRef.current) : res;
        },
      }),
    [],
  );
};

const useChatThreadRuntime = <UI_MESSAGE extends UIMessage = UIMessage>(
  options?: UseChatRuntimeOptions<UI_MESSAGE>,
): AssistantRuntime => {
  const { adapters, transport: transportOptions, toCreateMessage, ...chatOptions } = options ?? {};
  const transport = useDynamicChatTransport(
    transportOptions ?? new AssistantChatTransport(),
  );

  const id = useAuiState(({ threadListItem }) => threadListItem.id);
  const chat = useChat({
    ...chatOptions,
    id,
    transport,
  });

  const runtime = useAISDKRuntime(chat, {
    adapters,
    ...(toCreateMessage && { toCreateMessage }),
  });

  if (transport instanceof AssistantChatTransport) {
    transport.setRuntime(runtime);
  }

  return runtime;
};

const useConvexThreadListAdapter = (): unstable_RemoteThreadListAdapter => {
  const convex = useConvex();

  return useMemo(() => {
    const asThreadId = (value: string): ThreadId => value as ThreadId;

    return {
      list: async () => {
        const [regular, archived] = await Promise.all([
          convex.query(api.threads.list, { archived: false, limit: 200 }),
          convex.query(api.threads.list, { archived: true, limit: 200 }),
        ]);

        const toMetadata = (
          thread: (typeof regular)[number],
          status: 'regular' | 'archived',
        ) => ({
          status,
          remoteId: thread._id,
          title: thread.title,
          externalId: thread._id,
        });

        return {
          threads: [
            ...regular.map((thread) => toMetadata(thread, 'regular')),
            ...archived.map((thread) => toMetadata(thread, 'archived')),
          ],
        };
      },
      initialize: async (threadId) => {
        const remoteId = await convex.mutation(api.threads.create, {
          title: DEFAULT_NEW_TITLE,
          kind: 'chat',
        });
        return { remoteId, externalId: threadId };
      },
      rename: async (remoteId, newTitle) => {
        await convex.mutation(api.threads.updateTitle, {
          threadId: asThreadId(remoteId),
          title: newTitle,
        });
      },
      archive: async (remoteId) => {
        await convex.mutation(api.threads.archive, {
          threadId: asThreadId(remoteId),
          archived: true,
        });
      },
      unarchive: async (remoteId) => {
        await convex.mutation(api.threads.archive, {
          threadId: asThreadId(remoteId),
          archived: false,
        });
      },
      delete: async (remoteId) => {
        await convex.mutation(api.threads.archive, {
          threadId: asThreadId(remoteId),
          archived: true,
        });
      },
      fetch: async (threadId) => {
        const thread = await convex.query(api.threads.get, {
          threadId: asThreadId(threadId),
        });
        if (!thread) {
          throw new Error('Thread not found');
        }
        return {
          status: thread.archived ? 'archived' : 'regular',
          remoteId: thread._id,
          title: thread.title,
          externalId: thread._id,
        };
      },
      generateTitle: async (remoteId, messages) => {
        const title = deriveTitleFromMessages(messages);
        if (title) {
          await convex.mutation(api.threads.updateTitle, {
            threadId: asThreadId(remoteId),
            title,
          });
        }
        return createAssistantStream((controller) => {
          if (title) controller.appendText(title);
        });
      },
    };
  }, [convex]);
};

export const useOpenAgentsChatRuntime = <UI_MESSAGE extends UIMessage = UIMessage>(
  options?: UseChatRuntimeOptions<UI_MESSAGE>,
) => {
  const adapter = useConvexThreadListAdapter();

  return unstable_useRemoteThreadListRuntime({
    runtimeHook: () => useChatThreadRuntime(options),
    adapter,
    allowNesting: true,
  });
};
