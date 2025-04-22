import type { AgentState } from "./AgentStateTypes.js"
import { Effect, Layer, Either } from "effect"
import { PlanManager } from "./PlanManager.js"
import { GitHubClient } from "./GitHub.js"

/**
 * Service for executing tasks and managing agent state during execution
 */
export interface TaskExecutor {
  /**
   * Execute the next step in the agent's plan
   * @param currentState Current agent state
   * @returns Updated agent state after step execution
   */
  readonly executeNextStep: (currentState: AgentState) => Effect.Effect<AgentState, Error>
}

/**
 * Effect Tag for the TaskExecutor service
 */
export class TaskExecutor extends Effect.Tag("TaskExecutor")<
  TaskExecutor,
  {
    executeNextStep: (currentState: AgentState) => Effect.Effect<AgentState, Error>
  }
>() {}

/**
 * Layer that provides the TaskExecutor implementation
 */
export const TaskExecutorLayer = Layer.effect(
  TaskExecutor,
  Effect.gen(function*(_) {
    // Get dependencies from the context
    const planManager = yield* PlanManager
    const githubClient = yield* GitHubClient

    return {
      executeNextStep: (currentState: AgentState) => Effect.gen(function*() {
        // 1. Get current step
        const currentStep = yield* planManager.getCurrentStep(currentState)
        yield* Effect.logInfo(`Executing step ${currentStep.step_number}: ${currentStep.description}`)

        // 2. Update status to in_progress
        let workingState = yield* planManager.updateStepStatus(currentState, currentStep.id, "in_progress")
        
        // 3. Simulate work (placeholder)
        // In a real implementation, this would involve AI calls, tool calls, etc.
        const workEffect = Effect.logDebug("Simulating step work...")
          // .pipe(Effect.delay("10ms")) // Optional small delay for testing
          // .pipe(Effect.flatMap(() => Effect.fail(new Error("Simulated step failure!")))) // Uncomment to test failure path
          .pipe(Effect.map(() => ({ success: true, resultSummary: "Step simulated successfully." }))) // Simulate success
        
        const result = yield* Effect.either(workEffect) // Capture result/error
        
        // 4. Update based on result
        if (Either.isRight(result)) {
          // SUCCESS PATH
          yield* Effect.logInfo(`Step ${currentStep.step_number} completed successfully.`)
          // Update status to completed
          workingState = yield* planManager.updateStepStatus(
            workingState, 
            currentStep.id, 
            "completed", 
            result.right.resultSummary
          )
          // Advance step index
          workingState = {
            ...workingState,
            current_task: {
              ...workingState.current_task,
              current_step_index: workingState.current_task.current_step_index + 1
            }
          }
        } else {
          // FAILURE PATH
          const error = result.left as Error
          yield* Effect.logError(`Step ${currentStep.step_number} failed: ${error.message}`)
          // Update status to error
          workingState = yield* planManager.updateStepStatus(
            workingState, 
            currentStep.id, 
            "error", 
            `Failed: ${error.message}`
          )
          // Update error_state
          const now = new Date().toISOString()
          workingState = {
            ...workingState,
            error_state: {
              ...workingState.error_state,
              last_error: {
                timestamp: now,
                message: error.message,
                type: "internal", // Or determine type based on error
                details: error.stack ?? ""
              },
              consecutive_error_count: workingState.error_state.consecutive_error_count + 1
              // retry_count_for_current_action handled elsewhere
            }
          }
          // Note: Not advancing index on error
        }
        
        // 5. Save the final state
        yield* githubClient.saveAgentState(workingState)
        yield* Effect.logInfo(`Agent state saved for instance ${workingState.agent_info.instance_id}`)
        
        // 6. Return the final state
        return workingState
      })
    }
  })
)