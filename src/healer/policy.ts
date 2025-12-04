/**
 * Healer Policy
 *
 * Determines when Healer should run based on:
 * - OrchestratorEvents that indicate failures
 * - HealerConfig settings (enabled, scenarios, limits)
 * - HealerCounters (rate limiting)
 */
import type {
  OrchestratorEvent,
  InitScriptResult,
} from "../agent/orchestrator/types.js";
import type { HealerConfig } from "../tasks/schema.js";
import type {
  HealerScenario,
  HealerCounters,
  HealerPolicyDecision,
} from "./types.js";
import { mapInitFailureToScenario } from "./types.js";

// ============================================================================
// Event to Scenario Mapping
// ============================================================================

/**
 * Map an OrchestratorEvent to a HealerScenario.
 * Returns null if the event doesn't trigger Healer.
 */
export const mapEventToScenario = (
  event: OrchestratorEvent
): HealerScenario | null => {
  switch (event.type) {
    case "init_script_complete":
      // Only trigger if init script failed
      if (!event.result.success && event.result.failureType) {
        return mapInitFailureToScenario(event.result.failureType);
      }
      return null;

    case "subtask_failed":
      return "SubtaskFailed";

    case "verification_complete":
      // Only trigger if verification failed
      if (!event.passed) {
        return "VerificationFailed";
      }
      return null;

    case "error":
      return "RuntimeError";

    // These events don't trigger Healer
    case "session_start":
    case "lock_acquired":
    case "lock_stale_removed":
    case "lock_failed":
    case "lock_released":
    case "init_script_start":
    case "orientation_complete":
    case "task_selected":
    case "task_decomposed":
    case "subtask_start":
    case "subtask_complete":
    case "verification_start":
    case "commit_created":
    case "push_complete":
    case "task_updated":
    case "progress_written":
    case "session_complete":
      return null;

    default:
      return null;
  }
};

/**
 * Extract InitScriptResult from an event (if applicable).
 */
export const getInitScriptResult = (
  event: OrchestratorEvent
): InitScriptResult | null => {
  if (event.type === "init_script_complete") {
    return event.result;
  }
  return null;
};

/**
 * Extract error output from an event (if applicable).
 */
export const getErrorOutput = (event: OrchestratorEvent): string | null => {
  switch (event.type) {
    case "init_script_complete":
      return event.result.output ?? event.result.error ?? null;
    case "subtask_failed":
      return event.error;
    case "verification_complete":
      return event.output;
    case "error":
      return event.error;
    default:
      return null;
  }
};

// ============================================================================
// Scenario Config Checks
// ============================================================================

/**
 * Check if a scenario is enabled in the config.
 */
export const isScenarioEnabled = (
  scenario: HealerScenario,
  config: HealerConfig
): boolean => {
  const scenarios = config.scenarios;

  switch (scenario) {
    case "InitScriptTypecheckFailure":
    case "InitScriptTestFailure":
    case "InitScriptEnvironmentFailure":
      return scenarios?.onInitFailure ?? true;

    case "VerificationFailed":
      return scenarios?.onVerificationFailure ?? true;

    case "SubtaskFailed":
      return scenarios?.onSubtaskFailure ?? true;

    case "SubtaskStuck":
      return scenarios?.onStuckSubtask ?? false;

    case "RuntimeError":
      return scenarios?.onRuntimeError ?? true;

    default:
      return false;
  }
};

// ============================================================================
// Rate Limiting
// ============================================================================

/**
 * Check if Healer has exceeded session invocation limit.
 */
export const hasExceededSessionLimit = (
  counters: HealerCounters,
  config: HealerConfig
): boolean => {
  const limit = config.maxInvocationsPerSession ?? 2;
  return counters.sessionInvocations >= limit;
};

/**
 * Check if Healer has exceeded per-subtask invocation limit.
 */
export const hasExceededSubtaskLimit = (
  subtaskId: string | undefined,
  counters: HealerCounters,
  config: HealerConfig
): boolean => {
  if (!subtaskId) return false;
  const limit = config.maxInvocationsPerSubtask ?? 1;
  const count = counters.subtaskInvocations.get(subtaskId) ?? 0;
  return count >= limit;
};

/**
 * Increment counters after a Healer invocation.
 */
export const incrementCounters = (
  counters: HealerCounters,
  subtaskId?: string
): void => {
  counters.sessionInvocations += 1;
  if (subtaskId) {
    const current = counters.subtaskInvocations.get(subtaskId) ?? 0;
    counters.subtaskInvocations.set(subtaskId, current + 1);
  }
};

// ============================================================================
// Main Policy Decision
// ============================================================================

/**
 * Determine if Healer should run for a given event.
 *
 * Returns a decision with:
 * - run: boolean - whether to invoke Healer
 * - scenario: the detected scenario (if run=true)
 * - reason: explanation for the decision
 */
export const shouldRunHealer = (
  event: OrchestratorEvent,
  config: HealerConfig,
  counters: HealerCounters,
  subtaskId?: string
): HealerPolicyDecision => {
  // 1. Check if Healer is enabled
  if (config.enabled === false) {
    return {
      run: false,
      reason: "Healer is disabled in config",
    };
  }

  // 2. Map event to scenario
  const scenario = mapEventToScenario(event);
  if (!scenario) {
    return {
      run: false,
      reason: "Event does not trigger Healer",
    };
  }

  // 3. Check if scenario is enabled
  if (!isScenarioEnabled(scenario, config)) {
    return {
      run: false,
      scenario,
      reason: `Scenario '${scenario}' is disabled in config`,
    };
  }

  // 4. Check session limit
  if (hasExceededSessionLimit(counters, config)) {
    return {
      run: false,
      scenario,
      reason: `Session limit reached (${counters.sessionInvocations}/${config.maxInvocationsPerSession ?? 2})`,
    };
  }

  // 5. Check subtask limit (for subtask-related scenarios)
  if (scenario === "SubtaskFailed" && subtaskId) {
    if (hasExceededSubtaskLimit(subtaskId, counters, config)) {
      const count = counters.subtaskInvocations.get(subtaskId) ?? 0;
      return {
        run: false,
        scenario,
        reason: `Subtask limit reached for '${subtaskId}' (${count}/${config.maxInvocationsPerSubtask ?? 1})`,
      };
    }
  }

  // 6. All checks passed - Healer should run
  return {
    run: true,
    scenario,
    reason: `Triggering Healer for scenario '${scenario}'`,
  };
};

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Get a human-readable description of why Healer would or wouldn't run.
 */
export const explainDecision = (decision: HealerPolicyDecision): string => {
  if (decision.run) {
    return `✓ Healer will run: ${decision.reason}`;
  }
  return `✗ Healer skipped: ${decision.reason}`;
};

/**
 * Check if an init script failure is healable.
 */
export const isHealableInitFailure = (result: InitScriptResult): boolean => {
  return result.canSelfHeal === true;
};
