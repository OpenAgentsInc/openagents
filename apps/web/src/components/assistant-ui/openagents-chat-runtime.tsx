import { useMemo } from 'react';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { useConvex } from 'convex/react';
import {
  AssistantRuntime,
  useAuiState,
  unstable_RemoteThreadListAdapter,
  unstable_useRemoteThreadListRuntime,
  type ThreadMessage,
} from '@assistant-ui/react';
import {
  useAISDKRuntime,
  type UseChatRuntimeOptions,
} from '@assistant-ui/react-ai-sdk';
import { createAssistantStream } from 'assistant-stream';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import type { Id } from '../../../convex/_generated/dataModel';
import type { UIMessage } from '@ai-sdk/react';

type ThreadId = Id<'threads'>;

const LOCAL_THREAD_ID_PREFIX = '__LOCALID_';

function isLocalThreadId(id: string | undefined): boolean {
  return typeof id === 'string' && id.startsWith(LOCAL_THREAD_ID_PREFIX);
}

const TITLE_LIMIT = 60;

const trimTitle = (value: string): string => {
  const trimmed = value.trim().replace(/\s+/g, ' ');
  if (!trimmed) return '';
  return trimmed.length > TITLE_LIMIT
    ? `${trimmed.slice(0, TITLE_LIMIT - 1)}…`
    : trimmed;
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

const useChatThreadRuntime = <UI_MESSAGE extends UIMessage = UIMessage>(
  options?: UseChatRuntimeOptions<UI_MESSAGE>,
): AssistantRuntime => {
  const { adapters, toCreateMessage, ...chatOptions } = options ?? {};
  const threadState = useAuiState((state) => state.threadListItem);
  const isPending = threadState.status === 'new' || isLocalThreadId(threadState.id);
  const agentName =
    isPending ? 'pending' : threadState.remoteId ?? threadState.id;

  const agent = useAgent({
    agent: 'chat',
    name: agentName,
    startClosed: isPending,
  });

  const chat = useAgentChat({
    agent,
    ...(chatOptions as Omit<typeof chatOptions, 'onToolCall'>),
  }) as unknown as ReturnType<
    typeof import('@ai-sdk/react').useChat<UI_MESSAGE>
  >;

  return useAISDKRuntime(chat, {
    adapters,
    ...(toCreateMessage && { toCreateMessage }),
  });
};

const useConvexThreadListAdapter = (
  isAuthenticated: boolean,
): unstable_RemoteThreadListAdapter => {
  const convex = useConvex();

  return useMemo(() => {
    const asThreadId = (value: string): ThreadId => value as ThreadId;

    const ensureLiteclawThread = async () => {
      if (!isAuthenticated) return;
      try {
        await convex.mutation(api.threads.getOrCreateLiteclawThread, {});
      } catch {
        // Ignore failures (e.g. session expired); list will return empty.
      }
    };

    return {
      list: async () => {
        await ensureLiteclawThread();
        if (!isAuthenticated) {
          return { threads: [] };
        }
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
            ...regular
              .filter((thread) => thread.kind === 'liteclaw')
              .map((thread) => toMetadata(thread, 'regular')),
            ...archived
              .filter((thread) => thread.kind === 'liteclaw')
              .map((thread) => toMetadata(thread, 'archived')),
          ],
        };
      },
      initialize: async (threadId) => {
        if (!isAuthenticated) {
          throw new Error('Sign in to create a thread');
        }
        const remoteId = await convex.mutation(
          api.threads.getOrCreateLiteclawThread,
          {},
        );
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
        if (isLocalThreadId(threadId)) {
          throw new Error('Thread not found');
        }
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
  }, [convex, isAuthenticated]);
};

export const useOpenAgentsChatRuntime = <UI_MESSAGE extends UIMessage = UIMessage>(
  options?: UseChatRuntimeOptions<UI_MESSAGE>,
) => {
  const { user } = useAuth();
  const adapter = useConvexThreadListAdapter(Boolean(user));

  return unstable_useRemoteThreadListRuntime({
    runtimeHook: () => useChatThreadRuntime(options),
    adapter,
    allowNesting: true,
  });
};
