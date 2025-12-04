/**
 * Healer ATIF Integration
 *
 * Creates ATIF-compatible trajectories for Healer invocations.
 * Healer trajectories are linked to parent orchestrator trajectories
 * via subagent_trajectory_ref.
 */
import type {
  Agent,
  Step,
  Trajectory,
  ObservationResult,
  Metrics,
  SubagentTrajectoryRef,
} from "../atif/schema.js";
import {
  ATIF_SCHEMA_VERSION,
  timestamp,
  generateSessionId,
} from "../atif/schema.js";
import type {
  HealerContext,
  HealerOutcome,
  HealerSpellId,
  HealerSpellResult,
  HealerScenario,
} from "./types.js";
import type { HealerEvent } from "./service.js";

// ============================================================================
// Agent Factory
// ============================================================================

/**
 * Create an Agent for Healer subagent
 */
export const createHealerAgent = (
  modelName = "healer-v1",
  version = "1.0.0"
): Agent => ({
  name: "healer",
  version,
  model_name: modelName,
  extra: {
    type: "subagent",
    purpose: "self-healing",
  },
});

// ============================================================================
// Event to Step Conversion
// ============================================================================

/**
 * Convert a HealerEvent to an ATIF step
 */
export const healerEventToStep = (
  event: HealerEvent,
  stepId: number
): Step => {
  const ts = timestamp();

  switch (event.type) {
    case "healer_start":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Healer invoked for scenario: ${event.scenario}`,
        extra: {
          event_type: "healer_start",
          scenario: event.scenario,
          planned_spells: event.spells,
        },
      };

    case "healer_spell_start":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "agent",
        message: `Executing spell: ${event.spellId}`,
        tool_calls: [
          {
            tool_call_id: `spell-${event.spellId}-${stepId}`,
            function_name: event.spellId,
            arguments: {},
          },
        ],
        extra: {
          event_type: "healer_spell_start",
          spell_id: event.spellId,
        },
      };

    case "healer_spell_complete":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "agent",
        message: event.result.success
          ? `Spell ${event.spellId} succeeded: ${event.result.summary}`
          : `Spell ${event.spellId} failed: ${event.result.summary}`,
        observation: {
          results: [
            {
              source_call_id: `spell-${event.spellId}-${stepId - 1}`,
              content: {
                success: event.result.success,
                changes_applied: event.result.changesApplied,
                summary: event.result.summary,
                files_modified: event.result.filesModified,
                error: event.result.error,
              },
            },
          ],
        },
        extra: {
          event_type: "healer_spell_complete",
          spell_id: event.spellId,
          success: event.result.success,
        },
      };

    case "healer_complete":
      return {
        step_id: stepId,
        timestamp: ts,
        source: "system",
        message: `Healer completed with status: ${event.outcome.status}`,
        extra: {
          event_type: "healer_complete",
          status: event.outcome.status,
          reason: event.outcome.reason,
          spells_executed: event.outcome.spellsExecuted.length,
        },
      };
  }
};

/**
 * Convert multiple HealerEvents to ATIF steps
 */
export const healerEventsToSteps = (events: HealerEvent[]): Step[] => {
  return events.map((event, index) => healerEventToStep(event, index + 1));
};

// ============================================================================
// Trajectory Creation
// ============================================================================

/**
 * Create a Healer trajectory from an outcome and events
 */
export const createHealerTrajectory = (
  outcome: HealerOutcome,
  events: HealerEvent[],
  parentSessionId?: string
): Trajectory => {
  const sessionId = generateSessionId();
  const steps = healerEventsToSteps(events);
  const agent = createHealerAgent();

  // Calculate metrics from spell results
  let totalSteps = steps.length;
  let successfulSpells = 0;
  let failedSpells = 0;

  for (const executed of outcome.spellsExecuted) {
    if (executed.result.success) {
      successfulSpells++;
    } else {
      failedSpells++;
    }
  }

  const trajectory: Trajectory = {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    agent,
    steps,
    final_metrics: {
      total_prompt_tokens: 0, // Healer doesn't use tokens directly
      total_completion_tokens: 0,
      total_steps: totalSteps,
    },
    extra: {
      scenario: outcome.context.heuristics.scenario,
      status: outcome.status,
      reason: outcome.reason,
      spells_executed: outcome.spellsExecuted.length,
      successful_spells: successfulSpells,
      failed_spells: failedSpells,
      ...(parentSessionId ? { parent_session_id: parentSessionId } : {}),
    },
  };

  return trajectory;
};

/**
 * Create a minimal Healer trajectory (for when full events aren't captured)
 */
export const createMinimalHealerTrajectory = (
  outcome: HealerOutcome,
  parentSessionId?: string
): Trajectory => {
  const sessionId = generateSessionId();
  const agent = createHealerAgent();
  const ts = timestamp();

  // Create synthetic steps from the outcome
  const steps: Step[] = [];
  let stepId = 1;

  // Start step
  steps.push({
    step_id: stepId++,
    timestamp: ts,
    source: "system",
    message: `Healer invoked for scenario: ${outcome.context.heuristics.scenario}`,
    extra: {
      event_type: "healer_start",
      scenario: outcome.context.heuristics.scenario,
    },
  });

  // Spell execution steps
  for (const executed of outcome.spellsExecuted) {
    steps.push({
      step_id: stepId++,
      timestamp: ts,
      source: "agent",
      message: executed.result.success
        ? `Spell ${executed.spellId} succeeded: ${executed.result.summary}`
        : `Spell ${executed.spellId} failed: ${executed.result.summary}`,
      observation: {
        results: [
          {
            source_call_id: executed.spellId,
            content: {
              success: executed.result.success,
              changes_applied: executed.result.changesApplied,
              summary: executed.result.summary,
              files_modified: executed.result.filesModified,
              error: executed.result.error,
            },
          },
        ],
      },
      extra: {
        spell_id: executed.spellId,
        success: executed.result.success,
      },
    });
  }

  // Complete step
  steps.push({
    step_id: stepId++,
    timestamp: ts,
    source: "system",
    message: `Healer completed with status: ${outcome.status}`,
    extra: {
      status: outcome.status,
      reason: outcome.reason,
    },
  });

  return {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    agent,
    steps,
    final_metrics: {
      total_prompt_tokens: 0,
      total_completion_tokens: 0,
      total_steps: steps.length,
    },
    extra: {
      scenario: outcome.context.heuristics.scenario,
      status: outcome.status,
      ...(parentSessionId ? { parent_session_id: parentSessionId } : {}),
    },
  };
};

// ============================================================================
// Subagent Reference
// ============================================================================

/**
 * Create a SubagentTrajectoryRef for a Healer trajectory
 */
export const createHealerTrajectoryRef = (
  trajectory: Trajectory,
  trajectoryPath?: string
): SubagentTrajectoryRef => ({
  session_id: trajectory.session_id,
  trajectory_path: trajectoryPath,
  extra: {
    agent_name: trajectory.agent.name,
    status: trajectory.extra?.status,
    scenario: trajectory.extra?.scenario,
  },
});

/**
 * Create an ObservationResult for a Healer invocation
 * (for adding to parent orchestrator trajectory)
 */
export const createHealerObservation = (
  outcome: HealerOutcome,
  trajectoryRef?: SubagentTrajectoryRef
): ObservationResult => ({
  source_call_id: `healer-${outcome.context.heuristics.scenario}`,
  content: {
    status: outcome.status,
    reason: outcome.reason,
    spells_executed: outcome.spellsExecuted.map((s) => s.spellId),
    successful_spells: outcome.spellsExecuted.filter((s) => s.result.success).length,
    failed_spells: outcome.spellsExecuted.filter((s) => !s.result.success).length,
  },
  subagent_trajectory_ref: trajectoryRef ? [trajectoryRef] : undefined,
});

// ============================================================================
// Orchestrator Integration Helpers
// ============================================================================

/**
 * Create a Step for adding Healer invocation to orchestrator trajectory
 */
export const createHealerInvocationStep = (
  outcome: HealerOutcome,
  stepId: number,
  trajectoryRef?: SubagentTrajectoryRef
): Step => {
  const ts = timestamp();

  return {
    step_id: stepId,
    timestamp: ts,
    source: "agent",
    message: `Healer invoked for ${outcome.context.heuristics.scenario}: ${outcome.status}`,
    observation: {
      results: [createHealerObservation(outcome, trajectoryRef)],
    },
    extra: {
      event_type: "healer_invocation",
      scenario: outcome.context.heuristics.scenario,
      status: outcome.status,
      spells_executed: outcome.spellsExecuted.length,
    },
  };
};

/**
 * Collector helper for capturing Healer events during execution
 */
export class HealerEventCollector {
  private events: HealerEvent[] = [];
  private parentSessionId?: string;

  constructor(parentSessionId?: string) {
    this.parentSessionId = parentSessionId;
  }

  /**
   * Callback to pass to HealerService for event collection
   */
  onEvent = (event: HealerEvent): void => {
    this.events.push(event);
  };

  /**
   * Get all collected events
   */
  getEvents(): HealerEvent[] {
    return [...this.events];
  }

  /**
   * Create trajectory from collected events and outcome
   */
  createTrajectory(outcome: HealerOutcome): Trajectory {
    return createHealerTrajectory(outcome, this.events, this.parentSessionId);
  }

  /**
   * Reset collector for reuse
   */
  reset(): void {
    this.events = [];
  }
}
