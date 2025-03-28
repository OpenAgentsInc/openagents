import { UIMessage } from './types';
import { dummyMessages } from './dummyData'
import { useChat as vercelUseChat } from "@ai-sdk/react"
import { useCallback, useEffect, useRef } from 'react';
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
      return result;
    }
    
    // Check if this is a user message and parse commands
    if (message.role === 'user' && typeof message.content === 'string') {
      const commands = parseCommandsFromMessage(message.content);
      
      if (commands.length > 0 && result) {
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
  
  // Process assistant messages for command execution
  const processAssistantMessage = useCallback(async (message: UIMessage) => {
    if (message.role !== 'assistant' || typeof message.content !== 'string') {
      return;
    }
    
    const commands = parseCommandsFromMessage(message.content);
    if (commands.length === 0) {
      return;
    }
    
    // Execute each command and collect results
    const commandResults: Array<{ command: string; result: string | { error: string } }> = [];
    for (const command of commands) {
      try {
        // Notify about command execution
        onCommandStart?.(command);
        
        // Execute the command
        const result = await safeExecuteCommand(command, commandOptions);
        
        // Format the result
        const formattedResult = formatCommandOutput(command, result);
        
        // Add to results array
        commandResults.push({
          command,
          result: formattedResult
        });
        
        // Notify about command completion
        onCommandComplete?.(command, result);
      } catch (error) {
        commandResults.push({
          command,
          result: { error: error instanceof Error ? error.message : String(error) }
        });
      }
    }
    
    // Replace command tags with results in the message
    const updatedContent = replaceCommandTagsWithResults(message.content, commandResults);
    
    // If content changed, update the message using the original client
    if (updatedContent !== message.content) {
      // In a real implementation, we would update the message here
      // This is a placeholder since vercelChat doesn't expose an update method
      console.log('Updated message with command results', { id: message.id, updatedContent });
      
      // In a full implementation, we would do something like this:
      // await updateMessage(message.id, {
      //   ...message,
      //   content: updatedContent
      // });
    }
  }, [localCommandExecution, commandOptions, onCommandStart, onCommandComplete]);
  
  // Monitor for new assistant messages
  useEffect(() => {
    if (!localCommandExecution || messages.length === 0) {
      return;
    }
    
    // Find the last assistant message
    const lastAssistantMessage = [...messages]
      .reverse()
      .find(m => m.role === 'assistant');
    
    if (lastAssistantMessage) {
      processAssistantMessage(lastAssistantMessage);
    }
  }, [localCommandExecution, messages, processAssistantMessage]);
  
  return {
    ...rest,
    messages,
    append
  };
}
