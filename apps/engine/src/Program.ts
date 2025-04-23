import { AnthropicClient } from "@effect/ai-anthropic"
import { NodeContext, NodeHttpClient } from "@effect/platform-node"
import { Config, Console, Effect, Layer } from "effect"

// Import TAG CLASSES
import { PlanManager as PlanManagerTag } from "./github/PlanManager.js"
import { MemoryManager as MemoryManagerTag } from "./github/MemoryManager.js"
import { ContextManager as ContextManagerTag } from "./github/ContextManager.js"
import { GitHubClient as GitHubClientTag } from "./github/GitHub.js"
import { GitHubTools as GitHubToolsTag } from "./github/GitHubTools.js"
import { TaskExecutor as TaskExecutorTag } from "./github/TaskExecutor.js"

// Import LAYERS
import { MemoryManagerLayer } from "./github/MemoryManager.js"
import { ContextManagerLayer } from "./github/ContextManager.js"
import { GitHubClientLayer } from "./github/GitHub.js"
import { GitHubToolsDefault } from "./github/GitHubTools.js"
import { TaskExecutorDefault } from "./github/TaskExecutor.js"

// ----> RE-EXPORT TAGS <----
// Centralized single source of truth for all service Tags
export const PlanManager = PlanManagerTag
export const MemoryManager = MemoryManagerTag  
export const ContextManager = ContextManagerTag
export const GitHubClient = GitHubClientTag
export const GitHubTools = GitHubToolsTag
export const TaskExecutor = TaskExecutorTag
// ----> END TAG EXPORTS <----

// ----> START TAG IDENTITY LOGGING (PROGRAM) <----
console.log(`DEBUG: TAG_CHECK - PlanManager Tag DEFINED in Program.ts:`, PlanManager);
// ----> END TAG IDENTITY LOGGING (PROGRAM) <----

// Define Anthropic Layer
console.log("DEBUG: CRITICAL - Creating AnthropicClient layer with config")
const AnthropicLayer = AnthropicClient.layerConfig({
  apiKey: Config.secret("ANTHROPIC_API_KEY")
})
console.log("DEBUG: CRITICAL - AnthropicClient layer created successfully")

// Combined layers for the application
console.log("DEBUG: CRITICAL - Creating application layers composition")

// Create the HTTP client layer first
console.log("DEBUG: CRITICAL - Creating NodeHttpClient.layerUndici")
const httpClientLayer = NodeHttpClient.layerUndici
console.log("DEBUG: CRITICAL - Successfully created NodeHttpClient.layerUndici")

// Combine the Anthropic layer with HTTP client
console.log("DEBUG: CRITICAL - Providing HTTP layer to Anthropic layer")
const anthropicWithHttpLayer = Layer.provide(AnthropicLayer, httpClientLayer)
console.log("DEBUG: CRITICAL - Successfully created anthropicWithHttpLayer")

// Create a special layer just for PlanManager, because it seems to be problematic
console.log("DEBUG: CRITICAL - Creating special isolated PlanManagerLayer");
const PlanManagerLayerIsolated = Layer.succeed(
  PlanManager,
  {
    addPlanStep: (state, description) =>
      Effect.sync(() => {
        // Generate a unique step ID
        const stepId = `step-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`

        // Create the new step
        const newStep = {
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

    updateStepStatus: (
      state,
      stepId,
      newStatus,
      resultSummary = null
    ) =>
      Effect.sync(() => {
        const stepIndex = state.plan.findIndex((step) => step.id === stepId)

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
        const stepsCompleted = updatedPlan.filter((step) => step.status === "completed").length

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
        Effect.catchAll((error) => Effect.fail(error))
      ),

    addToolCallToStep: (state, stepId, toolCallData) =>
      Effect.sync(() => {
        const stepIndex = state.plan.findIndex((step) => step.id === stepId)

        // If step not found, throw an error
        if (stepIndex === -1) {
          throw new Error(`Plan step with id ${stepId} not found`)
        }

        // Create the tool call with timestamp
        const newToolCall = {
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
        Effect.catchAll((error) => Effect.fail(error))
      ),

    getCurrentStep: (state) => {
      const index = state.current_task.current_step_index

      // Check if the index is valid
      if (index >= 0 && index < state.plan.length) {
        return Effect.succeed(state.plan[index])
      } else {
        return Effect.fail(new Error(`Invalid current_step_index: ${index}`))
      }
    }
  }
);
console.log("DEBUG: CRITICAL - Created special isolated PlanManagerLayer successfully");

// Create base layer without PlanManager 
const BaseLayer = Layer.mergeAll(
  // Required platform layers
  NodeContext.layer,
  // Service layers
  GitHubClientLayer,
  // PlanManagerLayer - will be added separately
  ContextManagerLayer,
  MemoryManagerLayer,
  // Tools layer
  GitHubToolsDefault,
  // Task execution layer
  TaskExecutorDefault,
  // AI layer
  anthropicWithHttpLayer
);
console.log("DEBUG: CRITICAL - Created BaseLayer successfully");

// Final merged layer - add PlanManagerLayerIsolated after all other layers are merged
export const AllLayers = Layer.provide(BaseLayer, PlanManagerLayerIsolated).pipe(
  Layer.tap(() => Effect.sync(() => {
    console.log("DEBUG: CRITICAL - AllLayers composition created and initialized successfully")
  }))
)

// We don't start the server directly from Program.ts anymore
// This file is now just responsible for providing AllLayers
// Let the caller know the module was processed
console.log("DEBUG: Program.js module processed - AllLayers ready for use")
Effect.runPromise(Console.log("Program.js: AllLayers initialized successfully"))
