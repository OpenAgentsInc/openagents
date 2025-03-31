import { useChat, Message, UseChatOptions } from 'ai/react';
import { useState, useEffect, useCallback, useRef } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { threadRepository, messageRepository } from '../db/repositories';
import { getDatabase } from '../db/database';
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
  maxSteps?: number;
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
function messageToUIMessage(message: Message | CreateMessage, threadId?: string): UIMessage {
  return {
    id: 'id' in message ? message.id : uuidv4(),
    role: message.role,
    content: message.content,
    createdAt: new Date(),
    threadId,
    parts: message.parts || []
  };
}

/**
 * Custom hook that extends Vercel's useChat with persistence capabilities
 */
export function usePersistentChat(options: UsePersistentChatOptions = {}): UsePersistentChatReturn {
  const { persistenceEnabled = true, id: initialThreadId, onThreadChange } = options;
  const [currentThreadId, setCurrentThreadId] = useState<string | undefined>(initialThreadId);
  const [dbInitialized, setDbInitialized] = useState(false);
  const vercelChatState = useChat(options);
  const [messages, setMessages] = useState<UIMessage[]>([]);
  const messagesRef = useRef<UIMessage[]>([]);

  // Initialize the database
  useEffect(() => {
    const initDb = async () => {
      try {
        console.log('Initializing database for persistent chat');
        const db = await getDatabase();
        await messageRepository.initialize(db);
        setDbInitialized(true);
        console.log('Database initialized successfully');
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
        // If no thread ID is provided, check if any threads exist
        if (!currentThreadId) {
          console.log('No thread ID provided, checking for existing threads');
          const threads = await threadRepository.getAllThreads();
          
          if (threads.length > 0) {
            // Use the most recent thread
            const mostRecentThread = threads[0];
            console.log('Using most recent thread:', mostRecentThread.id);
            setCurrentThreadId(mostRecentThread.id);
            if (onThreadChange) {
              onThreadChange(mostRecentThread.id);
            }
          } else {
            // Create a default thread
            console.log('No threads found, creating a default thread');
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
          // Check if the provided thread exists
          const thread = await threadRepository.getThreadById(currentThreadId);
          if (!thread) {
            console.log('Thread not found, creating a new one with ID:', currentThreadId);
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

  // Load messages when thread changes
  useEffect(() => {
    const loadMessages = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId) return;

      try {
        console.log('Loading messages for thread:', currentThreadId);
        const threadMessages = await messageRepository.getMessagesByThreadId(currentThreadId);
        console.log('Loaded', threadMessages.length, 'messages from database');
        
        if (threadMessages.length > 0) {
          // Update both local state and Vercel state
          messagesRef.current = threadMessages;
          setMessages(threadMessages);
          
          // Convert to Vercel messages and update Vercel state
          const vercelMessages = threadMessages.map(toVercelMessage);
          vercelChatState.setMessages(vercelMessages);
        }
      } catch (error) {
        console.error('Error loading messages:', error);
      }
    };

    if (currentThreadId) {
      loadMessages();
    }
  }, [currentThreadId, dbInitialized, persistenceEnabled, vercelChatState]);

  // Update thread's last updated timestamp when messages change
  useEffect(() => {
    const updateThreadTimestamp = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId || messages.length === 0) return;

      try {
        // Only update if there was an actual change to avoid unnecessary updates
        if (messagesRef.current.length !== messages.length) {
          console.log('Updating thread timestamp for thread:', currentThreadId);
          await threadRepository.updateThread(currentThreadId, {
            updatedAt: Date.now()
          });
        }

        messagesRef.current = messages;
      } catch (error) {
        console.error('Error updating thread timestamp:', error);
      }
    };

    if (currentThreadId) {
      updateThreadTimestamp();
    }
  }, [messages, currentThreadId, dbInitialized, persistenceEnabled]);

  // Save messages to database when they change from Vercel
  useEffect(() => {
    const saveMessages = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId) return;

      try {
        const vercelMessages = vercelChatState.messages;
        // Skip if no messages
        if (vercelMessages.length === 0) return;

        const uiMessages = vercelMessages.map(message => {
          // Convert to UIMessage format
          const uiMessage = fromVercelMessage(message);
          // Add thread ID
          uiMessage.threadId = currentThreadId;
          return uiMessage;
        });

        // Check if we have new messages compared to what's in the database
        const dbMessages = messagesRef.current;
        const newMessages = uiMessages.filter(uiMsg => 
          !dbMessages.some(dbMsg => dbMsg.id === uiMsg.id)
        );

        if (newMessages.length > 0) {
          console.log('Saving', newMessages.length, 'new messages to database');
          for (const message of newMessages) {
            await messageRepository.createMessage({
              ...message,
              threadId: currentThreadId
            });
          }
        }

        // Update our local state
        messagesRef.current = uiMessages;
        setMessages(uiMessages);
      } catch (error) {
        console.error('Error saving messages:', error);
      }
    };

    if (persistenceEnabled && dbInitialized && currentThreadId) {
      saveMessages();
    }
  }, [vercelChatState.messages, currentThreadId, dbInitialized, persistenceEnabled]);

  // Custom append function that saves messages to database
  const append = async (message: UIMessage): Promise<string | null | undefined> => {
    // Assign thread ID to message if not present
    if (!message.threadId && currentThreadId) {
      message.threadId = currentThreadId;
    }

    // Convert to Vercel format
    const vercelMessage = toVercelMessage(message);
    
    try {
      // First, save user message to database
      if (persistenceEnabled && dbInitialized && currentThreadId && message.role === 'user') {
        console.log('Saving user message to database:', message.content.substring(0, 50) + (message.content.length > 50 ? '...' : ''));
        await messageRepository.createMessage({
          ...message,
          threadId: currentThreadId
        });
      }

      // Then, submit to Vercel AI SDK to get assistant response
      const result = await vercelChatState.append(vercelMessage);

      if (!result) {
        return null;
      }

      // Type guard to ensure result is an object with id
      if (result && typeof result === 'object' && 'id' in result) {
        const typedResult = result as { id: string };
        return typedResult.id;
      }

      // Handle string result case
      return typeof result === 'string' ? result : undefined;
    } catch (error) {
      console.error('Error in append:', error);
      return null;
    }
  };

  // Custom setMessages function that updates both local and Vercel state
  const setVercelMessages = (newMessages: UIMessage[]) => {
    // Update local state
    setMessages(newMessages);
    messagesRef.current = newMessages;
    
    // Convert to Vercel format and update Vercel state
    const vercelMessages = newMessages.map(toVercelMessage);
    vercelChatState.setMessages(vercelMessages);
    
    // Save to database if persistence is enabled
    if (persistenceEnabled && dbInitialized && currentThreadId) {
      // Save each message
      newMessages.forEach(async (message) => {
        try {
          if (!message.threadId) {
            message.threadId = currentThreadId;
          }
          
          await messageRepository.createMessage({
            ...message,
            threadId: currentThreadId
          });
        } catch (error) {
          console.error('Error saving message during setMessages:', error);
        }
      });
    }
  };

  // Custom handleSubmit function that adds thread ID
  const handleSubmit = (
    event?: { preventDefault?: () => void },
    options?: { experimental_attachments?: FileList }
  ) => {
    if (event?.preventDefault) {
      event.preventDefault();
    }
    
    // Prepare user message with thread ID for saving
    if (persistenceEnabled && dbInitialized && currentThreadId && vercelChatState.input) {
      const userMessage: UIMessage = {
        id: uuidv4(),
        role: 'user',
        content: vercelChatState.input,
        createdAt: new Date(),
        threadId: currentThreadId,
        parts: [{ type: 'text', text: vercelChatState.input }]
      };
      
      // Save user message before submitting
      (async () => {
        try {
          await messageRepository.createMessage(userMessage);
        } catch (error) {
          console.error('Error saving user message during handleSubmit:', error);
        }
      })();
    }
    
    // Call original handleSubmit
    vercelChatState.handleSubmit(event, options);
  };

  // Thread management functions
  const switchThread = async (threadId: string) => {
    if (!persistenceEnabled || !dbInitialized) return;
    
    try {
      const thread = await threadRepository.getThreadById(threadId);
      if (thread) {
        console.log('Switching to thread:', threadId);
        setCurrentThreadId(threadId);
        if (onThreadChange) {
          onThreadChange(threadId);
        }
      } else {
        console.error('Thread not found:', threadId);
      }
    } catch (error) {
      console.error('Error switching thread:', error);
    }
  };

  const createNewThread = async (title?: string): Promise<Thread> => {
    if (!persistenceEnabled || !dbInitialized) {
      throw new Error('Database not initialized or persistence disabled');
    }
    
    try {
      const thread = await threadRepository.createThread({
        title: title || 'New Chat',
        createdAt: Date.now(),
        updatedAt: Date.now(),
        modelId: '',
        systemPrompt: '',
        metadata: {}
      });
      
      console.log('Created new thread:', thread.id);
      
      // Switch to the new thread
      setCurrentThreadId(thread.id);
      if (onThreadChange) {
        onThreadChange(thread.id);
      }
      
      // Clear messages
      setMessages([]);
      messagesRef.current = [];
      vercelChatState.setMessages([]);
      
      return thread;
    } catch (error) {
      console.error('Error creating thread:', error);
      throw error;
    }
  };

  const deleteThread = async (threadId: string): Promise<boolean> => {
    if (!persistenceEnabled || !dbInitialized) {
      return false;
    }
    
    try {
      console.log('Deleting thread:', threadId);
      
      // Delete messages first
      await messageRepository.deleteMessagesByThreadId(threadId);
      
      // Then delete the thread
      const result = await threadRepository.deleteThread(threadId);
      
      // If the current thread was deleted, switch to another thread
      if (result && threadId === currentThreadId) {
        const threads = await threadRepository.getAllThreads();
        if (threads.length > 0) {
          // Switch to the most recent thread
          switchThread(threads[0].id);
        } else {
          // Create a new thread if no threads left
          const newThread = await createNewThread();
          switchThread(newThread.id);
        }
      }
      
      return result;
    } catch (error) {
      console.error('Error deleting thread:', error);
      return false;
    }
  };

  const updateThread = async (threadId: string, title: string): Promise<Thread | null> => {
    if (!persistenceEnabled || !dbInitialized) {
      return null;
    }
    
    try {
      console.log('Updating thread:', threadId, 'with title:', title);
      return await threadRepository.updateThread(threadId, { title });
    } catch (error) {
      console.error('Error updating thread:', error);
      return null;
    }
  };

  return {
    messages,
    append,
    setMessages: setVercelMessages,
    isLoading: vercelChatState.isLoading,
    error: vercelChatState.error,
    input: vercelChatState.input,
    handleInputChange: vercelChatState.handleInputChange,
    handleSubmit,
    stop: vercelChatState.stop,
    currentThreadId,
    switchThread,
    createNewThread,
    deleteThread,
    updateThread
  };
}
