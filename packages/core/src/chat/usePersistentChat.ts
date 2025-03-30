import { useChat, Message, UseChatOptions } from 'ai/react';
import { useState, useEffect, useCallback, useRef } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { threadRepository, messageRepository } from '../db/repositories';
import { Thread } from '../db/types';
import { UIMessage, TextUIPart, ReasoningUIPart, ToolInvocationUIPart, SourceUIPart, FileUIPart, toVercelMessage, fromVercelMessage } from './types';
import { CreateMessage } from 'ai/react';

/**
 * Options for the usePersistentChat hook
 */
export interface UsePersistentChatOptions extends UseChatOptions {
  id?: string;
  persistenceEnabled?: boolean;
  onThreadChange?: (threadId: string) => void;
}

/**
 * Return type for the usePersistentChat hook
 */
export interface UsePersistentChatReturn {
  messages: UIMessage[];
  append: (message: UIMessage) => Promise<string | null | undefined>;
  setMessages: (messages: UIMessage[]) => void;
  isLoading: boolean;
  error: Error | undefined;
}

// Convert UIMessage to Message for Vercel AI SDK
function uiMessageToMessage(message: UIMessage): Message {
  // Filter out StepStartUIPart from parts
  const parts = message.parts.filter(part =>
    part.type === 'text' ||
    part.type === 'reasoning' ||
    part.type === 'tool-invocation' ||
    part.type === 'source' ||
    part.type === 'file'
  ) as (TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart)[];

  return {
    id: message.id,
    role: message.role,
    content: message.content,
    parts
  };
}

// Convert Message to UIMessage
function messageToUIMessage(message: Message | CreateMessage): UIMessage {
  return {
    id: 'id' in message ? message.id : uuidv4(),
    role: message.role,
    content: message.content,
    createdAt: new Date(),
    parts: message.parts || []
  };
}

/**
 * Custom hook that extends Vercel's useChat with persistence capabilities
 */
export function usePersistentChat(options: UsePersistentChatOptions = {}): UsePersistentChatReturn {
  const vercelChatState = useChat(options);
  const [messages, setMessages] = useState<UIMessage[]>([]);

  // Update local messages when Vercel messages change
  useEffect(() => {
    const uiMessages = vercelChatState.messages.map(fromVercelMessage);
    setMessages(uiMessages);
  }, [vercelChatState.messages]);

  const append = async (message: UIMessage): Promise<string | null | undefined> => {
    const vercelMessage = toVercelMessage(message);
    try {
      const result = await vercelChatState.append(vercelMessage);

      if (result === null || result === undefined) {
        return null;
      }

      if (typeof result === 'object' && result !== null && 'id' in result) {
        return result.id as string;
      }

      return typeof result === 'string' ? result : undefined;
    } catch (error) {
      console.error('Error in append:', error);
      return null;
    }
  };

  const setVercelMessages = (messages: UIMessage[]) => {
    const vercelMessages = messages.map(toVercelMessage);
    vercelChatState.setMessages(vercelMessages);
  };

  return {
    messages,
    append,
    setMessages: setVercelMessages,
    isLoading: vercelChatState.isLoading,
    error: vercelChatState.error
  };
}
