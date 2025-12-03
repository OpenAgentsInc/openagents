/**
 * Cross-provider message transformation.
 *
 * This module handles message format transformations when switching between
 * providers mid-session. Key use case: converting thinking blocks to XML tags
 * when escalating from cheap models to expensive ones, or vice versa.
 *
 * @module
 */

import type { Api, Model } from "./model-types.js";
import type {
  ContentBlock as SDKContentBlock,
  ChatMessage as SDKChatMessage,
  ThinkingBlock,
  RedactedThinkingBlock,
  TextBlock,
} from "../schemas/sdk/messages.js";
import {
  isThinkingBlock,
  isRedactedThinkingBlock,
  isTextBlock,
} from "../schemas/sdk/messages.js";

// =============================================================================
// Types
// =============================================================================

/**
 * Options for message transformation.
 */
export interface TransformOptions {
  /**
   * Whether to preserve thinking blocks in their native form (for providers that support it).
   * Default: false (convert to XML tags)
   */
  preserveNativeThinking?: boolean;

  /**
   * Whether to include redacted thinking blocks in output.
   * Default: true (include as placeholder text)
   */
  includeRedactedThinking?: boolean;

  /**
   * Custom XML tag name for thinking blocks.
   * Default: "thinking"
   */
  thinkingTagName?: string;
}

/**
 * Provider capability flags.
 */
export interface ProviderCapabilities {
  /** Whether provider supports native thinking blocks */
  supportsNativeThinking: boolean;
  /** Whether provider supports images */
  supportsImages: boolean;
  /** Whether provider supports tool use */
  supportsTools: boolean;
}

// =============================================================================
// Provider Capabilities
// =============================================================================

/**
 * Get capabilities for a given API type.
 */
export const getApiCapabilities = (api: Api): ProviderCapabilities => {
  switch (api) {
    case "anthropic-messages":
      return {
        supportsNativeThinking: true,
        supportsImages: true,
        supportsTools: true,
      };
    case "openai-completions":
    case "openai-responses":
      return {
        supportsNativeThinking: false,
        supportsImages: true,
        supportsTools: true,
      };
    case "google-generative-ai":
      return {
        supportsNativeThinking: false,
        supportsImages: true,
        supportsTools: true,
      };
    default:
      // Default to most restrictive
      return {
        supportsNativeThinking: false,
        supportsImages: false,
        supportsTools: false,
      };
  }
};

/**
 * Get capabilities from a Model instance.
 */
export const getModelCapabilities = (model: Model): ProviderCapabilities => ({
  ...getApiCapabilities(model.api),
  supportsImages: model.input.includes("image"),
});

// =============================================================================
// Thinking Block Transformation
// =============================================================================

/**
 * Convert a thinking block to XML-tagged text.
 */
export const thinkingBlockToXml = (
  block: ThinkingBlock,
  tagName = "thinking"
): TextBlock => ({
  type: "text",
  text: `<${tagName}>\n${block.thinking}\n</${tagName}>`,
});

/**
 * Convert a redacted thinking block to placeholder text.
 */
export const redactedThinkingToText = (
  _block: RedactedThinkingBlock
): TextBlock => ({
  type: "text",
  text: "[redacted thinking]",
});

/**
 * Parse XML thinking tags back to thinking blocks.
 * Returns the original text block if no thinking tags found.
 */
export const parseThinkingFromXml = (
  block: TextBlock,
  tagName = "thinking"
): SDKContentBlock[] => {
  const pattern = new RegExp(
    `<${tagName}>\\n?([\\s\\S]*?)\\n?<\\/${tagName}>`,
    "g"
  );
  const text = block.text;
  const results: SDKContentBlock[] = [];
  let lastIndex = 0;

  let match;
  while ((match = pattern.exec(text)) !== null) {
    // Add any text before the thinking block
    if (match.index > lastIndex) {
      const beforeText = text.slice(lastIndex, match.index).trim();
      if (beforeText) {
        results.push({ type: "text", text: beforeText });
      }
    }

    // Add the thinking block
    results.push({
      type: "thinking",
      thinking: match[1],
    });

    lastIndex = pattern.lastIndex;
  }

  // Add any remaining text after the last match
  if (lastIndex < text.length) {
    const afterText = text.slice(lastIndex).trim();
    if (afterText) {
      results.push({ type: "text", text: afterText });
    }
  }

  // If no matches found, return original block
  return results.length > 0 ? results : [block];
};

// =============================================================================
// Content Block Transformation
// =============================================================================

/**
 * Transform a single content block for the target provider.
 */
export const transformContentBlock = (
  block: SDKContentBlock,
  capabilities: ProviderCapabilities,
  options: TransformOptions = {}
): SDKContentBlock[] => {
  const {
    preserveNativeThinking = false,
    includeRedactedThinking = true,
    thinkingTagName = "thinking",
  } = options;

  // Handle thinking blocks
  if (isThinkingBlock(block)) {
    // Preserve native thinking if provider supports it and option is set
    if (capabilities.supportsNativeThinking && preserveNativeThinking) {
      return [block];
    }
    // Convert to XML tags
    return [thinkingBlockToXml(block, thinkingTagName)];
  }

  // Handle redacted thinking blocks
  if (isRedactedThinkingBlock(block)) {
    if (!includeRedactedThinking) {
      return [];
    }
    // Preserve native if provider supports it
    if (capabilities.supportsNativeThinking && preserveNativeThinking) {
      return [block];
    }
    return [redactedThinkingToText(block)];
  }

  // Handle text blocks - check for embedded XML thinking tags
  if (isTextBlock(block) && capabilities.supportsNativeThinking && preserveNativeThinking) {
    // Parse XML tags back to native thinking blocks
    return parseThinkingFromXml(block, thinkingTagName);
  }

  // Handle images - filter if not supported
  if (block.type === "image" && !capabilities.supportsImages) {
    return [
      {
        type: "text",
        text: "[image content not supported by target provider]",
      },
    ];
  }

  // Pass through all other block types
  return [block];
};

/**
 * Transform content blocks for the target provider.
 */
export const transformContentBlocks = (
  blocks: readonly SDKContentBlock[],
  capabilities: ProviderCapabilities,
  options: TransformOptions = {}
): SDKContentBlock[] =>
  blocks.flatMap((block) => transformContentBlock(block, capabilities, options));

// =============================================================================
// Message Transformation
// =============================================================================

/**
 * Transform a single message for the target provider.
 */
export const transformMessage = (
  message: SDKChatMessage,
  capabilities: ProviderCapabilities,
  options: TransformOptions = {}
): SDKChatMessage => {
  // String content - no transformation needed
  if (typeof message.content === "string") {
    return message;
  }

  // Transform content blocks
  const transformedContent = transformContentBlocks(
    message.content,
    capabilities,
    options
  );

  // Merge adjacent text blocks for cleaner output
  const mergedContent = mergeAdjacentTextBlocks(transformedContent);

  return {
    ...message,
    content: mergedContent,
  };
};

/**
 * Merge adjacent text blocks into single blocks.
 */
export const mergeAdjacentTextBlocks = (
  blocks: SDKContentBlock[]
): SDKContentBlock[] => {
  if (blocks.length === 0) return blocks;

  const result: SDKContentBlock[] = [];
  let pendingText: string[] = [];

  const flushPendingText = () => {
    if (pendingText.length > 0) {
      result.push({
        type: "text",
        text: pendingText.join("\n\n"),
      });
      pendingText = [];
    }
  };

  for (const block of blocks) {
    if (isTextBlock(block)) {
      pendingText.push(block.text);
    } else {
      flushPendingText();
      result.push(block);
    }
  }

  flushPendingText();
  return result;
};

// =============================================================================
// Main API
// =============================================================================

/**
 * Transform messages for a target provider API.
 *
 * This is the main entry point for cross-provider message transformation.
 * Use this when switching providers mid-session to ensure message format
 * compatibility.
 *
 * @example
 * ```ts
 * // Escalate from cheap model to Claude with thinking
 * const transformed = transformMessagesForProvider(
 *   messages,
 *   "anthropic-messages",
 *   { preserveNativeThinking: true }
 * );
 *
 * // De-escalate from Claude to GPT (thinking -> XML tags)
 * const transformed = transformMessagesForProvider(
 *   messages,
 *   "openai-completions"
 * );
 * ```
 */
export const transformMessagesForProvider = (
  messages: SDKChatMessage[],
  targetApi: Api,
  options: TransformOptions = {}
): SDKChatMessage[] => {
  const capabilities = getApiCapabilities(targetApi);
  return messages.map((msg) => transformMessage(msg, capabilities, options));
};

/**
 * Transform messages for a target model.
 *
 * Like transformMessagesForProvider but uses model metadata for capabilities.
 */
export const transformMessagesForModel = (
  messages: SDKChatMessage[],
  targetModel: Model,
  options: TransformOptions = {}
): SDKChatMessage[] => {
  const capabilities = getModelCapabilities(targetModel);
  return messages.map((msg) => transformMessage(msg, capabilities, options));
};

/**
 * Check if messages contain thinking blocks that need transformation.
 */
export const hasThinkingBlocks = (messages: SDKChatMessage[]): boolean =>
  messages.some((msg) => {
    if (typeof msg.content === "string") return false;
    return msg.content.some(
      (block) => isThinkingBlock(block) || isRedactedThinkingBlock(block)
    );
  });

/**
 * Check if messages contain XML thinking tags.
 */
export const hasXmlThinkingTags = (
  messages: SDKChatMessage[],
  tagName = "thinking"
): boolean => {
  const pattern = new RegExp(`<${tagName}>[\\s\\S]*?<\\/${tagName}>`);
  return messages.some((msg) => {
    if (typeof msg.content === "string") {
      return pattern.test(msg.content);
    }
    return msg.content.some(
      (block) => isTextBlock(block) && pattern.test(block.text)
    );
  });
};
