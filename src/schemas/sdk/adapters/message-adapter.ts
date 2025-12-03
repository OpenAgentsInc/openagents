/**
 * Message Adapter for converting between OpenAgents and SDK message formats.
 *
 * This module provides bidirectional conversion between:
 * - Internal ChatMessage (from llm/openrouter.ts) ↔ SDK message types
 * - Internal ContentBlock ↔ SDK ContentBlock
 * - Internal ChatToolCall ↔ SDK ToolUseBlock
 *
 * @module
 */

import type {
  ChatMessage as InternalChatMessage,
  ContentBlock as InternalContentBlock,
  ChatToolCall,
} from "../../../llm/openrouter.js";

import type {
  ContentBlock as SDKContentBlock,
  TextBlock,
  ToolUseBlock,
  ToolResultBlock,
  ImageBlock,
  SDKUserMessage,
  SDKAssistantMessage,
  ChatMessage as SDKChatMessage,
} from "../messages.js";

import type { ToolContent } from "../tool-outputs.js";

// =============================================================================
// Types
// =============================================================================

/**
 * Result of converting SDK message to internal format.
 */
export interface MessageConversionResult {
  message: InternalChatMessage;
  toolCalls?: ChatToolCall[] | undefined;
}

/**
 * Options for message conversion.
 */
export interface MessageAdapterOptions {
  /** Session ID for SDK messages (default: "default") */
  sessionId?: string;
  /** Whether to preserve thinking blocks (default: false) */
  preserveThinking?: boolean;
  /** Parent tool use ID for nested tool calls */
  parentToolUseId?: string | null;
}

// =============================================================================
// Internal → SDK Content Block Conversion
// =============================================================================

/**
 * Convert internal ContentBlock to SDK ContentBlock.
 */
export const internalContentToSdk = (
  content: InternalContentBlock
): SDKContentBlock => {
  if (content.type === "text") {
    return {
      type: "text",
      text: content.text,
    } satisfies TextBlock;
  }

  if (content.type === "image") {
    return {
      type: "image",
      source: {
        type: "base64",
        media_type: content.mimeType,
        data: content.data,
      },
    } satisfies ImageBlock;
  }

  // Fallback - treat unknown as text
  return {
    type: "text",
    text: JSON.stringify(content),
  };
};

/**
 * Convert array of internal content to SDK content blocks.
 */
export const internalContentsToSdk = (
  contents: InternalContentBlock[]
): SDKContentBlock[] => contents.map(internalContentToSdk);

/**
 * Convert internal content to ToolContent (text/image only).
 * Used for tool results which have a more restricted content type.
 */
export const internalContentToToolContent = (
  content: InternalContentBlock
): ToolContent => {
  if (content.type === "text") {
    return { type: "text", text: content.text };
  }
  if (content.type === "image") {
    return { type: "image", data: content.data, mimeType: content.mimeType };
  }
  // Fallback - convert unknown to text
  return { type: "text", text: JSON.stringify(content) };
};

/**
 * Convert array of internal content to ToolContent array.
 */
export const internalContentsToToolContent = (
  contents: InternalContentBlock[]
): ToolContent[] => contents.map(internalContentToToolContent);

// =============================================================================
// SDK → Internal Content Block Conversion
// =============================================================================

/**
 * Convert SDK ContentBlock to internal ContentBlock.
 * Note: Some SDK types (thinking, tool_use, tool_result) don't map directly
 * and are converted to text representations.
 */
export const sdkContentToInternal = (
  content: SDKContentBlock
): InternalContentBlock => {
  switch (content.type) {
    case "text":
      return {
        type: "text",
        text: content.text,
      };

    case "image":
      return {
        type: "image",
        data: content.source.data,
        mimeType: content.source.media_type,
      };

    case "thinking":
      // Convert thinking to text for internal use
      return {
        type: "text",
        text: `[thinking] ${content.thinking}`,
      };

    case "redacted_thinking":
      return {
        type: "text",
        text: "[redacted thinking]",
      };

    case "tool_use":
      // Tool use blocks are typically handled separately
      return {
        type: "text",
        text: `[tool_use: ${content.name}]`,
      };

    case "tool_result":
      // Tool results are typically handled separately
      return {
        type: "text",
        text: `[tool_result: ${content.tool_use_id}]`,
      };

    default:
      return {
        type: "text",
        text: JSON.stringify(content),
      };
  }
};

/**
 * Convert array of SDK content blocks to internal format.
 */
export const sdkContentsToInternal = (
  contents: readonly SDKContentBlock[]
): InternalContentBlock[] => contents.map(sdkContentToInternal);

// =============================================================================
// Tool Call Conversion
// =============================================================================

/**
 * Convert ChatToolCall to SDK ToolUseBlock.
 */
export const toolCallToToolUseBlock = (toolCall: ChatToolCall): ToolUseBlock => ({
  type: "tool_use",
  id: toolCall.id,
  name: toolCall.name,
  input: JSON.parse(toolCall.arguments),
});

/**
 * Convert SDK ToolUseBlock to ChatToolCall.
 */
export const toolUseBlockToToolCall = (block: ToolUseBlock): ChatToolCall => ({
  id: block.id,
  name: block.name,
  arguments: typeof block.input === "string"
    ? block.input
    : JSON.stringify(block.input),
});

/**
 * Extract tool calls from SDK content blocks.
 */
export const extractToolCalls = (
  contents: readonly SDKContentBlock[]
): ChatToolCall[] =>
  contents
    .filter((c): c is ToolUseBlock => c.type === "tool_use")
    .map(toolUseBlockToToolCall);

// =============================================================================
// Internal → SDK Message Conversion
// =============================================================================

/**
 * Convert internal ChatMessage to SDK ChatMessage.
 */
export const internalMessageToSdkChat = (
  message: InternalChatMessage
): SDKChatMessage => {
  const content: string | SDKContentBlock[] =
    typeof message.content === "string"
      ? message.content
      : internalContentsToSdk(message.content);

  return {
    role: message.role === "tool" ? "user" : message.role,
    content,
    ...(message.name ? { name: message.name } : {}),
    ...(message.tool_call_id ? { tool_call_id: message.tool_call_id } : {}),
  };
};

/**
 * Convert internal ChatMessage to SDKUserMessage.
 */
export const internalToSdkUserMessage = (
  message: InternalChatMessage,
  options: MessageAdapterOptions = {}
): SDKUserMessage => {
  const { sessionId = "default", parentToolUseId = null } = options;

  const content: string | SDKContentBlock[] =
    typeof message.content === "string"
      ? message.content
      : internalContentsToSdk(message.content);

  return {
    type: "user",
    session_id: sessionId,
    message: {
      role: "user",
      content,
    },
    parent_tool_use_id: parentToolUseId,
  };
};

/**
 * Convert internal assistant response to SDKAssistantMessage.
 */
export const internalToSdkAssistantMessage = (
  message: InternalChatMessage,
  options: MessageAdapterOptions & { uuid?: string } = {}
): SDKAssistantMessage => {
  const {
    sessionId = "default",
    parentToolUseId = null,
    uuid = crypto.randomUUID(),
  } = options;

  return {
    type: "assistant",
    uuid,
    session_id: sessionId,
    message: {
      role: "assistant",
      content: typeof message.content === "string"
        ? message.content
        : internalContentsToSdk(message.content),
    },
    parent_tool_use_id: parentToolUseId,
  };
};

// =============================================================================
// SDK → Internal Message Conversion
// =============================================================================

/**
 * Convert SDK ChatMessage to internal ChatMessage.
 */
export const sdkChatToInternalMessage = (
  message: SDKChatMessage
): InternalChatMessage => {
  const content: string | InternalContentBlock[] =
    typeof message.content === "string"
      ? message.content
      : sdkContentsToInternal(message.content);

  return {
    role: message.role,
    content,
    ...(message.name ? { name: message.name } : {}),
    ...(message.tool_call_id ? { tool_call_id: message.tool_call_id } : {}),
  };
};

/**
 * Convert SDKUserMessage to internal ChatMessage.
 */
export const sdkUserToInternalMessage = (
  message: SDKUserMessage
): InternalChatMessage => {
  const content: string | InternalContentBlock[] =
    typeof message.message.content === "string"
      ? message.message.content
      : sdkContentsToInternal(message.message.content);

  return {
    role: "user",
    content,
  };
};

/**
 * Convert SDKAssistantMessage to internal ChatMessage with tool calls.
 */
export const sdkAssistantToInternalMessage = (
  message: SDKAssistantMessage
): MessageConversionResult => {
  const apiMessage = message.message as {
    role: string;
    content?: string | SDKContentBlock[];
  };

  const contentBlocks = Array.isArray(apiMessage.content)
    ? apiMessage.content as SDKContentBlock[]
    : [];

  // Extract tool calls
  const toolCalls = extractToolCalls(contentBlocks);

  // Convert content (excluding tool_use blocks for the main content)
  const textContent = contentBlocks
    .filter((c): c is TextBlock => c.type === "text")
    .map((c) => c.text)
    .join("");

  const internalMessage: InternalChatMessage = {
    role: "assistant",
    content: textContent || (typeof apiMessage.content === "string" ? apiMessage.content : ""),
  };

  return {
    message: internalMessage,
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
  };
};

// =============================================================================
// Tool Result Helpers
// =============================================================================

/**
 * Create a tool result message for the SDK.
 */
export const createToolResultBlock = (
  toolUseId: string,
  result: string | InternalContentBlock[],
  isError = false
): ToolResultBlock => ({
  type: "tool_result",
  tool_use_id: toolUseId,
  content: typeof result === "string"
    ? [{ type: "text", text: result }]
    : internalContentsToToolContent(result),
  is_error: isError,
});

/**
 * Create an internal tool result message.
 */
export const createInternalToolResult = (
  toolCallId: string,
  content: string,
  toolName?: string
): InternalChatMessage => ({
  role: "tool",
  content,
  tool_call_id: toolCallId,
  ...(toolName ? { name: toolName } : {}),
});

// =============================================================================
// Batch Conversion Helpers
// =============================================================================

/**
 * Convert array of internal messages to SDK ChatMessages.
 */
export const internalMessagesToSdkChat = (
  messages: InternalChatMessage[]
): SDKChatMessage[] => messages.map(internalMessageToSdkChat);

/**
 * Convert array of SDK ChatMessages to internal messages.
 */
export const sdkChatToInternalMessages = (
  messages: SDKChatMessage[]
): InternalChatMessage[] => messages.map(sdkChatToInternalMessage);
