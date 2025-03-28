import { UIMessage } from './types';
import { dummyMessages } from './dummyData'
import { useChat as vercelUseChat } from "@ai-sdk/react"
import { useCallback, useEffect, useRef, useState } from 'react';
import { parseCommandsFromMessage, replaceCommandTagsWithResults, formatCommandOutput } from '../utils/commandParser';
import { safeExecuteCommand, CommandExecutionOptions } from '../utils/commandExecutor';

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
}

export function useChat(options: UseChatWithCommandsOptions = {}): ReturnType<typeof vercelUseChat> {
  const {
    localCommandExecution = false,
    commandOptions,
    onCommandStart,
    onCommandComplete,
    ...chatOptions
  } = options;
  
  // Track the original useChat instance
  const vercelChat = vercelUseChat({
    ...chatOptions,
    maxSteps: 15,
    api: "https://chat.openagents.com",
    onError: (error) => {
      console.error('Chat error:', error);
      chatOptions.onError?.(error);
    },
  });
  
  // Extract the needed methods and state from vercelChat
  const { messages, append: originalAppend, ...rest } = vercelChat;
  
  // Reference to store pending command executions
  const pendingCommandsRef = useRef<{
    messageId: string;
    commands: string[];
    isProcessing: boolean;
  } | null>(null);
  
  // Custom append function that checks for commands
  const append = useCallback(async (message: any) => {
    const result = await originalAppend(message);
    
    // Skip command execution if it's not enabled
    if (!localCommandExecution) {
      console.log('ℹ️ USECHAT: Command execution disabled, skipping command check');
      return result;
    }
    
    // Check if this is a user message and parse commands
    if (message.role === 'user' && typeof message.content === 'string') {
      const commands = parseCommandsFromMessage(message.content);
      
      if (commands.length > 0 && result) {
        console.log(`🔍 USECHAT: Found ${commands.length} commands in user message`);
        // Store commands for processing after the response is received
        pendingCommandsRef.current = {
          messageId: typeof result === 'object' ? (result as any).id || 'unknown' : 'unknown',
          commands,
          isProcessing: false
        };
      }
    }
    
    return result;
  }, [localCommandExecution, originalAppend]);
  
  // Removed the processAssistantMessage callback
  // (consolidated into the main processing useEffect)
  
  // Removed duplicate useEffect for monitoring assistant messages
  // (consolidated into the main processing useEffect above)
  
  // Store processed messages with command execution results
  const [processedMessages, setProcessedMessages] = useState<UIMessage[]>([]);
  
  // Update processed messages whenever original messages change
  useEffect(() => {
    console.log(`🔄 USECHAT: Updating processed messages`);
    setProcessedMessages(messages);
  }, [messages]);
  
  // Additional function to manually update a message with command results
  const updateMessage = useCallback((messageId: string, newContent: string) => {
    console.log(`🔄 USECHAT: Manual message update for ID: ${messageId}`);
    setProcessedMessages(current => 
      current.map(msg => 
        msg.id === messageId 
          ? { ...msg, content: newContent } 
          : msg
      )
    );
  }, []);
  
  // Use a ref to store processed message IDs to persist across renders
  const processedMessageIds = useRef<Set<string>>(new Set());
  
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
        try {
          console.log(`⚙️ USECHAT: Executing command: ${command}`);
          onCommandStart?.(command);
          
          const result = await safeExecuteCommand(command, commandOptions);
          const formattedResult = formatCommandOutput(command, result);
          
          commandResults.push({ command, result: formattedResult });
          onCommandComplete?.(command, result);
        } catch (error) {
          console.error(`❌ USECHAT: Command execution error:`, error);
          commandResults.push({
            command,
            result: { error: error instanceof Error ? error.message : String(error) }
          });
        }
      }
      
      // Update the message content with command results
      await updateMessageWithCommandResults(message, commandResults);
    };
    
    // Process all unprocessed assistant messages
    const processNewMessages = async () => {
      // Get all assistant messages
      const assistantMessages = messages.filter(m => m.role === 'assistant');
      
      // Find messages that haven't been processed yet
      const unprocessedMessages = assistantMessages.filter(msg => !processedMessageIds.current.has(msg.id));
      
      if (unprocessedMessages.length > 0) {
        console.log(`🔍 USECHAT: Found ${unprocessedMessages.length} new assistant messages to process`);
        
        // Process each new message
        for (const message of unprocessedMessages) {
          await processSingleMessage(message);
        }
      }
    };
    
    // Run the processing
    processNewMessages();
  }, [messages, localCommandExecution, commandOptions, onCommandStart, onCommandComplete, updateMessage]);
  
  // Add a simple function to test if command execution is available
  const testCommandExecution = useCallback(async () => {
    try {
      const { safeExecuteCommand } = await import('../utils/commandExecutor');
      
      // Log all the available APIs on window
      if (typeof window !== 'undefined') {
        console.log('🔍 USECHAT: Window APIs available:', {
          commandExecution: !!window.commandExecution,
          electron: !!window.electron,
          electronIPC: !!(window.electron?.ipcRenderer)
        });
      }
      
      const result = await safeExecuteCommand('echo "Command execution test"', commandOptions);
      
      console.log('🧪 USECHAT: Command execution test result:', result);
      return result;
    } catch (error) {
      console.error('🧪 USECHAT: Command execution test error:', error);
      return { error: error instanceof Error ? error.message : String(error) };
    }
  }, [commandOptions]);

  // Prepare return value with proper typing
  const returnValue = {
    ...rest,
    // Return processed messages instead of original messages
    messages: processedMessages,
    append,
    // Add the test function
    testCommandExecution
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
    }
  });

  return returnValue;
}
