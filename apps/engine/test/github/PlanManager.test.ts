import { describe, it, expect } from "@effect/vitest"
import { Effect } from "effect"
import { PlanManager, PlanManagerLayer } from "../../src/github/PlanManager.js"
import type { AgentState, PlanStep } from "../../src/github/AgentStateTypes.js"

// Create a basic fixture for testing
const createTestState = (): AgentState => ({
  agent_info: {
    type: "solver",
    version: "1.0.0",
    instance_id: "test-instance-id",
    state_schema_version: "1.1"
  },
  timestamps: {
    created_at: "2025-04-22T12:00:00Z",
    last_saved_at: "2025-04-22T12:00:00Z",
    last_action_at: "2025-04-22T12:00:00Z"
  },
  current_task: {
    repo_owner: "user",
    repo_name: "repo",
    repo_branch: "main",
    issue_number: 123,
    issue_details_cache: {
      title: "Test Issue",
      description_snippet: "Test description",
      status: "open",
      labels: ["bug"],
      source_url: "https://github.com/user/repo/issues/123"
    },
    status: "planning",
    current_step_index: 0
  },
  plan: [
    {
      id: "step-1",
      step_number: 1,
      description: "Analyze issue",
      status: "pending",
      start_time: null,
      end_time: null,
      result_summary: null,
      tool_calls: []
    }
  ],
  execution_context: {
    current_file_focus: null,
    relevant_code_snippets: [],
    external_references: [],
    files_modified_in_session: []
  },
  tool_invocation_log: [],
  memory: {
    conversation_history: [],
    key_decisions: [],
    important_findings: [],
    scratchpad: ""
  },
  metrics: {
    steps_completed: 0,
    total_steps_in_plan: 1,
    session_start_time: "2025-04-22T12:00:00Z",
    total_time_spent_seconds: 0,
    llm_calls_made: 0,
    llm_tokens_used: {
      prompt: 0,
      completion: 0
    },
    tools_called: 0,
    commits_made: 0
  },
  error_state: {
    last_error: null,
    consecutive_error_count: 0,
    retry_count_for_current_action: 0,
    blocked_reason: null
  },
  configuration: {
    agent_goal: "Resolve issue #123: Test Issue",
    llm_config: {
      model: "claude-3-5-sonnet-latest",
      temperature: 0.7,
      max_tokens: 1024
    },
    max_retries_per_action: 3,
    allowed_actions: ["read_file", "write_file", "run_test", "update_issue"],
    restricted_paths: ["/.git/*", "/secrets/", "/config/prod.json"],
    action_timeout_seconds: 300,
    session_timeout_minutes: 120,
    github_token_available: true
  }
})

describe("PlanManager", () => {
  // Helper to run effects with the PlanManager service
  const runWithPlanManager = <A, E>(effectToRun: (planManager: PlanManager) => Effect.Effect<A, E>) => {
    // Get the actual PlanManager instance from the layer
    const planManager = PlanManagerLayer.build.pipe(
      Effect.map(layer => layer(PlanManager))
    ).pipe(Effect.runSync)
    
    // Run the effect with the actual PlanManager instance
    return Effect.runSync(effectToRun(planManager))
  }

  describe("addPlanStep", () => {
    it("should add a new step to the plan", () => {
      // Arrange
      const initialState = createTestState()
      const initialPlanLength = initialState.plan.length
      const initialTotalSteps = initialState.metrics.total_steps_in_plan
      const description = "Implement fix"

      // Act
      const newState = runWithPlanManager(planManager => planManager.addPlanStep(initialState, description))

      // Assert
      expect(newState.plan.length).toBe(initialPlanLength + 1)
      expect(newState.metrics.total_steps_in_plan).toBe(initialTotalSteps + 1)
      
      const newStep = newState.plan[newState.plan.length - 1]
      expect(newStep.description).toBe(description)
      expect(newStep.step_number).toBe(initialPlanLength + 1)
      expect(newStep.status).toBe("pending")
      expect(newStep.tool_calls).toEqual([])
      expect(newStep.start_time).toBeNull()
      expect(newStep.end_time).toBeNull()
      expect(newStep.result_summary).toBeNull()
      
      // Verify immutability
      expect(initialState).not.toBe(newState)
      expect(initialState.plan).not.toBe(newState.plan)
      expect(initialState.metrics).not.toBe(newState.metrics)
    })
  })

  describe("updateStepStatus", () => {
    it("should update a step status to in_progress", () => {
      // Arrange
      const initialState = createTestState()
      const stepId = initialState.plan[0].id

      // Act
      const newState = runWithPlanManager(planManager => planManager.updateStepStatus(initialState, stepId, "in_progress"))

      // Assert
      const updatedStep = newState.plan[0]
      expect(updatedStep.status).toBe("in_progress")
      expect(updatedStep.start_time).not.toBeNull()
      expect(updatedStep.end_time).toBeNull()
      expect(initialState.plan[0].start_time).toBeNull() // Verify immutability
    })

    it("should update a step status to completed", () => {
      // Arrange
      const initialState = createTestState()
      const stepId = initialState.plan[0].id
      const resultSummary = "Successfully fixed issue"

      // First update to in_progress
      const inProgressState = runWithPlanManager(planManager => 
        planManager.updateStepStatus(initialState, stepId, "in_progress")
      )
      
      // Act - update to completed
      const newState = runWithPlanManager(planManager => 
        planManager.updateStepStatus(inProgressState, stepId, "completed", resultSummary)
      )

      // Assert
      const updatedStep = newState.plan[0]
      expect(updatedStep.status).toBe("completed")
      expect(updatedStep.start_time).not.toBeNull()
      expect(updatedStep.end_time).not.toBeNull()
      expect(updatedStep.result_summary).toBe(resultSummary)
      expect(newState.metrics.steps_completed).toBe(1)
      
      // Verify immutability
      expect(inProgressState).not.toBe(newState)
      expect(inProgressState.plan).not.toBe(newState.plan)
    })

    it("should update a step status to error", () => {
      // Arrange
      const initialState = createTestState()
      const stepId = initialState.plan[0].id
      const errorMessage = "Failed: API timeout"

      // Act
      const newState = runWithPlanManager(planManager => 
        planManager.updateStepStatus(initialState, stepId, "error", errorMessage)
      )

      // Assert
      const updatedStep = newState.plan[0]
      expect(updatedStep.status).toBe("error")
      expect(updatedStep.end_time).not.toBeNull()
      expect(updatedStep.result_summary).toBe(errorMessage)
      expect(newState.metrics.steps_completed).toBe(0) // Errors don't count as completed
    })

    it("should fail when step ID doesn't exist", () => {
      // Arrange
      const initialState = createTestState()
      const nonExistentStepId = "non-existent-step"

      // Act & Assert
      expect(() => {
        runWithPlanManager(planManager => 
          planManager.updateStepStatus(initialState, nonExistentStepId, "completed")
        )
      }).toThrow(/not found/)
    })
  })

  describe("addToolCallToStep", () => {
    it("should add a tool call to a step", () => {
      // Arrange
      const initialState = createTestState()
      const stepId = initialState.plan[0].id
      const toolCallData = {
        tool_name: "fetchFileContents",
        parameters: { path: "src/main.ts" },
        status: "success",
        result_preview: "File contents retrieved",
        full_result_ref: null
      }

      // Act
      const newState = runWithPlanManager(planManager => 
        planManager.addToolCallToStep(initialState, stepId, toolCallData)
      )

      // Assert
      const updatedStep = newState.plan[0]
      expect(updatedStep.tool_calls.length).toBe(1)
      
      const addedToolCall = updatedStep.tool_calls[0]
      expect(addedToolCall.tool_name).toBe(toolCallData.tool_name)
      expect(addedToolCall.parameters).toEqual(toolCallData.parameters)
      expect(addedToolCall.status).toBe(toolCallData.status)
      expect(addedToolCall.result_preview).toBe(toolCallData.result_preview)
      expect(addedToolCall.timestamp).toBeDefined()
      
      // Verify immutability
      expect(initialState).not.toBe(newState)
      expect(initialState.plan).not.toBe(newState.plan)
      expect(initialState.plan[0].tool_calls).not.toBe(newState.plan[0].tool_calls)
    })

    it("should fail when step ID doesn't exist", () => {
      // Arrange
      const initialState = createTestState()
      const nonExistentStepId = "non-existent-step"
      const toolCallData = {
        tool_name: "fetchFileContents",
        parameters: { path: "src/main.ts" },
        status: "success",
        result_preview: "File contents retrieved",
        full_result_ref: null
      }

      // Act & Assert
      expect(() => {
        runWithPlanManager(planManager => 
          planManager.addToolCallToStep(initialState, nonExistentStepId, toolCallData)
        )
      }).toThrow(/not found/)
    })
  })

  describe("getCurrentStep", () => {
    it("should return the current step based on current_step_index", () => {
      // Arrange
      const initialState = createTestState()
      // Create a new state with the required index
      const testState = {
        ...initialState,
        current_task: {
          ...initialState.current_task,
          current_step_index: 0
        }
      }

      // Act
      const step = runWithPlanManager(planManager => planManager.getCurrentStep(testState))

      // Assert
      expect(step).toBe(testState.plan[0])
    })

    it("should fail when current_step_index is invalid", () => {
      // Arrange
      const initialState = createTestState()
      // Create a new state with an invalid index
      const testState = {
        ...initialState,
        current_task: {
          ...initialState.current_task,
          current_step_index: 999 // Invalid index
        }
      }

      // Act & Assert
      expect(() => {
        runWithPlanManager(planManager => planManager.getCurrentStep(testState))
      }).toThrow(/Invalid current_step_index/)
    })
  })
})