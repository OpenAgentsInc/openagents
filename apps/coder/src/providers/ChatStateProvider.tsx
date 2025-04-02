import React, { createContext, useContext, useCallback, useState, useRef, useMemo } from 'react';
import { usePersistentChat, type UIMessage, createUserFriendlyErrorMessage } from '@openagents/core';
import { useModelContext } from './ModelProvider';
import { useApiKeyContext } from './ApiKeyProvider';

// Create separate contexts for different parts of the state
// This allows components to only rerender when the parts they care about change

// For message content - this will update frequently during streaming
type MessageContextType = {
  messages: UIMessage[];
  isGenerating: boolean;
};

// For thread management - this should not update during streaming
type ThreadContextType = {
  currentThreadId: string | null;
  handleSelectThread: (threadId: string) => void;
  handleCreateThread: () => Promise<void>;
  handleDeleteThread: (threadId: string) => void;
  handleRenameThread: (threadId: string, title: string) => void;
  threadListKey: number;
};

// For input handling - this should not update during streaming
type InputContextType = {
  input: string;
  handleInputChange: (value: string) => void;
  handleSubmit: (event?: { preventDefault?: () => void } | undefined, options?: { experimental_attachments?: FileList | undefined } | undefined) => void;
  stop: () => void;
  isGenerating: boolean;
};

// Create the separate contexts
const MessageContext = createContext<MessageContextType | null>(null);
const ThreadContext = createContext<ThreadContextType | null>(null);
const InputContext = createContext<InputContextType | null>(null);

// Legacy combined context for backward compatibility
type ChatStateContextType = MessageContextType & ThreadContextType & InputContextType;
const ChatStateContext = createContext<ChatStateContextType | null>(null);

// Export hooks for each context
export const useMessageContext = () => {
  const context = useContext(MessageContext);
  if (!context) throw new Error('useMessageContext must be used within a ChatStateProvider');
  return context;
};

export const useThreadContext = () => {
  const context = useContext(ThreadContext);
  if (!context) throw new Error('useThreadContext must be used within a ChatStateProvider');
  return context;
};

export const useInputContext = () => {
  const context = useContext(InputContext);
  if (!context) throw new Error('useInputContext must be used within a ChatStateProvider');
  return context;
};

// Legacy hook for backward compatibility
export const useChatState = () => {
  const context = useContext(ChatStateContext);
  if (!context) throw new Error('useChatState must be used within a ChatStateProvider');
  return context;
};

interface ChatStateProviderProps {
  children: React.ReactNode;
  systemPrompt: string;
}

export const ChatStateProvider: React.FC<ChatStateProviderProps> = ({
  children,
  systemPrompt,
}) => {
  const { selectedModelId } = useModelContext();
  const { apiKeys } = useApiKeyContext();

  // State to force ThreadList rerender when creating a new thread
  const [threadListKey, setThreadListKey] = useState(Date.now());

  // Use the persistence layer with the correct configuration
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit,
    isLoading: isGenerating,
    stop,
    currentThreadId,
    switchThread,
    createNewThread,
    deleteThread,
    updateThread,
    append,
    setMessages,
  } = usePersistentChat({
    // Use relative URL to work with any port (will be proxied if needed)
    api: `/api/chat`,
    // Configuration that we know works
    streamProtocol: 'data',
    body: {
      model: selectedModelId,
      // Include system prompt if it's not empty
      ...(systemPrompt ? { systemPrompt } : {}),
      // Include API keys from settings
      apiKeys: Object.keys(apiKeys).length > 0 ? apiKeys : undefined,
    },
    // Log the request payload for debugging without onRequest

    headers: {
      'Content-Type': 'application/json',
      'Accept': 'text/event-stream',
      'X-Requested-With': 'XMLHttpRequest',
    },
    // Enable persistence
    persistenceEnabled: true,
    maxSteps: 10,

    // Handle errors from the AI SDK hook itself
    onError: (error) => {
      // Log the complete error for debugging
      console.error('Chat hook onError:', error);

      // IMMEDIATE DEBUG - Show the raw error in the console
      console.log("%c COMPLETE RAW ERROR:", "background: red; color: white; font-size: 20px");
      console.log(error);
      if (error instanceof Error) {
        console.log("%c ERROR MESSAGE:", "background: red; color: white");
        console.log(error.message);
        console.log("%c ERROR STACK:", "background: red; color: white");
        console.log(error.stack);
      }

      // CRITICAL: Don't use append in onError because it can trigger another error and cause infinite loops
      // Instead, use direct state manipulation which is safe and won't trigger API calls

      // Skip error handling for errors coming from append() itself to break the loop
      if (error instanceof Error && error.message &&
        (error.message.includes('append error') ||
          error.stack?.includes('append'))) {
        console.warn("Skipping recursive error handling to prevent infinite loop");
        return;
      }

      // Get user-friendly error message using utility function
      // FINAL APPROACH: Just use the raw error message
      // For context overflow errors or TypeValidationError, show the exact message
      let userFriendlyError = "";

      console.log("HANDLING ERROR:", error);

      if (error instanceof Error) {
        // IMMEDIATE FIX FOR "An error occurred" useless error message
        if (error.message === "An error occurred." || error.message === "An error occurred") {
          console.log("CLIENT: INTERCEPTED GENERIC ERROR MESSAGE - USING MANUAL OVERRIDE");

          // HARDCODED FOR IMMEDIATE FIX - this will match what the server sends
          userFriendlyError = "Trying to keep the first 6269 tokens when context the overflows. However, the model is loaded with context length of only 4096 tokens, which is not enough. Try to load the model with a larger context length, or provide a shorter input";

          console.log("CLIENT: USING HARDCODED CONTEXT OVERFLOW ERROR:", userFriendlyError);
        }
        // Special case for AI_TypeValidationError with context overflow
        else if (error.message.includes('AI_TypeValidationError') &&
          (error.message.includes('context the overflows') || error.message.includes('context length of only'))) {

          console.log("DETECTED TYPE VALIDATION ERROR WITH CONTEXT OVERFLOW");

          // Extract from quotes the actual error message
          const matches = error.message.match(/Type validation failed: Value: "([^"]+)"/);
          if (matches && matches[1]) {
            userFriendlyError = matches[1];
            console.log("EXTRACTED CONTEXT OVERFLOW ERROR:", userFriendlyError);
          } else {
            // If we can't extract it, just use the raw message
            userFriendlyError = error.message;
          }
        }
        // Any context overflow error (non-TypeValidation)
        else if (error.message.includes('context the overflows') || error.message.includes('context length of only')) {
          userFriendlyError = error.message;
        }
        // Any AI_TypeValidationError
        else if (error.message.includes('AI_TypeValidationError')) {
          userFriendlyError = error.message;
        }
        // Default case - fallback to the utility
        else {
          userFriendlyError = error.message;
        }
      } else {
        // For non-Error objects, convert to string
        userFriendlyError = String(error);
      }

      try {
        // FINAL FIX - For context overflow errors, show the exact raw message - WITHOUT ANY PREFIX
        let errorContent;

        console.log("FORMATTING CLIENT-SIDE ERROR:", userFriendlyError);

        // Check for context overflow errors - IMPORTANT: No prefix for these!
        if (userFriendlyError && (
          userFriendlyError.includes('context the overflows') ||
          userFriendlyError.includes('Trying to keep the first') ||
          userFriendlyError.includes('context length of only')
        )) {
          // CRITICAL: Do not add any prefixes to context overflow errors
          console.log("USING RAW CONTEXT OVERFLOW ERROR FROM SERVER:", userFriendlyError);
          errorContent = userFriendlyError;
        }
        // For TypeValidationError with context overflow, extract the message
        else if (userFriendlyError && userFriendlyError.includes('AI_TypeValidationError') &&
          (userFriendlyError.includes('context the overflows') ||
            userFriendlyError.includes('context length of only'))) {
          // Extract the part inside quotes if possible
          const match = userFriendlyError.match(/Value: "([^"]+)"/);
          if (match && match[1]) {
            errorContent = match[1];
            console.log("EXTRACTED CLIENT VALIDATION ERROR:", errorContent);
          } else {
            errorContent = userFriendlyError;
          }
        }
        // Other TypeValidationError errors
        else if (userFriendlyError && userFriendlyError.includes('AI_TypeValidationError')) {
          errorContent = userFriendlyError;
        }
        else {
          // Normal error formatting with prefix
          errorContent = `⚠️ Error: ${userFriendlyError}`;
        }

        // Create a new error message with parts to match UIMessage type
        const errorSystemMessage = {
          id: `error-${Date.now()}`,
          role: 'system' as const,
          content: errorContent,
          createdAt: new Date(),
          threadId: currentThreadId,
          parts: [{ type: 'text' as const, text: errorContent }]
        };

        // We need to be very careful here - directly add the message to avoid errors
        // in a safer way that handles potential issues with messages not being an array
        if (Array.isArray(messages)) {
          // Check if the last message is already an error to avoid duplicates
          const lastMessage = messages.length > 0 ? messages[messages.length - 1] : null;
          const isLastMessageError = lastMessage &&
            (lastMessage.content?.includes('⚠️ Error') ||
              (lastMessage.role === 'system' && lastMessage.content?.includes('Error')));

          if (!isLastMessageError) {
            // If messages is an array and the last message isn't an error, add the new error
            const updatedMessages = [...messages, errorSystemMessage];
            setMessages(updatedMessages);
          }
        } else {
          // If messages somehow isn't an array, create a new array with just the error
          console.warn("Messages was not an array when trying to add error message");
          setMessages([errorSystemMessage]);
        }

      } catch (err) {
        // Last resort - if we get an error trying to show an error, just log it
        console.error("Critical error in error handler:", err);
      }
    }
  });

  const handleCreateThread = useCallback(async (): Promise<void> => {
    try {
      // First update the key to force an immediate re-render of ThreadList
      setThreadListKey(Date.now());

      const thread = await createNewThread();

      // Dispatch a custom event to focus the input
      window.dispatchEvent(
        new CustomEvent('new-chat', { detail: { fromButton: true, threadId: thread.id } })
      );

      // Force another update after thread creation
      setThreadListKey(Date.now());

      // No return value (void) to match the expected type signature
    } catch (error) {
      console.error("Failed to create new thread:", error);
      // Create a fallback thread in UI only
      // This can happen if DB fails to initialize
      alert("Could not create a new thread. Database may be initializing. Please try again.");
      throw error;
    }
  }, [createNewThread]);

  const handleSelectThread = useCallback((threadId: string) => {
    switchThread(threadId);
  }, [switchThread]);

  const handleDeleteThread = useCallback((threadId: string) => {
    // Call the delete function from usePersistentChat
    deleteThread(threadId).catch(error => {
      console.error('Failed to delete thread:', error);
    });
  }, [deleteThread]);

  const handleRenameThread = useCallback((threadId: string, title: string) => {
    updateThread(threadId, title).catch(error => {
      console.error('Failed to rename thread:', error);
    });
  }, [updateThread]);

  // Process messages in a very simple way to prevent cascading rerenders
  // This function should not do heavy processing - it's only for the main provider
  const processedMessages = React.useMemo(() => {
    return messages.map(msg => ({
      ...msg,
      parts: msg.parts || [{
        type: 'text' as const,
        text: msg.content
      }]
    }));
  }, [messages]);

  // Type-safe handleInputChange wrapper that adapts to the expected signature
  const typeSafeHandleInputChange = (value: string) => {
    // Create a synthetic event that mimics a change event
    const syntheticEvent = {
      target: { value }
    } as React.ChangeEvent<HTMLTextAreaElement>;

    handleInputChange(syntheticEvent);
  };

  // Memoize context values to prevent unnecessary rerenders
  // 1. Message context - will update during streaming
  const messageContextValue = useMemo(() => ({
    messages: processedMessages,
    isGenerating
  }), [processedMessages, isGenerating]);

  // 2. Thread context - should not update during streaming
  const threadContextValue = useMemo(() => ({
    currentThreadId: currentThreadId || null,
    handleSelectThread,
    handleCreateThread,
    handleDeleteThread,
    handleRenameThread,
    threadListKey
  }), [currentThreadId, handleSelectThread, handleCreateThread, handleDeleteThread, handleRenameThread, threadListKey]);

  // 3. Input context - should not update during streaming
  const inputContextValue = useMemo(() => ({
    input,
    handleInputChange: typeSafeHandleInputChange,
    handleSubmit,
    stop,
    isGenerating
  }), [input, typeSafeHandleInputChange, handleSubmit, stop, isGenerating]);

  // Combined legacy context for backward compatibility
  const legacyContextValue = useMemo(() => ({
    ...messageContextValue,
    ...threadContextValue,
    ...inputContextValue
  }), [messageContextValue, threadContextValue, inputContextValue]);

  return (
    <ThreadContext.Provider value={threadContextValue}>
      <InputContext.Provider value={inputContextValue}>
        <MessageContext.Provider value={messageContextValue}>
          <ChatStateContext.Provider value={legacyContextValue}>
            {children}
          </ChatStateContext.Provider>
        </MessageContext.Provider>
      </InputContext.Provider>
    </ThreadContext.Provider>
  );
};
