/**
 * Session Schema - Claude Code-compatible session format
 *
 * Stores full conversation history for replayability, matching Claude Code's
 * `~/.claude/projects/<project-path>/<session-id>.jsonl` format.
 *
 * Each line is a complete JSON object with type, message, uuid, parentUuid, timestamp, sessionId.
 * Messages include full content (text + tool_use with ALL params).
 * Usage metrics with token counts and cache info.
 * Parent UUIDs for threading/conversation structure.
 */
import * as S from "effect/Schema";

// Usage metrics for tracking costs
export const UsageMetrics = S.Struct({
  inputTokens: S.optional(S.Number),
  outputTokens: S.optional(S.Number),
  cacheReadInputTokens: S.optional(S.Number),
  cacheCreationInputTokens: S.optional(S.Number),
  totalCostUsd: S.optional(S.Number),
});
export type UsageMetrics = S.Schema.Type<typeof UsageMetrics>;

// Tool use content block (full input preserved)
export const ToolUseBlock = S.Struct({
  type: S.Literal("tool_use"),
  id: S.String,
  name: S.String,
  input: S.Unknown, // Preserve full input object
});
export type ToolUseBlock = S.Schema.Type<typeof ToolUseBlock>;

// Tool result content block
export const ToolResultBlock = S.Struct({
  type: S.Literal("tool_result"),
  tool_use_id: S.String,
  content: S.Unknown, // Can be string or array of content blocks
  is_error: S.optional(S.Boolean),
});
export type ToolResultBlock = S.Schema.Type<typeof ToolResultBlock>;

// Text content block
export const TextBlock = S.Struct({
  type: S.Literal("text"),
  text: S.String,
});
export type TextBlock = S.Schema.Type<typeof TextBlock>;

// Content can be text, tool_use, or tool_result
export const ContentBlock = S.Union(TextBlock, ToolUseBlock, ToolResultBlock, S.Unknown);
export type ContentBlock = S.Schema.Type<typeof ContentBlock>;

// Message content - can be string or array of content blocks
export const MessageContent = S.Union(S.String, S.Array(ContentBlock));
export type MessageContent = S.Schema.Type<typeof MessageContent>;

// Base session entry fields (all entries have these)
const BaseEntry = S.Struct({
  uuid: S.String,
  timestamp: S.String,
  sessionId: S.String,
  parentUuid: S.NullOr(S.String),
});

// Session start entry - written when session begins
export const SessionStartEntry = S.Struct({
  ...BaseEntry.fields,
  type: S.Literal("session_start"),
  taskId: S.optional(S.String),
  cwd: S.String,
  model: S.optional(S.String),
  provider: S.optional(S.String),
  version: S.optional(S.String),
  gitBranch: S.optional(S.String),
});
export type SessionStartEntry = S.Schema.Type<typeof SessionStartEntry>;

// User message entry
export const UserMessageEntry = S.Struct({
  ...BaseEntry.fields,
  type: S.Literal("user"),
  message: S.Struct({
    role: S.Literal("user"),
    content: MessageContent,
  }),
  userType: S.optional(S.String), // "external", "tool_result", etc.
  cwd: S.optional(S.String),
});
export type UserMessageEntry = S.Schema.Type<typeof UserMessageEntry>;

// Assistant message entry (includes full tool calls)
export const AssistantMessageEntry = S.Struct({
  ...BaseEntry.fields,
  type: S.Literal("assistant"),
  message: S.Struct({
    model: S.optional(S.String),
    id: S.optional(S.String),
    role: S.Literal("assistant"),
    content: MessageContent,
    stop_reason: S.optional(S.NullOr(S.String)),
  }),
  usage: S.optional(UsageMetrics),
  requestId: S.optional(S.String),
});
export type AssistantMessageEntry = S.Schema.Type<typeof AssistantMessageEntry>;

// Tool result entry (user role with tool_result content)
export const ToolResultEntry = S.Struct({
  ...BaseEntry.fields,
  type: S.Literal("tool_result"),
  message: S.Struct({
    role: S.Literal("user"),
    content: S.Array(ToolResultBlock),
  }),
  toolUseResult: S.optional(S.Unknown), // Optional parsed result for convenience
});
export type ToolResultEntry = S.Schema.Type<typeof ToolResultEntry>;

// Session end entry - written when session completes
export const SessionEndEntry = S.Struct({
  ...BaseEntry.fields,
  type: S.Literal("session_end"),
  outcome: S.Literal("success", "failure", "blocked", "cancelled"),
  reason: S.optional(S.String),
  totalTurns: S.Number,
  totalUsage: S.optional(UsageMetrics),
  filesModified: S.optional(S.Array(S.String)),
  commits: S.optional(S.Array(S.String)),
});
export type SessionEndEntry = S.Schema.Type<typeof SessionEndEntry>;

// Union of all session entry types
export const SessionEntry = S.Union(
  SessionStartEntry,
  UserMessageEntry,
  AssistantMessageEntry,
  ToolResultEntry,
  SessionEndEntry,
);
export type SessionEntry = S.Schema.Type<typeof SessionEntry>;

// Session metadata for quick lookup (stored separately or computed)
export const SessionMetadata = S.Struct({
  sessionId: S.String,
  taskId: S.optional(S.String),
  startedAt: S.String,
  endedAt: S.optional(S.String),
  outcome: S.optional(S.Literal("success", "failure", "blocked", "cancelled")),
  totalTurns: S.Number,
  totalUsage: S.optional(UsageMetrics),
  filesModified: S.optional(S.Array(S.String)),
  commits: S.optional(S.Array(S.String)),
  model: S.optional(S.String),
  cwd: S.String,
  firstUserMessage: S.optional(S.String),
});
export type SessionMetadata = S.Schema.Type<typeof SessionMetadata>;

// Decode helpers
export const decodeSessionEntry = S.decodeUnknownSync(SessionEntry);
export const decodeSessionMetadata = S.decodeUnknownSync(SessionMetadata);

// Helper to generate a UUID
export const generateUuid = (): string => {
  return crypto.randomUUID();
};

// Helper to generate a session ID
export const generateSessionId = (): string => {
  const now = new Date();
  const iso = now.toISOString().replace(/[:.]/g, "-");
  const rand = Math.random().toString(36).slice(2, 8);
  return `session-${iso}-${rand}`;
};

// Helper to get current timestamp
export const timestamp = (): string => new Date().toISOString();

// Helper to extract text from message content
export const extractText = (content: MessageContent): string => {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .filter((block): block is TextBlock => typeof block === "object" && block !== null && "type" in block && block.type === "text")
      .map((block) => block.text)
      .join("\n");
  }
  return "";
};

// Helper to count tool uses in content
export const countToolUses = (content: MessageContent): number => {
  if (typeof content === "string") return 0;
  if (Array.isArray(content)) {
    return content.filter(
      (block): block is ToolUseBlock =>
        typeof block === "object" && block !== null && "type" in block && block.type === "tool_use",
    ).length;
  }
  return 0;
};
