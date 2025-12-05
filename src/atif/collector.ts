/**
 * TrajectoryCollector Service
 *
 * Captures agent/subagent interactions in real-time, building ATIF trajectories.
 * Follows the APMCollector pattern but uses Effect services for composability.
 */
import { Context, Effect, Layer, Ref } from "effect";
import {
  type Agent,
  type FinalMetrics,
  type Metrics,
  type Observation,
  type ObservationResult,
  type Step,
  type SubagentTrajectoryRef,
  type ToolCall,
  type Trajectory,
  ATIF_SCHEMA_VERSION,
  generateSessionId,
  generateToolCallId,
  timestamp,
} from "./schema.js";

// ============================================================================
// Error Types
// ============================================================================

export class TrajectoryCollectorError extends Error {
  readonly _tag = "TrajectoryCollectorError";

  constructor(
    readonly reason:
      | "not_started"
      | "already_started"
      | "already_finished"
      | "invalid_state",
    message: string,
  ) {
    super(message);
    this.name = "TrajectoryCollectorError";
  }
}

// ============================================================================
// State Types
// ============================================================================

export interface ActiveTrajectory {
  sessionId: string;
  parentSessionId: string | undefined;
  agent: Agent;
  steps: Step[];
  stepCounter: number;
  toolCallIds: Set<string>;
  subagentRefs: Map<string, SubagentTrajectoryRef>;
  startedAt: string;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  totalCachedTokens: number;
  totalCostUsd: number;
}

// ============================================================================
// Service Interface
// ============================================================================

export interface TrajectoryCollector {
  /**
   * Start a new trajectory
   */
  startTrajectory(options: {
    sessionId?: string;
    agent: Agent;
    parentSessionId?: string;
  }): Effect.Effect<string, TrajectoryCollectorError>;

  /**
   * Record a user message step
   */
  recordUserStep(
    message: unknown,
    extra?: Record<string, unknown>,
  ): Effect.Effect<Step, TrajectoryCollectorError>;

  /**
   * Record an agent message step
   */
  recordAgentStep(options: {
    message: unknown;
    modelName?: string;
    reasoningContent?: string;
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
    extra?: Record<string, unknown>;
  }): Effect.Effect<Step, TrajectoryCollectorError>;

  /**
   * Record a system message step
   */
  recordSystemStep(
    message: unknown,
    extra?: Record<string, unknown>,
  ): Effect.Effect<Step, TrajectoryCollectorError>;

  /**
   * Record observation results (tool outputs)
   */
  recordObservation(
    results: Array<{
      sourceCallId?: string;
      content?: unknown;
      subagentRefs?: Array<{
        sessionId: string;
        trajectoryPath?: string;
        extra?: Record<string, unknown>;
      }>;
    }>,
    extra?: Record<string, unknown>,
  ): Effect.Effect<Step, TrajectoryCollectorError>;

  /**
   * Register a subagent for linking
   */
  registerSubagent(
    sessionId: string,
    trajectoryPath?: string,
    extra?: Record<string, unknown>,
  ): Effect.Effect<void, TrajectoryCollectorError>;

  /**
   * Finalize and return the trajectory
   */
  finishTrajectory(
    notes?: string,
  ): Effect.Effect<Trajectory, TrajectoryCollectorError>;

  /**
   * Get current state (for inspection/debugging)
   */
  getCurrentState(): Effect.Effect<ActiveTrajectory | null, never>;

  /**
   * Check if a trajectory is in progress
   */
  isActive(): Effect.Effect<boolean, never>;
}

// ============================================================================
// Service Tag
// ============================================================================

export class TrajectoryCollectorTag extends Context.Tag("TrajectoryCollector")<
  TrajectoryCollectorTag,
  TrajectoryCollector
>() {}

// ============================================================================
// Service Implementation
// ============================================================================

const makeTrajectoryCollector = Effect.gen(function* () {
  const stateRef = yield* Ref.make<ActiveTrajectory | null>(null);

  const getActiveState = Effect.gen(function* () {
    const state = yield* Ref.get(stateRef);
    if (!state) {
      return yield* Effect.fail(
        new TrajectoryCollectorError(
          "not_started",
          "No trajectory in progress. Call startTrajectory first.",
        ),
      );
    }
    return state;
  });

  const createStep = (
    state: ActiveTrajectory,
    source: "user" | "agent" | "system",
    message: unknown,
    options?: {
      modelName?: string;
      reasoningContent?: string;
      toolCalls?: ToolCall[];
      observation?: Observation;
      metrics?: Metrics;
      extra?: Record<string, unknown>;
    },
  ): Step => {
    // Build the step object with all fields at once to satisfy readonly constraints
    return {
      step_id: state.stepCounter + 1,
      timestamp: timestamp(),
      source,
      message,
      ...(options?.modelName && { model_name: options.modelName }),
      ...(options?.reasoningContent && { reasoning_content: options.reasoningContent }),
      ...(options?.toolCalls && { tool_calls: options.toolCalls }),
      ...(options?.observation && { observation: options.observation }),
      ...(options?.metrics && { metrics: options.metrics }),
      ...(options?.extra && { extra: options.extra }),
    };
  };

  const service: TrajectoryCollector = {
    startTrajectory: (options) =>
      Effect.gen(function* () {
        const current = yield* Ref.get(stateRef);
        if (current) {
          return yield* Effect.fail(
            new TrajectoryCollectorError(
              "already_started",
              `Trajectory already in progress: ${current.sessionId}`,
            ),
          );
        }

        const sessionId = options.sessionId ?? generateSessionId();
        const now = timestamp();

        const newState: ActiveTrajectory = {
          sessionId,
          parentSessionId: options.parentSessionId,
          agent: options.agent,
          steps: [],
          stepCounter: 0,
          toolCallIds: new Set(),
          subagentRefs: new Map(),
          startedAt: now,
          totalPromptTokens: 0,
          totalCompletionTokens: 0,
          totalCachedTokens: 0,
          totalCostUsd: 0,
        };

        yield* Ref.set(stateRef, newState);
        return sessionId;
      }),

    recordUserStep: (message, extra) =>
      Effect.gen(function* () {
        const state = yield* getActiveState;
        const step = createStep(state, "user", message, extra ? { extra } : undefined);

        yield* Ref.update(stateRef, (s) =>
          s
            ? {
                ...s,
                steps: [...s.steps, step],
                stepCounter: s.stepCounter + 1,
              }
            : s,
        );

        return step;
      }),

    recordAgentStep: (options) =>
      Effect.gen(function* () {
        const state = yield* getActiveState;

        // Convert tool calls to ATIF format
        const toolCalls: ToolCall[] | undefined = options.toolCalls?.map(
          (tc) => {
            const toolCallId = tc.toolCallId ?? generateToolCallId();
            state.toolCallIds.add(toolCallId);
            return {
              tool_call_id: toolCallId,
              function_name: tc.functionName,
              arguments: tc.arguments,
            };
          },
        );

        // Convert metrics
        const metrics: Metrics | undefined = options.metrics
          ? {
              prompt_tokens: options.metrics.promptTokens,
              completion_tokens: options.metrics.completionTokens,
              cached_tokens: options.metrics.cachedTokens,
              cost_usd: options.metrics.costUsd,
            }
          : undefined;

        // Build options object with only defined values
        const stepOptions: {
          modelName?: string;
          reasoningContent?: string;
          toolCalls?: ToolCall[];
          metrics?: Metrics;
          extra?: Record<string, unknown>;
        } = {};
        if (options.modelName) stepOptions.modelName = options.modelName;
        if (options.reasoningContent) stepOptions.reasoningContent = options.reasoningContent;
        if (toolCalls) stepOptions.toolCalls = toolCalls;
        if (metrics) stepOptions.metrics = metrics;
        if (options.extra) stepOptions.extra = options.extra;

        const step = createStep(state, "agent", options.message, stepOptions);

        // Update totals
        yield* Ref.update(stateRef, (s) =>
          s
            ? {
                ...s,
                steps: [...s.steps, step],
                stepCounter: s.stepCounter + 1,
                totalPromptTokens:
                  s.totalPromptTokens + (options.metrics?.promptTokens ?? 0),
                totalCompletionTokens:
                  s.totalCompletionTokens +
                  (options.metrics?.completionTokens ?? 0),
                totalCachedTokens:
                  s.totalCachedTokens + (options.metrics?.cachedTokens ?? 0),
                totalCostUsd: s.totalCostUsd + (options.metrics?.costUsd ?? 0),
              }
            : s,
        );

        return step;
      }),

    recordSystemStep: (message, extra) =>
      Effect.gen(function* () {
        const state = yield* getActiveState;
        const step = createStep(state, "system", message, extra ? { extra } : undefined);

        yield* Ref.update(stateRef, (s) =>
          s
            ? {
                ...s,
                steps: [...s.steps, step],
                stepCounter: s.stepCounter + 1,
              }
            : s,
        );

        return step;
      }),

    recordObservation: (results, extra) =>
      Effect.gen(function* () {
        const state = yield* getActiveState;

        const observationResults: ObservationResult[] = results.map((r) => ({
          ...(r.sourceCallId && { source_call_id: r.sourceCallId }),
          ...(r.content !== undefined && { content: r.content }),
          ...(r.subagentRefs && {
            subagent_trajectory_ref: r.subagentRefs.map((ref) => ({
              session_id: ref.sessionId,
              ...(ref.trajectoryPath && { trajectory_path: ref.trajectoryPath }),
              ...(ref.extra && { extra: ref.extra }),
            })),
          }),
        }));

        const observation: Observation = { results: observationResults };

        const step = createStep(state, "system", "Tool execution results", {
          observation,
          ...(extra && { extra }),
        });

        yield* Ref.update(stateRef, (s) =>
          s
            ? {
                ...s,
                steps: [...s.steps, step],
                stepCounter: s.stepCounter + 1,
              }
            : s,
        );

        return step;
      }),

    registerSubagent: (sessionId, trajectoryPath, extra) =>
      Effect.gen(function* () {
        yield* getActiveState;

        yield* Ref.update(stateRef, (s) => {
          if (!s) return s;
          const newRefs = new Map(s.subagentRefs);
          newRefs.set(sessionId, {
            session_id: sessionId,
            trajectory_path: trajectoryPath,
            extra,
          });
          return { ...s, subagentRefs: newRefs };
        });
      }),

    finishTrajectory: (notes) =>
      Effect.gen(function* () {
        const state = yield* getActiveState;

        const finalMetrics: FinalMetrics = {
          total_prompt_tokens: state.totalPromptTokens,
          total_completion_tokens: state.totalCompletionTokens,
          total_cached_tokens:
            state.totalCachedTokens > 0 ? state.totalCachedTokens : undefined,
          total_cost_usd:
            state.totalCostUsd > 0 ? state.totalCostUsd : undefined,
          total_steps: state.steps.length,
        };

        const trajectory: Trajectory = {
          schema_version: ATIF_SCHEMA_VERSION,
          session_id: state.sessionId,
          agent: state.agent,
          steps: state.steps,
          notes,
          final_metrics: finalMetrics,
          extra: state.parentSessionId
            ? { parent_session_id: state.parentSessionId }
            : undefined,
        };

        // Clear state
        yield* Ref.set(stateRef, null);

        return trajectory;
      }),

    getCurrentState: () => Ref.get(stateRef),

    isActive: () =>
      Effect.gen(function* () {
        const state = yield* Ref.get(stateRef);
        return state !== null;
      }),
  };

  return service;
});

// ============================================================================
// Layer
// ============================================================================

export const TrajectoryCollectorLive = Layer.effect(
  TrajectoryCollectorTag,
  makeTrajectoryCollector,
);

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Create a standalone TrajectoryCollector (not as a service)
 * Useful for simple use cases or testing
 *
 * Optionally supports streaming to disk via StreamingWriter.
 */
export class StandaloneTrajectoryCollector {
  private state: ActiveTrajectory | null = null;
  private streamingWriter?: import("./streaming-writer.js").StreamingWriter;

  /**
   * Set streaming writer for incremental persistence
   */
  setStreamingWriter(writer: import("./streaming-writer.js").StreamingWriter): void {
    this.streamingWriter = writer;
  }

  startTrajectory(options: {
    sessionId?: string;
    agent: Agent;
    parentSessionId?: string;
  }): string {
    if (this.state) {
      throw new TrajectoryCollectorError(
        "already_started",
        `Trajectory already in progress: ${this.state.sessionId}`,
      );
    }

    const sessionId = options.sessionId ?? generateSessionId();
    const now = timestamp();

    this.state = {
      sessionId,
      parentSessionId: options.parentSessionId,
      agent: options.agent,
      steps: [],
      stepCounter: 0,
      toolCallIds: new Set(),
      subagentRefs: new Map(),
      startedAt: now,
      totalPromptTokens: 0,
      totalCompletionTokens: 0,
      totalCachedTokens: 0,
      totalCostUsd: 0,
    };

    return sessionId;
  }

  private getState(): ActiveTrajectory {
    if (!this.state) {
      throw new TrajectoryCollectorError(
        "not_started",
        "No trajectory in progress",
      );
    }
    return this.state;
  }

  recordUserStep(message: unknown, extra?: Record<string, unknown>): Step {
    const state = this.getState();
    const step: Step = {
      step_id: ++state.stepCounter,
      timestamp: timestamp(),
      source: "user",
      message,
      extra,
    };
    state.steps.push(step);

    // Stream to disk if writer configured
    if (this.streamingWriter) {
      this.streamingWriter.writeStep(step).catch((err) => {
        console.warn(`[ATIF] Failed to stream step ${step.step_id}: ${err}`);
      });
    }

    return step;
  }

  recordAgentStep(options: {
    message: unknown;
    modelName?: string;
    reasoningContent?: string;
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
    extra?: Record<string, unknown>;
  }): Step {
    const state = this.getState();

    const toolCalls: ToolCall[] | undefined = options.toolCalls?.map((tc) => {
      const toolCallId = tc.toolCallId ?? generateToolCallId();
      state.toolCallIds.add(toolCallId);
      return {
        tool_call_id: toolCallId,
        function_name: tc.functionName,
        arguments: tc.arguments,
      };
    });

    const metrics: Metrics | undefined = options.metrics
      ? {
          prompt_tokens: options.metrics.promptTokens,
          completion_tokens: options.metrics.completionTokens,
          cached_tokens: options.metrics.cachedTokens,
          cost_usd: options.metrics.costUsd,
        }
      : undefined;

    const step: Step = {
      step_id: ++state.stepCounter,
      timestamp: timestamp(),
      source: "agent",
      message: options.message,
      model_name: options.modelName,
      reasoning_content: options.reasoningContent,
      tool_calls: toolCalls,
      metrics,
      extra: options.extra,
    };

    state.steps.push(step);

    if (options.metrics) {
      state.totalPromptTokens += options.metrics.promptTokens ?? 0;
      state.totalCompletionTokens += options.metrics.completionTokens ?? 0;
      state.totalCachedTokens += options.metrics.cachedTokens ?? 0;
      state.totalCostUsd += options.metrics.costUsd ?? 0;
    }

    // Stream to disk if writer configured
    if (this.streamingWriter) {
      this.streamingWriter.writeStep(step).catch((err) => {
        console.warn(`[ATIF] Failed to stream step ${step.step_id}: ${err}`);
      });
    }

    return step;
  }

  recordSystemStep(message: unknown, extra?: Record<string, unknown>): Step {
    const state = this.getState();
    const step: Step = {
      step_id: ++state.stepCounter,
      timestamp: timestamp(),
      source: "system",
      message,
      extra,
    };
    state.steps.push(step);

    // Stream to disk if writer configured
    if (this.streamingWriter) {
      this.streamingWriter.writeStep(step).catch((err) => {
        console.warn(`[ATIF] Failed to stream step ${step.step_id}: ${err}`);
      });
    }

    return step;
  }

  recordObservation(
    results: Array<{
      sourceCallId?: string;
      content?: unknown;
      subagentRefs?: Array<{
        sessionId: string;
        trajectoryPath?: string;
        extra?: Record<string, unknown>;
      }>;
    }>,
    extra?: Record<string, unknown>,
  ): Step {
    const state = this.getState();

    const observationResults: ObservationResult[] = results.map((r) => ({
      source_call_id: r.sourceCallId,
      content: r.content,
      subagent_trajectory_ref: r.subagentRefs?.map((ref) => ({
        session_id: ref.sessionId,
        trajectory_path: ref.trajectoryPath,
        extra: ref.extra,
      })),
    }));

    const step: Step = {
      step_id: ++state.stepCounter,
      timestamp: timestamp(),
      source: "system",
      message: "Tool execution results",
      observation: { results: observationResults },
      extra,
    };

    state.steps.push(step);

    // Stream to disk if writer configured
    if (this.streamingWriter) {
      this.streamingWriter.writeStep(step).catch((err) => {
        console.warn(`[ATIF] Failed to stream step ${step.step_id}: ${err}`);
      });
    }

    return step;
  }

  registerSubagent(
    sessionId: string,
    trajectoryPath?: string,
    extra?: Record<string, unknown>,
  ): void {
    const state = this.getState();
    state.subagentRefs.set(sessionId, {
      session_id: sessionId,
      trajectory_path: trajectoryPath,
      extra,
    });
  }

  finishTrajectory(notes?: string): Trajectory {
    const state = this.getState();

    const finalMetrics: FinalMetrics = {
      total_prompt_tokens: state.totalPromptTokens,
      total_completion_tokens: state.totalCompletionTokens,
      total_cached_tokens:
        state.totalCachedTokens > 0 ? state.totalCachedTokens : undefined,
      total_cost_usd: state.totalCostUsd > 0 ? state.totalCostUsd : undefined,
      total_steps: state.steps.length,
    };

    const trajectory: Trajectory = {
      schema_version: ATIF_SCHEMA_VERSION,
      session_id: state.sessionId,
      agent: state.agent,
      steps: state.steps,
      notes,
      final_metrics: finalMetrics,
      extra: state.parentSessionId
        ? { parent_session_id: state.parentSessionId }
        : undefined,
    };

    // Finalize streaming writer if present
    if (this.streamingWriter) {
      this.streamingWriter.close(finalMetrics, "complete").catch((err) => {
        console.warn(`[ATIF] Failed to finalize streaming writer: ${err}`);
      });
    }

    this.state = null;
    return trajectory;
  }

  isActive(): boolean {
    return this.state !== null;
  }

  getCurrentState(): ActiveTrajectory | null {
    return this.state;
  }
}
