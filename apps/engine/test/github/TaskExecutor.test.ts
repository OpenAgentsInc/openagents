import { describe, expect, it, vi } from "@effect/vitest"
import { Completions } from "@effect/ai"
import type { Context } from "effect"
import { Config, Effect, Layer, Stream } from "effect"
import { FileSystem } from "@effect/platform"
import { NodeFileSystem } from "@effect/platform-node/FileSystem"
import type { AgentState } from "../../src/github/AgentStateTypes.js"
import { GitHubClient } from "../../src/github/GitHub.js"
import { GitHubTools } from "../../src/github/GitHubTools.js"
import { MemoryManager } from "../../src/github/MemoryManager.js"
import { PlanManager } from "../../src/github/PlanManager.js"
import { TaskExecutor, TaskExecutorLayer } from "../../src/github/TaskExecutor.js"

// Define the test environment context type
type TestEnv =
  | Context.Tag.Identifier<typeof TaskExecutor>
  | Context.Tag.Identifier<typeof GitHubClient>
  | Context.Tag.Identifier<typeof PlanManager>
  | Context.Tag.Identifier<typeof MemoryManager>
  | Context.Tag.Identifier<typeof GitHubTools>
  | Context.Tag.Identifier<typeof Completions.Completions>

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
  // Create config layers once for all tests
  const GitHubApiKeyLayer = Layer.succeed(
    Config.secret("GITHUB_API_KEY"),
    "test-api-key"
  )
  const AnthropicApiKeyLayer = Layer.succeed(
    Config.secret("ANTHROPIC_API_KEY"),
    "test-api-key"
  )
  
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
          // Find the step by ID in the current state (not using currentStep from outside)
          const stepIndex = state.plan.findIndex((s: { id: string }) => s.id === id)
          if (stepIndex === -1) return Effect.fail(new Error("Mock: Step not found for completed"))
          const originalStep = state.plan[stepIndex]
          // Create updated step based on the step from current state
          const updatedStep = {
            ...originalStep,
            status: "completed", // Set to completed status
            end_time: "2025-04-22T12:02:00Z",
            result_summary: summary
          }
          // Create a new plan array and update the step
          const updatedPlan = [...state.plan]
          updatedPlan[stepIndex] = updatedStep
          // Calculate completed steps count
          const stepsCompleted = updatedPlan.filter((step) => step.status === "completed").length
          // Return only the updated plan and metrics, NOT the current_step_index
          // The step index advancement happens in the TaskExecutor, outside this mock
          return Effect.succeed({
            ...state,
            plan: updatedPlan,
            metrics: { ...state.metrics, steps_completed: stepsCompleted }
          })
        })

      // Create a mock that tracks call count to handle both early and late calls
      let saveCallCount = 0
      const saveAgentStateMock = vi.fn().mockImplementation((state: AgentState) => {
        saveCallCount++

        if (saveCallCount === 1) {
          // First call happens when status is still in_progress
          expect(state.plan[0].status).toBe("in_progress")
          expect(state.current_task.current_step_index).toBe(0) // Not advanced yet
        } else if (saveCallCount === 2) {
          // Final call after step completion
          expect(state.plan[0].status).toBe("completed")
          expect(state.current_task.current_step_index).toBe(1) // Advanced by TaskExecutor
        }

        return Effect.succeed(state) // Return the state received without modifications
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

      // Mock for tool invocation log entries
      const addToolInvocationLogEntryMock = vi.fn().mockImplementation((state, toolCallData) => {
        return Effect.succeed({
          ...state,
          tool_invocation_log: [
            ...state.tool_invocation_log,
            {
              ...toolCallData,
              timestamp: expect.any(String)
            }
          ]
        })
      })

      // Mock for completions service
      const toolkitStreamMock = vi.fn().mockImplementation(() => {
        // Create mock response data
        const textPart = {
          _tag: "Text" as const,
          content: "I'm analyzing the issue. STEP COMPLETED: Analysis complete."
        }

        // Mock stream with just a text part (no tool calls in this test)
        const streamItem = {
          response: {
            parts: [textPart]
          },
          value: { _tag: "None" as const }
        }

        return Stream.make(streamItem)
      })

      // Create a mock completions service
      const mockCompletions = {
        toolkit: vi.fn(),
        toolkitStream: toolkitStreamMock,
        completionStream: vi.fn(),
        completion: vi.fn()
      }

      // Create mock services and their layers
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
          addToolInvocationLogEntry: addToolInvocationLogEntryMock
        })
      )

      const MockCompletions = Layer.succeed(
        Completions.Completions,
        Completions.Completions.of(mockCompletions as unknown as any)
      )

      const MockGitHubTools = Layer.succeed(
        GitHubTools,
        GitHubTools.of({
          tools: { tools: {} } as any,
          handlers: {
            GetGitHubIssue: vi.fn().mockReturnValue(Effect.succeed({ title: "Test Issue" })),
            ListGitHubIssues: vi.fn().mockReturnValue(Effect.succeed({ issues: [] })),
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
        listIssues: vi.fn().mockReturnValue(Effect.succeed({ issues: [] })),
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
        GitHubClient.of(mockGitHubClient)
      )

      // Merge all mock layers into a single test layer
      const TestLayer = Layer.mergeAll(
        MockPlanManager,
        MockGitHubClient,
        MockMemoryManager,
        MockGitHubTools,
        MockCompletions
      )

      // Create a complete TaskExecutor layer with dependencies provided by the TestLayer
      const TaskExecutorWithDeps = Layer.provide(TaskExecutorLayer, TestLayer)

      // Act
      // Use explicit type for the environment
      const effectToTest = Effect.gen(function*() {
        const executor = yield* TaskExecutor
        return yield* executor.executeNextStep(initialState)
      }) as Effect.Effect<AgentState, Error, TestEnv>

      // Use explicit type annotation and type assertion to resolve type issues
      const providedEffect = Effect.provide(effectToTest, TaskExecutorWithDeps)
      // Cast to remove the environment type since all dependencies are provided
      const effectWithNoEnv = providedEffect as Effect.Effect<AgentState, Error, never>
      const result = await Effect.runPromise(effectWithNoEnv)

      // Assert
      expect(getCurrentStepMock).toHaveBeenCalledTimes(1)
      expect(updateStepStatusMock).toHaveBeenCalledTimes(2)
      expect(saveAgentStateMock).toHaveBeenCalledTimes(2)
      expect(toolkitStreamMock).toHaveBeenCalledTimes(1)
      expect(addConversationMessageMock).toHaveBeenCalled()

      // Verify step advancement
      const typedResult = result as AgentState
      expect(typedResult.current_task.current_step_index).toBe(1)
      expect(typedResult.plan[0].status).toBe("completed")
      expect(typedResult.plan[0].result_summary).toBeDefined()
      // Verify saveAgentState was called twice (once after in_progress, once after completion)
      expect(saveAgentStateMock).toHaveBeenCalledTimes(2)
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

      // In the error case test, we're expecting an error state
      const saveAgentStateMock = vi.fn().mockImplementation((state) => {
        // First call happens when status is set to in_progress
        if (state.plan[0].status === "in_progress") {
          expect(state.current_task.current_step_index).toBe(0) // Not advanced
        }
        // Final call happens after error handling
        else if (state.plan[0].status === "error") {
          expect(state.error_state.last_error).not.toBeNull()
          expect(state.error_state.consecutive_error_count).toBe(1)
          expect(state.current_task.current_step_index).toBe(0) // Not advanced
        }

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

      // Mock for completions service that fails
      const toolkitStreamMock = vi.fn().mockImplementation(() => {
        // Create a stream that emits a failure with "STEP FAILED" message
        const textPart = {
          _tag: "Text" as const,
          content: "I'm trying but... STEP FAILED: Unable to proceed due to error."
        }

        return Stream.make({
          response: {
            parts: [textPart]
          },
          value: { _tag: "None" as const }
        })
      })

      // Create a mock completions service
      const mockCompletions = {
        toolkit: vi.fn(),
        toolkitStream: toolkitStreamMock,
        completionStream: vi.fn(),
        completion: vi.fn()
      }

      // Create the mock services
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

      const MockCompletions = Layer.succeed(
        Completions.Completions,
        Completions.Completions.of(mockCompletions as unknown as any)
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
        listIssues: vi.fn().mockReturnValue(Effect.succeed({ issues: [] })),
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
        GitHubClient.of(mockGitHubClient)
      )

      // Create GITHUB_API_KEY and ANTHROPIC_API_KEY config value layers


      
      // Merge all the mock layers


      
      const AllMockLayers = Layer.mergeAll(
        MockPlanManager,
        MockGitHubClient,
        MockMemoryManager,
        MockGitHubTools,
        MockCompletions,
        GitHubApiKeyLayer,
        AnthropicApiKeyLayer
      )

      // Create the TaskExecutor layer with dependencies
      const TaskExecutorWithDeps = Layer.provide(TaskExecutorLayer, AllMockLayers)

      // Act
      // Use explicit type for the environment
      const effectToTest = Effect.gen(function*() {
        const executor = yield* TaskExecutor
        return yield* executor.executeNextStep(initialState)
      }) as Effect.Effect<AgentState, Error, TestEnv>

      // Provide the combined layer to the effect
      const providedEffect = Effect.provide(effectToTest, TaskExecutorWithDeps)
      // Cast to remove the environment type since all dependencies are provided
      const effectWithNoEnv = providedEffect as Effect.Effect<AgentState, Error, never>
      const result = await Effect.runPromise(effectWithNoEnv)

      // Assert
      expect(getCurrentStepMock).toHaveBeenCalledTimes(1)
      expect(updateStepStatusMock).toHaveBeenCalledTimes(2)
      expect(saveAgentStateMock).toHaveBeenCalledTimes(2)
      expect(toolkitStreamMock).toHaveBeenCalledTimes(1)

      // Verify error handling
      const typedResult = result as AgentState
      expect(typedResult.current_task.current_step_index).toBe(0) // Not advanced
      expect(typedResult.plan[0].status).toBe("error")
      expect(typedResult.plan[0].result_summary).toContain("Failed")
      expect(typedResult.error_state.last_error).not.toBeNull()
      expect(typedResult.error_state.consecutive_error_count).toBe(1)
    })
  })
})