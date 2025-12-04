/**
 * ATIF Integration Helpers
 *
 * Provides easy-to-use hooks for integrating ATIF trajectory capture
 * into the agent loop, orchestrator, and subagents.
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import type { LoopEvent, AgentResult } from "../agent/loop.js";
import type { OrchestratorEvent, SubagentResult } from "../agent/orchestrator/types.js";
import type { Agent, Trajectory } from "./schema.js";
import {
  makeTrajectoryService,
  DEFAULT_TRAJECTORIES_DIR,
} from "./service.js";
import {
  createAgent,
  createMechaCoderAgent,
  createClaudeCodeAgent,
  createMinimalSubagent,
} from "./adapter.js";
import { StandaloneTrajectoryCollector } from "./collector.js";

// Re-export for convenience
export {
  createAgent,
  createMechaCoderAgent,
  createClaudeCodeAgent,
  createMinimalSubagent,
};

// ============================================================================
// Agent Loop Integration
// ============================================================================

export interface AgentLoopATIFOptions {
  /** Agent info */
  agent: Agent;
  /** Working directory for trajectory storage */
  cwd: string;
  /** Custom trajectories directory (default: .openagents/trajectories) */
  trajectoriesDir?: string;
  /** Parent session ID for subagent linking */
  parentSessionId?: string;
  /** Custom session ID (auto-generated if not provided) */
  sessionId?: string;
}

export interface AgentLoopATIFContext {
  /** The trajectory collector */
  collector: StandaloneTrajectoryCollector;
  /** Session ID for this trajectory */
  sessionId: string;
  /** Handler for LoopEvents - connect to agentLoop's onEvent */
  handleEvent: (event: LoopEvent) => void;
  /** Finalize and save the trajectory */
  finalize: (result?: AgentResult) => Promise<string>;
  /** Get the current trajectory (before finalization) */
  getTrajectory: () => Trajectory;
}

/**
 * Create an ATIF context for the agent loop.
 * Returns a collector and event handler that can be wired into agentLoop.
 *
 * @example
 * ```typescript
 * const atif = createAgentLoopATIF({
 *   agent: createMinimalSubagent("gpt-4"),
 *   cwd: process.cwd(),
 * });
 *
 * const result = await Effect.runPromise(
 *   agentLoop(userMessage, tools, {
 *     onEvent: atif.handleEvent,
 *   })
 * );
 *
 * const trajectoryPath = await atif.finalize(result);
 * ```
 */
export const createAgentLoopATIF = (
  options: AgentLoopATIFOptions
): AgentLoopATIFContext => {
  const collector = new StandaloneTrajectoryCollector();
  const sessionId = collector.startTrajectory({
    ...(options.sessionId && { sessionId: options.sessionId }),
    agent: options.agent,
    ...(options.parentSessionId && { parentSessionId: options.parentSessionId }),
  });

  // Track tool call IDs for correlation
  const pendingToolCalls = new Map<string, { name: string; args: unknown }>();

  const handleEvent = (event: LoopEvent): void => {
    switch (event.type) {
      case "turn_start":
        // Turn starts are implicit in step sequencing
        break;

      case "llm_request":
        // First turn's user message - record it
        if (event.turn === 1 && event.messages.length > 0) {
          const userMsg = event.messages.find((m) => m.role === "user");
          if (userMsg && typeof userMsg.content === "string") {
            collector.recordUserStep(userMsg.content);
          }
        }
        break;

      case "llm_response":
        // Record agent response
        if (event.message.content) {
          const toolCallsArray = event.toolCalls.length > 0
            ? event.toolCalls.map((tc) => ({
                functionName: tc.name,
                arguments: safeJsonParse(tc.arguments),
                toolCallId: tc.id,
              }))
            : null;

          collector.recordAgentStep({
            message: String(event.message.content),
            ...(toolCallsArray && { toolCalls: toolCallsArray }),
          });

          // Track pending tool calls
          for (const tc of event.toolCalls) {
            pendingToolCalls.set(tc.id, {
              name: tc.name,
              args: safeJsonParse(tc.arguments),
            });
          }
        }
        break;

      case "tool_result": {
        // Record observation
        const textContent = event.result.content
          .filter((c): c is { type: "text"; text: string } => c.type === "text")
          .map((c) => c.text)
          .join("\n");

        collector.recordObservation([
          {
            sourceCallId: event.toolCallId,
            content: textContent || (event.ok ? "Success" : "Error"),
          },
        ]);
        pendingToolCalls.delete(event.toolCallId);
        break;
      }

      // tool_call and edit_detected are informational, already captured in llm_response
    }
  };

  const finalize = async (result?: AgentResult): Promise<string> => {
    // Finalize the trajectory
    const trajectory = collector.finishTrajectory();

    // Save to disk
    const trajectoriesDir =
      options.trajectoriesDir ??
      `${options.cwd}/${DEFAULT_TRAJECTORIES_DIR}`;

    const filePath = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* makeTrajectoryService({
          trajectoriesDir,
          validateOnSave: true,
        });
        return yield* service.saveTrajectory(trajectory);
      }).pipe(Effect.provide(BunContext.layer))
    );

    return filePath;
  };

  return {
    collector,
    sessionId,
    handleEvent,
    finalize,
    getTrajectory: () => collector.finishTrajectory(),
  };
};

// ============================================================================
// Orchestrator Integration
// ============================================================================

export interface OrchestratorATIFOptions {
  /** Working directory */
  cwd: string;
  /** Model name for orchestrator */
  modelName: string;
  /** Custom trajectories directory */
  trajectoriesDir?: string;
  /** Custom session ID */
  sessionId?: string;
}

export interface OrchestratorATIFContext {
  /** The trajectory collector */
  collector: StandaloneTrajectoryCollector;
  /** Session ID for this orchestrator run */
  sessionId: string;
  /** Handler for OrchestratorEvents */
  handleEvent: (event: OrchestratorEvent) => void;
  /** Register a completed subagent trajectory */
  registerSubagent: (
    subagentSessionId: string,
    subtaskId: string,
    result: SubagentResult
  ) => void;
  /** Finalize and save the trajectory */
  finalize: () => Promise<string>;
  /** Get the current trajectory */
  getTrajectory: () => Trajectory;
}

/**
 * Create an ATIF context for the orchestrator.
 *
 * @example
 * ```typescript
 * const atif = createOrchestratorATIF({
 *   cwd: process.cwd(),
 *   modelName: "gpt-4",
 * });
 *
 * await runOrchestrator(config, (event) => {
 *   atif.handleEvent(event);
 *   // ... other handlers
 * });
 *
 * const trajectoryPath = await atif.finalize();
 * ```
 */
export const createOrchestratorATIF = (
  options: OrchestratorATIFOptions
): OrchestratorATIFContext => {
  const collector = new StandaloneTrajectoryCollector();
  const agent = createMechaCoderAgent(options.modelName);
  const sessionId = collector.startTrajectory({
    ...(options.sessionId && { sessionId: options.sessionId }),
    agent,
  });

  const handleEvent = (event: OrchestratorEvent): void => {
    // Convert orchestrator events to system steps
    switch (event.type) {
      case "session_start":
        collector.recordSystemStep(`Session started: ${event.sessionId}`, {
          event_type: "session_start",
        });
        break;

      case "task_selected":
        collector.recordSystemStep(`Selected task: ${event.task.title}`, {
          event_type: "task_selected",
          task_id: event.task.id,
          task_priority: event.task.priority,
        });
        break;

      case "task_decomposed":
        collector.recordAgentStep({
          message: `Decomposed task into ${event.subtasks.length} subtasks`,
          extra: {
            event_type: "task_decomposed",
            subtask_ids: event.subtasks.map((s) => s.id),
          },
        });
        break;

      case "subtask_start":
        collector.recordSystemStep(`Starting subtask: ${event.subtask.description}`, {
          event_type: "subtask_start",
          subtask_id: event.subtask.id,
        });
        break;

      case "subtask_complete": {
        // Record as observation with subagent reference
        const subagentRefsArray = event.result.claudeCodeSessionId
          ? [{ sessionId: event.result.claudeCodeSessionId }]
          : null;
        collector.recordObservation([
          {
            sourceCallId: event.subtask.id,
            content: {
              success: event.result.success,
              files_modified: event.result.filesModified,
              turns: event.result.turns,
              agent: event.result.agent,
            },
            ...(subagentRefsArray && { subagentRefs: subagentRefsArray }),
          },
        ], {
          event_type: "subtask_complete",
          subtask_id: event.subtask.id,
        });
        break;
      }

      case "subtask_failed":
        collector.recordSystemStep(`Subtask failed: ${event.error}`, {
          event_type: "subtask_failed",
          subtask_id: event.subtask.id,
          error: event.error,
        });
        break;

      case "verification_complete":
        collector.recordObservation([
          {
            content: {
              command: event.command,
              passed: event.passed,
              output: event.output,
            },
          },
        ], {
          event_type: "verification_complete",
          passed: event.passed,
        });
        break;

      case "commit_created":
        collector.recordAgentStep({
          message: `Created commit: ${event.sha.slice(0, 7)} - ${event.message}`,
          extra: {
            event_type: "commit_created",
            sha: event.sha,
          },
        });
        break;

      case "session_complete":
        collector.recordSystemStep(
          event.success
            ? `Session completed successfully: ${event.summary}`
            : `Session failed: ${event.summary}`,
          {
            event_type: "session_complete",
            success: event.success,
          }
        );
        break;

      case "error":
        collector.recordSystemStep(`Error in ${event.phase}: ${event.error}`, {
          event_type: "error",
          phase: event.phase,
          error: event.error,
        });
        break;

      // Skip events that don't need recording
      default:
        break;
    }
  };

  const registerSubagent = (
    subagentSessionId: string,
    subtaskId: string,
    result: SubagentResult
  ): void => {
    collector.registerSubagent(
      subagentSessionId,
      undefined, // trajectory path will be determined at save time
      result.sessionMetadata
        ? {
            tools_used: result.sessionMetadata.toolsUsed,
            summary: result.sessionMetadata.summary,
          }
        : undefined
    );
  };

  const finalize = async (): Promise<string> => {
    const trajectory = collector.finishTrajectory();

    const trajectoriesDir =
      options.trajectoriesDir ??
      `${options.cwd}/${DEFAULT_TRAJECTORIES_DIR}`;

    const filePath = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* makeTrajectoryService({
          trajectoriesDir,
          validateOnSave: true,
        });
        return yield* service.saveTrajectory(trajectory);
      }).pipe(Effect.provide(BunContext.layer))
    );

    return filePath;
  };

  return {
    collector,
    sessionId,
    handleEvent,
    registerSubagent,
    finalize,
    getTrajectory: () => collector.finishTrajectory(),
  };
};

// ============================================================================
// Subagent Integration
// ============================================================================

export interface SubagentATIFOptions {
  /** Agent type */
  agentType: "claude-code" | "minimal";
  /** Model name */
  modelName: string;
  /** Working directory */
  cwd: string;
  /** Parent orchestrator session ID */
  parentSessionId: string;
  /** Subtask ID */
  subtaskId: string;
  /** Custom trajectories directory */
  trajectoriesDir?: string;
}

export interface SubagentATIFContext {
  /** The trajectory collector */
  collector: StandaloneTrajectoryCollector;
  /** Session ID for this subagent run */
  sessionId: string;
  /** Record the initial prompt */
  recordPrompt: (prompt: string) => void;
  /** Record an agent response */
  recordResponse: (
    message: string,
    options?: {
      toolCalls?: Array<{
        functionName: string;
        arguments: unknown;
        toolCallId?: string;
      }>;
      metrics?: {
        promptTokens?: number;
        completionTokens?: number;
        cachedTokens?: number;
        costUsd?: number;
      };
    }
  ) => void;
  /** Record tool results */
  recordToolResults: (results: Array<{ callId: string; content: unknown }>) => void;
  /** Finalize and save trajectory */
  finalize: (result: SubagentResult) => Promise<string>;
  /** Get current trajectory */
  getTrajectory: () => Trajectory;
}

/**
 * Create an ATIF context for a subagent.
 */
export const createSubagentATIF = (
  options: SubagentATIFOptions
): SubagentATIFContext => {
  const collector = new StandaloneTrajectoryCollector();
  const agent =
    options.agentType === "claude-code"
      ? createClaudeCodeAgent(options.modelName)
      : createMinimalSubagent(options.modelName);

  const sessionId = collector.startTrajectory({
    agent,
    parentSessionId: options.parentSessionId,
  });

  return {
    collector,
    sessionId,
    recordPrompt: (prompt: string) => {
      collector.recordUserStep(prompt, { user_type: "orchestrator" });
    },
    recordResponse: (message, opts) => {
      collector.recordAgentStep({
        message,
        ...(opts?.toolCalls && { toolCalls: opts.toolCalls }),
        ...(opts?.metrics && { metrics: opts.metrics }),
      });
    },
    recordToolResults: (results) => {
      collector.recordObservation(
        results.map((r) => ({
          sourceCallId: r.callId,
          content: r.content,
        }))
      );
    },
    finalize: async (result) => {
      const trajectory = collector.finishTrajectory();

      const trajectoriesDir =
        options.trajectoriesDir ??
        `${options.cwd}/${DEFAULT_TRAJECTORIES_DIR}`;

      const filePath = await Effect.runPromise(
        Effect.gen(function* () {
          const service = yield* makeTrajectoryService({
            trajectoriesDir,
            validateOnSave: true,
          });
          return yield* service.saveTrajectory(trajectory);
        }).pipe(Effect.provide(BunContext.layer))
      );

      return filePath;
    },
    getTrajectory: () => collector.finishTrajectory(),
  };
};

// ============================================================================
// Utility: Create ATIF-enabled event handler wrapper
// ============================================================================

/**
 * Wrap an existing event handler to also capture ATIF data.
 */
export const wrapEventHandler = <E>(
  originalHandler: ((event: E) => void) | undefined,
  atifHandler: (event: E) => void
): ((event: E) => void) => {
  return (event: E) => {
    atifHandler(event);
    originalHandler?.(event);
  };
};

// ============================================================================
// Helpers
// ============================================================================

const safeJsonParse = (str: string): unknown => {
  try {
    return JSON.parse(str);
  } catch {
    return str;
  }
};
