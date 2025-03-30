import { useChat as vercelUseChat, UseChatOptions as VercelUseChatOptions } from '@ai-sdk/react';
import { useState, useEffect, useCallback, useRef } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { threadRepository, messageRepository } from '../db/repositories';
import { Thread } from '../db/types';
import { UIMessage } from './types';

// Extend Vercel's options to include our own options
export interface UsePersistentChatOptions extends VercelUseChatOptions {
  /**
   * Enable persistence (default: true)
   */
  persistenceEnabled?: boolean;
  
  /**
   * Callback when thread is changed
   */
  onThreadChange?: (threadId: string) => void;
}

// Return type with our additional properties
export interface UsePersistentChatReturn extends ReturnType<typeof vercelUseChat> {
  /**
   * Current thread ID
   */
  currentThreadId: string;
  
  /**
   * Switch to another thread
   */
  switchThread: (threadId: string) => Promise<void>;
  
  /**
   * Create a new thread
   */
  createNewThread: (title?: string) => Promise<Thread>;
  
  /**
   * Get all threads
   */
  getAllThreads: () => Promise<Thread[]>;
  
  /**
   * Delete a thread
   */
  deleteThread: (threadId: string) => Promise<boolean>;
  
  /**
   * Update thread metadata
   */
  updateThread: (threadId: string, updates: Partial<Thread>) => Promise<Thread | null>;
  
  /**
   * Delete a message
   */
  deleteMessage: (messageId: string) => Promise<boolean>;
}

/**
 * Custom hook that extends Vercel's useChat with persistence capabilities
 */
export function usePersistentChat(options: UsePersistentChatOptions = {}): UsePersistentChatReturn {
  // Extract our custom options
  const {
    id: providedThreadId,
    persistenceEnabled = true,
    onThreadChange,
    // Extract all other options for passing to Vercel's hook
    ...vercelOptions
  } = options;
  
  // Current thread ID state
  const [currentThreadId, setCurrentThreadId] = useState<string>(providedThreadId || 'default');
  
  // Track if initial messages have been loaded
  const initialMessagesLoaded = useRef<boolean>(false);
  
  // Load or create the thread on initial render
  useEffect(() => {
    if (!persistenceEnabled) return;
    
    const initializeThread = async () => {
      try {
        // Check if thread exists
        let thread = await threadRepository.getThreadById(currentThreadId);
        
        // If thread doesn't exist, create it
        if (!thread) {
          thread = await threadRepository.createThread({
            id: currentThreadId,
            title: 'New Chat',
          });
          console.log('Created new thread:', thread.id);
        }
        
        // Only load messages if initial messages haven't been loaded yet
        if (!initialMessagesLoaded.current) {
          // Load messages for this thread
          const messages = await messageRepository.getMessagesByThreadId(currentThreadId);
          console.log(`Loaded ${messages.length} messages for thread ${currentThreadId}`);
          
          // Set initial messages if there are any
          if (messages.length > 0) {
            // This will be handled by setMessages from the chat hook
            vercelChatState.setMessages(messages);
            initialMessagesLoaded.current = true;
          }
        }
      } catch (error) {
        console.error('Error initializing thread:', error);
      }
    };
    
    initializeThread();
  }, [currentThreadId, persistenceEnabled]);
  
  // Use Vercel's useChat hook
  const vercelChatState = vercelUseChat({
    id: currentThreadId,
    ...vercelOptions
  });
  
  // Extract the append and setMessages functions from Vercel's hook
  const { append: vercelAppend, setMessages: vercelSetMessages } = vercelChatState;
  
  // Custom append function that saves messages to the database
  const append = useCallback(async (message: UIMessage) => {
    // If persistence is disabled, just use Vercel's append
    if (!persistenceEnabled) {
      return vercelAppend(message);
    }
    
    // Ensure message has an ID
    if (!message.id) {
      message.id = uuidv4();
    }
    
    // Ensure message has a timestamp
    if (!message.createdAt) {
      message.createdAt = new Date();
    }
    
    // If this is a user message, save it to the database immediately
    if (message.role === 'user') {
      try {
        await messageRepository.createMessage({
          ...message,
          threadId: currentThreadId
        });
        
        // Update thread's updatedAt timestamp
        await threadRepository.updateThread(currentThreadId, {
          updatedAt: Date.now()
        });
      } catch (error) {
        console.error('Error saving user message:', error);
      }
    }
    
    // Call Vercel's append
    const result = await vercelAppend(message);
    
    return result;
  }, [vercelAppend, currentThreadId, persistenceEnabled]);
  
  // Custom setMessages function that also updates the database
  const setMessages = useCallback(async (messages: UIMessage[]) => {
    // If persistence is disabled or no messages, just use Vercel's setMessages
    if (!persistenceEnabled || messages.length === 0) {
      return vercelSetMessages(messages);
    }
    
    // Delete all existing messages for this thread
    try {
      await messageRepository.deleteMessagesByThreadId(currentThreadId);
      
      // Save all the new messages
      await Promise.all(
        messages.map(message => 
          messageRepository.createMessage({
            ...message,
            threadId: currentThreadId
          })
        )
      );
      
      // Update thread's updatedAt timestamp
      await threadRepository.updateThread(currentThreadId, {
        updatedAt: Date.now()
      });
    } catch (error) {
      console.error('Error updating messages:', error);
    }
    
    // Call Vercel's setMessages
    return vercelSetMessages(messages);
  }, [vercelSetMessages, currentThreadId, persistenceEnabled]);
  
  // Listen for newly completed messages and save them
  useEffect(() => {
    if (!persistenceEnabled) return;
    
    // Look for the last assistant message in the list
    const messages = vercelChatState.messages;
    if (messages.length === 0) return;
    
    const lastMessage = messages[messages.length - 1];
    
    // Only process assistant messages that are not being streamed
    if (lastMessage.role === 'assistant' && !vercelChatState.isLoading) {
      const saveMessage = async () => {
        try {
          // Check if this message already exists in the database
          const existingMessage = await messageRepository.getMessageById(lastMessage.id);
          
          if (!existingMessage) {
            // Save the message
            await messageRepository.createMessage({
              ...lastMessage,
              threadId: currentThreadId
            });
            
            // Update thread's updatedAt timestamp
            await threadRepository.updateThread(currentThreadId, {
              updatedAt: Date.now()
            });
            
            console.log('Saved assistant message:', lastMessage.id);
          }
        } catch (error) {
          console.error('Error saving assistant message:', error);
        }
      };
      
      saveMessage();
    }
  }, [vercelChatState.messages, vercelChatState.isLoading, currentThreadId, persistenceEnabled]);
  
  // Switch to another thread
  const switchThread = useCallback(async (threadId: string) => {
    try {
      // Check if thread exists
      const thread = await threadRepository.getThreadById(threadId);
      
      if (!thread) {
        throw new Error(`Thread ${threadId} not found`);
      }
      
      // Load messages for this thread
      const messages = await messageRepository.getMessagesByThreadId(threadId);
      
      // Reset loading state and clear current messages
      vercelChatState.setMessages([]);
      initialMessagesLoaded.current = false;
      
      // Update the current thread ID
      setCurrentThreadId(threadId);
      
      // Set the messages
      if (messages.length > 0) {
        vercelChatState.setMessages(messages);
        initialMessagesLoaded.current = true;
      }
      
      // Call the callback if provided
      onThreadChange?.(threadId);
      
      console.log(`Switched to thread ${threadId} with ${messages.length} messages`);
    } catch (error) {
      console.error('Error switching thread:', error);
      throw error;
    }
  }, [vercelChatState.setMessages, onThreadChange]);
  
  // Create a new thread
  const createNewThread = useCallback(async (title?: string) => {
    try {
      // Create a new thread
      const newThread = await threadRepository.createThread({
        title: title || 'New Chat',
      });
      
      // Reset loading state and clear current messages
      vercelChatState.setMessages([]);
      initialMessagesLoaded.current = false;
      
      // Update the current thread ID
      setCurrentThreadId(newThread.id);
      
      // Call the callback if provided
      onThreadChange?.(newThread.id);
      
      console.log('Created new thread:', newThread.id);
      
      return newThread;
    } catch (error) {
      console.error('Error creating new thread:', error);
      throw error;
    }
  }, [vercelChatState.setMessages, onThreadChange]);
  
  // Delete a thread
  const deleteThread = useCallback(async (threadId: string) => {
    try {
      // Delete all messages for this thread
      await messageRepository.deleteMessagesByThreadId(threadId);
      
      // Delete the thread
      const result = await threadRepository.deleteThread(threadId);
      
      // If we deleted the current thread, create a new one
      if (threadId === currentThreadId) {
        const threads = await threadRepository.getAllThreads();
        
        if (threads.length > 0) {
          // Switch to the first thread
          await switchThread(threads[0].id);
        } else {
          // Create a new thread
          await createNewThread();
        }
      }
      
      return result;
    } catch (error) {
      console.error('Error deleting thread:', error);
      return false;
    }
  }, [currentThreadId, switchThread, createNewThread]);
  
  // Update thread metadata
  const updateThread = useCallback(async (threadId: string, updates: Partial<Thread>) => {
    try {
      return await threadRepository.updateThread(threadId, updates);
    } catch (error) {
      console.error('Error updating thread:', error);
      return null;
    }
  }, []);
  
  // Delete a message
  const deleteMessage = useCallback(async (messageId: string) => {
    try {
      // Delete from the database
      const result = await messageRepository.deleteMessage(messageId);
      
      // If successful, also remove from the UI
      if (result) {
        const updatedMessages = vercelChatState.messages.filter(
          message => message.id !== messageId
        );
        
        vercelChatState.setMessages(updatedMessages);
      }
      
      return result;
    } catch (error) {
      console.error('Error deleting message:', error);
      return false;
    }
  }, [vercelChatState.messages, vercelChatState.setMessages]);
  
  // Get all threads
  const getAllThreads = useCallback(async () => {
    try {
      return await threadRepository.getAllThreads();
    } catch (error) {
      console.error('Error getting all threads:', error);
      return [];
    }
  }, []);
  
  // Combine all functions and state
  return {
    ...vercelChatState,
    append,
    setMessages,
    currentThreadId,
    switchThread,
    createNewThread,
    getAllThreads,
    deleteThread,
    updateThread,
    deleteMessage
  };
}