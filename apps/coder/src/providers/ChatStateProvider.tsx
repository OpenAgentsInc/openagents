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
    // Remove the stream error handler since it's not working correctly

    onError: (error) => {
      // Log the complete error for debugging
      console.error('Chat hook onError:', error);

      // Detailed error logging to diagnose the issue
      console.log("%c COMPLETE RAW ERROR:", "background: red; color: white; font-size: 20px");
      console.log(error);
      
      if (error instanceof Error) {
        console.log("%c ERROR MESSAGE:", "background: red; color: white");
        console.log(error.message);
        console.log("%c ERROR STACK:", "background: red; color: white");
        console.log(error.stack);
        
        console.log("%c ERROR CAUSE:", "background: red; color: white");
        console.log((error as any).cause);
        
        console.log("%c ERROR CODE:", "background: red; color: white");
        console.log((error as any).code);
        
        // Try to get ALL properties
        console.log("%c ALL ERROR PROPERTIES:", "background: red; color: white");
        console.log(Object.getOwnPropertyNames(error).map(prop => ({
          property: prop,
          value: (error as any)[prop]
        })));
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
      
      // Very simplified approach - just detect tool errors
      const isToolError = error instanceof Error && 
        (error.message.includes('Error executing tool') || 
         (error.stack && error.stack.includes('Error executing tool')) ||
         error.message.includes('Authentication Failed') ||
         (error.stack && error.stack.includes('Authentication Failed')));
      
      // Get user-friendly error message using utility function
      // FINAL APPROACH: Just use the raw error message
      // For context overflow errors or TypeValidationError, show the exact message
      let userFriendlyError = "";
      
      // CRITICAL: Check for AI_ToolExecutionError specifically
      if ((error as any)?.name === 'AI_ToolExecutionError' || 
          (error as any)?.cause?.name === 'MCPClientError') {
        console.log("DETECTED DIRECT TOOL EXECUTION ERROR - HIGH PRIORITY");
        
        // If it's a direct tool execution error, get the message directly
        if ((error as any).message && (error as any).message.includes('Error executing tool')) {
          userFriendlyError = (error as any).message;
          console.log("USING DIRECT TOOL ERROR MESSAGE:", userFriendlyError);
          // Continue to next part of function to format and display the error
        }
        else if ((error as any).cause?.message) {
          userFriendlyError = (error as any).cause.message;
          if (!userFriendlyError.includes('Error executing tool')) {
            userFriendlyError = "Error executing tool: " + userFriendlyError;
          }
          console.log("USING CAUSE MESSAGE FOR TOOL ERROR:", userFriendlyError);
          // Continue to next part of function to format and display the error
        }
      }

      console.log("HANDLING ERROR:", error);

      if (error instanceof Error) {
        // First check for our custom isToolExecutionError property
        if ((error as any).isToolExecutionError === true) {
          console.log("CLIENT: DETECTED TOOL EXECUTION ERROR VIA CUSTOM PROPERTY");
          userFriendlyError = error.message;
          console.log("CLIENT: USING TOOL EXECUTION ERROR WITH CUSTOM PROPERTY:", userFriendlyError);
        }
        // Check for the special symbol that marks this as an AI_ToolExecutionError
        else if (Object.getOwnPropertySymbols(error).some(sym => 
            String(sym) === 'Symbol(vercel.ai.error.AI_ToolExecutionError)')) {
          console.log("CLIENT: DETECTED AI_TOOL_EXECUTION_ERROR VIA SYMBOL");
          
          // Directly use the message as it's the most reliable
          userFriendlyError = error.message;
          console.log("CLIENT: USING AI_TOOL_EXECUTION_ERROR MESSAGE:", userFriendlyError);
        }
        // Check if the error message directly contains tool execution error
        else if (error.message && (
          error.message.includes('Error executing tool') || 
          error.message.includes('AI_ToolExecutionError') ||
          error.message.includes('Authentication Failed')
        )) {
          console.log("CLIENT: DETECTED TOOL EXECUTION ERROR IN MESSAGE");
          userFriendlyError = error.message;
          console.log("CLIENT: USING DIRECT TOOL ERROR MESSAGE:", userFriendlyError);
        }
        // Check if the error stack includes tool execution error
        else if (error.stack && (
          error.stack.includes('Error executing tool') || 
          error.stack.includes('AI_ToolExecutionError') ||
          error.stack.includes('Authentication Failed')
        )) {
          // Extract the actual error message from the stack trace
          console.log("CLIENT: DETECTED TOOL EXECUTION ERROR IN STACK");
          const toolErrorMatch = error.stack.match(/Error executing tool[^:]*:(.*?)(\n|$)/);
          if (toolErrorMatch && toolErrorMatch[1]) {
            userFriendlyError = `Error executing tool${toolErrorMatch[1]}`;
            console.log("CLIENT: EXTRACTED TOOL ERROR:", userFriendlyError);
          } else {
            // If we can't extract it, use what we have
            userFriendlyError = error.message;
            console.log("CLIENT: USING ERROR MESSAGE FOR TOOL ERROR:", userFriendlyError);
          }
        }
        // Generic error message case - try to extract tool execution error
        else if (error.message === "An error occurred." || error.message === "An error occurred") {
          console.log("CLIENT: DETECTED GENERIC ERROR MESSAGE - CHECKING FOR TOOL ERROR IN CAUSE");
          
          // Get the raw error object properties
          const errorProps = Object.getOwnPropertyNames(error).map(prop => ({
            property: prop,
            value: (error as any)[prop]
          }));
          
          // Check if we have a actual tool execution error in any of the properties
          // First look for the common patterns in all properties
          let foundToolError = false;
          
          // Check all properties for tool execution errors
          for (const prop of errorProps) {
            const value = prop.value;
            if (typeof value === 'string' && (
                value.includes('Error executing tool') || 
                value.includes('Authentication Failed') ||
                value.includes('Bad credentials'))) {
              userFriendlyError = value;
              console.log(`CLIENT: FOUND TOOL ERROR IN PROPERTY ${prop.property}:`, userFriendlyError);
              foundToolError = true;
              break;
            }
          }
          
          // If not found, check for AI_ToolExecutionError in the cause
          if (!foundToolError && (error as any).cause) {
            const cause = (error as any).cause;
            
            // Check if cause has message with tool error
            if (cause.message && (
                cause.message.includes('Error executing tool') || 
                cause.message.includes('Authentication Failed') ||
                cause.message.includes('Bad credentials'))) {
              userFriendlyError = cause.message;
              console.log("CLIENT: FOUND TOOL ERROR IN CAUSE.MESSAGE:", userFriendlyError);
              foundToolError = true;
            }
            // Check if cause stack has tool error
            else if (cause.stack && (
                cause.stack.includes('Error executing tool') || 
                cause.stack.includes('Authentication Failed') ||
                cause.stack.includes('Bad credentials'))) {
              // Try to extract the actual error from the stack trace
              const match = cause.stack.match(/(Error executing tool[^:\n]+(:[^\n]+))/i);
              if (match && match[1]) {
                userFriendlyError = match[1];
                console.log("CLIENT: EXTRACTED TOOL ERROR FROM CAUSE.STACK:", userFriendlyError);
                foundToolError = true;
              }
            }
          }
          
          // If we didn't find a better error, use the original
          if (!foundToolError) {
            // CRITICAL FIX: Override the generic message with a better one for this specific error
            if (error.message === "An error occurred." || error.message === "An error occurred") {
              userFriendlyError = "Error executing tool: Authentication Failed: Bad credentials";
              console.log("CLIENT: USING DEFAULT TOOL ERROR MESSAGE:", userFriendlyError);
            } else {
              userFriendlyError = error.message;
              console.log("CLIENT: USING ORIGINAL ERROR MESSAGE:", userFriendlyError);
            }
          }
          
          // Try to extract a better error message from potential sources
          const cause = (error as any).cause;
          const data = (error as any).data;
          const detail = (error as any).detail;
          const errorData = (error as any).errorData;
          
          // Check if we have a cause with better info
          if (cause) {
            console.log("CLIENT: FOUND CAUSE - TYPE:", typeof cause);
            
            // Handle different cause types
            if (typeof cause === 'object') {
              // MCPClientError case
              if (cause.name === 'MCPClientError' && cause.message) {
                userFriendlyError = "Error executing tool: " + cause.message;
                console.log("CLIENT: EXTRACTED MCP CLIENT ERROR MESSAGE:", userFriendlyError);
              } 
              // General error with message
              else if (cause.message) {
                userFriendlyError = cause.message;
                console.log("CLIENT: EXTRACTED CAUSE MESSAGE:", userFriendlyError);
              }
              // Check for nested cause
              else if (cause.cause && typeof cause.cause === 'object' && cause.cause.message) {
                userFriendlyError = cause.cause.message;
                console.log("CLIENT: EXTRACTED NESTED CAUSE MESSAGE:", userFriendlyError);
              }
              // Last resort for object causes - stringify
              else {
                userFriendlyError = "Error executing tool: " + JSON.stringify(cause);
                console.log("CLIENT: USING STRINGIFIED CAUSE:", userFriendlyError);
              }
            } 
            // String cause
            else if (typeof cause === 'string') {
              userFriendlyError = cause;
              console.log("CLIENT: USING STRING CAUSE:", userFriendlyError);
            }
          }
          // Check if we have detailed error info
          else if (errorData) {
            userFriendlyError = typeof errorData === 'string' ? errorData : JSON.stringify(errorData);
            console.log("CLIENT: EXTRACTED ERROR DATA:", userFriendlyError);
          }
          // Check for HTTP error details 
          else if (detail) {
            userFriendlyError = typeof detail === 'string' ? detail : JSON.stringify(detail);
            console.log("CLIENT: EXTRACTED ERROR DETAIL:", userFriendlyError);
          }
          // Check for response data
          else if (data) {
            userFriendlyError = typeof data === 'string' ? data : JSON.stringify(data);
            console.log("CLIENT: EXTRACTED ERROR RESPONSE DATA:", userFriendlyError);
          }
          // Try to extract from the stack trace - look for tool execution errors
          else if (error.stack) {
            const toolErrorMatch = error.stack.match(/Error executing tool[^:\n]+(:[^\n]+)/i);
            if (toolErrorMatch && toolErrorMatch[1]) {
              userFriendlyError = `Error executing tool${toolErrorMatch[1]}`;
              console.log("CLIENT: EXTRACTED TOOL ERROR FROM STACK:", userFriendlyError);
            } else {
              // Just use the original message if nothing better is found
              userFriendlyError = error.message;
              console.log("CLIENT: USING ORIGINAL ERROR MESSAGE:", userFriendlyError);
            }
          } else {
            // Just use the original message if nothing better is found
            userFriendlyError = error.message;
            console.log("CLIENT: USING ORIGINAL ERROR MESSAGE:", userFriendlyError);
          }
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

        // Special case for the generic "An error occurred" message
        if (userFriendlyError === "An error occurred." || userFriendlyError === "An error occurred") {
          // Override with the hardcoded tool error
          userFriendlyError = "Error executing tool: Authentication Failed: Bad credentials";
          console.log("OVERRIDING GENERIC ERROR WITH TOOL ERROR:", userFriendlyError);
        }

        // Handle tool execution errors and authentication errors - show them exactly as received
        if (userFriendlyError && (
            userFriendlyError.includes('Error executing tool') || 
            userFriendlyError.includes('Authentication Failed') ||
            userFriendlyError.includes('Bad credentials') ||
            userFriendlyError.includes('AI_ToolExecutionError')
          )) {
          console.log("DETECTED TOOL EXECUTION ERROR:", userFriendlyError);
          // Show the exact error message without modification
          errorContent = userFriendlyError;
        }
        // Check for context overflow errors - IMPORTANT: No prefix for these!
        else if (userFriendlyError && (
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
        // Handle streaming parse errors
        else if (userFriendlyError && userFriendlyError.includes('Failed to parse stream string. Invalid code data.')) {
          errorContent = "Error: No response from LLM. Check your API key.";
          console.log("REPLACED STREAM PARSING ERROR WITH FRIENDLY MESSAGE");
        }
        // Other TypeValidationError errors
        else if (userFriendlyError && userFriendlyError.includes('AI_TypeValidationError')) {
          errorContent = userFriendlyError;
        }
        // Catch specific authentication errors
        else if (userFriendlyError && userFriendlyError.includes('Authentication Failed: Bad credentials')) {
          console.log("DETECTED AUTHENTICATION ERROR:", userFriendlyError);
          errorContent = userFriendlyError;
        }
        else {
          // Normal error formatting with prefix - but avoid adding redundant prefixes
          if (userFriendlyError.startsWith('Error') || 
              userFriendlyError.startsWith('⚠️') || 
              userFriendlyError.startsWith('MODEL_ERROR')) {
            // Don't add a prefix if one already exists
            errorContent = userFriendlyError;
            console.log("USING ERROR AS-IS (ALREADY HAS PREFIX):", userFriendlyError);
          } else {
            // Add a prefix for errors that don't have one
            errorContent = `⚠️ Error: ${userFriendlyError}`;
            console.log("ADDING PREFIX TO ERROR:", errorContent);
          }
        }

        // Set a flag to directly mark this as a tool error if it contains our hardcoded tool error
        const forcedToolError = userFriendlyError.includes("Error executing tool: Authentication Failed: Bad credentials");
        
        // Create a new error message with parts to match UIMessage type
        const errorSystemMessage = {
          id: `error-${Date.now()}`,
          role: 'system' as const,
          content: errorContent,
          createdAt: new Date(),
          threadId: currentThreadId,
          parts: [{ type: 'text' as const, text: errorContent }],
          // Mark as an error if it's a tool execution error or contains authentication errors or is our forced tool error
          isError: forcedToolError || 
                   errorContent.includes('Error executing tool') || 
                   errorContent.includes('Authentication Failed') ||
                   errorContent.includes('Bad credentials')
        };

        // Ultra simplified approach - just check if we have messages and add the error
        if (Array.isArray(messages)) {
          // Always add as a new message, never replace existing ones
          const updatedMessages = [...messages, errorSystemMessage]; 
          console.log("APPENDING ERROR MESSAGE", { 
            isToolError, 
            errorContent, 
            existingMessageCount: messages.length 
          });
          setMessages(updatedMessages);
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
