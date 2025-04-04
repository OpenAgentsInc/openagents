/**
 * Request validation utilities
 */

import type { Message } from 'ai';
import { 
  MessageValidationError, 
  ContentValidationError,
  ModelValidationError
} from '@openagents/core/src/chat/errors';

/**
 * Validate chat request body
 */
export function validateChatRequest(body: any): void {
  // Check for required fields
  if (!body) {
    throw new MessageValidationError({
      message: 'Missing request body',
      userMessage: 'Invalid request: missing body'
    });
  }
  
  // Validate model
  if (!body.model) {
    throw new ModelValidationError({
      message: 'Missing model in request',
      userMessage: 'Please specify a model for your request'
    });
  }
  
  // Validate messages
  if (!body.messages || !Array.isArray(body.messages) || body.messages.length === 0) {
    throw new MessageValidationError({
      message: 'Invalid or missing messages array',
      userMessage: 'Your request must include a non-empty messages array'
    });
  }
}

/**
 * Sanitize messages to ensure valid format
 */
export function sanitizeMessages(messages: Message[]): Message[] {
  return messages.map(msg => {
    // Ensure every message has valid content
    if (!msg.content && (!msg.parts || msg.parts.length === 0)) {
      console.log(`[Server] Found empty message with role ${msg.role}, adding placeholder content`);
      return {
        ...msg,
        content: msg.role === 'user' ? 'Please continue.' : 'I understand.',
        parts: [{ type: 'text', text: msg.role === 'user' ? 'Please continue.' : 'I understand.' }]
      };
    }

    // If content exists but parts is missing or empty, create parts from content
    if (msg.content && (!msg.parts || msg.parts.length === 0)) {
      return {
        ...msg,
        parts: [{ type: 'text', text: msg.content }]
      };
    }

    return msg;
  });
}

/**
 * Normalize system messages in conversation history
 * This ensures system messages are at the beginning and properly formatted
 */
export function normalizeSystemMessages(messages: Message[]): Message[] {
  // Find all system messages
  const systemMessageIndices = messages
    .map((msg, index) => msg.role === 'system' ? index : -1)
    .filter(index => index !== -1);
  
  // If there are no system messages or the first one is already at the beginning, return original
  if (systemMessageIndices.length === 0 || 
      (systemMessageIndices.length === 1 && systemMessageIndices[0] === 0)) {
    return messages;
  }
  
  // If system messages need reorganization
  console.log("[Server] Reorganizing system messages");
  
  // Extract all system messages
  const systemMessages = messages.filter(msg => msg.role === 'system');
  
  // Combine their content
  const combinedSystemContent = systemMessages
    .map(msg => msg.content)
    .join("\n\n");
  
  // Filter out all system messages
  const nonSystemMessages = messages.filter(msg => msg.role !== 'system');
  
  // Create a new array with a single system message at the beginning
  if (combinedSystemContent) {
    return [
      {
        role: 'system',
        content: combinedSystemContent,
        id: `system-${Date.now()}`,
        parts: [{ type: 'text', text: combinedSystemContent }]
      },
      ...nonSystemMessages
    ];
  }
  
  return nonSystemMessages;
}

/**
 * Filter out invalid messages
 */
export function filterInvalidMessages(messages: Message[]): Message[] {
  return messages.filter(msg => {
    if (!msg.content && (!msg.parts || msg.parts.length === 0)) {
      console.log(`[Server] Filtering out message with empty content and parts`);
      return false;
    }
    return true;
  });
}