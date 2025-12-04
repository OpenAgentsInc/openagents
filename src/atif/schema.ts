/**
 * ATIF (Agent Trajectory Interchange Format) Schema v1.4
 *
 * Standardized JSON format for logging complete interaction histories of
 * autonomous LLM agents. Designed for interoperability with debugging,
 * visualization, SFT, and RL pipelines.
 *
 * @see https://harborframework.com/docs/trajectory-format
 */
import * as S from "effect/Schema";

// ============================================================================
// Version and Constants
// ============================================================================

export const ATIF_SCHEMA_VERSION = "ATIF-v1.4" as const;

// ============================================================================
// Core Types
// ============================================================================

/**
 * Source of a step - restricted per ATIF spec
 */
export const StepSource = S.Literal("user", "agent", "system");
export type StepSource = S.Schema.Type<typeof StepSource>;

/**
 * Agent metadata
 */
export const Agent = S.Struct({
  name: S.String,
  version: S.String,
  model_name: S.String,
  extra: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type Agent = S.Schema.Type<typeof Agent>;

/**
 * Tool call within a step
 */
export const ToolCall = S.Struct({
  tool_call_id: S.String,
  function_name: S.String,
  arguments: S.Unknown, // JSON object, can be empty {}
});
export type ToolCall = S.Schema.Type<typeof ToolCall>;

/**
 * Reference to a subagent trajectory (for hierarchical agent systems)
 */
export const SubagentTrajectoryRef = S.Struct({
  session_id: S.String,
  trajectory_path: S.optional(S.String),
  extra: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type SubagentTrajectoryRef = S.Schema.Type<typeof SubagentTrajectoryRef>;

/**
 * Observation result from a tool call or subagent
 */
export const ObservationResult = S.Struct({
  source_call_id: S.optional(S.String), // References ToolCall.tool_call_id
  content: S.optional(S.Unknown), // Textual output or structured result
  subagent_trajectory_ref: S.optional(S.Array(SubagentTrajectoryRef)),
});
export type ObservationResult = S.Schema.Type<typeof ObservationResult>;

/**
 * Observation containing results from tool calls
 */
export const Observation = S.Struct({
  results: S.Array(ObservationResult),
});
export type Observation = S.Schema.Type<typeof Observation>;

/**
 * Metrics for token usage and cost (per-step)
 *
 * Token accounting: prompt_tokens includes all input tokens;
 * cached_tokens is a subset counted within it, not separate.
 */
export const Metrics = S.Struct({
  prompt_tokens: S.optional(S.Number),
  completion_tokens: S.optional(S.Number),
  cached_tokens: S.optional(S.Number), // Subset of prompt_tokens from cache
  cost_usd: S.optional(S.Number),
  logprobs: S.optional(S.Array(S.Number)),
  completion_token_ids: S.optional(S.Array(S.Number)), // v1.3+
  prompt_token_ids: S.optional(S.Array(S.Number)), // v1.4+
  extra: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type Metrics = S.Schema.Type<typeof Metrics>;

/**
 * Final aggregated metrics for the trajectory
 */
export const FinalMetrics = S.Struct({
  total_prompt_tokens: S.Number,
  total_completion_tokens: S.Number,
  total_cached_tokens: S.optional(S.Number),
  total_cost_usd: S.optional(S.Number),
  total_steps: S.Number,
  extra: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type FinalMetrics = S.Schema.Type<typeof FinalMetrics>;

/**
 * A single step in the trajectory
 *
 * Validation rules:
 * - step_id must be sequential starting from 1
 * - timestamp must be ISO 8601 format
 * - source restricted to: user, agent, system
 * - model_name, reasoning_content only valid on agent steps
 */
export const Step = S.Struct({
  step_id: S.Number.pipe(S.int(), S.positive()),
  timestamp: S.String, // ISO 8601
  source: StepSource,
  message: S.Unknown, // Text content or structured message
  reasoning_content: S.optional(S.String), // Agent-only: internal reasoning/thinking
  model_name: S.optional(S.String), // Agent-only: LLM used for this step
  tool_calls: S.optional(S.Array(ToolCall)),
  observation: S.optional(Observation),
  metrics: S.optional(Metrics),
  extra: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type Step = S.Schema.Type<typeof Step>;

/**
 * Root trajectory object
 */
export const Trajectory = S.Struct({
  schema_version: S.Literal(ATIF_SCHEMA_VERSION),
  session_id: S.String,
  agent: Agent,
  steps: S.Array(Step),
  notes: S.optional(S.String), // Developer annotations
  final_metrics: S.optional(FinalMetrics),
  extra: S.optional(S.Record({ key: S.String, value: S.Unknown })),
});
export type Trajectory = S.Schema.Type<typeof Trajectory>;

// ============================================================================
// Decode/Encode Helpers
// ============================================================================

export const decodeTrajectory = S.decodeUnknownSync(Trajectory);
export const encodeTrajectory = S.encodeSync(Trajectory);

export const decodeStep = S.decodeUnknownSync(Step);
export const encodeStep = S.encodeSync(Step);

export const decodeAgent = S.decodeUnknownSync(Agent);
export const encodeAgent = S.encodeSync(Agent);

// ============================================================================
// Type Guards and Helpers
// ============================================================================

/**
 * Check if a step is from the agent
 */
export const isAgentStep = (step: Step): boolean => step.source === "agent";

/**
 * Check if a step is from the user
 */
export const isUserStep = (step: Step): boolean => step.source === "user";

/**
 * Check if a step is from the system
 */
export const isSystemStep = (step: Step): boolean => step.source === "system";

/**
 * Check if a step has tool calls
 */
export const hasToolCalls = (step: Step): boolean =>
  (step.tool_calls?.length ?? 0) > 0;

/**
 * Check if a step has observations
 */
export const hasObservation = (step: Step): boolean =>
  step.observation !== undefined && step.observation.results.length > 0;

/**
 * Check if a step has subagent references
 */
export const hasSubagentRefs = (step: Step): boolean => {
  if (!step.observation) return false;
  return step.observation.results.some(
    (r) => r.subagent_trajectory_ref && r.subagent_trajectory_ref.length > 0,
  );
};

/**
 * Extract all subagent session IDs from a trajectory
 */
export const extractSubagentSessionIds = (trajectory: Trajectory): string[] => {
  const ids: string[] = [];
  for (const step of trajectory.steps) {
    if (step.observation) {
      for (const result of step.observation.results) {
        if (result.subagent_trajectory_ref) {
          for (const ref of result.subagent_trajectory_ref) {
            ids.push(ref.session_id);
          }
        }
      }
    }
  }
  return ids;
};

/**
 * Get all tool call IDs from a trajectory
 */
export const extractToolCallIds = (trajectory: Trajectory): Set<string> => {
  const ids = new Set<string>();
  for (const step of trajectory.steps) {
    if (step.tool_calls) {
      for (const tc of step.tool_calls) {
        ids.add(tc.tool_call_id);
      }
    }
  }
  return ids;
};

/**
 * Get total token counts from a trajectory
 */
export const getTotalTokens = (trajectory: Trajectory): {
  prompt: number;
  completion: number;
  cached: number;
} => {
  if (trajectory.final_metrics) {
    return {
      prompt: trajectory.final_metrics.total_prompt_tokens,
      completion: trajectory.final_metrics.total_completion_tokens,
      cached: trajectory.final_metrics.total_cached_tokens ?? 0,
    };
  }

  // Calculate from steps if no final_metrics
  let prompt = 0;
  let completion = 0;
  let cached = 0;
  for (const step of trajectory.steps) {
    if (step.metrics) {
      prompt += step.metrics.prompt_tokens ?? 0;
      completion += step.metrics.completion_tokens ?? 0;
      cached += step.metrics.cached_tokens ?? 0;
    }
  }
  return { prompt, completion, cached };
};

/**
 * Extract text message from step.message (handles string or structured)
 */
export const extractStepText = (step: Step): string => {
  if (typeof step.message === "string") return step.message;
  if (typeof step.message === "object" && step.message !== null) {
    const msg = step.message as Record<string, unknown>;
    if (typeof msg.content === "string") return msg.content;
    if (typeof msg.text === "string") return msg.text;
  }
  return "";
};

// ============================================================================
// ID Generation Helpers
// ============================================================================

/**
 * Generate a unique session ID for a trajectory
 */
export const generateSessionId = (): string => {
  const now = new Date();
  const iso = now.toISOString().replace(/[:.]/g, "-");
  const rand = Math.random().toString(36).slice(2, 8);
  return `session-${iso}-${rand}`;
};

/**
 * Generate a unique tool call ID
 */
export const generateToolCallId = (): string => {
  return `tc-${crypto.randomUUID()}`;
};

/**
 * Get current timestamp in ISO 8601 format
 */
export const timestamp = (): string => new Date().toISOString();
