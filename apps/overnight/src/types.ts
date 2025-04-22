// Custom types for Anthropic SDK extensions
import { MessageParam, MessageCreateParamsBase, MessageCreateParams } from '@anthropic-ai/sdk/resources/messages';

/**
 * Extends Anthropic SDK's MessageParam for conversation history
 */
export interface ExtendedMessageParam extends Omit<MessageParam, 'content'> {
  role: "user" | "assistant";
  content: string | ExtendedContentBlock[];
}

/**
 * Extends Anthropic SDK's MessageCreateParams to support tools with proper typing
 */
export interface MessageCreateParamsWithTools extends Omit<MessageCreateParams, 'messages' | 'system'> {
  tools?: ToolDefinition[];
  messages: ExtendedMessageParam[];
  system?: string; // Make system explicitly optional string
}

/**
 * Tool Definition for Anthropic Claude
 * Updated to match the latest Anthropic API format (v0.39.0)
 */
export interface ToolDefinition {
  name: string;
  description: string;
  input_schema: {
    type: 'object';
    properties: Record<string, {
      type: string;
      description: string;
    }>;
    required: string[];
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
 * Updates the TextBlock to match the SDK v0.39.0
 */
export interface CustomTextBlock {
  type: 'text';
  text: string;
  citations?: Array<{ start: number; end: number; text: string; }> | null; // Match SDK structure
}

export type ExtendedContentBlock = CustomTextBlock | ContentBlockToolUse | ContentBlockToolResult;

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