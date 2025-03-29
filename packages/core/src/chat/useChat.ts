import { UIMessage } from './types';
import { dummyMessages } from './dummyData'
import { useChat as vercelUseChat } from "@ai-sdk/react"
import { useCallback, useEffect, useRef, useState } from 'react';
import { parseCommandsFromMessage, replaceCommandTagsWithResults, formatCommandOutput } from '../utils/commandParser';
import { safeExecuteCommand, CommandExecutionOptions } from '../utils/commandExecutor';
// Use our local interfaces instead of direct imports
import { 
  createAgentConnection, 
  fetchAgentMessages, 
  sendMessageToAgent, 
  createAgentUtils,
  AgentConnectionOptions,
  AgentClient
} from './agent-connection';

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

export function useChat(options: UseChatWithCommandsOptions = {}): ReturnType<typeof vercelUseChat> & {
  agentConnection: { 
    isConnected: boolean; 
    client: AgentClient | null;
    utils: ReturnType<typeof createAgentUtils> | null;
  };
} {
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
  
  // State for tracking agent connection
  const [agentConnection, setAgentConnection] = useState<{
    isConnected: boolean;
    client: AgentClient | null;
    utils: ReturnType<typeof createAgentUtils> | null;
  }>({
    isConnected: false,
    client: null,
    utils: null
  });
  
  // Flag to determine if we should use the agent
  const shouldUseAgent = Boolean(agentId || (agentOptions && agentOptions.agentName));
  
  // Handle initial agent connection
  useEffect(() => {
    // Only attempt connection if an agent ID is provided
    if (!shouldUseAgent) {
      return;
    }
    
    const connectToAgent = async () => {
      try {
        console.log('🔌 USECHAT: Connecting to agent:', agentId || (agentOptions?.agentName ? 'CoderAgent' : 'unknown'));
        
        // Create connection options
        const connectionOptions: AgentConnectionOptions = {
          agentId: agentId || 'CoderAgent', // Default to CoderAgent if only name is provided
          agentName: agentName || agentOptions?.agentName || 'default',
          serverUrl: agentServerUrl || agentOptions?.serverUrl || 'https://agents.openagents.com',
          // Try different path patterns to increase chance of successful connection
          pathPattern: agentOptions?.pathPattern || 'api/agent',
          token: agentAuthToken || agentOptions?.token,
          onStateUpdate: agentOptions?.onStateUpdate
        };
        
        try {
          // Connect to the agent
          const client = await createAgentConnection(connectionOptions);
          
          // Connection client created, but WebSocket might not be connected yet
          console.log('✅ USECHAT: Agent client created successfully');
          
          // Create utilities for interacting with the agent
          const utils = createAgentUtils(client);
          
          // Set connection state now to allow user interface to update
          setAgentConnection({
            isConnected: true,
            client,
            utils
          });
          
          // Notify of connection change
          onAgentConnectionChange?.(true);
          
          // Set project context if provided - client will handle queueing if needed
          if (agentOptions?.projectContext) {
            try {
              await utils.setProjectContext(agentOptions.projectContext);
              console.log('📁 USECHAT: Set project context for agent');
            } catch (contextError) {
              console.warn('Failed to set project context:', contextError);
            }
          }
          
        } catch (connectionError) {
          console.error('❌ USECHAT: Failed to create agent client:', connectionError);
          throw connectionError;
        }
        
      } catch (error) {
        // Handle the connection error
        console.error('❌ USECHAT: Failed to connect to agent:', error);
        setAgentConnection({
          isConnected: false,
          client: null,
          utils: null
        });
        onAgentConnectionChange?.(false);
        chatOptions.onError?.(error instanceof Error ? error : new Error(String(error)));
      }
    };
    
    connectToAgent();
    
    // Cleanup function to disconnect from agent
    return () => {
      if (agentConnection.client) {
        console.log('🔌 USECHAT: Disconnecting from agent');
        agentConnection.utils?.disconnect();
        setAgentConnection({
          isConnected: false,
          client: null,
          utils: null
        });
        onAgentConnectionChange?.(false);
      }
    };
  }, [agentId, agentName, agentServerUrl, agentAuthToken, 
      // Only include serializable parts of agentOptions to prevent unnecessary re-connects
      agentOptions?.agentName, agentOptions?.serverUrl, agentOptions?.token,
      JSON.stringify(agentOptions?.projectContext)]);
  
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
  
  // Store messages from agent
  const [agentMessages, setAgentMessages] = useState<UIMessage[]>([]);
  
  // Fetch initial messages from agent
  useEffect(() => {
    if (!shouldUseAgent || !agentConnection.isConnected || !agentConnection.utils) {
      return;
    }
    
    const fetchInitialMessages = async () => {
      try {
        console.log('📥 USECHAT: Fetching initial messages from agent');
        // Add non-null assertion since we've already checked for null in the condition above
        const messages = await agentConnection.utils!.fetchMessages();
        console.log(`✅ USECHAT: Received ${messages.length} messages from agent`);
        setAgentMessages(messages);
      } catch (error) {
        console.error('❌ USECHAT: Failed to fetch messages from agent:', error);
        
        // If this fails, try again once more after a longer delay
        setTimeout(async () => {
          try {
            if (agentConnection.isConnected && agentConnection.utils) {
              console.log('📥 USECHAT: Retry fetching initial messages from agent');
              const retryMessages = await agentConnection.utils.fetchMessages();
              console.log(`✅ USECHAT: Retry received ${retryMessages.length} messages from agent`);
              setAgentMessages(retryMessages);
            }
          } catch (retryError) {
            console.error('❌ USECHAT: Retry failed to fetch messages from agent:', retryError);
          }
        }, 3000);
      }
    };
    
    fetchInitialMessages();
  }, [agentConnection.isConnected]);
  
  // Final messages to display - either from agent or local chat
  const messages = shouldUseAgent && agentConnection.isConnected ? agentMessages : vercelMessages;
  
  // Reference to store pending command executions
  const pendingCommandsRef = useRef<{
    messageId: string;
    commands: string[];
    isProcessing: boolean;
  } | null>(null);
  
  // Custom append function that checks for commands or routes to agent
  const append = useCallback(async (message: any) => {
    // If using agent and connected, send message to agent
    if (shouldUseAgent && agentConnection.isConnected && agentConnection.utils) {
      try {
        console.log('📤 USECHAT: Sending message to agent:', message.role);
        
        // Add the message to local state immediately for better UX
        const messageId = generateId();
        const newMessage = {
          ...message,
          id: messageId,
          createdAt: new Date(),
          parts: message.parts || [{
            type: 'text',
            text: message.content
          }]
        } as UIMessage;
        
        // Add to agent messages
        setAgentMessages(prev => [...prev, newMessage]);
        
        // Send to agent
        const result = await agentConnection.utils.sendMessage(message);
        console.log('✅ USECHAT: Message sent to agent successfully, id:', result);
        
        // Since we've already added the message to our state, just return the ID
        return result;
      } catch (error) {
        console.error('❌ USECHAT: Failed to send message to agent:', error);
        // Fall back to original append if sending to agent fails
        chatOptions.onError?.(error instanceof Error ? error : new Error(String(error)));
        return originalAppend(message);
      }
    }
    
    // If not using agent, use the original append function
    const result = await originalAppend(message);
    
    // Skip command execution if it's not enabled
    if (!localCommandExecution) {
      // console.log('ℹ️ USECHAT: Command execution disabled, skipping command check');
      return result;
    }
    
    // Check if this is a user message and parse commands
    if (message.role === 'user' && typeof message.content === 'string') {
      const commands = parseCommandsFromMessage(message.content);
      
      if (commands.length > 0 && result) {
        // console.log(`🔍 USECHAT: Found ${commands.length} commands in user message`);
        // Store commands for processing after the response is received
        pendingCommandsRef.current = {
          messageId: typeof result === 'object' ? (result as any).id || 'unknown' : 'unknown',
          commands,
          isProcessing: false
        };
      }
    }
    
    return result;
  }, [
    shouldUseAgent, 
    agentConnection.isConnected, 
    agentConnection.utils, 
    localCommandExecution, 
    originalAppend
  ]);
  
  // Helper function to generate a unique ID (simplified version)
  const generateId = () => {
    return `msg_${Math.random().toString(36).substring(2, 15)}`;
  };
  
  // Removed the processAssistantMessage callback
  // (consolidated into the main processing useEffect)
  
  // Removed duplicate useEffect for monitoring assistant messages
  // (consolidated into the main processing useEffect above)
  
  // Store processed messages with command execution results
  const [processedMessages, setProcessedMessages] = useState<UIMessage[]>([]);
  
  // Update processed messages whenever original messages change
  useEffect(() => {
    // console.log(`🔄 USECHAT: Updating processed messages`);
    
    // Get a reference to the current processed messages
    // This avoids the circular dependency but still allows us to check
    // existing processed messages
    setProcessedMessages(prevProcessedMessages => {
      // Careful update that preserves our processed messages with command outputs
      return messages.map(newMsg => {
        // Check if we have this message in our processed messages
        const existingMsg = prevProcessedMessages.find(m => m.id === newMsg.id);
        
        if (existingMsg) {
          // If our existing message has a different content (e.g., with command results),
          // keep that content instead of overwriting with the original
          if (existingMsg.content !== newMsg.content && 
              (existingMsg.content.includes('**Command Result**') || 
               existingMsg.content.includes('**Command Error**'))) {
            console.log(`🔄 USECHAT: Preserving command results in message: ${newMsg.id}`);
            return existingMsg;
          }
        }
        
        // Otherwise use the new message
        return newMsg;
      });
    });
  }, [messages]); // Removed processedMessages from dependencies
  
  // Additional function to manually update a message with command results
  const updateMessage = useCallback((messageId: string, newContent: string) => {
    console.log(`🔄 USECHAT: Manual message update for ID: ${messageId}`);
    
    // First, update the actual message in the Vercel AI SDK
    // This is a very important step - find the "append" function implementation
    // in the original useChat and see if we can call an internal update function
    
    // Then update our processed messages state 
    setProcessedMessages(current => {
      console.log('🔄 USECHAT: Updating message content:', newContent.substring(0, 50) + '...');
      
      // Create a new array with the updated message content
      return current.map(msg => {
        if (msg.id === messageId) {
          console.log('🔄 USECHAT: Found message to update in state');
          return { ...msg, content: newContent };
        }
        return msg;
      });
    });
    
    // Force another UI refresh by delaying a state update
    setTimeout(() => {
      setProcessedMessages(current => [...current]);
    }, 100);
  }, []);
  
  // Use a ref to store processed message IDs to persist across renders
  const processedMessageIds = useRef<Set<string>>(new Set());
  
  // Use a ref to track executed commands to prevent duplicate executions
  const executedCommands = useRef<Set<string>>(new Set());
  
  // Modified processAssistantMessage to update the message in our local state
  useEffect(() => {
    // Skip if command execution is disabled or no messages
    if (!localCommandExecution || messages.length === 0) {
      return;
    }
    
    // Define a function to update message content
    const updateMessageWithCommandResults = async (message: UIMessage, results: Array<{ command: string; result: string | { error: string } }>) => {
      if (results.length === 0) return;
      
      console.log(`🔄 USECHAT: Updating message with command results`);
      const updatedContent = replaceCommandTagsWithResults(message.content, results);
      
      if (updatedContent !== message.content) {
        console.log(`✅ USECHAT: Content changed, updating message in state`);
        updateMessage(message.id, updatedContent);
      }
    };
    
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
            
            const result = await safeExecuteCommand(command, commandOptions);
            // console.log('🔍 USECHAT: Raw command result:', JSON.stringify(result));
            
            const formattedResult = formatCommandOutput(command, result);
            // console.log('📋 USECHAT: Formatted result:', formattedResult);
            
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
      
      // Create a unique key for this update to prevent duplicate updates
      const updateKey = `update-${message.id}`;
      
      // Check if we've already updated this message to prevent infinite loops
      if (executedCommands.current.has(updateKey)) {
        console.log(`⏺️ USECHAT: Message ${message.id} already updated with command results, skipping`);
        return;
      }
      
      // Mark as updated to prevent duplicate updates
      executedCommands.current.add(updateKey);
      
      console.log(`🔥 USECHAT: Updating message ${message.id} with command results (first time)`);
      
      // Get the updated content with command results
      const updatedContent = replaceCommandTagsWithResults(message.content, commandResults);
      
      // COMPLETELY CHANGED APPROACH: Always create a new message with command results
      // This ensures results are always visible even if tag replacement fails
      const appendKey = `append-${message.id}`;
      
      if (!executedCommands.current.has(appendKey)) {
        // Mark as appended to prevent duplicate appends
        executedCommands.current.add(appendKey);
        
        console.log(`🚨 USECHAT: Creating new message with command results (reliable approach)`);
        
        // Format command results clearly in a new message without markdown headers
        // Just the command and results - clean and simple
        const resultContent = commandResults.map(({command, result}) => {
            const resultText = typeof result === 'string' 
              ? result 
              : `Error: ${result.error}`;
            
            return `**Command:** \`${command}\`\n\n\`\`\`\n${resultText}\n\`\`\``;
          }).join("\n\n");
        
        // Insert a new message with just the command results
        console.log(`🚀 USECHAT: Appending new message with command results (guaranteed)`);
        
        setTimeout(() => {
          originalAppend({
            role: 'assistant',
            content: resultContent
          });
        }, 100);
        
        // Still try to update the original message as well
        if (updatedContent !== message.content) {
          console.log(`✅ USECHAT: Also updating original message with command results`);
          updateMessage(message.id, updatedContent);
        }
      } else {
        console.log(`⏺️ USECHAT: Already appended command results for message ${message.id}`);
      }
      
      // Force a refresh of messages exactly once to ensure UI updates
      const refreshKey = `refresh-${message.id}`;
      
      if (!executedCommands.current.has(refreshKey)) {
        // Mark as refreshed to prevent multiple refreshes
        executedCommands.current.add(refreshKey);
        
        setTimeout(() => {
          console.log(`🔄 USECHAT: Forcing one-time message refresh for UI update`);
          setProcessedMessages(prevMessages => [...prevMessages]);
        }, 200);
      }
    };
    
    // Process all unprocessed assistant messages
    const processNewMessages = async () => {
      // Get all assistant messages
      const assistantMessages = messages.filter(m => m.role === 'assistant');
      
      // Find messages that haven't been processed yet
      const unprocessedMessages = assistantMessages.filter(msg => !processedMessageIds.current.has(msg.id));
      
      if (unprocessedMessages.length > 0) {
        // console.log(`🔍 USECHAT: Found ${unprocessedMessages.length} new assistant messages to process`);
        
        // Process each new message
        for (const message of unprocessedMessages) {
          await processSingleMessage(message);
        }
      }
    };
    
    // Run the processing
    processNewMessages();
  }, [messages, localCommandExecution, commandOptions, onCommandStart, onCommandComplete, updateMessage]);
  
  // This function will be replaced by the enhanced version below

  // Command execution for agent
  const executeAgentCommand = useCallback(async (command: string) => {
    if (!shouldUseAgent || !agentConnection.isConnected || !agentConnection.utils) {
      console.log('ℹ️ USECHAT: Agent not connected, falling back to local command execution');
      return safeExecuteCommand(command, commandOptions);
    }
    
    try {
      console.log('⚙️ USECHAT: Executing command on agent:', command);
      onCommandStart?.(command);
      const result = await agentConnection.utils.executeCommand(command);
      onCommandComplete?.(command, result);
      return result;
    } catch (error) {
      console.error('❌ USECHAT: Agent command execution failed:', error);
      throw error;
    }
  }, [
    shouldUseAgent, 
    agentConnection.isConnected, 
    agentConnection.utils, 
    commandOptions,
    onCommandStart,
    onCommandComplete
  ]);
  
  // Extended test command execution to check both local and agent capabilities
  const testCommandExecution = useCallback(async () => {
    // Test local command execution
    const localResult = await safeExecuteCommand('echo "Testing local command execution"', commandOptions).catch(() => null);
    
    // Test agent command execution if connected
    let agentResult: any = null;
    if (shouldUseAgent && agentConnection.isConnected && agentConnection.utils) {
      agentResult = await agentConnection.utils.executeCommand('echo "Testing agent command execution"').catch(() => null);
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
    agentConnection.utils,
    commandOptions
  ]);
  
  // Prepare return value with proper typing
  const returnValue = {
    ...rest,
    // Return the appropriate messages based on the active mode
    messages: shouldUseAgent && agentConnection.isConnected ? agentMessages : processedMessages,
    append,
    // Testing and debugging utilities
    testCommandExecution,
    // Add agent connection info
    agentConnection: {
      isConnected: agentConnection.isConnected,
      client: agentConnection.client,
      utils: agentConnection.utils
    },
    // Add command execution capability that automatically routes to agent or local
    executeCommand: shouldUseAgent && agentConnection.isConnected && agentConnection.utils 
      ? executeAgentCommand 
      : (command: string) => safeExecuteCommand(command, commandOptions),
    // Also keep the specific agent command function for explicit agent calls
    executeAgentCommand
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

  return returnValue;
}
