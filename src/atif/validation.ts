/**
 * ATIF Trajectory Validation Service
 *
 * Implements the 5 ATIF v1.4 validation rules:
 * 1. step_id must be sequential starting from 1
 * 2. timestamps must be valid ISO 8601 format
 * 3. source values restricted to: user, agent, system
 * 4. tool call references in observations must match existing tool_call_ids
 * 5. Agent-only fields (model_name, reasoning_content) only on agent steps
 */
import { Effect } from "effect";
import type { Trajectory, Step } from "./schema.js";

// ============================================================================
// Error Types
// ============================================================================

export type ValidationErrorReason =
  | "invalid_step_sequence"
  | "invalid_timestamp"
  | "invalid_source"
  | "orphan_tool_reference"
  | "agent_only_field_on_non_agent"
  | "missing_required_field"
  | "invalid_schema_version";

export class TrajectoryValidationError extends Error {
  readonly _tag = "TrajectoryValidationError";

  constructor(
    readonly reason: ValidationErrorReason,
    readonly stepId: number | null,
    message: string,
    readonly details?: Record<string, unknown>,
  ) {
    super(message);
    this.name = "TrajectoryValidationError";
  }
}

// ============================================================================
// Validation Helpers
// ============================================================================

/**
 * Check if a string is valid ISO 8601 timestamp
 */
const isValidISO8601 = (timestamp: string): boolean => {
  const date = new Date(timestamp);
  return !isNaN(date.getTime()) && timestamp.includes("T");
};

/**
 * Validate a single step
 */
const validateStep = (
  step: Step,
  expectedStepId: number,
  toolCallIds: Set<string>,
): Effect.Effect<void, TrajectoryValidationError> =>
  Effect.gen(function* () {
    // Rule 1: Sequential step_id
    if (step.step_id !== expectedStepId) {
      return yield* Effect.fail(
        new TrajectoryValidationError(
          "invalid_step_sequence",
          step.step_id,
          `Expected step_id ${expectedStepId}, got ${step.step_id}`,
          { expected: expectedStepId, actual: step.step_id },
        ),
      );
    }

    // Rule 2: Valid ISO 8601 timestamp
    if (!isValidISO8601(step.timestamp)) {
      return yield* Effect.fail(
        new TrajectoryValidationError(
          "invalid_timestamp",
          step.step_id,
          `Invalid ISO 8601 timestamp: ${step.timestamp}`,
          { timestamp: step.timestamp },
        ),
      );
    }

    // Rule 3: Valid source (handled by schema, but double-check)
    const validSources = ["user", "agent", "system"];
    if (!validSources.includes(step.source)) {
      return yield* Effect.fail(
        new TrajectoryValidationError(
          "invalid_source",
          step.step_id,
          `Invalid source: ${step.source}. Must be one of: ${validSources.join(", ")}`,
          { source: step.source },
        ),
      );
    }

    // Rule 5: Agent-only fields check
    if (step.source !== "agent") {
      if (step.model_name !== undefined) {
        return yield* Effect.fail(
          new TrajectoryValidationError(
            "agent_only_field_on_non_agent",
            step.step_id,
            `model_name is only allowed on agent steps, found on ${step.source} step`,
            { field: "model_name", source: step.source },
          ),
        );
      }
      if (step.reasoning_content !== undefined) {
        return yield* Effect.fail(
          new TrajectoryValidationError(
            "agent_only_field_on_non_agent",
            step.step_id,
            `reasoning_content is only allowed on agent steps, found on ${step.source} step`,
            { field: "reasoning_content", source: step.source },
          ),
        );
      }
    }

    // Collect tool call IDs from this step
    if (step.tool_calls) {
      for (const tc of step.tool_calls) {
        toolCallIds.add(tc.tool_call_id);
      }
    }

    // Rule 4: Validate observation references
    if (step.observation) {
      for (const result of step.observation.results) {
        if (result.source_call_id && !toolCallIds.has(result.source_call_id)) {
          return yield* Effect.fail(
            new TrajectoryValidationError(
              "orphan_tool_reference",
              step.step_id,
              `Observation references unknown tool_call_id: ${result.source_call_id}`,
              {
                referenced_id: result.source_call_id,
                known_ids: Array.from(toolCallIds),
              },
            ),
          );
        }
      }
    }
  });

// ============================================================================
// Main Validation Function
// ============================================================================

/**
 * Validate a trajectory against ATIF v1.4 rules.
 *
 * Returns the trajectory if valid, or fails with TrajectoryValidationError.
 */
export const validateTrajectory = (
  trajectory: Trajectory,
): Effect.Effect<Trajectory, TrajectoryValidationError> =>
  Effect.gen(function* () {
    // Check schema version
    if (trajectory.schema_version !== "ATIF-v1.4") {
      return yield* Effect.fail(
        new TrajectoryValidationError(
          "invalid_schema_version",
          null,
          `Unsupported schema version: ${trajectory.schema_version}. Expected ATIF-v1.4`,
          { version: trajectory.schema_version },
        ),
      );
    }

    // Track tool call IDs across all steps for reference validation
    const toolCallIds = new Set<string>();

    // Validate each step
    for (let i = 0; i < trajectory.steps.length; i++) {
      const step = trajectory.steps[i];
      const expectedStepId = i + 1;
      yield* validateStep(step, expectedStepId, toolCallIds);
    }

    // Validate final_metrics.total_steps matches actual step count
    if (trajectory.final_metrics) {
      if (trajectory.final_metrics.total_steps !== trajectory.steps.length) {
        // This is a warning, not an error - just log it
        // In production you might want to auto-correct this
      }
    }

    return trajectory;
  });

/**
 * Validate a trajectory synchronously (throws on error)
 */
export const validateTrajectorySync = (trajectory: Trajectory): Trajectory => {
  return Effect.runSync(validateTrajectory(trajectory));
};

/**
 * Check if a trajectory is valid (returns boolean, doesn't throw)
 */
export const isValidTrajectory = (trajectory: Trajectory): boolean => {
  try {
    validateTrajectorySync(trajectory);
    return true;
  } catch {
    return false;
  }
};

/**
 * Collect all validation errors from a trajectory (instead of failing on first)
 */
export const collectValidationErrors = (
  trajectory: Trajectory,
): TrajectoryValidationError[] => {
  const errors: TrajectoryValidationError[] = [];

  // Check schema version
  if (trajectory.schema_version !== "ATIF-v1.4") {
    errors.push(
      new TrajectoryValidationError(
        "invalid_schema_version",
        null,
        `Unsupported schema version: ${trajectory.schema_version}. Expected ATIF-v1.4`,
        { version: trajectory.schema_version },
      ),
    );
  }

  const toolCallIds = new Set<string>();

  for (let i = 0; i < trajectory.steps.length; i++) {
    const step = trajectory.steps[i];
    const expectedStepId = i + 1;

    // Rule 1: Sequential step_id
    if (step.step_id !== expectedStepId) {
      errors.push(
        new TrajectoryValidationError(
          "invalid_step_sequence",
          step.step_id,
          `Expected step_id ${expectedStepId}, got ${step.step_id}`,
          { expected: expectedStepId, actual: step.step_id },
        ),
      );
    }

    // Rule 2: Valid ISO 8601 timestamp
    if (!isValidISO8601(step.timestamp)) {
      errors.push(
        new TrajectoryValidationError(
          "invalid_timestamp",
          step.step_id,
          `Invalid ISO 8601 timestamp: ${step.timestamp}`,
          { timestamp: step.timestamp },
        ),
      );
    }

    // Rule 5: Agent-only fields check
    if (step.source !== "agent") {
      if (step.model_name !== undefined) {
        errors.push(
          new TrajectoryValidationError(
            "agent_only_field_on_non_agent",
            step.step_id,
            `model_name is only allowed on agent steps, found on ${step.source} step`,
            { field: "model_name", source: step.source },
          ),
        );
      }
      if (step.reasoning_content !== undefined) {
        errors.push(
          new TrajectoryValidationError(
            "agent_only_field_on_non_agent",
            step.step_id,
            `reasoning_content is only allowed on agent steps, found on ${step.source} step`,
            { field: "reasoning_content", source: step.source },
          ),
        );
      }
    }

    // Collect tool call IDs
    if (step.tool_calls) {
      for (const tc of step.tool_calls) {
        toolCallIds.add(tc.tool_call_id);
      }
    }

    // Rule 4: Validate observation references
    if (step.observation) {
      for (const result of step.observation.results) {
        if (result.source_call_id && !toolCallIds.has(result.source_call_id)) {
          errors.push(
            new TrajectoryValidationError(
              "orphan_tool_reference",
              step.step_id,
              `Observation references unknown tool_call_id: ${result.source_call_id}`,
              {
                referenced_id: result.source_call_id,
                known_ids: Array.from(toolCallIds),
              },
            ),
          );
        }
      }
    }
  }

  return errors;
};
