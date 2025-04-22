import type { AgentState, PlanStep, ToolCall } from "./AgentStateTypes.js"
import { Effect, Layer } from "effect"

/**
 * Service for managing the execution plan within the agent state
 */
export interface PlanManager {
  /**
   * Add a new step to the plan
   * @param state Current agent state
   * @param description Description of the new step
   * @returns Updated agent state with the new step added
   */
  readonly addPlanStep: (state: AgentState, description: string) => Effect.Effect<AgentState>

  /**
   * Update a step's status and related fields
   * @param state Current agent state
   * @param stepId The ID of the step to update
   * @param newStatus The new status to set
   * @param resultSummary Optional summary of the step result
   * @returns Updated agent state with the step status changed
   */
  readonly updateStepStatus: (
    state: AgentState,
    stepId: string,
    newStatus: PlanStep["status"],
    resultSummary?: string | null
  ) => Effect.Effect<AgentState>

  /**
   * Add a tool call to a specific step
   * @param state Current agent state
   * @param stepId The ID of the step to add the tool call to
   * @param toolCallData The tool call data (timestamp will be added automatically)
   * @returns Updated agent state with the tool call added
   */
  readonly addToolCallToStep: (
    state: AgentState,
    stepId: string,
    toolCallData: Omit<ToolCall, "timestamp">
  ) => Effect.Effect<AgentState>

  /**
   * Get the current step from the plan
   * @param state Current agent state
   * @returns The current plan step
   */
  readonly getCurrentStep: (state: AgentState) => Effect.Effect<PlanStep, Error>
}

/**
 * Effect Tag for the PlanManager service
 */
export class PlanManager extends Effect.Tag("PlanManager")<
  PlanManager,
  {
    addPlanStep: (state: AgentState, description: string) => Effect.Effect<AgentState>
    updateStepStatus: (
      state: AgentState,
      stepId: string, 
      newStatus: PlanStep["status"],
      resultSummary?: string | null
    ) => Effect.Effect<AgentState>
    addToolCallToStep: (
      state: AgentState,
      stepId: string,
      toolCallData: Omit<ToolCall, "timestamp">
    ) => Effect.Effect<AgentState>
    getCurrentStep: (state: AgentState) => Effect.Effect<PlanStep, Error>
  }
>() {}

/**
 * Layer that provides the PlanManager implementation
 */
export const PlanManagerLayer = Layer.succeed(
  PlanManager,
  {
    addPlanStep: (state: AgentState, description: string) => Effect.sync(() => {
      // Generate a unique step ID
      const stepId = `step-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`
      
      // Create the new step
      const newStep: PlanStep = {
        id: stepId,
        step_number: state.plan.length + 1, // 1-based index
        description,
        status: "pending",
        start_time: null,
        end_time: null,
        result_summary: null,
        tool_calls: []
      }

      // Return a new state with the step added to the plan
      return {
        ...state,
        plan: [...state.plan, newStep],
        metrics: {
          ...state.metrics,
          total_steps_in_plan: state.metrics.total_steps_in_plan + 1
        }
      }
    }),

    updateStepStatus: (state: AgentState, stepId: string, newStatus: PlanStep["status"], resultSummary: string | null = null) => Effect.sync(() => {
      const stepIndex = state.plan.findIndex((step: PlanStep) => step.id === stepId)
      
      // If step not found, throw an error that will be converted to Effect.fail
      if (stepIndex === -1) {
        throw new Error(`Plan step with id ${stepId} not found`)
      }

      const now = new Date().toISOString()
      const originalStep = state.plan[stepIndex]
      
      // Create updated step with new status and appropriate timestamps
      const updatedStep = {
        ...originalStep,
        status: newStatus,
        // Set start_time if moving to in_progress and not already set
        start_time: originalStep.start_time ?? (newStatus === "in_progress" ? now : null),
        // Set end_time if completing, skipping, or erroring
        end_time: (newStatus === "completed" || newStatus === "skipped" || newStatus === "error") 
          ? now 
          : originalStep.end_time,
        result_summary: resultSummary !== null ? resultSummary : originalStep.result_summary
      }

      // Create a new plan array with the updated step
      const updatedPlan = [...state.plan]
      updatedPlan[stepIndex] = updatedStep

      // Count completed steps for metrics
      const stepsCompleted = updatedPlan.filter(step => step.status === "completed").length

      // Return updated state
      return {
        ...state,
        plan: updatedPlan,
        metrics: {
          ...state.metrics,
          steps_completed: stepsCompleted
        }
      }
    }).pipe(
      Effect.catchAll(error => Effect.fail(error))
    ),

    addToolCallToStep: (state: AgentState, stepId: string, toolCallData: Omit<ToolCall, "timestamp">) => Effect.sync(() => {
      const stepIndex = state.plan.findIndex((step: PlanStep) => step.id === stepId)
      
      // If step not found, throw an error
      if (stepIndex === -1) {
        throw new Error(`Plan step with id ${stepId} not found`)
      }

      // Create the tool call with timestamp
      const newToolCall: ToolCall = {
        ...toolCallData,
        timestamp: new Date().toISOString()
      }

      // Create an updated step with the new tool call
      const originalStep = state.plan[stepIndex]
      const updatedStep = {
        ...originalStep,
        tool_calls: [...originalStep.tool_calls, newToolCall]
      }

      // Create a new plan array with the updated step
      const updatedPlan = [...state.plan]
      updatedPlan[stepIndex] = updatedStep

      // Return updated state
      return {
        ...state,
        plan: updatedPlan
      }
    }).pipe(
      Effect.catchAll(error => Effect.fail(error))
    ),

    getCurrentStep: (state: AgentState) => {
      const index = state.current_task.current_step_index
      
      // Check if the index is valid
      if (index >= 0 && index < state.plan.length) {
        return Effect.succeed(state.plan[index])
      } else {
        return Effect.fail(new Error(`Invalid current_step_index: ${index}`))
      }
    }
  }
)