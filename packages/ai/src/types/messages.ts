/**
 * Standardized message types aligned with Vercel AI SDK v5
 * @since 1.0.0
 */
import * as Schema from "effect/Schema"
import * as InternalCommon from "../core/internal/common.js"

// =============================================================================
// Base Types
// =============================================================================

/**
 * Token usage information
 * @since 1.0.0
 * @category Models
 */
export class TokenUsage extends Schema.Class<TokenUsage>("TokenUsage")({
  promptTokens: Schema.Number,
  completionTokens: Schema.Number,
  totalTokens: Schema.Number
}) {}

/**
 * Provider-specific metadata
 * @since 1.0.0
 * @category Models
 */
export const ProviderMetadata = Schema.Record({
  key: Schema.String,
  value: Schema.Record({
    key: Schema.String,
    value: Schema.Unknown
  })
})

/**
 * @since 1.0.0
 * @category Models
 */
export type ProviderMetadata = typeof ProviderMetadata.Type

/**
 * Base message metadata
 * @since 1.0.0
 * @category Models
 */
export class MessageMetadata extends Schema.Class<MessageMetadata>("MessageMetadata")({
  id: Schema.optional(Schema.String),
  createdAt: Schema.optional(Schema.Date),
  model: Schema.optional(Schema.String),
  usage: Schema.optional(TokenUsage),
  custom: Schema.optional(Schema.Record({
    key: Schema.String,
    value: Schema.Unknown
  }))
}) {}

// =============================================================================
// Content Parts (Vercel AI SDK Compatible)
// =============================================================================

/**
 * Text content part
 * @since 1.0.0
 * @category Models
 */
export class TextPart extends Schema.TaggedClass<TextPart>("TextPart")("text", {
  text: Schema.String,
  annotations: Schema.optional(Schema.Array(Schema.Unknown))
}) {}

/**
 * Image content part
 * @since 1.0.0
 * @category Models
 */
export class ImagePart extends Schema.TaggedClass<ImagePart>("ImagePart")("image", {
  image: Schema.Union(
    Schema.String, // base64
    Schema.instanceOf(Uint8Array)
  ),
  mimeType: Schema.optional(Schema.String)
}) {}

/**
 * Image URL content part
 * @since 1.0.0
 * @category Models
 */
export class ImageUrlPart extends Schema.TaggedClass<ImageUrlPart>("ImageUrlPart")("image-url", {
  url: Schema.String
}) {}

/**
 * File content part
 * @since 1.0.0
 * @category Models
 */
export class FilePart extends Schema.TaggedClass<FilePart>("FilePart")("file", {
  data: Schema.Union(
    Schema.String, // base64
    Schema.instanceOf(Uint8Array)
  ),
  mimeType: Schema.String,
  name: Schema.optional(Schema.String)
}) {}

/**
 * File URL content part
 * @since 1.0.0
 * @category Models
 */
export class FileUrlPart extends Schema.TaggedClass<FileUrlPart>("FileUrlPart")("file-url", {
  url: Schema.String
}) {}

/**
 * Tool call identifier (branded type)
 * @since 1.0.0
 * @category Models
 */
export const ToolCallId = InternalCommon.ToolCallId

/**
 * @since 1.0.0
 * @category Models
 */
export type ToolCallId = typeof ToolCallId.Type

/**
 * Tool call content part
 * @since 1.0.0
 * @category Models
 */
export class ToolCallPart extends Schema.TaggedClass<ToolCallPart>("ToolCallPart")("tool-call", {
  toolCallId: ToolCallId,
  toolName: Schema.String,
  args: Schema.Unknown
}) {}

/**
 * Tool result content part
 * @since 1.0.0
 * @category Models
 */
export class ToolResultPart extends Schema.TaggedClass<ToolResultPart>("ToolResultPart")("tool-result", {
  toolCallId: ToolCallId,
  toolName: Schema.String,
  result: Schema.Unknown,
  isError: Schema.optional(Schema.Boolean)
}) {}

/**
 * Reasoning content part (for o1, Claude thinking)
 * @since 1.0.0
 * @category Models
 */
export class ReasoningPart extends Schema.TaggedClass<ReasoningPart>("ReasoningPart")("reasoning", {
  reasoning: Schema.String,
  signature: Schema.optional(Schema.String),
  isRedacted: Schema.optional(Schema.Boolean)
}) {}

/**
 * All content part types
 * @since 1.0.0
 * @category Models
 */
export const ContentPart = Schema.Union(
  TextPart,
  ImagePart,
  ImageUrlPart,
  FilePart,
  FileUrlPart,
  ToolCallPart,
  ToolResultPart,
  ReasoningPart
)

/**
 * @since 1.0.0
 * @category Models
 */
export type ContentPart = typeof ContentPart.Type

// =============================================================================
// Message Types (Vercel AI SDK Compatible)
// =============================================================================

/**
 * User message with multi-modal content
 * @since 1.0.0
 * @category Models
 */
export class UserMessage extends Schema.Class<UserMessage>("UserMessage")({
  role: Schema.Literal("user"),
  content: Schema.Array(Schema.Union(
    TextPart,
    ImagePart,
    ImageUrlPart,
    FilePart,
    FileUrlPart
  )),
  name: Schema.optional(Schema.String),
  metadata: Schema.optional(MessageMetadata)
}) {}

/**
 * Assistant message with reasoning and tool calls
 * @since 1.0.0
 * @category Models
 */
export class AssistantMessage extends Schema.Class<AssistantMessage>("AssistantMessage")({
  role: Schema.Literal("assistant"),
  content: Schema.Array(Schema.Union(
    TextPart,
    ReasoningPart,
    ToolCallPart
  )),
  metadata: Schema.optional(MessageMetadata)
}) {}

/**
 * System message
 * @since 1.0.0
 * @category Models
 */
export class SystemMessage extends Schema.Class<SystemMessage>("SystemMessage")({
  role: Schema.Literal("system"),
  content: Schema.String,
  metadata: Schema.optional(MessageMetadata)
}) {}

/**
 * Tool result message
 * @since 1.0.0
 * @category Models
 */
export class ToolMessage extends Schema.Class<ToolMessage>("ToolMessage")({
  role: Schema.Literal("tool"),
  content: Schema.Array(ToolResultPart),
  metadata: Schema.optional(MessageMetadata)
}) {}

/**
 * Union of all message types
 * @since 1.0.0
 * @category Models
 */
export const Message = Schema.Union(
  UserMessage,
  AssistantMessage,
  SystemMessage,
  ToolMessage
)

/**
 * @since 1.0.0
 * @category Models
 */
export type Message = typeof Message.Type

// =============================================================================
// Response Metadata
// =============================================================================

/**
 * Reason why generation stopped
 * @since 1.0.0
 * @category Models
 */
export const FinishReason = Schema.Literal(
  "stop",
  "length",
  "content-filter",
  "tool-calls",
  "error",
  "other"
)

/**
 * @since 1.0.0
 * @category Models
 */
export type FinishReason = typeof FinishReason.Type

/**
 * Response metadata
 * @since 1.0.0
 * @category Models
 */
export class ResponseMetadata extends Schema.Class<ResponseMetadata>("ResponseMetadata")({
  id: Schema.optional(Schema.String),
  model: Schema.optional(Schema.String),
  timestamp: Schema.optional(Schema.Date),
  usage: Schema.optional(TokenUsage),
  finishReason: Schema.optional(FinishReason),
  providerMetadata: Schema.optional(ProviderMetadata)
}) {}

// =============================================================================
// Helper Types for Compatibility
// =============================================================================

/**
 * Plain JavaScript message type (for Vercel AI SDK compatibility)
 * @since 1.0.0
 * @category Models
 */
export interface PlainMessage {
  role: "user" | "assistant" | "system" | "tool"
  content:
    | string
    | Array<{
      type: string
      [key: string]: any
    }>
  id?: string
  createdAt?: Date
  metadata?: Record<string, unknown>
}

// =============================================================================
// Conversion Utilities
// =============================================================================

/**
 * Convert from our tagged message format to plain format
 * @since 1.0.0
 * @category Conversions
 */
export const toPlainMessage = (message: Message): PlainMessage => {
  switch (message.role) {
    case "system":
      return {
        role: "system",
        content: message.content,
        ...(message.metadata?.id && { id: message.metadata.id }),
        ...(message.metadata?.createdAt && { createdAt: message.metadata.createdAt }),
        ...(message.metadata?.custom && { metadata: message.metadata.custom })
      }

    case "user":
    case "assistant":
    case "tool":
      return {
        role: message.role,
        content: message.content.map((part) => ({
          type: part._tag,
          ...part
        })),
        ...(message.metadata?.id && { id: message.metadata.id }),
        ...(message.metadata?.createdAt && { createdAt: message.metadata.createdAt }),
        ...(message.metadata?.custom && { metadata: message.metadata.custom })
      }
  }
}

/**
 * Convert from plain format to our tagged message format
 * @since 1.0.0
 * @category Conversions
 */
export const fromPlainMessage = (message: PlainMessage): Message => {
  const metadata = message.id || message.createdAt || message.metadata
    ? new MessageMetadata({
      ...(message.id && { id: message.id }),
      ...(message.createdAt && { createdAt: message.createdAt }),
      ...(message.metadata && { custom: message.metadata })
    })
    : undefined

  switch (message.role) {
    case "system":
      return new SystemMessage({
        role: "system",
        content: typeof message.content === "string" ? message.content : "",
        ...(metadata && { metadata })
      })

    case "user":
      return new UserMessage({
        role: "user",
        content: Array.isArray(message.content)
          ? message.content.map((part) => {
            switch (part.type) {
              case "text":
                return new TextPart({ text: part.text })
              case "image":
                return new ImagePart({ image: part.image, mimeType: part.mimeType })
              case "image-url":
                return new ImageUrlPart({ url: part.url })
              case "file":
                return new FilePart({ data: part.data, mimeType: part.mimeType, name: part.name })
              case "file-url":
                return new FileUrlPart({ url: part.url })
              default:
                return new TextPart({ text: JSON.stringify(part) })
            }
          })
          : [new TextPart({ text: message.content })],
        ...(metadata && { metadata })
      })

    case "assistant":
      return new AssistantMessage({
        role: "assistant",
        content: Array.isArray(message.content)
          ? message.content.map((part) => {
            switch (part.type) {
              case "text":
                return new TextPart({ text: part.text })
              case "reasoning":
                return new ReasoningPart({
                  reasoning: part.reasoning,
                  signature: part.signature,
                  isRedacted: part.isRedacted
                })
              case "tool-call":
                return new ToolCallPart({
                  toolCallId: part.toolCallId as ToolCallId,
                  toolName: part.toolName,
                  args: part.args
                })
              default:
                return new TextPart({ text: JSON.stringify(part) })
            }
          })
          : [new TextPart({ text: message.content })],
        ...(metadata && { metadata })
      })

    case "tool":
      return new ToolMessage({
        role: "tool",
        content: Array.isArray(message.content)
          ? message.content
            .filter((part) => part.type === "tool-result")
            .map((part) =>
              new ToolResultPart({
                toolCallId: part.toolCallId as ToolCallId,
                toolName: part.toolName,
                result: part.result,
                isError: part.isError
              })
            )
          : [],
        ...(metadata && { metadata })
      })
  }
}

