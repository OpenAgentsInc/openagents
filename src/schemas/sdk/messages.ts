/**
 * SDK-compatible message schemas.
 *
 * These schemas define the message types used in agent communication,
 * matching Claude Agent SDK conventions for content blocks and message types.
 *
 * @module
 */

import * as S from "effect/Schema";
import { ToolContent } from "./tool-outputs.js";

// =============================================================================
// Content Block Types
// =============================================================================

/**
 * Text content block in a message.
 */
export const TextBlock = S.Struct({
  type: S.Literal("text"),
  text: S.String,
});
export type TextBlock = S.Schema.Type<typeof TextBlock>;

/**
 * Thinking/reasoning content block (extended thinking).
 */
export const ThinkingBlock = S.Struct({
  type: S.Literal("thinking"),
  thinking: S.String,
  signature: S.optional(S.String),
});
export type ThinkingBlock = S.Schema.Type<typeof ThinkingBlock>;

/**
 * Redacted thinking block (when thinking is hidden).
 */
export const RedactedThinkingBlock = S.Struct({
  type: S.Literal("redacted_thinking"),
  data: S.String,
});
export type RedactedThinkingBlock = S.Schema.Type<typeof RedactedThinkingBlock>;

/**
 * Tool use request block.
 */
export const ToolUseBlock = S.Struct({
  type: S.Literal("tool_use"),
  id: S.String.pipe(
    S.annotations({ description: "Unique ID for this tool use" }),
  ),
  name: S.String.pipe(
    S.annotations({ description: "Name of the tool being called" }),
  ),
  input: S.Unknown.pipe(
    S.annotations({ description: "Tool input parameters" }),
  ),
});
export type ToolUseBlock = S.Schema.Type<typeof ToolUseBlock>;

/**
 * Tool result block (response to tool use).
 */
export const ToolResultBlock = S.Struct({
  type: S.Literal("tool_result"),
  tool_use_id: S.String.pipe(
    S.annotations({ description: "ID of the tool use this responds to" }),
  ),
  content: S.Array(ToolContent),
  is_error: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Whether this result represents an error" }),
    ),
  ),
});
export type ToolResultBlock = S.Schema.Type<typeof ToolResultBlock>;

/**
 * Image block in messages.
 */
export const ImageBlock = S.Struct({
  type: S.Literal("image"),
  source: S.Struct({
    type: S.Literal("base64"),
    media_type: S.String,
    data: S.String,
  }),
});
export type ImageBlock = S.Schema.Type<typeof ImageBlock>;

/**
 * Union of all content block types that can appear in messages.
 */
export const ContentBlock = S.Union(
  TextBlock,
  ThinkingBlock,
  RedactedThinkingBlock,
  ToolUseBlock,
  ToolResultBlock,
  ImageBlock,
);
export type ContentBlock = S.Schema.Type<typeof ContentBlock>;

// =============================================================================
// Content Block Type Guards
// =============================================================================

export const isTextBlock = (block: ContentBlock): block is TextBlock =>
  block.type === "text";

export const isThinkingBlock = (block: ContentBlock): block is ThinkingBlock =>
  block.type === "thinking";

export const isRedactedThinkingBlock = (block: ContentBlock): block is RedactedThinkingBlock =>
  block.type === "redacted_thinking";

export const isToolUseBlock = (block: ContentBlock): block is ToolUseBlock =>
  block.type === "tool_use";

export const isToolResultBlock = (block: ContentBlock): block is ToolResultBlock =>
  block.type === "tool_result";

export const isImageBlock = (block: ContentBlock): block is ImageBlock =>
  block.type === "image";

// =============================================================================
// Message Types
// =============================================================================

/**
 * User message in a conversation.
 */
export const SDKUserMessage = S.Struct({
  type: S.Literal("user"),
  uuid: S.optional(S.String),
  session_id: S.String,
  message: S.Struct({
    role: S.Literal("user"),
    content: S.Union(S.String, S.Array(ContentBlock)),
  }),
  parent_tool_use_id: S.NullOr(S.String),
});
export type SDKUserMessage = S.Schema.Type<typeof SDKUserMessage>;

/**
 * Assistant message in a conversation.
 */
export const SDKAssistantMessage = S.Struct({
  type: S.Literal("assistant"),
  uuid: S.String,
  session_id: S.String,
  message: S.Unknown, // APIAssistantMessage from Anthropic SDK
  parent_tool_use_id: S.NullOr(S.String),
});
export type SDKAssistantMessage = S.Schema.Type<typeof SDKAssistantMessage>;

/**
 * Result message subtypes.
 */
export const ResultSubtype = S.Literal(
  "success",
  "error_max_turns",
  "error_during_execution",
);
export type ResultSubtype = S.Schema.Type<typeof ResultSubtype>;

/**
 * Result message indicating end of conversation.
 */
export const SDKResultMessage = S.Struct({
  type: S.Literal("result"),
  subtype: ResultSubtype,
  uuid: S.String,
  session_id: S.String,
  duration_ms: S.Number,
  duration_api_ms: S.Number,
  is_error: S.Boolean,
  num_turns: S.Number,
  result: S.optional(S.String),
  total_cost_usd: S.Number,
  usage: S.Unknown, // NonNullableUsage
  permission_denials: S.Array(S.Unknown),
});
export type SDKResultMessage = S.Schema.Type<typeof SDKResultMessage>;

/**
 * System message subtypes.
 */
export const SystemSubtype = S.Literal("init");
export type SystemSubtype = S.Schema.Type<typeof SystemSubtype>;

/**
 * System initialization message.
 */
export const SDKSystemMessage = S.Struct({
  type: S.Literal("system"),
  subtype: SystemSubtype,
  session_id: S.String,
  cwd: S.String,
  tools: S.Array(S.String),
  mcp_servers: S.Record({ key: S.String, value: S.Unknown }),
  model: S.String,
  permission_mode: S.String,
  api_key_source: S.String,
  max_turns: S.Number,
});
export type SDKSystemMessage = S.Schema.Type<typeof SDKSystemMessage>;

/**
 * Union of all SDK message types.
 */
export const SDKMessage = S.Union(
  SDKUserMessage,
  SDKAssistantMessage,
  SDKResultMessage,
  SDKSystemMessage,
);
export type SDKMessage = S.Schema.Type<typeof SDKMessage>;

// =============================================================================
// Message Type Guards
// =============================================================================

export const isSDKUserMessage = (msg: SDKMessage): msg is SDKUserMessage =>
  msg.type === "user";

export const isSDKAssistantMessage = (msg: SDKMessage): msg is SDKAssistantMessage =>
  msg.type === "assistant";

export const isSDKResultMessage = (msg: SDKMessage): msg is SDKResultMessage =>
  msg.type === "result";

export const isSDKSystemMessage = (msg: SDKMessage): msg is SDKSystemMessage =>
  msg.type === "system";

// =============================================================================
// Simplified Chat Message (for internal use)
// =============================================================================

/**
 * Message role.
 */
export const MessageRole = S.Literal("user", "assistant", "system");
export type MessageRole = S.Schema.Type<typeof MessageRole>;

/**
 * Simplified chat message for internal use.
 * Can be converted to/from SDKMessage using adapters.
 */
export const ChatMessage = S.Struct({
  role: MessageRole,
  content: S.Union(S.String, S.Array(ContentBlock)),
  name: S.optional(S.String),
  tool_call_id: S.optional(S.String),
});
export type ChatMessage = S.Schema.Type<typeof ChatMessage>;

// =============================================================================
// Streaming Message Types
// =============================================================================

/**
 * Delta for incremental text updates.
 */
export const TextDelta = S.Struct({
  type: S.Literal("text_delta"),
  text: S.String,
});
export type TextDelta = S.Schema.Type<typeof TextDelta>;

/**
 * Delta for thinking block updates.
 */
export const ThinkingDelta = S.Struct({
  type: S.Literal("thinking_delta"),
  thinking: S.String,
});
export type ThinkingDelta = S.Schema.Type<typeof ThinkingDelta>;

/**
 * Delta for input JSON updates.
 */
export const InputJsonDelta = S.Struct({
  type: S.Literal("input_json_delta"),
  partial_json: S.String,
});
export type InputJsonDelta = S.Schema.Type<typeof InputJsonDelta>;

/**
 * Union of all delta types.
 */
export const ContentDelta = S.Union(TextDelta, ThinkingDelta, InputJsonDelta);
export type ContentDelta = S.Schema.Type<typeof ContentDelta>;

/**
 * Stream event types.
 */
export const StreamEventType = S.Literal(
  "message_start",
  "content_block_start",
  "content_block_delta",
  "content_block_stop",
  "message_delta",
  "message_stop",
);
export type StreamEventType = S.Schema.Type<typeof StreamEventType>;

/**
 * Content block start event.
 */
export const ContentBlockStartEvent = S.Struct({
  type: S.Literal("content_block_start"),
  index: S.Number,
  content_block: ContentBlock,
});
export type ContentBlockStartEvent = S.Schema.Type<typeof ContentBlockStartEvent>;

/**
 * Content block delta event.
 */
export const ContentBlockDeltaEvent = S.Struct({
  type: S.Literal("content_block_delta"),
  index: S.Number,
  delta: ContentDelta,
});
export type ContentBlockDeltaEvent = S.Schema.Type<typeof ContentBlockDeltaEvent>;

/**
 * Content block stop event.
 */
export const ContentBlockStopEvent = S.Struct({
  type: S.Literal("content_block_stop"),
  index: S.Number,
});
export type ContentBlockStopEvent = S.Schema.Type<typeof ContentBlockStopEvent>;

// =============================================================================
// Conversation Types
// =============================================================================

/**
 * A conversation consisting of messages.
 */
export const Conversation = S.Struct({
  id: S.String,
  messages: S.Array(SDKMessage),
  created_at: S.String,
  updated_at: S.String,
  metadata: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type Conversation = S.Schema.Type<typeof Conversation>;

/**
 * Conversation turn (request + response pair).
 */
export const ConversationTurn = S.Struct({
  user_message: SDKUserMessage,
  assistant_message: S.optional(SDKAssistantMessage),
  tool_results: S.optional(S.Array(ToolResultBlock)),
});
export type ConversationTurn = S.Schema.Type<typeof ConversationTurn>;
