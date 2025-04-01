import { useChat, Message, UseChatOptions } from '@ai-sdk/react';
import { useState, useEffect, useRef } from 'react';
import * as React from 'react';
import { threadRepository, messageRepository } from '../db/repositories';
import { getDatabase } from '../db/database';
import { Thread } from '../db/types';
import { UIMessage, toVercelMessage, fromVercelMessage } from './types';

// Version information
// AI SDK: @ai-sdk/react
// React version: React?.version

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

/**
 * Custom hook that extends Vercel's useChat with persistence capabilities
 */
/**
 * This hook extends Vercel's useChat with persistence capabilities.
 * It handles saving and loading messages from a database.
 */
export function usePersistentChat(options: UsePersistentChatOptions = {}): UsePersistentChatReturn {
  const { persistenceEnabled = true, id: initialThreadId, onThreadChange } = options;
  const [currentThreadId, setCurrentThreadId] = useState<string | undefined>(initialThreadId);
  const [dbInitialized, setDbInitialized] = useState(false);
  const [messages, setMessages] = useState<UIMessage[]>([]);
  const messagesRef = useRef<UIMessage[]>([]);

  // Add refs for tracking
  const savedMessageIdsRef = useRef<Set<string>>(new Set());

  // Pass through all the original options to avoid breaking anything
  const customOptions = {
    ...options,
    id: options.id,
    maxSteps: options.maxSteps || 10,
    // Set explicit AI API options to ensure compatibility
    api: options.api || 'https://chat.openagents.com',
    body: {
      ...options.body,
      // Add any missing required parameters for the AI API
      debug: true,
      format: 'json',
    },
    headers: {
      ...options.headers,
      'Content-Type': 'application/json',
      'Accept': 'text/event-stream',
    },
    // Force the stream protocol to be data for compatibility
    streamProtocol: 'data' as 'data',

    // Custom response handler
    onResponse: async (response: Response) => {
      // Check if the response has an error status
      if (!response.ok) {
        // Handle error case
      }

      // Call the original onResponse if provided
      if (options.onResponse) {
        await options.onResponse(response);
      }
    },

    onFinish: async (message: Message, finishOptions: any) => {
      // Call the original onFinish if provided
      if (options.onFinish) {
        options.onFinish(message, finishOptions);
      }

      // Save the completed assistant message to the database
      if (persistenceEnabled && dbInitialized && currentThreadId && message.role === 'assistant') {
        try {
          console.log('🔴 onFinish called for assistant message:', {
            id: message.id,
            content: message.content.substring(0, 50)
          });

          // Get all current messages and find the corresponding user message
          const currentMessages = vercelChatState.messages;
          console.log('🔴 Current messages:', currentMessages.map(m => ({
            id: m.id,
            role: m.role,
            timestamp: m.createdAt,
            content: m.content.substring(0, 30)
          })));

          // Find the last user message that doesn't have a corresponding assistant message
          const userMessages = currentMessages.filter(m => m.role === 'user');
          const assistantMessages = currentMessages.filter(m => m.role === 'assistant');

          // Find the latest user message that doesn't have a paired assistant message
          const unpairedUserMessage = userMessages.reverse().find(userMsg => {
            const hasAssistantResponse = assistantMessages.some(assistantMsg => {
              const userTime = userMsg.createdAt ? new Date(userMsg.createdAt).getTime() : 0;
              const assistantTime = assistantMsg.createdAt ? new Date(assistantMsg.createdAt).getTime() : 0;
              return assistantTime > userTime && assistantTime - userTime < 5000; // Within 5 seconds
            });
            return !hasAssistantResponse;
          });

          // Calculate a timestamp that ensures proper ordering with clear time difference
          // Find the latest timestamp of all messages
          const allTimestamps = currentMessages.map(m => 
            m.createdAt ? new Date(m.createdAt).getTime() : 0
          );
          
          const latestTimestamp = Math.max(...allTimestamps, 0);
          
          // Force at least a 500ms gap from the latest message
          const forcedGap = 500; // half-second gap for assistant responses
          const timestamp = new Date(Math.max(
            Date.now(),
            latestTimestamp + forcedGap
          ));
          
          console.log('🔴 Setting assistant message timestamp with forced gap:', timestamp);

          // Convert to UIMessage format with threadId guaranteed
          const uiMessage: UIMessage & { threadId: string } = {
            ...fromVercelMessage(message),
            threadId: currentThreadId,
            createdAt: timestamp
          };

          // Save the message
          const savedMessage = await messageRepository.createMessage(uiMessage);
          console.log('🔴 Saved assistant message:', {
            id: savedMessage.id,
            timestamp: timestamp
          });

          // Mark as saved to prevent duplicates
          savedMessageIdsRef.current.add(message.id);

          // Update thread timestamp
          await threadRepository.updateThread(currentThreadId, {
            updatedAt: Date.now()
          });
        } catch (error) {
          console.error('🔴 Error in onFinish:', error);
        }
      }
    }
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
        // Failed to initialize database
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
          // No thread ID provided, check for existing threads
          const threads = await threadRepository.getAllThreads();

          if (threads.length > 0) {
            // Use the most recent thread
            const mostRecentThread = threads[0];
            // Use the most recent thread
            setCurrentThreadId(mostRecentThread.id);
            if (onThreadChange) {
              onThreadChange(mostRecentThread.id);
            }
          } else {
            // Create a default thread
            // No threads found, create a default thread
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
            // Thread not found, create a new one with the specified ID
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

  // Track when we've loaded messages for a thread to prevent infinite loops
  const loadedThreadsRef = useRef<Set<string>>(new Set());

  // Load messages when thread changes
  useEffect(() => {
    const loadMessages = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId) return;

      // Skip if we've already loaded messages for this thread
      if (loadedThreadsRef.current.has(currentThreadId)) {
        // Thread already loaded, skip loading
        return;
      }

      try {
        // Load messages for the current thread
        const threadMessages = await messageRepository.getMessagesByThreadId(currentThreadId);

        // Mark all loaded messages as "saved" to prevent re-saving
        threadMessages.forEach(msg => {
          savedMessageIdsRef.current.add(msg.id);
        });

        // Mark this thread as loaded
        loadedThreadsRef.current.add(currentThreadId);

        if (threadMessages.length > 0) {
          // Update both local state and Vercel state
          messagesRef.current = threadMessages;

          // First update our local state
          setMessages(threadMessages);

          // Then update Vercel's state - but don't trigger our save mechanism
          // This is a one-way update operation
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

  // Update thread's last updated timestamp when messages change, with debouncing
  useEffect(() => {
    // Create a timeout ID inside the effect so it's properly scoped
    let timeoutId: NodeJS.Timeout | undefined;

    const updateThreadTimestamp = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId || messages.length === 0) return;

      try {
        // Only update if there was an actual change to avoid unnecessary updates
        if (messagesRef.current.length !== messages.length) {
          // Update the thread timestamp with retry mechanism
          await threadRepository.updateThread(currentThreadId, {
            updatedAt: Date.now()
          });
          // console.log(`Successfully updated timestamp for thread ${currentThreadId}`);
        }

        messagesRef.current = messages;
      } catch (error) {
        // Log but don't block the UI for timestamp updates
        console.error('Error updating thread timestamp:', error);
      }
    };

    if (currentThreadId) {
      // Debounce the update to avoid too many conflicting writes
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      timeoutId = setTimeout(() => {
        updateThreadTimestamp();
      }, 100);
    }

    return () => {
      // Cleanup on unmount
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [messages, currentThreadId, dbInitialized, persistenceEnabled]);

  // This ref is declared earlier in the component

  // Save messages to database when they change from Vercel
  useEffect(() => {
    const saveMessages = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId) return;

      try {
        const vercelMessages = vercelChatState.messages;
        // Skip if no messages
        if (vercelMessages.length === 0) return;

        // Only save user messages here - assistant messages are saved by onFinish
        // This prevents double saving of assistant messages
        const userMessages = vercelMessages
          .filter(msg => msg.role === 'user')
          .map(message => {
            // Convert to UIMessage format
            const uiMessage = fromVercelMessage(message);
            // Add thread ID
            uiMessage.threadId = currentThreadId;
            return uiMessage;
          });

        const newUserMessages = userMessages.filter(uiMsg =>
          !savedMessageIdsRef.current.has(uiMsg.id)
        );

        if (newUserMessages.length > 0) {
          // console.log('Saving', newUserMessages.length, 'new user messages to database');
          for (const message of newUserMessages) {
            console.log(`Saving user message: ${message.id}`);
            // The message already has threadId set above
            await messageRepository.createMessage(message as UIMessage & { threadId: string });

            // Mark as saved
            savedMessageIdsRef.current.add(message.id);
          }
        }
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
    // Ensure we have a threadId
    if (!currentThreadId) {
      return null;
    }

    console.log('🔵 Append called with message:', {
      id: message.id,
      role: message.role,
      content: message.content.substring(0, 50),
      timestamp: message.createdAt
    });

    // Calculate appropriate timestamp with forced minimum gap
    // Get all messages and their timestamps
    const allMessages = vercelChatState.messages;
    const allTimestamps = allMessages.map(m => 
      m.createdAt ? new Date(m.createdAt).getTime() : 0
    );
    
    // Find latest timestamp
    const latestTimestamp = allTimestamps.length ? Math.max(...allTimestamps) : 0;
    
    // Ensure at least 300ms between messages
    const minimumGap = 300; 
    const newTimestamp = new Date(Math.max(
      Date.now(),
      latestTimestamp + minimumGap
    ));
    
    console.log('🔵 Using calculated timestamp with gap:', newTimestamp);
    
    // Create a copy with the threadId explicitly set
    const messageWithThread: UIMessage & { threadId: string } = {
      ...message,
      threadId: currentThreadId,
      // Use the calculated timestamp
      createdAt: newTimestamp
    };

    console.log('🔵 Created message with timestamp:', messageWithThread.createdAt);

    // Convert to Vercel format
    const vercelMessage = toVercelMessage(messageWithThread);

    try {
      // First, save user message to database if it's a user message
      if (persistenceEnabled && dbInitialized && messageWithThread.role === 'user') {
        const savedMessage = await messageRepository.createMessage(messageWithThread);
        console.log('🔵 Saved user message to DB:', {
          id: savedMessage.id,
          timestamp: messageWithThread.createdAt
        });

        // Mark this message as saved
        savedMessageIdsRef.current.add(messageWithThread.id);
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
  };

  // Custom handleSubmit function that adds thread ID
  const handleSubmit = (
    event?: { preventDefault?: () => void },
    options?: { experimental_attachments?: FileList }
  ) => {
    if (event?.preventDefault) {
      event.preventDefault();
    }

    // We need to make sure we're seeing the user message immediately
    // Let Vercel's state management handle displaying the message immediately
    // Persistence will happen in the normal message flow

    // Get the current input to create a user message
    const userInput = vercelChatState.input;

    if (!userInput || !currentThreadId) {
      // No input or thread, just call the original submit
      vercelChatState.handleSubmit(event, options);
      return;
    }

    // Call original handleSubmit to trigger the AI process
    vercelChatState.handleSubmit(event, options);
  };

  // Thread management functions
  const switchThread = async (threadId: string) => {
    if (!persistenceEnabled || !dbInitialized) return;

    try {
      const thread = await threadRepository.getThreadById(threadId);
      if (thread) {
        // console.log('Switching to thread:', threadId);

        // Clear messages in Vercel state
        vercelChatState.setMessages([]);

        // Clear messages in our local state
        setMessages([]);
        messagesRef.current = [];

        // When switching threads, clear the loaded status for the new thread
        // so that we'll load its messages fresh
        loadedThreadsRef.current.delete(threadId);

        // Update thread ID, which will trigger message loading
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

      // Clear messages
      vercelChatState.setMessages([]);
      setMessages([]);
      messagesRef.current = [];

      // For a new thread, we should definitely clear all cached statuses
      loadedThreadsRef.current.delete(thread.id);

      // Switch to the new thread
      setCurrentThreadId(thread.id);
      if (onThreadChange) {
        onThreadChange(thread.id);
      }

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

  // Don't wait for dirtyPersistenceState - ALWAYS convert Vercel's messages
  useEffect(() => {
    if (vercelChatState.messages.length > 0 && currentThreadId) {
      // Convert them to our format for storage/persistence
      const uiMessages = vercelChatState.messages.map(message => {
        const uiMessage = fromVercelMessage(message);
        // Ensure threadId is set
        uiMessage.threadId = currentThreadId;
        return uiMessage;
      });

      // Update our state
      setMessages(uiMessages);
    }
  }, [vercelChatState.messages, currentThreadId]); // Direct dependency on vercelChatState.messages

  // COMPLETELY BYPASS OUR STATE - use Vercel's messages directly
  // Convert on the fly for the return value and sort by createdAt
  const displayMessages = vercelChatState.messages.map(m => {
    const msg = fromVercelMessage(m);
    if (currentThreadId) {
      msg.threadId = currentThreadId;
    }
    return msg;
  });

  // Log the messages before sorting
  console.log('🔵 Messages before sorting:', displayMessages.map(m => ({
    id: m.id,
    role: m.role,
    timestamp: m.createdAt,
    content: m.content.substring(0, 30)
  })));

  // Sort messages with multiple fallbacks for identical timestamps
  const sortedMessages = displayMessages.slice().sort((a, b) => {
    // Primary sort: by timestamp
    const timeA = a.createdAt?.getTime() || 0;
    const timeB = b.createdAt?.getTime() || 0;
    
    if (timeA !== timeB) {
      return timeA - timeB; // Ascending order - older messages first
    }
    
    // Secondary sort: by conversation flow (user messages come before assistant responses)
    if (a.role !== b.role) {
      // User messages should come before assistant messages when timestamps are equal
      return a.role === 'user' ? -1 : 1;
    }
    
    // Tertiary sort: by ID to ensure complete stability
    return a.id.localeCompare(b.id);
  });

  // Log the messages after sorting
  console.log('🔵 Messages after sorting:', sortedMessages.map(m => ({
    id: m.id,
    role: m.role,
    timestamp: m.createdAt,
    content: m.content.substring(0, 30)
  })));

  return {
    // ALWAYS use Vercel's messages, converted to our format
    messages: sortedMessages,
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
