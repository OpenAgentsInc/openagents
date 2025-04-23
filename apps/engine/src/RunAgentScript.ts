import * as dotenv from "dotenv"
import { Console, Effect } from "effect"
import { AllLayers } from "./Program.js" // Use the single source of truth layer
import { GitHubClient, TaskExecutor, PlanManager } from "./Program.js" // Import Tags from Program
import type { AgentState } from "./github/AgentStateTypes.js"

// Load environment variables
dotenv.config()

// --- Configuration ---
const config = {
  owner: process.env.RUN_SCRIPT_OWNER || process.env.GITHUB_REPO_OWNER || "openagentsinc",
  repo: process.env.RUN_SCRIPT_REPO || process.env.GITHUB_REPO_NAME || "openagents",
  issueNumber: parseInt(process.env.RUN_SCRIPT_ISSUE || "796", 10),
  instanceId: process.env.RUN_SCRIPT_INSTANCE_ID, // Optional: load existing state
  maxSteps: parseInt(process.env.RUN_SCRIPT_MAX_STEPS || "20", 10),
  verbose: process.env.RUN_SCRIPT_VERBOSE === "true"
}

// Ensure API Keys are present
if (!process.env.ANTHROPIC_API_KEY) {
  console.error("Error: ANTHROPIC_API_KEY must be set in environment")
  process.exit(1)
}

if (!process.env.GITHUB_TOKEN) {
  console.error("Error: GITHUB_TOKEN must be set in environment")
  process.exit(1)
}

// Helper to log different levels based on verbosity
const log = {
  info: (message: string) => console.log(`[INFO] ${message}`),
  debug: (message: string) => config.verbose && console.log(`[DEBUG] ${message}`),
  error: (message: string) => console.error(`[ERROR] ${message}`),
  success: (message: string) => console.log(`[SUCCESS] ${message}`)
}

// --- Core Agent Execution Effect ---
const agentExecutionPipeline = Effect.gen(function*() {
  log.info(`Starting agent execution for ${config.owner}/${config.repo}#${config.issueNumber}`)
  
  // Check if all required services are available (diagnostic step)
  log.debug("Checking service availability...")
  try {
    yield* PlanManager
    log.debug("PlanManager service is available")
    
    yield* GitHubClient
    log.debug("GitHubClient service is available")
    
    yield* TaskExecutor
    log.debug("TaskExecutor service is available")
    
    log.success("All required services are available")
  } catch (error) {
    log.error(`Service availability check failed: ${error}`)
    return yield* Effect.fail(`ServiceAvailabilityCheckError: ${error}`)
  }

  // Access services for use throughout the pipeline
  const githubClient = yield* GitHubClient
  const taskExecutor = yield* TaskExecutor
  
  // 1. Load or Create Initial State
  let initialState: AgentState
  
  if (config.instanceId) {
    // Try to load existing state if instance ID is provided
    log.info(`Loading existing state with ID: ${config.instanceId}`)
    try {
      initialState = yield* githubClient.loadAgentState(config.instanceId)
      log.success(`Loaded existing state created at ${initialState.timestamps.created_at}`)
      log.debug(`State details: ${JSON.stringify({
        status: initialState.current_task.status,
        currentStepIndex: initialState.current_task.current_step_index,
        totalSteps: initialState.plan.length,
        stepsCompleted: initialState.metrics.steps_completed,
      })}`)
    } catch (error) {
      log.error(`Failed to load state: ${error}`)
      return yield* Effect.fail("StateLoadError")
    }
  } else {
    // Create new state for the specified issue
    log.info(`Creating new state for ${config.owner}/${config.repo}#${config.issueNumber}`)
    try {
      initialState = yield* githubClient.createAgentStateForIssue(
        config.owner,
        config.repo,
        config.issueNumber
      )
      log.success(`Created initial state with ID: ${initialState.agent_info.instance_id}`)
      log.debug(`State details: ${JSON.stringify({
        status: initialState.current_task.status,
        currentStepIndex: initialState.current_task.current_step_index,
        totalSteps: initialState.plan.length,
      })}`)
    } catch (error) {
      log.error(`Failed to create initial state: ${error}`)
      return yield* Effect.fail("InitialStateCreationError")
    }
  }

  let currentState = initialState
  log.info(`Current task status: ${currentState.current_task.status}`)
  log.info(`Current step index: ${currentState.current_task.current_step_index}`)
  log.info(`Total steps in plan: ${currentState.plan.length}`)

  // 2. Execution Loop
  let stepsRun = 0
  
  while (stepsRun < config.maxSteps) {
    stepsRun++
    log.info(`\n--- Executing Step ${currentState.current_task.current_step_index + 1} (Run ${stepsRun}) ---`)
    
    // Check for terminal status before execution
    const status = currentState.current_task.status
    if (status === "completed" || status === "error" || status === "blocked") {
      log.info(`Execution loop terminated. Final Status: ${status}`)
      if (status === "error" || status === "blocked") {
        log.error(`Error State: ${JSON.stringify(currentState.error_state, null, 2)}`)
      }
      break
    }
    
    // Check if plan index is valid
    if (currentState.current_task.current_step_index >= currentState.plan.length) {
      log.info(`Execution loop terminated. Reached end of plan.`)
      // Update status to completed if not already error/blocked
      if (status !== "error" && status !== "blocked") {
        currentState = { 
          ...currentState, 
          current_task: { 
            ...currentState.current_task, 
            status: "completed" 
          } 
        }
      }
      break
    }
    
    // Get the current step information for logging
    const currentStep = currentState.plan[currentState.current_task.current_step_index]
    log.info(`Executing step ${currentStep.step_number}: ${currentStep.description}`)
    log.debug(`Step details: ${JSON.stringify({
      id: currentStep.id,
      status: currentStep.status,
      toolCalls: currentStep.tool_calls.length 
    })}`)
    
    // Execute the next step
    try {
      log.debug(`Calling taskExecutor.executeNextStep - Starting`)
      const nextState = yield* taskExecutor.executeNextStep(currentState)
      log.debug(`taskExecutor.executeNextStep completed successfully`)
      
      // Update state for the next iteration
      currentState = nextState
      
      // Get the executed step result summary if available
      const executedStep = currentState.plan[Math.max(0, currentState.current_task.current_step_index - 1)]
      const resultSummary = executedStep?.result_summary || "No result summary available"
      
      log.success(`Step execution completed. New status: ${currentState.current_task.status}`)
      log.debug(`Step result: ${resultSummary}`)
      
      // If step execution advanced the index to the end of the plan and completed the task,
      // break out of the loop to avoid unnecessary iterations
      if (currentState.current_task.current_step_index >= currentState.plan.length &&
          currentState.current_task.status === "completed") {
        log.info(`All steps completed. Execution finished successfully.`)
        break
      }
    } catch (error) {
      log.error(`Runtime error during executeNextStep (Loop ${stepsRun}): ${error}`)
      
      // Attempt to save state even after error, potentially updating error fields
      const errorState = {
        ...currentState, // Use state *before* the failed step attempt
        current_task: { ...currentState.current_task, status: "error" as const },
        error_state: {
          ...currentState.error_state,
          last_error: { 
            timestamp: new Date().toISOString(), 
            message: `Unhandled runtime error: ${error}`, 
            type: "internal" as const, 
            details: String(error) 
          },
          consecutive_error_count: currentState.error_state.consecutive_error_count + 1
        }
      }
      
      try {
        yield* githubClient.saveAgentState(errorState)
        log.debug(`Saved error state for instance ${errorState.agent_info.instance_id}`)
        currentState = errorState
      } catch (saveError) {
        log.error(`Failed to save error state: ${saveError}`)
      }
      
      break // Exit the execution loop after an error
    }
  }
  
  if (stepsRun >= config.maxSteps) {
    log.info(`Execution loop terminated after reaching max steps (${config.maxSteps}).`)
    
    // Update status to indicate max steps reached if not already in terminal state
    if (currentState.current_task.status !== "completed" && 
        currentState.current_task.status !== "error" && 
        currentState.current_task.status !== "blocked") {
      
      currentState = {
        ...currentState,
        current_task: { 
          ...currentState.current_task,
          status: "blocked" as const
        },
        error_state: {
          ...currentState.error_state,
          blocked_reason: `Maximum execution steps (${config.maxSteps}) reached`
        }
      }
      
      // Save the updated state
      try {
        yield* githubClient.saveAgentState(currentState)
        log.debug(`Saved state with max steps reached for instance ${currentState.agent_info.instance_id}`)
      } catch (saveError) {
        log.error(`Failed to save final state after max steps: ${saveError}`)
      }
    }
  }
  
  // Ensure final state is saved regardless of how we exited the loop
  try {
    const finalTimestamp = new Date().toISOString()
    
    // Update timestamps for the final save
    currentState = {
      ...currentState,
      timestamps: {
        ...currentState.timestamps,
        last_action_at: finalTimestamp,
        last_saved_at: finalTimestamp
      }
    }
    
    yield* githubClient.saveAgentState(currentState)
    log.debug(`Saved final state for instance ${currentState.agent_info.instance_id}`)
  } catch (saveError) {
    log.error(`Failed to save final state: ${saveError}`)
  }
  
  // 3. Final Summary
  log.info(`\n--- Agent Execution Finished ---`)
  log.info(`Final State ID: ${currentState.agent_info.instance_id}`)
  log.info(`Final Status: ${currentState.current_task.status}`)
  log.info(`Steps Completed: ${currentState.metrics.steps_completed}/${currentState.metrics.total_steps_in_plan}`)
  log.info(`LLM Calls Made: ${currentState.metrics.llm_calls_made}`)
  log.info(`Tools Called: ${currentState.metrics.tools_called}`)
  
  return currentState // Return final state
})

// --- Run the Script ---
log.info("Starting agent execution script...")
log.debug(`Configuration: ${JSON.stringify(config, null, 2)}`)

const runPipeline = async () => {
  try {
    // Cast to any to bypass the type checking issue with Effect.runPromise
    const providedEffect = Effect.provide(agentExecutionPipeline, AllLayers)
      .pipe(
        Effect.tapBoth({
          onFailure: (error) => Console.error(`Pipeline Failed: ${error}`),
          onSuccess: (finalState) => Console.log(`Pipeline Succeeded. Final Status: ${finalState.current_task.status}`)
        })
      );
    
    const result = await (Effect.runPromise as any)(providedEffect);
    
    log.info("Script finished successfully.");
    process.exit(0);
    return result;
  } catch (err: any) {
    log.error(`Script failed with unhandled error: ${err}`);
    process.exit(1);
    throw err; // To satisfy TypeScript
  }
};

// Execute the pipeline
runPipeline();