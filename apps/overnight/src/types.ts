// Custom types for Anthropic SDK extensions
import { MessageParam, MessageCreateParamsBase, TextBlock } from '@anthropic-ai/sdk/resources/messages';

/**
 * Extends Anthropic SDK's MessageParam for conversation history
 */
export interface ExtendedMessageParam extends Omit<MessageParam, 'content'> {
  role: "user" | "assistant";
  content: string | ExtendedContentBlock[];
}

/**
 * Extends Anthropic SDK's MessageCreateParamsBase to support tools
 */
export interface MessageCreateParamsWithTools extends Omit<MessageCreateParamsBase, 'messages'> {
  tools?: ToolDefinition[];
  messages: ExtendedMessageParam[];
}

/**
 * Tool Definition for Anthropic Claude
 */
export interface ToolDefinition {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: {
      type: 'object';
      properties: Record<string, {
        type: string;
        description: string;
      }>;
      required: string[];
    };
  };
}

/**
 * Content Block for Tool Use (not defined in SDK)
 */
export interface ContentBlockToolUse {
  type: 'tool_use';
  tool_use: {
    id: string;
    name: string;
    input: Record<string, unknown>;
  };
}

/**
 * Content Block for Tool Result (not defined in SDK)
 */
export interface ContentBlockToolResult {
  type: 'tool_result';
  tool_use_id: string;
  content: string;
}

/**
 * Extended Content Block Type
 */
export type ExtendedContentBlock = TextBlock | ContentBlockToolUse | ContentBlockToolResult;

/**
 * Message Response from Anthropic API with tools
 */
export interface ExtendedMessage {
  id: string;
  content: ExtendedContentBlock[];
  role: string;
  model: string;
  stop_reason: string | null;
  stop_sequence: string | null;
  usage: {
    input_tokens: number;
    output_tokens: number;
  };
}