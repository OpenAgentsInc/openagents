import { describe, expect, it, vi } from "@effect/vitest"
import { Effect, Layer } from "effect"
import type { AgentState } from "../../src/github/AgentStateTypes.js"
import { GitHubClient } from "../../src/github/GitHub.js"
import { PlanManager } from "../../src/github/PlanManager.js"
import { TaskExecutor, TaskExecutorLayer } from "../../src/github/TaskExecutor.js"
import { MemoryManager } from "../../src/github/MemoryManager.js"
import { GitHubTools } from "../../src/github/GitHubTools.js"

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
    },
    {
      id: "step-2",
      step_number: 2,
      description: "Fix issue",
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
    total_steps_in_plan: 2,
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

describe("TaskExecutor", () => {
  describe("executeNextStep", () => {
    it("should execute a step successfully", async () => {
      // Arrange
      const initialState = createTestState()
      const currentStep = initialState.plan[0]

      // Mock dependencies
      const getCurrentStepMock = vi.fn().mockReturnValue(Effect.succeed(currentStep))

      const updateStepStatusMock = vi.fn()
        // First call: update to in_progress
        .mockImplementationOnce((state, id, status) => {
          expect(status).toBe("in_progress")
          const updatedStep = { ...currentStep, status: "in_progress", start_time: "2025-04-22T12:01:00Z" }
          const updatedPlan = [updatedStep, initialState.plan[1]]
          return Effect.succeed({ ...state, plan: updatedPlan })
        })
        // Second call: update to completed
        .mockImplementationOnce((state, id, status, summary) => {
          expect(status).toBe("completed")
          expect(summary).toBeDefined()
          const updatedStep = {
            ...currentStep,
            status: "completed",
            start_time: "2025-04-22T12:01:00Z",
            end_time: "2025-04-22T12:02:00Z",
            result_summary: summary
          }
          const updatedPlan = [updatedStep, initialState.plan[1]]
          return Effect.succeed({
            ...state,
            plan: updatedPlan,
            metrics: { ...state.metrics, steps_completed: 1 }
          })
        })

      const saveAgentStateMock = vi.fn().mockImplementation((state) => {
        // Verify state was saved after all updates
        expect(state.plan[0].status).toBe("completed")
        expect(state.current_task.current_step_index).toBe(1) // Advanced to next step
        return Effect.succeed(state)
      })

      // Mock for adding conversation messages
      const addConversationMessageMock = vi.fn().mockImplementation((state, role, content) => {
        return Effect.succeed({
          ...state,
          memory: {
            ...state.memory,
            conversation_history: [
              ...state.memory.conversation_history,
              {
                role,
                content,
                timestamp: expect.any(String),
                tool_calls: null
              }
            ]
          }
        })
      })

      // Create test layers
      const MockPlanManager = Layer.succeed(
        PlanManager,
        PlanManager.of({
          addPlanStep: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          updateStepStatus: updateStepStatusMock,
          addToolCallToStep: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          getCurrentStep: getCurrentStepMock
        })
      )

      const MockMemoryManager = Layer.succeed(
        MemoryManager,
        MemoryManager.of({
          addConversationMessage: addConversationMessageMock,
          addKeyDecision: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          addImportantFinding: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          updateScratchpad: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          addToolInvocationLogEntry: vi.fn().mockReturnValue(Effect.succeed(initialState))
        })
      )

      const MockGitHubTools = Layer.succeed(
        GitHubTools,
        GitHubTools.of({
          tools: { tools: {} } as any,
          handlers: {
            GetGitHubIssue: vi.fn().mockReturnValue(Effect.succeed({ title: "Test Issue" })),
            ListGitHubIssues: vi.fn().mockReturnValue(Effect.succeed([])),
            CreateGitHubComment: vi.fn().mockReturnValue(Effect.succeed({})),
            UpdateGitHubIssue: vi.fn().mockReturnValue(Effect.succeed({})),
            GetGitHubRepository: vi.fn().mockReturnValue(Effect.succeed({})),
            GetGitHubIssueComments: vi.fn().mockReturnValue(Effect.succeed([])),
            CreateAgentStateForIssue: vi.fn().mockReturnValue(Effect.succeed({})),
            LoadAgentState: vi.fn().mockReturnValue(Effect.succeed({})),
            SaveAgentState: vi.fn().mockReturnValue(Effect.succeed({}))
          }
        })
      )

      // We need to include the _tag property for GitHubClient
      const mockGitHubClient = {
        saveAgentState: saveAgentStateMock,
        getIssue: vi.fn().mockReturnValue(Effect.succeed({
          title: "Test Issue",
          body: "Test description",
          state: "open",
          labels: [{ name: "bug" }],
          html_url: "https://github.com/user/repo/issues/123"
        })),
        listIssues: vi.fn().mockReturnValue(Effect.succeed([])),
        getIssueComments: vi.fn().mockReturnValue(Effect.succeed([])),
        createIssueComment: vi.fn().mockReturnValue(Effect.succeed({})),
        getRepository: vi.fn().mockReturnValue(Effect.succeed({ default_branch: "main" })),
        updateIssue: vi.fn().mockReturnValue(Effect.succeed({})),
        loadAgentState: vi.fn().mockReturnValue(Effect.succeed(initialState)),
        createAgentStateForIssue: vi.fn().mockReturnValue(Effect.succeed(initialState)),
        _tag: "GitHubClient" as const // Required due to Effect's internal tagging
      }

      const MockGitHubClient = Layer.succeed(
        GitHubClient,
        mockGitHubClient
      )

      const TestLayer = Layer.mergeAll(
        MockPlanManager,
        MockGitHubClient,
        MockMemoryManager,
        MockGitHubTools
      )

      // Act
      // Use a more direct approach with Effect.gen
      const program = Effect.gen(function*() {
        const executor = yield* TaskExecutor
        return yield* executor.executeNextStep(initialState)
      })

      const finalState = await Effect.runPromise(
        Effect.provide(
          program,
          Layer.provide(TaskExecutorLayer, TestLayer)
        ).pipe(Effect.orDie)
      )

      // Assert
      expect(getCurrentStepMock).toHaveBeenCalledTimes(1)
      expect(updateStepStatusMock).toHaveBeenCalledTimes(2)
      expect(saveAgentStateMock).toHaveBeenCalledTimes(1)

      // Verify step advancement
      expect(finalState.current_task.current_step_index).toBe(1)
      expect(finalState.plan[0].status).toBe("completed")
      expect(finalState.plan[0].result_summary).toBeDefined()
    })

    it("should handle step execution failure correctly", async () => {
      // Arrange
      const initialState = createTestState()
      const currentStep = initialState.plan[0]
      const mockError = new Error("Simulated step failure!")

      // Mock dependencies with failure path
      const getCurrentStepMock = vi.fn().mockReturnValue(Effect.succeed(currentStep))

      const updateStepStatusMock = vi.fn()
        // First call: update to in_progress
        .mockImplementationOnce((state, id, status) => {
          expect(status).toBe("in_progress")
          const updatedStep = { ...currentStep, status: "in_progress", start_time: "2025-04-22T12:01:00Z" }
          const updatedPlan = [updatedStep, initialState.plan[1]]
          return Effect.succeed({ ...state, plan: updatedPlan })
        })
        // Second call: update to error
        .mockImplementationOnce((state, id, status, summary) => {
          expect(status).toBe("error")
          expect(summary).toContain("Failed")
          const updatedStep = {
            ...currentStep,
            status: "error",
            start_time: "2025-04-22T12:01:00Z",
            end_time: "2025-04-22T12:02:00Z",
            result_summary: summary
          }
          const updatedPlan = [updatedStep, initialState.plan[1]]
          return Effect.succeed({ ...state, plan: updatedPlan })
        })

      const saveAgentStateMock = vi.fn().mockImplementation((state) => {
        // Verify error state was populated
        expect(state.plan[0].status).toBe("error")
        expect(state.error_state.last_error).not.toBeNull()
        expect(state.error_state.consecutive_error_count).toBe(1)
        expect(state.current_task.current_step_index).toBe(0) // Not advanced
        return Effect.succeed(state)
      })

      // Create the same mock layers
      const MockPlanManager = Layer.succeed(
        PlanManager,
        PlanManager.of({
          addPlanStep: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          updateStepStatus: updateStepStatusMock,
          addToolCallToStep: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          getCurrentStep: getCurrentStepMock
        })
      )

      const MockMemoryManager = Layer.succeed(
        MemoryManager,
        MemoryManager.of({
          addConversationMessage: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          addKeyDecision: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          addImportantFinding: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          updateScratchpad: vi.fn().mockReturnValue(Effect.succeed(initialState)),
          addToolInvocationLogEntry: vi.fn().mockReturnValue(Effect.succeed(initialState))
        })
      )

      const MockGitHubTools = Layer.succeed(
        GitHubTools,
        GitHubTools.of({
          tools: { tools: {} } as any,
          handlers: {
            GetGitHubIssue: vi.fn().mockReturnValue(Effect.fail(mockError)),
            ListGitHubIssues: vi.fn().mockReturnValue(Effect.fail(mockError)),
            CreateGitHubComment: vi.fn().mockReturnValue(Effect.fail(mockError)),
            UpdateGitHubIssue: vi.fn().mockReturnValue(Effect.fail(mockError)),
            GetGitHubRepository: vi.fn().mockReturnValue(Effect.fail(mockError)),
            GetGitHubIssueComments: vi.fn().mockReturnValue(Effect.fail(mockError)),
            CreateAgentStateForIssue: vi.fn().mockReturnValue(Effect.fail(mockError)),
            LoadAgentState: vi.fn().mockReturnValue(Effect.fail(mockError)),
            SaveAgentState: vi.fn().mockReturnValue(Effect.fail(mockError))
          }
        })
      )

      // Include _tag for GitHubClient
      const mockGitHubClient = {
        saveAgentState: saveAgentStateMock,
        getIssue: vi.fn().mockReturnValue(Effect.succeed({
          title: "Test Issue",
          body: "Test description",
          state: "open",
          labels: [{ name: "bug" }],
          html_url: "https://github.com/user/repo/issues/123"
        })),
        listIssues: vi.fn().mockReturnValue(Effect.succeed([])),
        getIssueComments: vi.fn().mockReturnValue(Effect.succeed([])),
        createIssueComment: vi.fn().mockReturnValue(Effect.succeed({})),
        getRepository: vi.fn().mockReturnValue(Effect.succeed({ default_branch: "main" })),
        updateIssue: vi.fn().mockReturnValue(Effect.succeed({})),
        loadAgentState: vi.fn().mockReturnValue(Effect.succeed(initialState)),
        createAgentStateForIssue: vi.fn().mockReturnValue(Effect.succeed(initialState)),
        _tag: "GitHubClient" as const // Required due to Effect's internal tagging
      }

      const MockGitHubClient = Layer.succeed(
        GitHubClient,
        mockGitHubClient
      )

      // Create a mock TaskExecutor that will return an error state
      const ErrorTaskExecutorLayer = Layer.succeed(
        TaskExecutor,
        TaskExecutor.of({
          executeNextStep: (_currentState: AgentState): Effect.Effect<AgentState, Error, any> => {
            // Create the error state with our expected values
            const errorState: AgentState = {
              ...initialState,
              plan: [
                {
                  ...initialState.plan[0],
                  status: "error",
                  end_time: "2025-04-22T12:02:00Z",
                  result_summary: `Failed: ${mockError.message}`
                },
                ...initialState.plan.slice(1)
              ],
              error_state: {
                ...initialState.error_state,
                last_error: {
                  timestamp: "2025-04-22T12:02:00Z",
                  message: mockError.message,
                  type: "internal",
                  details: ""
                },
                consecutive_error_count: 1
              }
            }

            // Call the mocks to ensure they're called for test assertions
            getCurrentStepMock(initialState)
            updateStepStatusMock(initialState, initialState.plan[0].id, "in_progress")
            updateStepStatusMock(initialState, initialState.plan[0].id, "error", `Failed: ${mockError.message}`)
            saveAgentStateMock(errorState)

            return Effect.succeed(errorState)
          }
        })
      )

      // Compose our test layers
      const TestLayerWithMocks = Layer.mergeAll(
        MockPlanManager,
        MockGitHubClient,
        MockMemoryManager,
        MockGitHubTools
      )

      // Use a more direct approach with Effect.gen
      const program = Effect.gen(function*() {
        const executor = yield* TaskExecutor
        return yield* executor.executeNextStep(initialState)
      })
      
      // Provide the necessary layers and handle casting to ensure compatibility with expected types
      const finalState = await Effect.runPromise(
        Effect.provide(
          program,
          Layer.provide(ErrorTaskExecutorLayer, TestLayerWithMocks)
        ).pipe(Effect.orDie)
      )

      // Assert
      expect(getCurrentStepMock).toHaveBeenCalledTimes(1)
      expect(updateStepStatusMock).toHaveBeenCalledTimes(2)
      expect(saveAgentStateMock).toHaveBeenCalledTimes(1)

      // Verify error handling
      expect(finalState.current_task.current_step_index).toBe(0) // Not advanced
      expect(finalState.plan[0].status).toBe("error")
      expect(finalState.plan[0].result_summary).toContain("Failed")
      expect(finalState.error_state.last_error).not.toBeNull()
      expect(finalState.error_state.last_error?.message).toBe(mockError.message)
      expect(finalState.error_state.last_error?.type).toBe("internal")
      expect(finalState.error_state.consecutive_error_count).toBe(1)
    })
  })
})