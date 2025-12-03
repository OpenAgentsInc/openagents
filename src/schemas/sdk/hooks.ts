/**
 * SDK-compatible hook schemas for Claude Agent SDK integration.
 *
 * Defines hook events, inputs, and outputs matching the Claude Agent SDK hook system.
 * These schemas enable type-safe hook implementations for observability and control flow.
 *
 * @module
 *
 * @example
 * ```typescript
 * import { PostToolUseHookInput, HookJSONOutput } from "./schemas/sdk/hooks";
 * import * as S from "effect/Schema";
 *
 * // Validate hook input
 * const input = S.decodeUnknownSync(PostToolUseHookInput)({
 *   hook_event_name: "PostToolUse",
 *   session_id: "sess-123",
 *   transcript_path: "/path/to/transcript",
 *   cwd: "/workspace",
 *   tool_name: "Edit",
 *   tool_input: { file_path: "test.ts", old_string: "old", new_string: "new" },
 *   tool_response: { success: true },
 * });
 * ```
 */

import * as S from "effect/Schema";

// =============================================================================
// Hook Event Types
// =============================================================================

/**
 * Available hook events in Claude Agent SDK.
 */
export const HookEvent = S.Literal(
  "PreToolUse",
  "PostToolUse",
  "Notification",
  "UserPromptSubmit",
  "SessionStart",
  "SessionEnd",
  "Stop",
  "SubagentStop",
  "PreCompact"
);

export type HookEvent = S.Schema.Type<typeof HookEvent>;

// =============================================================================
// Base Hook Input
// =============================================================================

/**
 * Base interface that all hook input types extend.
 */
export class BaseHookInput extends S.Class<BaseHookInput>("BaseHookInput")({
  session_id: S.String,
  transcript_path: S.String,
  cwd: S.String,
  permission_mode: S.optional(S.String),
}) {}

// =============================================================================
// Specific Hook Input Types
// =============================================================================

/**
 * PreToolUse hook input - fired before a tool is executed.
 */
export class PreToolUseHookInput extends BaseHookInput.extend<PreToolUseHookInput>("PreToolUseHookInput")({
  hook_event_name: S.Literal("PreToolUse"),
  tool_name: S.String,
  tool_input: S.Record({ key: S.String, value: S.Unknown }),
}) {}

/**
 * PostToolUse hook input - fired after a tool is executed.
 */
export class PostToolUseHookInput extends BaseHookInput.extend<PostToolUseHookInput>("PostToolUseHookInput")({
  hook_event_name: S.Literal("PostToolUse"),
  tool_name: S.String,
  tool_input: S.Record({ key: S.String, value: S.Unknown }),
  tool_response: S.Unknown,
}) {}

/**
 * Notification hook input - fired when a notification is displayed.
 */
export class NotificationHookInput extends BaseHookInput.extend<NotificationHookInput>("NotificationHookInput")({
  hook_event_name: S.Literal("Notification"),
  message: S.String,
  title: S.optional(S.String),
}) {}

/**
 * UserPromptSubmit hook input - fired when user submits a prompt.
 */
export class UserPromptSubmitHookInput extends BaseHookInput.extend<UserPromptSubmitHookInput>(
  "UserPromptSubmitHookInput"
)({
  hook_event_name: S.Literal("UserPromptSubmit"),
  prompt: S.String,
}) {}

/**
 * SessionStart hook input - fired when a session starts.
 */
export class SessionStartHookInput extends BaseHookInput.extend<SessionStartHookInput>("SessionStartHookInput")({
  hook_event_name: S.Literal("SessionStart"),
  source: S.Literal("startup", "resume", "clear", "compact"),
}) {}

/**
 * SessionEnd hook input - fired when a session ends.
 */
export class SessionEndHookInput extends BaseHookInput.extend<SessionEndHookInput>("SessionEndHookInput")({
  hook_event_name: S.Literal("SessionEnd"),
  reason: S.Literal("clear", "logout", "prompt_input_exit", "other"),
}) {}

/**
 * Stop hook input - fired when a stop is requested.
 */
export class StopHookInput extends BaseHookInput.extend<StopHookInput>("StopHookInput")({
  hook_event_name: S.Literal("Stop"),
  stop_hook_active: S.Boolean,
}) {}

/**
 * SubagentStop hook input - fired when a subagent stop is requested.
 */
export class SubagentStopHookInput extends BaseHookInput.extend<SubagentStopHookInput>("SubagentStopHookInput")({
  hook_event_name: S.Literal("SubagentStop"),
  stop_hook_active: S.Boolean,
}) {}

/**
 * PreCompact hook input - fired before context compaction.
 */
export class PreCompactHookInput extends BaseHookInput.extend<PreCompactHookInput>("PreCompactHookInput")({
  hook_event_name: S.Literal("PreCompact"),
  trigger: S.Literal("manual", "auto"),
  custom_instructions: S.NullOr(S.String),
}) {}

/**
 * Union of all hook input types.
 */
export const HookInput = S.Union(
  PreToolUseHookInput,
  PostToolUseHookInput,
  NotificationHookInput,
  UserPromptSubmitHookInput,
  SessionStartHookInput,
  SessionEndHookInput,
  StopHookInput,
  SubagentStopHookInput,
  PreCompactHookInput
);

export type HookInput = S.Schema.Type<typeof HookInput>;

// =============================================================================
// Hook Output Types
// =============================================================================

/**
 * Async hook output - indicates hook is running asynchronously.
 */
export class AsyncHookJSONOutput extends S.Class<AsyncHookJSONOutput>("AsyncHookJSONOutput")({
  async: S.Literal(true),
  asyncTimeout: S.optional(S.Number),
}) {}

/**
 * PreToolUse-specific hook output fields.
 */
export class PreToolUseHookSpecificOutput extends S.Class<PreToolUseHookSpecificOutput>("PreToolUseHookSpecificOutput")(
  {
    hookEventName: S.Literal("PreToolUse"),
    permissionDecision: S.optional(S.Literal("allow", "deny", "ask")),
    permissionDecisionReason: S.optional(S.String),
  }
) {}

/**
 * UserPromptSubmit-specific hook output fields.
 */
export class UserPromptSubmitHookSpecificOutput extends S.Class<UserPromptSubmitHookSpecificOutput>(
  "UserPromptSubmitHookSpecificOutput"
)({
  hookEventName: S.Literal("UserPromptSubmit"),
  additionalContext: S.optional(S.String),
}) {}

/**
 * SessionStart-specific hook output fields.
 */
export class SessionStartHookSpecificOutput extends S.Class<SessionStartHookSpecificOutput>(
  "SessionStartHookSpecificOutput"
)({
  hookEventName: S.Literal("SessionStart"),
  additionalContext: S.optional(S.String),
}) {}

/**
 * Sync hook output - hook completed synchronously with optional control fields.
 */
export class SyncHookJSONOutput extends S.Class<SyncHookJSONOutput>("SyncHookJSONOutput")({
  continue: S.optional(S.Boolean),
  suppressOutput: S.optional(S.Boolean),
  stopReason: S.optional(S.String),
  decision: S.optional(S.Literal("approve", "block")),
  systemMessage: S.optional(S.String),
  reason: S.optional(S.String),
  hookSpecificOutput: S.optional(
    S.Union(PreToolUseHookSpecificOutput, UserPromptSubmitHookSpecificOutput, SessionStartHookSpecificOutput)
  ),
}) {}

/**
 * Hook output - union of async and sync outputs.
 */
export const HookJSONOutput = S.Union(AsyncHookJSONOutput, SyncHookJSONOutput);

export type HookJSONOutput = S.Schema.Type<typeof HookJSONOutput>;

// =============================================================================
// Hook Callback Types
// =============================================================================

/**
 * Hook callback matcher configuration.
 */
export class HookCallbackMatcher extends S.Class<HookCallbackMatcher>("HookCallbackMatcher")({
  matcher: S.optional(S.String),
  /** Timeout in seconds for all hooks in this matcher (default: 60) */
  timeout: S.optional(S.Number),
  // Note: hooks array of HookCallback functions cannot be represented in Effect Schema
  // as it requires runtime function types. Use TypeScript types for this field.
}) {}

// =============================================================================
// Type Guards
// =============================================================================

/**
 * Type guard for PreToolUseHookInput.
 */
export const isPreToolUseHookInput = (input: HookInput): input is S.Schema.Type<typeof PreToolUseHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "PreToolUse";

/**
 * Type guard for PostToolUseHookInput.
 */
export const isPostToolUseHookInput = (input: HookInput): input is S.Schema.Type<typeof PostToolUseHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "PostToolUse";

/**
 * Type guard for SessionStartHookInput.
 */
export const isSessionStartHookInput = (input: HookInput): input is S.Schema.Type<typeof SessionStartHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "SessionStart";

/**
 * Type guard for SessionEndHookInput.
 */
export const isSessionEndHookInput = (input: HookInput): input is S.Schema.Type<typeof SessionEndHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "SessionEnd";

/**
 * Type guard for NotificationHookInput.
 */
export const isNotificationHookInput = (input: HookInput): input is S.Schema.Type<typeof NotificationHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "Notification";

/**
 * Type guard for UserPromptSubmitHookInput.
 */
export const isUserPromptSubmitHookInput = (
  input: HookInput
): input is S.Schema.Type<typeof UserPromptSubmitHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "UserPromptSubmit";

/**
 * Type guard for StopHookInput.
 */
export const isStopHookInput = (input: HookInput): input is S.Schema.Type<typeof StopHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "Stop";

/**
 * Type guard for SubagentStopHookInput.
 */
export const isSubagentStopHookInput = (input: HookInput): input is S.Schema.Type<typeof SubagentStopHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "SubagentStop";

/**
 * Type guard for PreCompactHookInput.
 */
export const isPreCompactHookInput = (input: HookInput): input is S.Schema.Type<typeof PreCompactHookInput> =>
  "hook_event_name" in input && input.hook_event_name === "PreCompact";
