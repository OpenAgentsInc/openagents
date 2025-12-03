/**
 * SDK-compatible agent schemas for Claude Agent SDK integration.
 *
 * Defines agent definitions, subagent configurations, and result types
 * matching the Claude Agent SDK patterns for orchestration and composition.
 *
 * @module
 *
 * @example
 * ```typescript
 * import { AgentDefinition, SubagentConfig } from "./schemas/sdk/agents";
 * import * as S from "effect/Schema";
 *
 * // Define a specialized subagent
 * const reviewer: S.Schema.Type<typeof AgentDefinition> = {
 *   description: "Expert code reviewer",
 *   prompt: "You are a code review specialist...",
 *   tools: ["Read", "Grep", "Glob"],
 *   model: "sonnet",
 * };
 * ```
 */

import * as S from "effect/Schema";
import { PermissionMode } from "./permissions.js";

// =============================================================================
// Model Selection
// =============================================================================

/**
 * Available Claude model tiers for agent execution.
 */
export const AgentModel = S.Literal("sonnet", "opus", "haiku", "inherit");

export type AgentModel = S.Schema.Type<typeof AgentModel>;

// =============================================================================
// Agent Definition
// =============================================================================

/**
 * Programmatic agent definition matching Claude Agent SDK.
 *
 * Defines a specialized subagent with custom instructions, tool restrictions,
 * and model selection. Used in the `agents` parameter of query().
 */
export class AgentDefinition extends S.Class<AgentDefinition>("AgentDefinition")({
  /** Natural language description of when to use this agent */
  description: S.String,
  /** The agent's system prompt defining its role and behavior */
  prompt: S.String,
  /** Array of allowed tool names. If omitted, inherits all tools from parent */
  tools: S.optional(S.Array(S.String)),
  /** Model override for this agent. Defaults to main model if omitted */
  model: S.optional(AgentModel),
}) {}

// =============================================================================
// Subagent Configuration
// =============================================================================

/**
 * Configuration for invoking a subagent to complete a subtask.
 *
 * Combines SDK agent definition patterns with orchestrator requirements.
 */
export class SubagentConfig extends S.Class<SubagentConfig>("SubagentConfig")({
  /** Task or subtask description */
  description: S.String,
  /** Working directory for the subagent */
  cwd: S.String,
  /** Available tools (names of tools the subagent can use) */
  tools: S.Array(S.String),
  /** Model to use for the subagent */
  model: S.optional(S.String),
  /** Permission mode for the subagent */
  permissionMode: S.optional(PermissionMode),
  /** Max turns before giving up */
  maxTurns: S.optional(S.Number),
  /** Session ID to resume from (for long-running tasks) */
  resumeSessionId: S.optional(S.String),
  /** Whether to fork the session instead of continuing */
  forkSession: S.optional(S.Boolean),
  /** Custom system prompt append */
  systemPromptAppend: S.optional(S.String),
}) {}

// =============================================================================
// Session Resume Support
// =============================================================================

/**
 * Session resume strategy for multi-run subagents.
 */
export const ResumeStrategy = S.Literal("continue", "fork");

export type ResumeStrategy = S.Schema.Type<typeof ResumeStrategy>;

/**
 * Session metadata for resuming long-running subagent work.
 */
export class SessionMetadata extends S.Class<SessionMetadata>("SessionMetadata")({
  /** Active session ID */
  sessionId: S.optional(S.String),
  /** Session ID this was forked from (if branched) */
  forkedFromSessionId: S.optional(S.String),
  /** Resume strategy for next invocation */
  resumeStrategy: S.optional(ResumeStrategy),
  /** Tools used during session with counts */
  toolsUsed: S.optional(S.Record({ key: S.String, value: S.Number })),
  /** Blockers or errors encountered */
  blockers: S.optional(S.Array(S.String)),
  /** Suggested next steps from agent */
  suggestedNextSteps: S.optional(S.Array(S.String)),
  /** Final assistant message or summary */
  summary: S.optional(S.String),
  /** Token usage from Claude API */
  usage: S.optional(
    S.Struct({
      inputTokens: S.optional(S.Number),
      outputTokens: S.optional(S.Number),
      cacheReadInputTokens: S.optional(S.Number),
      cacheCreationInputTokens: S.optional(S.Number),
    })
  ),
  /** Total cost in USD from Claude API */
  totalCostUsd: S.optional(S.Number),
}) {}

// =============================================================================
// Subagent Result
// =============================================================================

/**
 * Result of a subagent execution.
 *
 * Captures success/failure, file modifications, session metadata,
 * and metrics for orchestrator decision-making.
 */
export class SubagentResult extends S.Class<SubagentResult>("SubagentResult")({
  /** Whether the subagent completed successfully */
  success: S.Boolean,
  /** ID of the subtask that was completed */
  subtaskId: S.String,
  /** Files modified during execution */
  filesModified: S.Array(S.String),
  /** Error message if failed */
  error: S.optional(S.String),
  /** Number of turns consumed */
  turns: S.Number,
  /** Which agent implementation was used */
  agent: S.optional(S.Literal("claude-code", "minimal")),
  /** Session ID for resumption (Claude Code only) */
  claudeCodeSessionId: S.optional(S.String),
  /** Forked from session ID (if branched) */
  claudeCodeForkedFromSessionId: S.optional(S.String),
  /** Token usage metrics */
  tokenUsage: S.optional(
    S.Struct({
      input: S.Number,
      output: S.Number,
    })
  ),
  /** Verification outputs (test results, linter output, etc.) */
  verificationOutputs: S.optional(S.Array(S.String)),
  /** Session metadata for progress bridging */
  sessionMetadata: S.optional(SessionMetadata),
}) {}

// =============================================================================
// Agent Registry
// =============================================================================

/**
 * Registry of available agents by name.
 *
 * Maps agent names to their definitions for programmatic agent selection.
 */
export class AgentRegistry extends S.Class<AgentRegistry>("AgentRegistry")({
  agents: S.Record({ key: S.String, value: AgentDefinition }),
}) {}

// =============================================================================
// Type Guards
// =============================================================================

/**
 * Type guard for AgentDefinition with tools restriction.
 */
export const hasToolRestriction = (def: S.Schema.Type<typeof AgentDefinition>): def is S.Schema.Type<
  typeof AgentDefinition
> & {
  tools: string[];
} => def.tools !== undefined && def.tools.length > 0;

/**
 * Type guard for AgentDefinition with model override.
 */
export const hasModelOverride = (def: S.Schema.Type<typeof AgentDefinition>): def is S.Schema.Type<
  typeof AgentDefinition
> & {
  model: S.Schema.Type<typeof AgentModel>;
} => def.model !== undefined;

/**
 * Type guard for SubagentResult indicating success.
 */
export const isSuccessfulResult = (
  result: S.Schema.Type<typeof SubagentResult>
): result is S.Schema.Type<typeof SubagentResult> & { success: true } => result.success === true;

/**
 * Type guard for SubagentResult indicating failure.
 */
export const isFailedResult = (
  result: S.Schema.Type<typeof SubagentResult>
): result is S.Schema.Type<typeof SubagentResult> & { success: false; error: string } =>
  result.success === false;

/**
 * Type guard for SubagentResult with Claude Code session.
 */
export const hasClaudeCodeSession = (
  result: S.Schema.Type<typeof SubagentResult>
): result is S.Schema.Type<typeof SubagentResult> & { claudeCodeSessionId: string } =>
  result.claudeCodeSessionId !== undefined;
