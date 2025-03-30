import { UIMessage } from './types';
import { dummyMessages } from './dummyData'
import { useChat as vercelUseChat } from "@ai-sdk/react"
import { useCallback, useEffect, useRef, useState } from 'react';
import { parseCommandsFromMessage, replaceCommandTagsWithResults, formatCommandOutput } from '../utils/commandParser';
import { safeExecuteCommand, CommandExecutionOptions } from '../utils/commandExecutor';
// Import the official SDK hooks
import { useAgent } from 'agents/react';
// Import the hook and Message type from ai-react
import { useAgentChat } from 'agents/ai-react';
// Import the Message type from the ai package (used by agents/ai-react)
import { Message } from 'ai';
// Import types from agent-connection
import type { AgentConnectionOptions, AgentClient } from './agent-connection';

// Define our own chat options interface
export interface UseChatWithCommandsOptions {
  // Standard options that might be in the original useChat
  api?: string;
  id?: string;
  initialMessages?: any[];
  initialInput?: string;
  maxSteps?: number;
  headers?: Record<string, string>;
  body?: object;
  onError?: (error: Error) => void;
  onFinish?: (message: any) => void;
  
  /**
   * Custom fetch implementation for platforms that need their own fetch
   */
  fetch?: typeof globalThis.fetch;
  
  // Command execution specific options
  /**
   * Enable local command execution (only works in Electron environment)
   */
  localCommandExecution?: boolean;
  
  /**
   * Options for command execution
   */
  commandOptions?: CommandExecutionOptions;
  
  /**
   * Callback when a command execution starts
   */
  onCommandStart?: (command: string) => void;
  
  /**
   * Callback when a command execution completes
   */
  onCommandComplete?: (command: string, result: any) => void;
  
  // Agent-specific options
  /**
   * ID of the Cloudflare Agent to connect to (e.g. 'coder-agent')
   * When provided, chat interactions will be routed to this agent
   */
  agentId?: string;
  
  /**
   * Name of the specific agent instance
   * Allows connecting to different instances of the same agent type
   */
  agentName?: string;
  
  /**
   * Full connection options for the agent
   * Alternative to providing agentId/agentName separately
   */
  agentOptions?: Omit<AgentConnectionOptions, 'agentId'> & {
    /**
     * Optional project context to set for the agent
     * Primarily used with the CoderAgent
     */
    projectContext?: {
      repoOwner?: string;
      repoName?: string;
      branch?: string;
      path?: string;
    }
  };
  
  /**
   * Base URL for the agent server
   * @default 'https://agents.openagents.com'
   */
  agentServerUrl?: string;
  
  /**
   * Callback when agent connection status changes
   */
  onAgentConnectionChange?: (connected: boolean) => void;
  
  /**
   * Authentication token for agent connection
   */
  agentAuthToken?: string;
}

// Define the return type to properly merge vercelUseChat with our extensions
export type UseChatReturn = ReturnType<typeof vercelUseChat> & {
  agentConnection: { 
    isConnected: boolean; 
    client: AgentClient | null; 
  };
  // Additional methods for agent interaction
  fetchMessages?: () => Promise<UIMessage[]>;
  executeAgentCommand?: (command: string) => Promise<any>;
  testCommandExecution?: () => Promise<{
    local: { available: boolean; enabled: boolean; result: any | null };
    agent: { available: boolean; connected: boolean; result: any | null };
  }>;
  // Properties added via Object.defineProperties
  localCommandExecution?: boolean;
  isCommandExecutionEnabled?: boolean;
  isAgentConnected?: boolean;
  isUsingAgent?: boolean;
};

export function useChat(options: UseChatWithCommandsOptions = {}): UseChatReturn {
  const {
    localCommandExecution = false,
    commandOptions,
    onCommandStart,
    onCommandComplete,
    agentId,
    agentName,
    agentOptions,
    agentServerUrl,
    onAgentConnectionChange,
    agentAuthToken,
    ...chatOptions
  } = options;
  
  // Flag to determine if we should use the agent
  const shouldUseAgent = Boolean(agentId || (agentOptions && agentOptions.agentName));
  
  // State for tracking agent connection
  const [agentConnection, setAgentConnection] = useState<{
    isConnected: boolean;
    client: AgentClient | null; // Use AgentClient type
  }>({
    isConnected: false,
    client: null
  });
  
  // Ref to track if initial messages have been fetched from agent
  const initialMessagesFetchedRef = useRef(false);
  
  // Always call useAgent with the same parameters (React hooks must be called unconditionally)
  // Create options object first, then add the headers property if needed
  // Always normalize agent ID to lowercase to prevent reconnection loops
  const normalizedAgentId = agentId?.toLowerCase() || 'coderagent';
  
  // If the original agent ID was in a different case than the normalized one,
  // log a warning but only once (not on every render) to avoid console spam
  useEffect(() => {
    if (agentId && agentId !== normalizedAgentId) {
      console.log(`⚠️ USECHAT: Agent name "${agentId}" has been normalized to lowercase "${normalizedAgentId}" to prevent connection issues.`);
    }
  }, [agentId, normalizedAgentId]);
  
  const agentOptions1 = {
    agent: normalizedAgentId, // Ensure agent ID is always lowercase
    name: agentName || agentOptions?.agentName || 'default',
    host: agentServerUrl || agentOptions?.serverUrl || 'https://agents.openagents.com',
    onStateUpdate: agentOptions?.onStateUpdate,
  };
  
  // Add headers through type assertion if auth token is provided
  const agentOptionsWithHeaders = agentAuthToken 
    ? { ...agentOptions1, headers: { Authorization: `Bearer ${agentAuthToken}` } } as any
    : agentOptions1;
    
  const agent = useAgent(agentOptionsWithHeaders);
  
  // Always call useAgentChat (React hooks must be called unconditionally)
  const agentChat = useAgentChat({
    agent, // Always pass the agent returned by useAgent
    initialMessages: chatOptions.initialMessages,
    // Disable the automatic fetch of initial messages that causes CORS errors
    getInitialMessages: null,
    // The connection will only be used if shouldUseAgent is true (checked in useEffect)
  });
  
  // Use a ref to track agent configuration to prevent connection/disconnection loops
  const agentConfigRef = useRef({
    agentId: normalizedAgentId,
    agentName: agentName || agentOptions?.agentName || 'default',
    serverUrl: agentServerUrl || agentOptions?.serverUrl || 'https://agents.openagents.com',
    projectContext: agentOptions?.projectContext
  });
  
  // Update the ref when config changes, but don't trigger effect re-runs
  useEffect(() => {
    agentConfigRef.current = {
      agentId: normalizedAgentId,
      agentName: agentName || agentOptions?.agentName || 'default',
      serverUrl: agentServerUrl || agentOptions?.serverUrl || 'https://agents.openagents.com',
      projectContext: agentOptions?.projectContext
    };
  }, [normalizedAgentId, agentName, agentOptions?.agentName, agentServerUrl, 
      agentOptions?.serverUrl, agentOptions?.projectContext]);
  
  // Set up agent connection when agent is available and should be used
  useEffect(() => {
    // Reset fetch status if agent is deselected
    if (!shouldUseAgent) {
      initialMessagesFetchedRef.current = false;
    }

    // If agent shouldn't be used, make sure connection state is reset
    if (!shouldUseAgent) {
      if (agentConnection.isConnected) {
        console.log('🔌 USECHAT: Agent not selected, resetting connection state');
        setAgentConnection({
          isConnected: false,
          client: null
        });
        onAgentConnectionChange?.(false);
      }
      return;
    }
    
    // Skip if agent isn't available yet
    // Note: useAgent returns the agent instance asynchronously.
    // We rely on the agent object becoming available to trigger connection logic.
    if (!agent || typeof agent.call !== 'function') { // Check if agent is fully initialized
      // If we were previously connected, update state
      if (agentConnection.isConnected) {
        console.log('🔌 USECHAT: Agent instance became unavailable, resetting connection state');
        setAgentConnection({ isConnected: false, client: null });
        onAgentConnectionChange?.(false);
        initialMessagesFetchedRef.current = false; // Reset fetch flag
      }
      return;
    }

    // If already connected, do nothing (this prevents loops if agent object reference changes)
    if (agentConnection.isConnected && agentConnection.client === agent) {
      return;
    }
    
    console.log('🔌 USECHAT: Connected to agent via official SDK:', agent.agent);
    
    // Update connection state
    setAgentConnection({
      isConnected: true,
      client: agent
    });
    
    // Notify of successful connection
    onAgentConnectionChange?.(true);
    
    // Set project context if provided - use the ref to avoid dependency changes
    if (agentConfigRef.current.projectContext) {
      try {
        agent.call('setProjectContext', [agentConfigRef.current.projectContext])
          .then(() => {
            console.log('📁 USECHAT: Set project context for agent');
          })
          .catch((error: Error) => {
            console.warn('Failed to set project context:', error);
          });
      } catch (contextError) {
        console.warn('Failed to set project context:', contextError);
      }
    }
    
    // Cleanup function to disconnect from agent - ONLY RUN ON UNMOUNT
    // NOT when dependencies change (to prevent infinite reconnection loops)
    return () => {
      // We rely on the component unmounting to trigger this cleanup
      // Do not disconnect on dependency changes or re-renders
      if (shouldUseAgent && agent) {
        console.log('🔌 USECHAT: Component unmounting, disconnecting from agent');
        // Close the agent connection
        agent.close();
        // Update local state
        setAgentConnection({
          isConnected: false,
          client: null
        });
        // Notify about disconnection
        onAgentConnectionChange?.(false);
        initialMessagesFetchedRef.current = false; // Reset fetch flag on unmount
      }
    };
  // IMPORTANT: Only depend on shouldUseAgent to prevent infinite connection/disconnection loops
  // Changes to agent or onAgentConnectionChange should NOT trigger reconnection
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shouldUseAgent]);
  
  // --- Simplified Effect to Fetch Initial Messages ---
  useEffect(() => {
    // Skip if agent isn't active or we've already fetched messages
    // Note: using shouldUseAgent && agentConnection.isConnected instead of isAgentActive 
    // since isAgentActive is defined later in the file
    if (!(shouldUseAgent && agentConnection.isConnected) || !agent || initialMessagesFetchedRef.current === true) {
      return;
    }
    
    console.log('📄 USECHAT: Agent active, attempting to fetch initial messages...');
    initialMessagesFetchedRef.current = true; // Set flag immediately to prevent re-fetch attempts

    // Use the agent's getMessages RPC call to fetch messages
    agent.call('getMessages')
      .then((fetchedMessages: unknown) => { 
        // Cast to Message[] after receiving
        const typedMessages = fetchedMessages as Message[];
        
        if (typedMessages && Array.isArray(typedMessages) && typedMessages.length > 0) {
          console.log(`✅ USECHAT: Fetched ${typedMessages.length} initial messages from agent.`);
          // Use the agent chat's setMessages function
          agentChat.setMessages(typedMessages);
        } else {
          console.log('ℹ️ USECHAT: No initial messages found on agent or fetch returned empty/invalid.');
          
          // Fall back to initialMessages if provided
          if (chatOptions.initialMessages && chatOptions.initialMessages.length > 0) {
            console.log('ℹ️ USECHAT: Falling back to initialMessages.');
            agentChat.setMessages(chatOptions.initialMessages);
          } else {
            // Ensure we have at least an empty array
            agentChat.setMessages([]);
          }
        }
      })
      .catch((error: Error) => {
        console.error('❌ USECHAT: Failed to fetch initial messages from agent:', error);
        
        // Fall back to initialMessages on error if available
        if (chatOptions.initialMessages && chatOptions.initialMessages.length > 0) {
          console.log('ℹ️ USECHAT: Falling back to initialMessages due to fetch error.');
          agentChat.setMessages(chatOptions.initialMessages);
        }
      });
  }, [
    shouldUseAgent, 
    agentConnection.isConnected,
    agent,
    agentChat.setMessages,
    chatOptions.initialMessages
  ]);
  // --- End New Effect ---
  
  // Track the original useChat instance
  const vercelChat = vercelUseChat({
    ...chatOptions,
    maxSteps: 15,
    api: shouldUseAgent ? undefined : "https://chat.openagents.com", // Don't use API if using agent
    onError: (error) => {
      console.error('Chat error:', error);
      chatOptions.onError?.(error);
    },
  });
  
  // Extract the needed methods and state from vercelChat
  const { messages: vercelMessages, append: originalAppend, ...rest } = vercelChat;
  
  // When using agent, use the messages from agentChat, otherwise use vercelMessages
  const agentMessages = agentChat?.messages || [];
  
  // Final messages to display - either from agent or local chat
  const messages = shouldUseAgent && agentConnection.isConnected ? agentMessages : vercelMessages;
  
  // Reference to store pending command executions
  const pendingCommandsRef = useRef<{
    messageId: string;
    commands: string[];
    isProcessing: boolean;
  } | null>(null);
  
  // Custom append function - delegates to active implementation
  const append = useCallback(async (message: any /* TODO: Type */) => {
    // Using shouldUseAgent && agentConnection.isConnected instead of isAgentActive
    const isAgentMode = shouldUseAgent && agentConnection.isConnected;
    
    try {
      if (isAgentMode && agentChat?.append) {
        console.log('📤 USECHAT: Appending via agentChat');
        return await agentChat.append(message);
      } else if (!isAgentMode && vercelChat?.append) {
        console.log('📤 USECHAT: Appending via vercelChat');
        return await vercelChat.append(message);
      } else {
        console.error("❌ USECHAT: Cannot append. No active chat implementation available.");
        return null; // Or throw an error
      }
    } catch (error) {
        console.error(`❌ USECHAT: Error during append (active: ${isAgentMode}):`, error);
        // Use the onError from the *options* passed to the main useChat hook
        options.onError?.(error instanceof Error ? error : new Error(String(error)));
        return null;
    }
  }, [shouldUseAgent, agentConnection.isConnected, agentChat, vercelChat, options.onError]);
  
  // Custom handleSubmit to work with combined append and active input/setters
  const handleSubmit = useCallback((e?: React.FormEvent<HTMLFormElement>) => {
      e?.preventDefault();
      // Using shouldUseAgent && agentConnection.isConnected instead of isAgentActive
      const isAgentMode = shouldUseAgent && agentConnection.isConnected;
      
      // Get input value from the *active* chat hook
      const messageToSend = isAgentMode ? agentChat?.input : vercelChat?.input;
      if (!messageToSend) return;

      console.log(`📤 USECHAT: handleSubmit called. Active: ${isAgentMode}. Message: "${messageToSend}"`);
      // Call the combined append function
      append({ role: 'user', content: messageToSend });

      // Manually clear input using the *active* chat's setter
      if (isAgentMode && agentChat?.setInput) {
          agentChat.setInput('');
      } else if (!isAgentMode && vercelChat?.setInput) {
          vercelChat.setInput('');
      }
  }, [append, shouldUseAgent, agentConnection.isConnected, agentChat, vercelChat]);
  
  // Helper function to generate a unique ID (simplified version)
  const generateId = () => {
    return `msg_${Math.random().toString(36).substring(2, 15)}`;
  };
  
  // No longer storing processed messages with command execution results
  // Command results will be integrated directly into vercelChat.messages state
  
  // Use a ref to store processed message IDs to persist across renders
  const processedMessageIds = useRef<Set<string>>(new Set());
  
  // Use a ref to track executed commands to prevent duplicate executions
  const executedCommands = useRef<Set<string>>(new Set());
  
  // Process local commands in assistant messages when using vercelChat (non-agent mode)
  useEffect(() => {
    // Skip if command execution is disabled or if agent is active
    // Using shouldUseAgent && agentConnection.isConnected instead of isAgentActive
    if ((shouldUseAgent && agentConnection.isConnected) || !localCommandExecution) {
      // Reset refs if mode changes or local execution disabled
      processedMessageIds.current.clear();
      executedCommands.current.clear();
      return;
    }

    // Get current messages directly from vercelChat
    const currentMessages = vercelChat.messages as UIMessage[];
    if (currentMessages.length === 0) return;
    
    // Function to process a single message
    const processSingleMessage = async (message: UIMessage) => {
      // Skip already processed messages to prevent infinite loops
      if (processedMessageIds.current.has(message.id)) {
        console.log(`ℹ️ USECHAT: Message ${message.id} already processed, skipping`);
        return;
      }
      
      if (message.role !== 'assistant' || typeof message.content !== 'string') return;
      
      const commands = parseCommandsFromMessage(message.content);
      if (commands.length === 0) return;
      
      // Mark message as processed BEFORE executing commands to prevent concurrent processing
      processedMessageIds.current.add(message.id);
      console.log(`🚀 USECHAT: Processing ${commands.length} commands for message ${message.id}`);
      
      // Execute commands and collect results
      const commandResults: Array<{ command: string; result: string | { error: string } }> = [];
      
      for (const command of commands) {
        // Create a unique key for this message+command combination to prevent duplicate executions
        const commandKey = `${message.id}-${command}`;
        
        // Only execute this command if we haven't already executed it for this message
        if (!executedCommands.current.has(commandKey)) {
          // Mark as executed BEFORE actually executing to prevent concurrent executions
          executedCommands.current.add(commandKey);
          console.log(`⚙️ USECHAT: Executing command: ${command} (first time for this message)`);
          
          try {
            onCommandStart?.(command);
            
            // In this refactored version, we're only using local command execution
            // The agent case is handled elsewhere
            const result = await safeExecuteCommand(command, commandOptions);
            
            // Type assertion to handle unknown result from API
            const typedResult = result as { stdout: string; stderr: string; exitCode: number; } | { error: string; };
            const formattedResult = formatCommandOutput(command, typedResult);
            commandResults.push({ command, result: formattedResult });
            onCommandComplete?.(command, result);
          } catch (error) {
            console.error(`❌ USECHAT: Command execution error:`, error);
            commandResults.push({
              command,
              result: { error: error instanceof Error ? error.message : String(error) }
            });
          }
        } else {
          console.log(`⏺️ USECHAT: Skipping already executed command: ${command} for message ${message.id}`);
        }
      }
      
      // Only proceed with updating if we actually have command results to show
      if (commandResults.length === 0) {
        console.log(`ℹ️ USECHAT: No command results to display for message ${message.id}`);
        return;
      }
      
      console.log(`🔥 USECHAT: Updating message ${message.id} with command results`);
      
      // Get the updated content with command results
      const updatedContent = replaceCommandTagsWithResults(message.content, commandResults);
      
      // Direct replacement of message content in vercelChat.messages
      if (updatedContent !== message.content) {
        // Mark that we're updating this message to prevent loops
        const updateKey = `update-${message.id}`;
        if (!executedCommands.current.has(updateKey)) {
          // Mark as updated to prevent duplicate updates
          executedCommands.current.add(updateKey);
          
          console.log(`✅ USECHAT: Content changed, updating message directly in vercelChat state`);
          
          // Find the message in the current messages array by ID
          const messageIndex = currentMessages.findIndex(msg => msg.id === message.id);
          
          if (messageIndex !== -1) {
            // Create a new messages array with the updated message
            const updatedMessages = [...currentMessages];
            updatedMessages[messageIndex] = {
              ...currentMessages[messageIndex],
              content: updatedContent
            };
            
            // Update the messages in vercelChat
            console.log(`🔄 USECHAT: Setting updated messages in vercelChat`);
            // Use a function to update messages to avoid type issues
            vercelChat.setMessages((prevMessages) => {
              // Create a new array based on the previous messages
              return prevMessages.map((msg, idx) => {
                // If this is the message we want to update
                if (idx === messageIndex) {
                  // Return a new message object with the updated content
                  return {
                    ...msg,
                    content: updatedContent
                  };
                }
                // Otherwise return the original message
                return msg;
              });
            });
          }
        } else {
          console.log(`⏺️ USECHAT: Already updated message ${message.id} with command results`);
        }
      }
    };
    
    // Process all unprocessed assistant messages
    const processNewMessages = async () => {
        // Get all assistant messages directly from vercelChat
      const assistantMessages = currentMessages.filter((m: UIMessage) => m.role === 'assistant');
      
      // Find messages that haven't been processed yet
      const unprocessedMessages = assistantMessages.filter((msg: UIMessage) => !processedMessageIds.current.has(msg.id));
      
      if (unprocessedMessages.length > 0) {
        // Process each new message
        for (const message of unprocessedMessages) {
          await processSingleMessage(message);
        }
      }
    };
    
    // Run the processing
    processNewMessages();
  }, [localCommandExecution, commandOptions, onCommandStart, onCommandComplete,
      shouldUseAgent, agentConnection.isConnected, agent, vercelChat.messages, vercelChat.setMessages]);
  
  // Command execution for agent (RPC call)
  const executeAgentCommand = useCallback(async (command: string) => {
    // Guard clauses remain the same
    if (!shouldUseAgent || !agentConnection.isConnected || !agent) {
        // Fallback to local if enabled, otherwise error/noop?
        if (localCommandExecution) {
            console.log('ℹ️ USECHAT: Agent not connected for executeAgentCommand, falling back to local command execution');
            return safeExecuteCommand(command, commandOptions);
        } else {
            const errorMsg = 'Agent not connected and local execution disabled.';
            console.error(`❌ USECHAT: ${errorMsg}`);
            throw new Error(errorMsg);
        }
    }

    try {
      console.log('⚙️ USECHAT: Executing command on agent (RPC):', command);
      onCommandStart?.(command);
      // Ensure agent.call exists before calling
      const result = agent.call ? await agent.call('executeCommand', [command]) : Promise.reject("Agent client not fully initialized");
      onCommandComplete?.(command, result);
      return result;
    } catch (error) {
      console.error('❌ USECHAT: Agent command execution failed:', error);
      throw error; // Re-throw
    }
  }, [
    shouldUseAgent,
    agentConnection.isConnected,
    agent,
    commandOptions,
    onCommandStart,
    onCommandComplete,
    localCommandExecution // Added for fallback logic
  ]);
  
  // Helper function to fetch messages from the agent (RPC call)
  const fetchMessages = useCallback(async (): Promise<UIMessage[]> => { // Return UIMessage[]
    if (!shouldUseAgent || !agentConnection.isConnected || !agent || typeof agent.call !== 'function') {
      console.log('ℹ️ USECHAT: Agent not connected or ready, cannot fetch messages via RPC.');
      return [];
    }

    try {
      console.log('📄 USECHAT: Fetching messages from agent via RPC call');
      // Use the proper Message type from the ai package
      const agentMsgs: Message[] = await agent.call('getMessages');
      // Cast to UIMessage[] - both types have compatible properties for our needs
      // They have role, content, id and function_call properties in common
      return agentMsgs as unknown as UIMessage[];
    } catch (error) {
      console.error('❌ USECHAT: Failed to fetch messages from agent via RPC:', error);
      return [];
    }
  }, [shouldUseAgent, agentConnection.isConnected, agent]);
  
  // Extended test command execution to check both local and agent capabilities
  const testCommandExecution = useCallback(async () => {
    // Test local command execution
    const localResult = await safeExecuteCommand('echo "Testing local command execution"', commandOptions).catch(() => null);
    
    // Test agent command execution if connected
    let agentResult: any = null;
    if (shouldUseAgent && agentConnection.isConnected && agent) {
      agentResult = await agent.call('executeCommand', ['echo "Testing agent command execution"']).catch(() => null);
    }
    
    return {
      local: {
        available: !!localResult,
        enabled: localCommandExecution,
        result: localResult
      },
      agent: {
        available: !!agentResult,
        connected: agentConnection.isConnected,
        result: agentResult
      }
    };
  }, [
    localCommandExecution, 
    shouldUseAgent, 
    agentConnection.isConnected, 
    agent,
    commandOptions
  ]);
  
  // Flag to determine if agent mode is active (both should use agent AND connection is established)
  const isAgentActive = shouldUseAgent && agentConnection.isConnected;
  
  // Define activeChat based on whether agent is active
  const activeChat = isAgentActive ? agentChat : vercelChat;
  
  // Prepare return value with proper typing
  const returnValue = {
    // Core chat properties from the active chat implementation
    messages: activeChat.messages, // Simply use the active chat's messages directly
    isLoading: activeChat.isLoading,
    error: activeChat.error,
    input: activeChat.input,
    handleInputChange: activeChat.handleInputChange,
    setMessages: activeChat.setMessages,
    reload: activeChat.reload,
    stop: activeChat.stop,
    
    append,
    handleSubmit,
    
    // Agent connection info
    agentConnection: {
      isConnected: agentConnection.isConnected,
      client: agentConnection.client
    },
    
    // Utilities specific to this implementation
    testCommandExecution,
    fetchMessages,
    
    // Combined command execution capability
    executeCommand: isAgentActive
      ? executeAgentCommand 
      : (command: string) => {
          if (localCommandExecution) {
              return safeExecuteCommand(command, commandOptions);
          } else {
              console.error("❌ USECHAT: Cannot execute command. Agent not connected and local execution disabled.");
              return Promise.reject("Command execution not available.");
          }
        },
    executeAgentCommand,
  };
  
  // Add debugging properties
  Object.defineProperties(returnValue, {
    localCommandExecution: {
      enumerable: true,
      value: localCommandExecution
    },
    isCommandExecutionEnabled: {
      enumerable: true,
      value: Boolean(localCommandExecution)
    },
    isAgentConnected: {
      enumerable: true,
      value: agentConnection.isConnected
    },
    isUsingAgent: {
      enumerable: true,
      value: shouldUseAgent && agentConnection.isConnected
    }
  });
  
  // We must cast to unknown first, then to the expected return type
  // This is necessary because the types from different libraries are not directly compatible
  // even though they have the same structure in practice.
  return returnValue as unknown as UseChatReturn;
}