import { useChat, type Message, type UseChatOptions } from '@ai-sdk/react';
import { useState, useEffect, useRef } from 'react';
import * as React from 'react';
import { threadRepository, messageRepository } from '../db/repositories';
import { getDatabase } from '../db/database';
import type { Thread } from '../db/types';
import { type UIMessage, toVercelMessage, fromVercelMessage } from './types';

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
 * Extended submission options type to support our custom fields
 */
export interface SubmissionOptions {
  selectedToolIds?: string[];
  experimental_attachments?: FileList;
  body?: Record<string, any>;
  debug_tool_selection?: boolean;
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
    // Always use absolute URL in production builds for better reliability
    // Get the API port from the IPC bridge or fall back to the default port
    api: options.api || '/api/chat',  // Using relative URL is safer with our redirector
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
      'X-Requested-With': 'XMLHttpRequest',
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
          // Check if this message already exists in the database
          const existingMessage = await messageRepository.getMessageById(message.id);

          // Get the most accurate message with all parts from Vercel state
          const latestMessage = vercelChatState.messages.find(m => m.id === message.id) || message;

          if (existingMessage) {
            // Message exists - update it with new parts/content instead of creating
            await messageRepository.updateMessage(message.id, {
              content: latestMessage.content,
              parts: latestMessage.parts
            });
          } else {
            // Message doesn't exist yet - create it
            const uiMessage: UIMessage & { threadId: string } = {
              id: message.id,
              role: message.role,
              content: latestMessage.content,
              createdAt: new Date(),
              threadId: currentThreadId,
              parts: latestMessage.parts || [],
              toolInvocations: latestMessage.toolInvocations,
              experimental_attachments: latestMessage.experimental_attachments || []
            };

            await messageRepository.createMessage(uiMessage);
          }

          // Update thread timestamp
          await threadRepository.updateThread(currentThreadId, {
            updatedAt: Date.now()
          });
        } catch (error) {
          console.error('Error in onFinish:', error);
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
  const handleSubmit = async (
    event?: { preventDefault?: () => void },
    options?: { selectedToolIds?: string[], experimental_attachments?: FileList, debug_tool_selection?: boolean }
  ) => {
    if (event?.preventDefault) {
      event.preventDefault();
    }

    // Ensure database is initialized
    if (persistenceEnabled && !dbInitialized) {
      try {
        console.log("Database not initialized yet, attempting initialization now...");
        const db = await getDatabase();
        await messageRepository.initialize(db);
        await threadRepository.initialize(db);
        setDbInitialized(true);
        console.log("Database initialized successfully");
      } catch (error) {
        console.error("Failed to initialize database:", error);
        // Continue even if database fails - will use in-memory state
      }
    }

    // Log debug information if requested
    if (options?.debug_tool_selection) {
      console.log('[usePersistentChat] Handling submit with options:', {
        selectedToolIds: options.selectedToolIds || [],
        hasSelectedTools: Array.isArray(options.selectedToolIds) && options.selectedToolIds.length > 0,
        hasAttachments: options.experimental_attachments !== undefined
      });
    }

    // Extract selectedToolIds from options, if provided
    const selectedToolIds = options?.selectedToolIds;

    // We need to make sure we're seeing the user message immediately
    // Let Vercel's state management handle displaying the message immediately
    // Persistence will happen in the normal message flow

    // Get the current input to create a user message
    const userInput = vercelChatState.input;

    if (!userInput || !currentThreadId) {
      // No input or thread, just call the original submit
      // But make sure to forward the selectedToolIds
      vercelChatState.handleSubmit(event, options);
      return;
    }

    // Ensure we have a thread to save messages to
    if (persistenceEnabled && !currentThreadId) {
      try {
        // Create a default thread if needed
        const thread = await createNewThread("New Chat");
        // Don't wait for thread creation to submit the message
      } catch (error) {
        console.error("Failed to create thread:", error);
      }
    }

    // Prepare the options for submission
    // We need to create a new options object that includes the selected tools
    const submissionOptions = { ...options } as SubmissionOptions;

    // If tools are explicitly selected, add them to the options
    if (Array.isArray(selectedToolIds)) {
      console.log('[usePersistentChat] Forwarding selected tool IDs to API:', selectedToolIds);

      // Create the body if it doesn't exist
      if (!submissionOptions.body) {
        submissionOptions.body = {};
      }

      // Set the selectedToolIds in two places:
      // 1. Directly in the options object for our server handler
      submissionOptions.selectedToolIds = selectedToolIds;

      // 2. In the body object for the vercel/ai SDK
      submissionOptions.body = {
        ...submissionOptions.body,
        selectedToolIds: selectedToolIds,
      };

      console.log('[usePersistentChat] FINAL SUBMISSION OPTIONS WITH TOOLS:',
        JSON.stringify(submissionOptions, null, 2));
    } else {
      console.log('[usePersistentChat] No selected tools provided, using defaults');
    }

    // Call original handleSubmit with our modified options that include the tools
    vercelChatState.handleSubmit(event, submissionOptions);
  };

  // Thread management functions
  const switchThread = async (threadId: string) => {
    if (!persistenceEnabled) return;

    // Try to initialize database if not already done
    if (!dbInitialized) {
      try {
        console.log("Database not initialized yet, attempting initialization now...");
        const db = await getDatabase();
        await messageRepository.initialize(db);
        await threadRepository.initialize(db);
        setDbInitialized(true);
        console.log("Database initialized successfully");
      } catch (error) {
        console.error("Failed to initialize database:", error);
        return; // Don't throw here, just return
      }
    }

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
    if (!persistenceEnabled) {
      throw new Error('Persistence disabled');
    }

    // Try to initialize database if not already done
    if (!dbInitialized) {
      try {
        console.log("Database not initialized yet, attempting initialization now...");
        const db = await getDatabase();
        await messageRepository.initialize(db);
        await threadRepository.initialize(db);
        setDbInitialized(true);
        console.log("Database initialized successfully");
      } catch (error) {
        console.error("Failed to initialize database:", error);
        throw new Error('Database initialization failed');
      }
    }

    try {
      // Clear messages for the new thread right away
      vercelChatState.setMessages([]);
      setMessages([]);
      messagesRef.current = [];

      // Create the actual thread in the database
      const thread = await threadRepository.createThread({
        title: title || 'New Chat',
        createdAt: Date.now(),
        updatedAt: Date.now(),
        modelId: '',
        systemPrompt: '',
        metadata: {}
      });

      // Dispatch a custom event to notify all components that a thread was created
      window.dispatchEvent(new CustomEvent('thread-changed', {
        detail: { action: 'create', threadId: thread.id }
      }));

      // console.log('Created new thread:', thread.id);

      // For a new thread, we should definitely clear all cached statuses
      loadedThreadsRef.current.delete(thread.id);

      // Now switch to the actual thread ID from the database
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

      // If the current thread is being deleted, switch to another thread first
      // This provides a better user experience because the UI updates immediately
      if (threadId === currentThreadId) {
        // Get all threads except the one being deleted
        const allThreads = await threadRepository.getAllThreads();
        const remainingThreads = allThreads.filter(t => t.id !== threadId);

        if (remainingThreads.length > 0) {
          // Switch to the most recent thread first
          await switchThread(remainingThreads[0].id);
        } else {
          // Create a new thread first if no threads will be left
          const newThread = await createNewThread();
          await switchThread(newThread.id);
        }
      }

      // Delete messages
      await messageRepository.deleteMessagesByThreadId(threadId);

      // Then delete the thread
      const result = await threadRepository.deleteThread(threadId);

      // Dispatch a custom event to notify all components that a thread was deleted
      window.dispatchEvent(new CustomEvent('thread-changed', {
        detail: { action: 'delete', threadId: threadId }
      }));

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
  // console.log('🔵 Messages before sorting:', displayMessages.map(m => ({
  //   id: m.id,
  //   role: m.role,
  //   timestamp: m.createdAt,
  //   content: m.content.substring(0, 30)
  // })));

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
  // console.log('🔵 Messages after sorting:', sortedMessages.map(m => ({
  //   id: m.id,
  //   role: m.role,
  //   timestamp: m.createdAt,
  //   content: m.content.substring(0, 30)
  // })));

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
