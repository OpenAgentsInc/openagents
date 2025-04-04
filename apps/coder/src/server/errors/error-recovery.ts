/**
 * Error recovery mechanisms
 */

import type { Message } from 'ai';
import {
  ToolError,
  ChatError,
  transformUnknownError
} from '@openagents/core/src/chat/errors';

/**
 * Error recovery options
 */
export interface ErrorRecoveryOptions {
  // Whether to keep partial output from failed tool calls
  keepPartialResults?: boolean;
  
  // Custom error message to include in the system message
  customErrorMessage?: string;
  
  // Whether to add an assistant message explaining the error
  addAssistantMessage?: boolean;
}

/**
 * Clean up messages with failed tool calls
 */
export async function cleanupMessagesWithFailedToolCalls(
  messagesWithToolCalls: Message[],
  error?: unknown,
  options: ErrorRecoveryOptions = {}
): Promise<Message[]> {
  console.log("🧹 Running cleanup function for messages with failed tool calls");
  
  try {
    // Process the error if provided
    let errorMessage = "There was an issue with a previous tool call.";
    if (error) {
      const chatError = error instanceof ChatError ? error : transformUnknownError(error);
      
      // Get error details
      if (chatError instanceof ToolError) {
        errorMessage = `There was an issue with a previous tool call to '${chatError.toolName}': ${chatError.userMessage}`;
      } else {
        errorMessage = `There was an issue with a previous tool call: ${chatError.userMessage}`;
      }
    }
    
    // Use custom error message if provided
    if (options.customErrorMessage) {
      errorMessage = options.customErrorMessage;
    }
    
    // Extract the system message if it exists
    const systemMessage = messagesWithToolCalls.find((msg: Message) => msg.role === 'system');
    const nonSystemMessages = messagesWithToolCalls.filter((msg: Message) => msg.role !== 'system');

    // Filter out assistant messages with tool calls from non-system messages
    const filteredMessages = nonSystemMessages.filter((msg: Message) => {
      // Keep messages that are not assistant messages
      if (msg.role !== 'assistant') {
        return true;
      }
      
      // Check if this assistant message has tool calls
      const hasToolCalls = 
        (msg.toolInvocations && msg.toolInvocations.length > 0) ||
        (msg.parts && msg.parts.some((p: any) => p.type === 'tool-invocation'));
      
      // If it has tool calls and we're not keeping partial results, remove it
      if (hasToolCalls && !options.keepPartialResults) {
        console.log(`Removing assistant message with tool calls: ${msg.id || 'unnamed'}`);
        return false;
      }
      
      // Otherwise keep the message
      return true;
    });

    // Get the last user message if it exists
    const lastUserMessage = filteredMessages.length > 0 &&
      filteredMessages[filteredMessages.length - 1].role === 'user' ?
      filteredMessages[filteredMessages.length - 1] : null;

    // Create a modified system message that includes the error information
    const errorSystemMessage: Message = {
      id: `system-${Date.now()}`,
      role: 'system',
      content: ((systemMessage?.content || "") +
        `\n\nNOTE: ${errorMessage} The problematic message has been removed so the conversation can continue.`).trim(),
      createdAt: new Date(),
      parts: [{
        type: 'text',
        text: ((systemMessage?.content || "") +
          `\n\nNOTE: ${errorMessage} The problematic message has been removed so the conversation can continue.`).trim()
      }]
    };

    // Create the final cleaned messages with system message first, then alternating user/assistant
    let cleanedMessages = [errorSystemMessage, ...filteredMessages];

    // If the last message is not from the user and we want to add an assistant message, add a special assistant message explaining the issue
    if ((lastUserMessage === null ||
        (filteredMessages.length > 0 && filteredMessages[filteredMessages.length - 1].role !== 'user')) && 
        options.addAssistantMessage) {
      
      // Add an assistant message explaining the error
      cleanedMessages.push({
        id: `assistant-${Date.now()}`,
        role: 'assistant',
        content: errorMessage + " Please try a different request or continue our conversation.",
        createdAt: new Date(),
        parts: [{
          type: 'text',
          text: errorMessage + " Please try a different request or continue our conversation."
        }]
      } as Message);
    }

    // Return cleaned messages for use in the stream
    console.log("✅ Cleanup complete - returning cleaned messages");
    return cleanedMessages;
  } catch (err) {
    console.error("Cleanup function failed:", err);
    throw new Error("Failed to clean up messages with tool calls");
  }
}

/**
 * Create a recovery model configuration with simplified options
 * This creates a configuration that avoids features that may have caused errors
 */
export function createRecoveryModelOptions(originalOptions: any): any {
  // Create a simplified configuration without tools or complex features
  return {
    ...(originalOptions || {}),
    // Exclude any tools to prevent the same error
    tools: undefined,
    toolCallStreaming: false,
    // Use a more conservative temperature
    temperature: 0.5,
    // Remove any extra options that might cause issues
    functions: undefined,
    functionCallStreaming: false
  };
}