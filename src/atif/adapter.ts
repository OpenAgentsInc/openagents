/**
 * ATIF Event Adapters
 *
 * Converts existing openagents event types to ATIF format:
 * - LoopEvent -> Steps
 * - OrchestratorEvent -> Steps
 * - SessionEntry -> Steps
 * - SubagentResult -> Observation with subagent_trajectory_ref
 */
import type {
  OrchestratorEvent,
  SubagentResult,
} from "../agent/orchestrator/types.js";
import type {
  SessionEntry,
  AssistantMessageEntry,
  UserMessageEntry,
  ToolResultEntry,
} from "../sessions/schema.js";
import type {
  Agent,
  Metrics,
  ObservationResult,
  Step,
  ToolCall,
  Trajectory,
} from "./schema.js";
import {
  ATIF_SCHEMA_VERSION,
  timestamp,
  generateSessionId,
} from "./schema.js";

// ============================================================================
// Agent Factories
// ============================================================================

/**
 * Create an Agent for MechaCoder orchestrator
 */
export const createMechaCoderAgent = (
  modelName: string,
  version = "1.0.0",
): Agent => ({
  name: "mechacoder-orchestrator",
  version,
  model_name: modelName,
  extra: { type: "orchestrator" },
});

/**
 * Create an Agent for Claude Code subagent
 */
export const createClaudeCodeAgent = (
  modelName: string,
  version = "1.0.0",
): Agent => ({
  name: "claude-code",
  version,
  model_name: modelName,
  extra: { type: "subagent", provider: "anthropic" },
});

/**
 * Create an Agent for minimal subagent
 */
export const createMinimalSubagent = (
  modelName: string,
  version = "1.0.0",
): Agent => ({
  name: "minimal-subagent",
  version,
  model_name: modelName,
  extra: { type: "subagent" },
});

/**
 * Create a generic Agent
 */
export const createAgent = (
  name: string,
  modelName: string,
  version = "1.0.0",
  extra?: Record<string, unknown>,
): Agent => ({
  name,
  version,
  model_name: modelName,
  extra,
});

// ============================================================================
// Session Entry Adapters
// ============================================================================

/**
 * Convert a user message entry to an ATIF step
 */
export const userMessageEntryToStep = (
  entry: UserMessageEntry,
  stepId: number,
): Step => ({
  step_id: stepId,
  timestamp: entry.timestamp,
  source: "user",
  message: entry.message.content,
  extra: entry.userType ? { user_type: entry.userType } : undefined,
});

/**
 * Convert an assistant message entry to an ATIF step
 */
export const assistantMessageEntryToStep = (
  entry: AssistantMessageEntry,
  stepId: number,
): Step => {
  const content = entry.message.content;

  // Extract tool calls from content if present
  const toolCalls: ToolCall[] = [];
  if (Array.isArray(content)) {
    for (const block of content) {
      if (
        typeof block === "object" &&
        block !== null &&
        "type" in block &&
        block.type === "tool_use"
      ) {
        const toolUse = block as {
          type: "tool_use";
          id: string;
          name: string;
          input: unknown;
        };
        toolCalls.push({
          tool_call_id: toolUse.id,
          function_name: toolUse.name,
          arguments: toolUse.input,
        });
      }
    }
  }

  // Extract text content
  let textContent: string;
  if (typeof content === "string") {
    textContent = content;
  } else if (Array.isArray(content)) {
    textContent = content
      .filter(
        (block): block is { type: "text"; text: string } =>
          typeof block === "object" &&
          block !== null &&
          "type" in block &&
          block.type === "text",
      )
      .map((block) => block.text)
      .join("\n");
  } else {
    textContent = "";
  }

  // Convert usage to metrics
  const metrics: Metrics | undefined = entry.usage
    ? {
        prompt_tokens: entry.usage.inputTokens,
        completion_tokens: entry.usage.outputTokens,
        cached_tokens: entry.usage.cacheReadInputTokens,
        cost_usd: entry.usage.totalCostUsd,
      }
    : undefined;

  return {
    step_id: stepId,
    timestamp: entry.timestamp,
    source: "agent",
    message: textContent,
    model_name: entry.message.model,
    tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
    metrics,
    extra: entry.requestId ? { request_id: entry.requestId } : undefined,
  };
};

/**
 * Convert a tool result entry to an ATIF step (observation)
 */
export const toolResultEntryToStep = (
  entry: ToolResultEntry,
  stepId: number,
): Step => {
  const results: ObservationResult[] = entry.message.content.map((block) => ({
    source_call_id: block.tool_use_id,
    content: block.content,
  }));

  return {
    step_id: stepId,
    timestamp: entry.timestamp,
    source: "system",
    message: "Tool execution results",
    observation: { results },
  };
};

/**
 * Convert session entries to ATIF steps
 */
export const sessionEntriesToSteps = (entries: SessionEntry[]): Step[] => {
  const steps: Step[] = [];
  let stepId = 1;

  for (const entry of entries) {
    switch (entry.type) {
      case "user":
        steps.push(userMessageEntryToStep(entry, stepId++));
        break;
      case "assistant":
        steps.push(assistantMessageEntryToStep(entry, stepId++));
        break;
      case "tool_result":
        steps.push(toolResultEntryToStep(entry, stepId++));
        break;
      // Skip session_start and session_end - they're metadata
    }
  }

  return steps;
};

/**
 * Convert a full session to a trajectory
 */
export const sessionEntriesToTrajectory = (
  sessionId: string,
  entries: SessionEntry[],
  agent: Agent,
): Trajectory => {
  const steps = sessionEntriesToSteps(entries);

  // Calculate totals from steps
  let totalPromptTokens = 0;
  let totalCompletionTokens = 0;
  let totalCachedTokens = 0;
  let totalCostUsd = 0;

  for (const step of steps) {
    if (step.metrics) {
      totalPromptTokens += step.metrics.prompt_tokens ?? 0;
      totalCompletionTokens += step.metrics.completion_tokens ?? 0;
      totalCachedTokens += step.metrics.cached_tokens ?? 0;
      totalCostUsd += step.metrics.cost_usd ?? 0;
    }
  }

  return {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    agent,
    steps,
    final_metrics: {
      total_prompt_tokens: totalPromptTokens,
      total_completion_tokens: totalCompletionTokens,
      total_cached_tokens: totalCachedTokens > 0 ? totalCachedTokens : undefined,
      total_cost_usd: totalCostUsd > 0 ? totalCostUsd : undefined,
      total_steps: steps.length,
    },
  };
};

// ============================================================================
// Orchestrator Event Adapters
// ============================================================================

/**
 * Convert an OrchestratorEvent to an ATIF step (if applicable)
 * Returns null for events that don't map to steps
 */
export const orchestratorEventToStep = (
  event: OrchestratorEvent,
  stepId: number,
): Step | null => {
  const ts = "timestamp" in event ? (event.timestamp as string) : timestamp();

  switch (event.type) {
    case "session_start":
      return {
        step_id: stepId,
        timestamp: event.timestamp,
        source: "system",
        message: `Session started: ${event.sessionId}`,
        extra: { event_type: "session_start" },
      };

    case "task_selected":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Selected task: ${event.task.title}`,
        extra: {
          event_type: "task_selected",
          task_id: event.task.id,
          task_priority: event.task.priority,
        },
      };

    case "task_decomposed":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "agent",
        message: `Decomposed task into ${event.subtasks.length} subtasks`,
        extra: {
          event_type: "task_decomposed",
          subtask_ids: event.subtasks.map((s) => s.id),
        },
      };

    case "subtask_start":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Starting subtask: ${event.subtask.description}`,
        extra: {
          event_type: "subtask_start",
          subtask_id: event.subtask.id,
        },
      };

    case "subtask_complete": {
      const result = event.result;
      const observation: ObservationResult = {
        source_call_id: event.subtask.id,
        content: {
          success: result.success,
          files_modified: result.filesModified,
          turns: result.turns,
          agent: result.agent,
        },
        subagent_trajectory_ref: result.claudeCodeSessionId
          ? [{ session_id: result.claudeCodeSessionId }]
          : undefined,
      };

      const metrics: Metrics | undefined = result.sessionMetadata?.usage
        ? {
            prompt_tokens: result.sessionMetadata.usage.inputTokens,
            completion_tokens: result.sessionMetadata.usage.outputTokens,
            cached_tokens: result.sessionMetadata.usage.cacheReadInputTokens,
            cost_usd: result.sessionMetadata.totalCostUsd,
          }
        : undefined;

      return {
        step_id: stepId,
        timestamp: ts,
        source: "agent",
        message: `Completed subtask: ${event.subtask.description}`,
        observation: { results: [observation] },
        metrics,
        extra: {
          event_type: "subtask_complete",
          subtask_id: event.subtask.id,
          agent_type: result.agent,
        },
      };
    }

    case "subtask_failed":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Subtask failed: ${event.error}`,
        extra: {
          event_type: "subtask_failed",
          subtask_id: event.subtask.id,
          error: event.error,
        },
      };

    case "verification_start":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Running verification: ${event.command}`,
        extra: { event_type: "verification_start" },
      };

    case "verification_complete":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: event.passed
          ? `Verification passed: ${event.command}`
          : `Verification failed: ${event.command}`,
        observation: {
          results: [
            {
              content: {
                command: event.command,
                passed: event.passed,
                output: event.output,
              },
            },
          ],
        },
        extra: {
          event_type: "verification_complete",
          passed: event.passed,
        },
      };

    case "commit_created":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "agent",
        message: `Created commit: ${event.sha.slice(0, 7)} - ${event.message}`,
        extra: {
          event_type: "commit_created",
          sha: event.sha,
        },
      };

    case "push_complete":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Pushed to ${event.branch}`,
        extra: { event_type: "push_complete", branch: event.branch },
      };

    case "session_complete":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: event.success
          ? `Session completed successfully: ${event.summary}`
          : `Session failed: ${event.summary}`,
        extra: {
          event_type: "session_complete",
          success: event.success,
        },
      };

    case "error":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Error in ${event.phase}: ${event.error}`,
        extra: {
          event_type: "error",
          phase: event.phase,
          error: event.error,
        },
      };

    // Events that don't map to steps
    case "lock_acquired":
    case "lock_stale_removed":
    case "lock_failed":
    case "lock_released":
    case "init_script_start":
    case "init_script_complete":
    case "orientation_complete":
    case "task_updated":
    case "progress_written":
      return null;

    default:
      return null;
  }
};

/**
 * Convert a list of orchestrator events to ATIF steps
 */
export const orchestratorEventsToSteps = (
  events: OrchestratorEvent[],
): Step[] => {
  const steps: Step[] = [];
  let stepId = 1;

  for (const event of events) {
    const step = orchestratorEventToStep(event, stepId);
    if (step) {
      steps.push(step);
      stepId++;
    }
  }

  return steps;
};

// ============================================================================
// SubagentResult Adapter
// ============================================================================

/**
 * Convert a SubagentResult to an ObservationResult
 */
export const subagentResultToObservation = (
  result: SubagentResult,
  toolCallId?: string,
): ObservationResult => ({
  source_call_id: toolCallId ?? result.subtaskId,
  content: {
    success: result.success,
    files_modified: result.filesModified,
    turns: result.turns,
    agent: result.agent,
    error: result.error,
  },
  subagent_trajectory_ref: result.claudeCodeSessionId
    ? [
        {
          session_id: result.claudeCodeSessionId,
          extra: result.sessionMetadata
            ? {
                tools_used: result.sessionMetadata.toolsUsed,
                summary: result.sessionMetadata.summary,
              }
            : undefined,
        },
      ]
    : undefined,
});

/**
 * Extract metrics from a SubagentResult
 */
export const subagentResultToMetrics = (result: SubagentResult): Metrics | undefined => {
  if (!result.sessionMetadata?.usage) return undefined;

  return {
    prompt_tokens: result.sessionMetadata.usage.inputTokens,
    completion_tokens: result.sessionMetadata.usage.outputTokens,
    cached_tokens: result.sessionMetadata.usage.cacheReadInputTokens,
    cost_usd: result.sessionMetadata.totalCostUsd,
  };
};

// ============================================================================
// Conversion Helpers
// ============================================================================

/**
 * Create an empty trajectory with just agent info
 */
export const createEmptyTrajectory = (
  sessionId: string | undefined,
  agent: Agent,
  parentSessionId?: string,
): Trajectory => ({
  schema_version: ATIF_SCHEMA_VERSION,
  session_id: sessionId ?? generateSessionId(),
  agent,
  steps: [],
  final_metrics: {
    total_prompt_tokens: 0,
    total_completion_tokens: 0,
    total_steps: 0,
  },
  extra: parentSessionId ? { parent_session_id: parentSessionId } : undefined,
});

/**
 * Merge multiple trajectories (e.g., for combining subagent trajectories)
 */
export const mergeTrajectories = (
  parent: Trajectory,
  children: Trajectory[],
): Trajectory => {
  // Re-number steps
  let stepId = 1;
  const allSteps: Step[] = [];

  for (const step of parent.steps) {
    allSteps.push({ ...step, step_id: stepId++ });
  }

  // Add child steps as nested observations
  for (const child of children) {
    for (const step of child.steps) {
      allSteps.push({
        ...step,
        step_id: stepId++,
        extra: {
          ...step.extra,
          from_subagent: child.agent.name,
          original_session_id: child.session_id,
        },
      });
    }
  }

  // Calculate combined metrics
  let totalPromptTokens = parent.final_metrics?.total_prompt_tokens ?? 0;
  let totalCompletionTokens = parent.final_metrics?.total_completion_tokens ?? 0;
  let totalCachedTokens = parent.final_metrics?.total_cached_tokens ?? 0;
  let totalCostUsd = parent.final_metrics?.total_cost_usd ?? 0;

  for (const child of children) {
    totalPromptTokens += child.final_metrics?.total_prompt_tokens ?? 0;
    totalCompletionTokens += child.final_metrics?.total_completion_tokens ?? 0;
    totalCachedTokens += child.final_metrics?.total_cached_tokens ?? 0;
    totalCostUsd += child.final_metrics?.total_cost_usd ?? 0;
  }

  return {
    ...parent,
    steps: allSteps,
    final_metrics: {
      total_prompt_tokens: totalPromptTokens,
      total_completion_tokens: totalCompletionTokens,
      total_cached_tokens: totalCachedTokens > 0 ? totalCachedTokens : undefined,
      total_cost_usd: totalCostUsd > 0 ? totalCostUsd : undefined,
      total_steps: allSteps.length,
    },
  };
};
