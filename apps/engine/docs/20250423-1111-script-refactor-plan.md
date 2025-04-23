# Script-Based Agent Execution Refactoring Plan

## 1. Executive Summary

This document outlines a comprehensive plan to implement a script-based approach to agent execution, addressing the persistent "Service not found: PlanManager" runtime error within the `Effect.runFork` context of Server.ts. This plan builds upon the proposed solution in `docs/20250423-next.md` and integrates findings from our technical investigation to create a reliable execution path that will unblock development of core agent functionality.

The refactoring will create a new entry point (`src/RunAgentScript.ts`) that bypasses the problematic server context while reusing the existing layers, services, and state management system. The script will provide a controlled environment for testing and developing agent capabilities independent of web server complexities.

## 2. Problem Background

### 2.1 Current Issue

The OpenAgents Engine is experiencing a persistent error when attempting to execute the agent pipeline via the `/fetch-issue` endpoint in `src/Server.ts`:

```
Error: Service not found: PlanManager
```

This occurs despite:

1. Confirming the `PlanManager` Tag identity is consistent across modules
2. Fixing TypeScript build configuration issues
3. Verifying that `AllLayers` composition includes `PlanManagerLayer`
4. Successfully accessing `PlanManager` in isolation
5. Successfully accessing `PlanManager` directly in non-forked contexts

The most plausible hypothesis is a subtle runtime issue with Context propagation or resolution specifically within the `Effect.runFork` boundary, preventing the `PlanManager` service from being properly resolved when needed within the forked pipeline.

### 2.2 Solution Approach

Rather than spending additional time debugging the deeply nested Effect runtime behavior, we will implement a standalone script that:

1. Executes using the standard `Effect.runPromise` instead of `Effect.runFork`
2. Reuses the same `AllLayers` composition for service provision
3. Implements a similar execution flow to the server endpoint, but in a more linear fashion
4. Provides more detailed logging and error handling
5. Can be run directly from the command line with environment variables

This approach will isolate the core agent execution logic from the server-specific context that's causing issues, while maintaining full functionality.

## 3. Implementation Plan

### 3.1 New Files

1. **`src/RunAgentScript.ts`**
   - Main entry point for script-based execution
   - Command-line interface with environment variable configuration
   - Implements the agent execution pipeline

2. **`src/RunAgentScriptUtils.ts`** (Optional)
   - Helper functions and utilities specific to the script execution
   - Error handling, state validation, and logging helpers

### 3.2 Implementation Details: `RunAgentScript.ts`

```typescript
// src/RunAgentScript.ts
import * as dotenv from "dotenv"
import { Console, Effect, Layer } from "effect"
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
    const planManager = yield* PlanManager
    log.debug("PlanManager service is available")
    
    const githubClient = yield* GitHubClient
    log.debug("GitHubClient service is available")
    
    const taskExecutor = yield* TaskExecutor
    log.debug("TaskExecutor service is available")
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
      // Optionally update status to completed if not already error/blocked
      if (status !== "error" && status !== "blocked") {
        currentState = { 
          ...currentState, 
          current_task: { 
            ...currentState.current_task, 
            status: "completed" 
          } 
        }
        yield* githubClient.saveAgentState(currentState) // Save final state
      }
      break
    }
    
    // Get the current step information for logging
    const currentStep = currentState.plan[currentState.current_task.current_step_index]
    log.info(`Executing step ${currentStep.step_number}: ${currentStep.description}`)
    
    // Execute the next step
    try {
      log.debug(`Calling taskExecutor.executeNextStep - Starting`)
      const nextState = yield* taskExecutor.executeNextStep(currentState)
      log.debug(`taskExecutor.executeNextStep completed successfully`)
      
      // Update state for the next iteration
      currentState = nextState
      log.success(`Step execution completed. New status: ${currentState.current_task.status}`)
      
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

Effect.provide(agentExecutionPipeline, AllLayers).pipe(
  Effect.tapBoth({
    onFailure: (error) => Console.error(`Pipeline Failed: ${error}`),
    onSuccess: (finalState) => Console.log(`Pipeline Succeeded. Final Status: ${finalState.current_task.status}`)
  }),
  Effect.runPromise // Execute the entire pipeline as a Promise
).then(() => {
  log.info("Script finished successfully.")
  process.exit(0)
}).catch(err => {
  log.error(`Script failed with unhandled error: ${err}`)
  process.exit(1)
})
```

### 3.3 Package.json Script Additions

Add the following to the `"scripts"` section in `package.json`:

```json
"scripts": {
  // existing scripts...
  "run-agent": "pnpm build-esm && node build/esm/RunAgentScript.js",
  "run-agent:debug": "pnpm build-esm && RUN_SCRIPT_VERBOSE=true node build/esm/RunAgentScript.js",
  "run-agent:dev": "tsx src/RunAgentScript.ts"
}
```

## 4. Testing and Verification Plan

### 4.1 Testing Steps

1. **Build Verification**
   - Verify that `RunAgentScript.ts` compiles correctly with `pnpm build-esm`
   - Confirm the output JavaScript file is created at `build/esm/RunAgentScript.js`
   - Run `./build-diagnostics.sh` to ensure all necessary files are compiled

2. **Basic Execution Test**
   - Test running the script with a simple, known issue: `pnpm run-agent`
   - Verify the script connects to GitHub API and loads/creates state
   - Check for any "Service not found" errors in the output
   - Confirm the script completes without crashing

3. **Service Resolution Test**
   - Add specific debug output to verify each service is accessible
   - If `PlanManager` is not available, confirm that this occurs in the script as well
   - If `PlanManager` *is* available in this context but not in `Server.ts`, this confirms our hypothesis about `Effect.runFork`

4. **State Persistence Test**
   - Verify that state files are created in the `state/` directory
   - Run the script with `RUN_SCRIPT_INSTANCE_ID` set to an existing state file
   - Confirm that execution continues from saved state

5. **Full Pipeline Test**
   - Test a complete run on a real issue, tracking progress through each step
   - Verify that AI calls are made correctly
   - Verify that tool calls are logged in state

### 4.2 Success Criteria

1. **Primary Success Criterion:**
   - The script executes without "Service not found" errors
   - The agent state progresses through at least one plan step
   - State is correctly saved to disk

2. **Secondary Success Criteria:**
   - Script can be run multiple times with state persistence/loading
   - Agent can execute tools and AI calls as expected
   - Error handling works correctly for various failure scenarios

## 5. Implementation Strategy

### 5.1 Implementation Phases

1. **Phase 1: Core Script Setup (4 hours)**
   - Create `RunAgentScript.ts` with basic structure
   - Add configuration handling and command-line arguments
   - Implement initial pipeline without step execution

2. **Phase 2: Execution Loop Implementation (4 hours)**
   - Add step execution logic
   - Implement state transitions and validations
   - Create robust error handling

3. **Phase 3: Testing and Debugging (4 hours)**
   - Test against real GitHub issues
   - Debug any service resolution or execution issues
   - Document successful paths and remaining issues

4. **Phase 4: Finalization (2 hours)**
   - Add detailed logging for operational use
   - Document the script's usage and limitations
   - Create examples for common use cases

### 5.2 Fallback Plan

If service resolution issues persist even in the script environment, we will:

1. Create a simplified, minimal reproduction of the service resolution error
2. Consider a more drastic refactoring to avoid the Effect.js dependency injection system
3. Implement a direct function composition approach rather than using Effect Tags and Layers

## 6. Future Enhancements

After implementing the initial script:

1. **Advanced Command Line Interface**
   - Add commands for different agent operations
   - Support for detailed output formatting
   - JSON report generation

2. **Integration with Test Framework**
   - Create specific test utilities based on the script
   - Enable automated testing of agent behavior

3. **Reintegration with Server**
   - Apply lessons learned to fix the server implementation
   - Share code between script and server where possible
   - Determine if specific Effect.js patterns should be avoided

## 7. Conclusion

This script-based agent execution approach addresses the immediate blocker preventing development of core agent functionality. By bypassing the problematic server context while reusing existing services and layers, we create a path forward that:

1. **Unblocks Development:** Allows continued work on core agent logic without waiting for a full resolution of the complex service resolution error
2. **Validates Layers:** Confirms whether the issue is specific to the `Effect.runFork` context
3. **Creates a Useful Tool:** Provides a CLI for running agent tasks that will remain valuable even after fixing the server
4. **Improves Testability:** Makes it easier to create predictable, repeatable tests of agent behavior

The implementation approach outlined here is focused on minimizing new code while creating a reliable execution path. The same services, layers, and agent state model will be used, ensuring the script accurately reflects the intended behavior of the server-based execution.