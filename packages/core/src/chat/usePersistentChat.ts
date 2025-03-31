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
/**
 * This hook extends Vercel's useChat with persistence capabilities.
 * It handles saving and loading messages from a database.
 */
export function usePersistentChat(options: UsePersistentChatOptions = {}): UsePersistentChatReturn {
  console.log('usePersistentChat called with options:', JSON.stringify({
    id: options.id,
    persistenceEnabled: options.persistenceEnabled,
    maxSteps: options.maxSteps,
    api: options.api
  }));
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
    experimental_onToolCall: options.experimental_onToolCall,
    api: options.api,
    body: options.body,
    headers: options.headers,
    
    // This is important for ensuring we see messages immediately
    onResponse: async (response: Response) => {
      // Call the original onResponse if provided
      if (options.onResponse) {
        await options.onResponse(response);
      }
      
      console.log('Server response received');
    },
    
    onFinish: async (message: Message, finishOptions: any) => {
      console.log('onFinish called for message:', message.id, message.role);
      
      // Call the original onFinish if provided
      if (options.onFinish) {
        options.onFinish(message, finishOptions);
      }
      
      // Save the completed assistant message to the database
      if (persistenceEnabled && dbInitialized && currentThreadId && message.role === 'assistant') {
        try {
          console.log('Saving completed assistant message to database');
          
          // Convert to UIMessage format with threadId guaranteed
          const uiMessage: UIMessage & { threadId: string } = {
            ...fromVercelMessage(message),
            threadId: currentThreadId
          };
          
          // Save the message
          await messageRepository.createMessage(uiMessage);
          
          // Mark as saved to prevent duplicates
          savedMessageIdsRef.current.add(message.id);
          console.log(`Marked assistant message ${message.id} as saved`);
          
          // Update thread timestamp
          await threadRepository.updateThread(currentThreadId, {
            updatedAt: Date.now()
          });
        } catch (error) {
          console.error('Error saving assistant message in onFinish:', error);
        }
      }
    }
  };
  
  const vercelChatState = useChat(customOptions);

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

  // Track when we've loaded messages for a thread to prevent infinite loops
  const loadedThreadsRef = useRef<Set<string>>(new Set());
  
  // Load messages when thread changes
  useEffect(() => {
    const loadMessages = async () => {
      if (!persistenceEnabled || !dbInitialized || !currentThreadId) return;
      
      // Skip if we've already loaded messages for this thread
      if (loadedThreadsRef.current.has(currentThreadId)) {
        console.log(`Thread ${currentThreadId} already loaded, skipping load`);
        return;
      }
      
      try {
        console.log('Loading messages for thread:', currentThreadId);
        const threadMessages = await messageRepository.getMessagesByThreadId(currentThreadId);
        console.log('Loaded', threadMessages.length, 'messages from database');
        
        // Mark all loaded messages as "saved" to prevent re-saving
        threadMessages.forEach(msg => {
          savedMessageIdsRef.current.add(msg.id);
          console.log(`Marked message ${msg.id} (${msg.role}) as saved`);
        });
        
        // Mark this thread as loaded
        loadedThreadsRef.current.add(currentThreadId);
        console.log(`Marked thread ${currentThreadId} as loaded`);
        
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
          console.log('Saving', newUserMessages.length, 'new user messages to database');
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
      console.error('Cannot append message: No current thread ID');
      return null;
    }

    // Create a copy with the threadId explicitly set
    const messageWithThread: UIMessage & { threadId: string } = {
      ...message,
      threadId: currentThreadId
    };

    // Convert to Vercel format
    const vercelMessage = toVercelMessage(messageWithThread);
    
    try {
      // First, save user message to database if it's a user message
      if (persistenceEnabled && dbInitialized && messageWithThread.role === 'user') {
        console.log('Saving user message to database:', messageWithThread.content.substring(0, 50) + (messageWithThread.content.length > 50 ? '...' : ''));
        await messageRepository.createMessage(messageWithThread);
        
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
    
    // When explicitly setting messages, we'll save them all
    // This is used mainly for loading saved messages, so we don't need to save again
    // Just log what's happening
    console.log(`setVercelMessages called with ${newMessages.length} messages - not saving to avoid duplication`);
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
        console.log('Switching to thread:', threadId);
        
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
    console.log('Vercel message state changed - now has', vercelChatState.messages.length, 'messages');
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
  
  // Add debugging to see what's happening with Vercel's message state
  useEffect(() => {
    console.log('Vercel messages changed:', JSON.stringify(vercelChatState.messages.map(m => ({
      id: m.id,
      role: m.role,
      content: m.content.substring(0, 30) + (m.content.length > 30 ? '...' : '')
    }))));
  }, [vercelChatState.messages]);
  
  // COMPLETELY BYPASS OUR STATE - use Vercel's messages directly
  // Convert on the fly for the return value
  const displayMessages = vercelChatState.messages.map(m => {
    const msg = fromVercelMessage(m);
    if (currentThreadId) {
      msg.threadId = currentThreadId;
    }
    return msg;
  });
  
  console.log('Returning', displayMessages.length, 'messages to UI');
  
  return {
    // ALWAYS use Vercel's messages, converted to our format
    messages: displayMessages,
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
