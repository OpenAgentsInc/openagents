import { useChat, Message, UseChatOptions } from '@ai-sdk/react';
import { useState, useEffect, useRef } from 'react';
import * as React from 'react';
import { threadRepository, messageRepository } from '../db/repositories';
import { getDatabase } from '../db/database';
import { Thread } from '../db/types';
import { UIMessage, toVercelMessage, fromVercelMessage } from './types';
import { AgentRouterProvider } from '../agentrouter/provider';
import { inferRouted } from '../agentrouter';

/**
 * Options for the useAgentChat hook
 */
export interface UseAgentChatOptions extends UseChatOptions {
  id?: string;
  persistenceEnabled?: boolean;
  onThreadChange?: (threadId: string) => void;
  maxSteps?: number;
  agentRouterProvider: AgentRouterProvider;
}

/**
 * Return type for the useAgentChat hook
 */
export interface UseAgentChatReturn {
  messages: UIMessage[];
  append: (message: UIMessage) => Promise<string | null | undefined>;
  setMessages: (messages: UIMessage[]) => void;
  isLoading: boolean;
  error: Error | undefined;
  input: string;
  handleInputChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
  handleSubmit: (
    event?: { preventDefault?: () => void },
    options?: { experimental_attachments?: FileList }
  ) => void;
  stop: () => void;
  currentThreadId: string | undefined;
  switchThread: (threadId: string) => void;
  createNewThread: (title?: string) => Promise<Thread>;
  deleteThread: (threadId: string) => Promise<boolean>;
  updateThread: (threadId: string, title: string) => Promise<Thread | null>;
}

/**
 * Custom hook that extends Vercel's useChat with agent routing capabilities
 */
export function useAgentChat(options: UseAgentChatOptions): UseAgentChatReturn {
  const {
    persistenceEnabled = true,
    id: initialThreadId,
    onThreadChange,
    agentRouterProvider
  } = options;

  const [currentThreadId, setCurrentThreadId] = useState<string | undefined>(initialThreadId);
  const [dbInitialized, setDbInitialized] = useState(false);
  const [messages, setMessages] = useState<UIMessage[]>([]);
  const messagesRef = useRef<UIMessage[]>([]);
  const savedMessageIdsRef = useRef<Set<string>>(new Set());

  // Custom fetch function that intercepts requests and uses agent router
  const customFetch = async (url: string, init?: RequestInit) => {
    try {
      // Extract the user message from the request body
      const body = init?.body ? JSON.parse(init.body as string) : {};
      const userMessage = body.messages?.[body.messages.length - 1]?.content;

      if (!userMessage) {
        throw new Error('No user message found in request');
      }

      // Route the message using our agent router
      const routedResult = await inferRouted(agentRouterProvider, userMessage);

      if (!routedResult) {
        throw new Error('Failed to route message to an agent');
      }

      // Create a response that matches what the chat hook expects
      const response = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: JSON.stringify({
          agent: routedResult.agent_name,
          why: routedResult.why,
          instructions: routedResult.instructions_for_agent,
          original_prompt: routedResult.user_prompt
        }),
        createdAt: new Date()
      };

      // Create a ReadableStream to simulate streaming
      const stream = new ReadableStream({
        start(controller) {
          controller.enqueue(`data: ${JSON.stringify(response)}\n\n`);
          controller.close();
        }
      });

      return new Response(stream, {
        headers: {
          'Content-Type': 'text/event-stream',
        }
      });
    } catch (error) {
      console.error('Error in agent routing:', error);
      return new Response(null, { status: 500 });
    }
  };

  // Pass through all the original options but override the fetch function
  const customOptions = {
    ...options,
    id: options.id,
    maxSteps: options.maxSteps || 10,
    api: options.api || '/api/chat',
    body: {
      ...options.body,
      debug: true,
      format: 'json',
    },
    headers: {
      ...options.headers,
      'Content-Type': 'application/json',
      'Accept': 'text/event-stream',
      'X-Requested-With': 'XMLHttpRequest',
    },
    streamProtocol: 'data' as 'data',
    fetch: customFetch
  };

  const vercelChatState = useChat(customOptions);

  // Initialize the database
  useEffect(() => {
    const initDb = async () => {
      try {
        const db = await getDatabase();
        await messageRepository.initialize(db);
        setDbInitialized(true);
      } catch (error) {
        console.error('Failed to initialize database:', error);
      }
    };

    if (persistenceEnabled && !dbInitialized) {
      initDb();
    }
  }, [persistenceEnabled, dbInitialized]);

  // Load initial thread or create a default one
  useEffect(() => {
    const initializeThread = async () => {
      if (!dbInitialized) return;

      try {
        if (!currentThreadId) {
          const threads = await threadRepository.getAllThreads();
          if (threads.length > 0) {
            const mostRecentThread = threads[0];
            setCurrentThreadId(mostRecentThread.id);
            if (onThreadChange) {
              onThreadChange(mostRecentThread.id);
            }
          } else {
            const newThread = await threadRepository.createThread({
              title: 'New Chat',
              createdAt: Date.now(),
              updatedAt: Date.now(),
              modelId: '',
              systemPrompt: '',
              metadata: {}
            });
            setCurrentThreadId(newThread.id);
            if (onThreadChange) {
              onThreadChange(newThread.id);
            }
          }
        } else {
          const thread = await threadRepository.getThreadById(currentThreadId);
          if (!thread) {
            await threadRepository.createThread({
              id: currentThreadId,
              title: 'New Chat',
              createdAt: Date.now(),
              updatedAt: Date.now(),
              modelId: '',
              systemPrompt: '',
              metadata: {}
            });
          }
        }
      } catch (error) {
        console.error('Error initializing thread:', error);
      }
    };

    if (persistenceEnabled && dbInitialized) {
      initializeThread();
    }
  }, [currentThreadId, dbInitialized, persistenceEnabled, onThreadChange]);

  // Convert Vercel messages to UI messages
  const uiMessages = vercelChatState.messages.map(fromVercelMessage);

  // Create a type-safe append function
  const append = async (message: UIMessage) => {
    const vercelMessage = toVercelMessage(message);
    return vercelChatState.append(vercelMessage);
  };

  // Create a type-safe setMessages function
  const setUIMessages = (messages: UIMessage[]) => {
    vercelChatState.setMessages(messages.map(toVercelMessage));
  };

  return {
    messages: uiMessages,
    append,
    setMessages: setUIMessages,
    isLoading: vercelChatState.isLoading,
    error: vercelChatState.error,
    input: vercelChatState.input,
    handleInputChange: vercelChatState.handleInputChange,
    handleSubmit: vercelChatState.handleSubmit,
    stop: vercelChatState.stop,
    currentThreadId,
    switchThread: async (threadId: string) => {
      setCurrentThreadId(threadId);
      if (onThreadChange) {
        onThreadChange(threadId);
      }
    },
    createNewThread: async (title?: string): Promise<Thread> => {
      const newThread = await threadRepository.createThread({
        title: title || 'New Chat',
        createdAt: Date.now(),
        updatedAt: Date.now(),
        modelId: '',
        systemPrompt: '',
        metadata: {}
      });
      setCurrentThreadId(newThread.id);
      if (onThreadChange) {
        onThreadChange(newThread.id);
      }
      return newThread;
    },
    deleteThread: async (threadId: string): Promise<boolean> => {
      const success = await threadRepository.deleteThread(threadId);
      if (success && currentThreadId === threadId) {
        const threads = await threadRepository.getAllThreads();
        if (threads.length > 0) {
          setCurrentThreadId(threads[0].id);
          if (onThreadChange) {
            onThreadChange(threads[0].id);
          }
        } else {
          setCurrentThreadId(undefined);
          if (onThreadChange) {
            onThreadChange('');
          }
        }
      }
      return success;
    },
    updateThread: async (threadId: string, title: string): Promise<Thread | null> => {
      return threadRepository.updateThread(threadId, { title });
    }
  };
}
