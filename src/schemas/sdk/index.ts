/**
 * SDK-compatible Effect schemas for OpenAgents.
 *
 * This module provides type definitions that match Claude Agent SDK conventions,
 * enabling seamless interoperability whether using Claude Code or the minimal subagent.
 *
 * @module
 *
 * @example
 * ```typescript
 * import { FileReadInput, FileEditInput, SDKMessage } from "./schemas/sdk";
 * import * as S from "effect/Schema";
 *
 * // Validate input
 * const parsed = S.decodeUnknownSync(FileEditInput)({
 *   file_path: "/path/to/file.ts",
 *   old_string: "const x = 1",
 *   new_string: "const x = 2",
 * });
 * ```
 */

// =============================================================================
// Tool Input Schemas
// =============================================================================

export {
  // File operations
  FileReadInput,
  FileEditInput,
  FileWriteInput,
  // Shell operations
  BashInput,
  BashOutputInput,
  KillShellInput,
  // Search operations
  GrepInput,
  GrepOutputMode,
  GlobInput,
  // Web operations
  WebFetchInput,
  WebSearchInput,
  // Task operations
  TodoWriteInput,
  TodoItem,
  TodoStatus,
  TaskInput,
  SubagentType,
  ModelSelection,
  // Notebook operations
  NotebookEditInput,
  NotebookCellType,
  NotebookEditMode,
  // User interaction
  AskUserQuestionInput,
  Question,
  QuestionOption,
  // Skill & command operations
  SkillInput,
  SlashCommandInput,
  // Export map
  ToolInputSchemas,
} from "./tool-inputs.js";

export type {
  FileReadInput as FileReadInputType,
  FileEditInput as FileEditInputType,
  FileWriteInput as FileWriteInputType,
  BashInput as BashInputType,
  BashOutputInput as BashOutputInputType,
  KillShellInput as KillShellInputType,
  GrepInput as GrepInputType,
  GrepOutputMode as GrepOutputModeType,
  GlobInput as GlobInputType,
  WebFetchInput as WebFetchInputType,
  WebSearchInput as WebSearchInputType,
  TodoWriteInput as TodoWriteInputType,
  TodoItem as TodoItemType,
  TodoStatus as TodoStatusType,
  TaskInput as TaskInputType,
  SubagentType as SubagentTypeValue,
  ModelSelection as ModelSelectionValue,
  NotebookEditInput as NotebookEditInputType,
  NotebookCellType as NotebookCellTypeValue,
  NotebookEditMode as NotebookEditModeValue,
  AskUserQuestionInput as AskUserQuestionInputType,
  Question as QuestionType,
  QuestionOption as QuestionOptionType,
  SkillInput as SkillInputType,
  SlashCommandInput as SlashCommandInputType,
} from "./tool-inputs.js";

// =============================================================================
// Tool Output Schemas
// =============================================================================

export {
  // Content types
  TextContent,
  ImageContent,
  ToolContent,
  isTextContent,
  isImageContent,
  // File operation outputs
  ReadOutput,
  EditOutput,
  WriteOutput,
  // Shell operation outputs
  BashOutput,
  BashOutputResult,
  KillShellOutput,
  // Search operation outputs
  GrepMatch,
  GrepContentOutput,
  GrepFilesOutput,
  GrepCountOutput,
  GrepOutput,
  GlobOutput,
  // Web operation outputs
  WebFetchOutput,
  SearchResult,
  WebSearchOutput,
  // Task operation outputs
  TodoWriteOutput,
  TaskOutput,
  // Notebook operation outputs
  NotebookEditOutput,
  // User interaction outputs
  AskUserQuestionOutput,
  // Generic tool result
  ToolResult,
  BaseToolResult,
  // Error types
  ToolErrorReason,
  ToolError,
  // Export map
  ToolOutputSchemas,
} from "./tool-outputs.js";

export type {
  TextContent as TextContentType,
  ImageContent as ImageContentType,
  ToolContent as ToolContentType,
  ReadOutput as ReadOutputType,
  EditOutput as EditOutputType,
  WriteOutput as WriteOutputType,
  BashOutput as BashOutputType,
  BashOutputResult as BashOutputResultType,
  KillShellOutput as KillShellOutputType,
  GrepMatch as GrepMatchType,
  GrepContentOutput as GrepContentOutputType,
  GrepFilesOutput as GrepFilesOutputType,
  GrepCountOutput as GrepCountOutputType,
  GrepOutput as GrepOutputType,
  GlobOutput as GlobOutputType,
  WebFetchOutput as WebFetchOutputType,
  SearchResult as SearchResultType,
  WebSearchOutput as WebSearchOutputType,
  TodoWriteOutput as TodoWriteOutputType,
  TaskOutput as TaskOutputType,
  NotebookEditOutput as NotebookEditOutputType,
  AskUserQuestionOutput as AskUserQuestionOutputType,
  BaseToolResult as BaseToolResultType,
  ToolErrorReason as ToolErrorReasonValue,
  ToolError as ToolErrorType,
} from "./tool-outputs.js";

// =============================================================================
// Message Schemas
// =============================================================================

export {
  // Content blocks
  TextBlock,
  ThinkingBlock,
  RedactedThinkingBlock,
  ToolUseBlock,
  ToolResultBlock,
  ImageBlock,
  ContentBlock,
  // Content block type guards
  isTextBlock,
  isThinkingBlock,
  isRedactedThinkingBlock,
  isToolUseBlock,
  isToolResultBlock,
  isImageBlock,
  // Message types
  SDKUserMessage,
  SDKAssistantMessage,
  SDKResultMessage,
  SDKSystemMessage,
  SDKMessage,
  ResultSubtype,
  SystemSubtype,
  // Message type guards
  isSDKUserMessage,
  isSDKAssistantMessage,
  isSDKResultMessage,
  isSDKSystemMessage,
  // Chat message
  ChatMessage,
  MessageRole,
  // Streaming types
  TextDelta,
  ThinkingDelta,
  InputJsonDelta,
  ContentDelta,
  StreamEventType,
  ContentBlockStartEvent,
  ContentBlockDeltaEvent,
  ContentBlockStopEvent,
  // Conversation types
  Conversation,
  ConversationTurn,
} from "./messages.js";

export type {
  TextBlock as TextBlockType,
  ThinkingBlock as ThinkingBlockType,
  RedactedThinkingBlock as RedactedThinkingBlockType,
  ToolUseBlock as ToolUseBlockType,
  ToolResultBlock as ToolResultBlockType,
  ImageBlock as ImageBlockType,
  ContentBlock as ContentBlockType,
  SDKUserMessage as SDKUserMessageType,
  SDKAssistantMessage as SDKAssistantMessageType,
  SDKResultMessage as SDKResultMessageType,
  SDKSystemMessage as SDKSystemMessageType,
  SDKMessage as SDKMessageType,
  ResultSubtype as ResultSubtypeValue,
  SystemSubtype as SystemSubtypeValue,
  ChatMessage as ChatMessageType,
  MessageRole as MessageRoleValue,
  TextDelta as TextDeltaType,
  ThinkingDelta as ThinkingDeltaType,
  InputJsonDelta as InputJsonDeltaType,
  ContentDelta as ContentDeltaType,
  StreamEventType as StreamEventTypeValue,
  ContentBlockStartEvent as ContentBlockStartEventType,
  ContentBlockDeltaEvent as ContentBlockDeltaEventType,
  ContentBlockStopEvent as ContentBlockStopEventType,
  Conversation as ConversationType,
  ConversationTurn as ConversationTurnType,
} from "./messages.js";

// =============================================================================
// Adapters
// =============================================================================

export {
  // Effect Schema to Zod conversion
  effectSchemaToZod,
  effectSchemaToJsonSchema,
  SchemaConversionError,
  // Tool adaptation
  effectToolToMcp,
  effectToolsToMcp,
  sdkTool,
  ToolRegistry,
  toolContentToMcpContent,
  effectResultToMcpResult,
  effectErrorToMcpResult,
  // Message adaptation
  internalContentToSdk,
  internalContentsToSdk,
  sdkContentToInternal,
  sdkContentsToInternal,
  toolCallToToolUseBlock,
  toolUseBlockToToolCall,
  extractToolCalls,
  internalMessageToSdkChat,
  internalToSdkUserMessage,
  internalToSdkAssistantMessage,
  sdkChatToInternalMessage,
  sdkUserToInternalMessage,
  sdkAssistantToInternalMessage,
  createToolResultBlock,
  createInternalToolResult,
  internalMessagesToSdkChat,
  sdkChatToInternalMessages,
} from "./adapters/index.js";

export type {
  McpContentItem,
  McpCallToolResult,
  McpToolDefinition,
  EffectTool,
  EffectToolResult,
  EffectToolExecutionError,
  ToolAdapterOptions,
  MessageConversionResult,
  MessageAdapterOptions,
} from "./adapters/index.js";

// =============================================================================
// Re-exports for convenience
// =============================================================================

/**
 * Version of the SDK schema module.
 * Matches the Claude Agent SDK version these schemas are compatible with.
 */
export const SDK_SCHEMA_VERSION = "1.0.0";
