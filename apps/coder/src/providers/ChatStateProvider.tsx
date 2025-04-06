import React, { createContext, useContext, useCallback, useState, useRef, useMemo, useEffect } from 'react';
import { type UIMessage, createUserFriendlyErrorMessage, useAgentChat, createAgentRouterProvider } from '@openagents/core';
import { useModelContext } from './ModelProvider';
import { useApiKeyContext } from './ApiKeyProvider';
import { showNetworkErrorToast } from '@/components/ui/NetworkErrorNotification';
import { Link } from '@tanstack/react-router';

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
  const [error, setError] = useState<Error | null>(null);

  // State to force ThreadList rerender when creating a new thread
  const [threadListKey, setThreadListKey] = useState(Date.now());

  // Create the agent router provider
  const agentRouterProvider = useMemo(() => {
    try {
      if (!apiKeys?.openrouter) {
        throw new Error('OpenRouter API key is required. Please add your API key in Settings.');
      }
      return createAgentRouterProvider(selectedModelId, apiKeys.openrouter, {
        baseURL: '/api/openrouter'
      });
    } catch (e) {
      setError(e as Error);
      return null;
    }
  }, [selectedModelId, apiKeys?.openrouter]);

  // Create empty contexts if there's an error
  const emptyContexts = {
    messages: [],
    isGenerating: false,
    input: '',
    handleInputChange: () => { },
    handleSubmit: () => { },
    stop: () => { },
    currentThreadId: null,
    handleSelectThread: () => { },
    handleCreateThread: async () => { },
    handleDeleteThread: () => { },
    handleRenameThread: () => { },
    threadListKey: Date.now(),
  };

  // If there's an error or no provider, provide empty contexts but still render children
  if (error || !agentRouterProvider) {
    const messageContextValue = { messages: [], isGenerating: false };
    const threadContextValue = {
      currentThreadId: null,
      handleSelectThread: () => { },
      handleCreateThread: async () => { },
      handleDeleteThread: () => { },
      handleRenameThread: () => { },
      threadListKey: Date.now(),
    };
    const inputContextValue = {
      input: '',
      handleInputChange: (value: string) => { },
      handleSubmit: (event?: { preventDefault?: () => void }) => { event?.preventDefault?.(); },
      stop: () => { },
      isGenerating: false,
    };
    const chatStateContextValue = { ...messageContextValue, ...threadContextValue, ...inputContextValue };

    return (
      <MessageContext.Provider value={messageContextValue}>
        <ThreadContext.Provider value={threadContextValue}>
          <InputContext.Provider value={inputContextValue}>
            <ChatStateContext.Provider value={chatStateContextValue}>
              {children}
              {error && (
                <div className="flex flex-col items-center justify-center h-full p-4">
                  <div className="bg-red-50 border border-red-200 rounded-lg p-4 max-w-md">
                    <h3 className="text-red-800 font-semibold mb-2">Configuration Error</h3>
                    <p className="text-red-600">{error.message}</p>
                    <div className="flex items-center justify-center mt-4">
                      <Link to="/settings/models" className="bg-red-100 hover:bg-red-200 text-red-700 px-4 py-2 rounded-md text-sm font-medium transition-colors">
                        Go to Settings
                      </Link>
                    </div>
                  </div>
                </div>
              )}
            </ChatStateContext.Provider>
          </InputContext.Provider>
        </ThreadContext.Provider>
      </MessageContext.Provider>
    );
  }

  // Use our new agent chat hook
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
  } = useAgentChat({
    agentRouterProvider,
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
    headers: {
      'Content-Type': 'application/json',
      'Accept': 'text/event-stream',
      'X-Requested-With': 'XMLHttpRequest',
    },
    // Enable persistence
    persistenceEnabled: true,
    maxSteps: 50,

    // Handle errors from the AI SDK hook itself
    onError: (error: Error) => {
      console.error('Chat hook onError:', error);

      if (error.message === 'Failed to fetch') {
        const networkErrorMessage: UIMessage = {
          id: `error-${Date.now()}`,
          role: 'system',
          content: "⚠️ Network Error: Unable to connect to AI service. Please check your network connection and try again.",
          createdAt: new Date(),
          threadId: currentThreadId || undefined,
          parts: [{
            type: 'text',
            text: "⚠️ Network Error: Unable to connect to AI service. Please check your network connection and try again."
          }]
        };

        showNetworkErrorToast(() => {
          console.log("User requested connection retry");
        });

        const currentMessages = [...messages, networkErrorMessage];
        setMessages(currentMessages);
        return;
      }

      // Create user-friendly error message
      const userFriendlyError = createUserFriendlyErrorMessage(error);
      const errorMessage: UIMessage = {
        id: `error-${Date.now()}`,
        role: 'system',
        content: userFriendlyError,
        createdAt: new Date(),
        threadId: currentThreadId || undefined,
        parts: [{
          type: 'text',
          text: userFriendlyError
        }]
      };

      const currentMessages = [...messages, errorMessage];
      setMessages(currentMessages);
    }
  });

  // Debug logging for useAgentChat state
  useEffect(() => {
    console.log('[ChatStateProvider] Current messages:', messages);
    console.log('[ChatStateProvider] Current thread ID:', currentThreadId);
    console.log('[ChatStateProvider] Is generating:', isGenerating);
  }, [messages, currentThreadId, isGenerating]);

  // Thread management handlers
  const handleSelectThread = useCallback((threadId: string) => {
    switchThread(threadId);
  }, [switchThread]);

  const handleCreateThread = useCallback(async () => {
    await createNewThread();
    setThreadListKey(Date.now());
  }, [createNewThread]);

  const handleDeleteThread = useCallback(async (threadId: string) => {
    await deleteThread(threadId);
    setThreadListKey(Date.now());
  }, [deleteThread]);

  const handleRenameThread = useCallback(async (threadId: string, title: string) => {
    await updateThread(threadId, title);
    setThreadListKey(Date.now());
  }, [updateThread]);

  // Create a wrapper for handleInputChange to match the expected type
  const handleInputChangeWrapper = useCallback((value: string) => {
    console.log('[ChatStateProvider] Input changed:', value);
    handleInputChange({ target: { value } } as React.ChangeEvent<HTMLTextAreaElement>);
  }, [handleInputChange]);

  // Create a wrapper for handleSubmit to prevent page refresh
  const handleSubmitWrapper = useCallback((
    event?: { preventDefault?: () => void },
    options?: { experimental_attachments?: FileList }
  ) => {
    console.log('[ChatStateProvider] Submitting with options:', options);
    console.log('[ChatStateProvider] Current input state:', input);
    console.log('[ChatStateProvider] Current messages:', messages);
    event?.preventDefault?.();
    console.log('[ChatStateProvider] Calling handleSubmit from useAgentChat');
    const result = handleSubmit(event, options);
    console.log('[ChatStateProvider] handleSubmit result:', result);
    return result;
  }, [handleSubmit, input, messages]);

  // Create the separate context values
  const messageContextValue = useMemo(() => {
    console.log('[ChatStateProvider] Creating message context with:', { messages, isGenerating });
    return {
      messages,
      isGenerating,
    };
  }, [messages, isGenerating]);

  const threadContextValue = useMemo(() => {
    console.log('[ChatStateProvider] Creating thread context with:', { currentThreadId });
    return {
      currentThreadId: currentThreadId || null,
      handleSelectThread,
      handleCreateThread,
      handleDeleteThread,
      handleRenameThread,
      threadListKey,
    };
  }, [currentThreadId, handleSelectThread, handleCreateThread, handleDeleteThread, handleRenameThread, threadListKey]);

  const inputContextValue = useMemo(() => {
    console.log('[ChatStateProvider] Creating input context with:', { input, isGenerating });
    return {
      input,
      handleInputChange: handleInputChangeWrapper,
      handleSubmit: handleSubmitWrapper,
      stop,
      isGenerating,
    };
  }, [input, handleInputChangeWrapper, handleSubmitWrapper, stop, isGenerating]);

  // Legacy combined context value
  const chatStateContextValue = useMemo(() => ({
    ...messageContextValue,
    ...threadContextValue,
    ...inputContextValue,
  }), [messageContextValue, threadContextValue, inputContextValue]);

  return (
    <MessageContext.Provider value={messageContextValue}>
      <ThreadContext.Provider value={threadContextValue}>
        <InputContext.Provider value={inputContextValue}>
          <ChatStateContext.Provider value={chatStateContextValue}>
            {children}
          </ChatStateContext.Provider>
        </InputContext.Provider>
      </ThreadContext.Provider>
    </MessageContext.Provider>
  );
};
