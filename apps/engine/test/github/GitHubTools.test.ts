import { NodeFileSystem } from "@effect/platform-node"
import { describe, expect, it, vi } from "@effect/vitest"
import { Effect, Layer, Ref } from "effect"
import type { AgentState } from "../../src/github/AgentStateTypes.js"
import { GitHubClient } from "../../src/github/GitHub.js"
import { GitHubTools, GitHubToolsLayer, StatefulToolContext } from "../../src/github/GitHubTools.js"
import { MemoryManager } from "../../src/github/MemoryManager.js"
import { PlanManager } from "../../src/github/PlanManager.js"

// Create a test state helper
const getTestState = (): AgentState => ({
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

describe("GitHubTools", () => {
  // Create test environment layers for all tests
  const TestEnvLayer = Layer.mergeAll(
    // Just provide the NodeFileSystem for now, skip config in tests
    NodeFileSystem.layer
  )

  it("should define GitHubTools", () => {
    expect(GitHubTools).toBeDefined()
  })

  it("should have the right class name", () => {
    expect(GitHubTools.fullName).toBe("GitHubTools")
  })

  it.skip("should update state when handlers are successful", async () => {
    // Create initial state and stateRef
    const initialState = getTestState()
    const stateRef = await Effect.runPromise(Ref.make(initialState))

    // Set up mocks for dependencies
    const getIssueMock = vi.fn().mockReturnValue(Effect.succeed({
      title: "Test Issue",
      body: "Test description",
      state: "open",
      labels: [{ name: "bug" }],
      html_url: "https://github.com/user/repo/issues/123"
    }))

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

    const addToolCallToStepMock = vi.fn().mockImplementation((state, stepId, toolCallData) => {
      return Effect.succeed({
        ...state,
        plan: state.plan.map((step: any) =>
          step.id === stepId
            ? { ...step, tool_calls: [...step.tool_calls, { ...toolCallData, timestamp: expect.any(String) }] }
            : step
        )
      })
    })

    // Mock GitHub client
    const mockGitHubClient = {
      getIssue: getIssueMock,
      listIssues: vi.fn().mockReturnValue(Effect.succeed({ issues: [] })),
      getIssueComments: vi.fn().mockReturnValue(Effect.succeed([])),
      createIssueComment: vi.fn().mockReturnValue(Effect.succeed({})),
      getRepository: vi.fn().mockReturnValue(Effect.succeed({ default_branch: "main" })),
      updateIssue: vi.fn().mockReturnValue(Effect.succeed({})),
      loadAgentState: vi.fn().mockReturnValue(Effect.succeed(initialState)),
      createAgentStateForIssue: vi.fn().mockReturnValue(Effect.succeed(initialState)),
      saveAgentState: vi.fn().mockReturnValue(Effect.succeed(initialState)),
      _tag: "GitHubClient" as const
    }

    const MockGitHubClient = Layer.succeed(
      GitHubClient,
      GitHubClient.of(mockGitHubClient as unknown as GitHubClient)
    )

    // Mock memory manager
    const mockMemoryManager = {
      addConversationMessage: vi.fn().mockImplementation((state, _role, _content) => Effect.succeed(state)),
      addKeyDecision: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      addImportantFinding: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      updateScratchpad: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      addToolInvocationLogEntry: addToolInvocationLogEntryMock
    }

    const MockMemoryManager = Layer.succeed(
      MemoryManager,
      MemoryManager.of(mockMemoryManager)
    )

    // Mock plan manager
    const mockPlanManager = {
      addPlanStep: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      updateStepStatus: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      addToolCallToStep: addToolCallToStepMock,
      getCurrentStep: vi.fn().mockImplementation((state) => Effect.succeed(state.plan[0]))
    }

    const MockPlanManager = Layer.succeed(
      PlanManager,
      PlanManager.of(mockPlanManager)
    )

    // Create StatefulToolContext
    const toolContext = {
      stateRef,
      planManager: mockPlanManager,
      memoryManager: mockMemoryManager
    }

    const StatefulToolContextLayer = Layer.succeed(
      StatefulToolContext,
      StatefulToolContext.of(
        toolContext as unknown as {
          readonly stateRef: Ref.Ref<AgentState>
          readonly planManager: PlanManager
          readonly memoryManager: MemoryManager
        }
      )
    )

    // Create a combined layer with all dependencies
    const TestLayer = Layer.mergeAll(
      MockGitHubClient,
      MockMemoryManager,
      MockPlanManager,
      StatefulToolContextLayer,
      TestEnvLayer
    )

    // Get GitHubTools with test layer
    const GitHubToolsWithDeps = Layer.provide(GitHubToolsLayer, TestLayer)

    // Run the test effect
    const testEffect = Effect.gen(function*() {
      const githubTools = yield* GitHubTools

      // Call the GetGitHubIssue handler
      const params = { owner: "user", repo: "repo", issueNumber: 123 }
      const result = yield* githubTools.handlers.GetGitHubIssue(params)

      // Check result
      expect(result).toEqual({
        title: "Test Issue",
        body: "Test description",
        state: "open",
        labels: [{ name: "bug" }],
        html_url: "https://github.com/user/repo/issues/123"
      })

      // Check state was updated
      const finalState = yield* Ref.get(stateRef)

      // Verify the state changes
      expect(finalState.tool_invocation_log.length).toBeGreaterThan(0)
      expect(finalState.metrics.tools_called).toBe(1)
      expect(addToolInvocationLogEntryMock).toHaveBeenCalled()
      expect(addToolCallToStepMock).toHaveBeenCalled()

      return result
    })

    const providedEffect = Effect.provide(testEffect, GitHubToolsWithDeps) as Effect.Effect<any, unknown, never>
    await Effect.runPromise(providedEffect)
  })

  it.skip("should handle errors properly", async () => {
    // Create initial state and stateRef
    const initialState = getTestState()
    const stateRef = await Effect.runPromise(Ref.make(initialState))

    // Simulated error
    const mockError = new Error("API error")

    // Set up mocks for dependencies
    const getIssueMock = vi.fn().mockReturnValue(Effect.fail(mockError))

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

    const addToolCallToStepMock = vi.fn().mockImplementation((state, stepId, toolCallData) => {
      return Effect.succeed({
        ...state,
        plan: state.plan.map((step: any) =>
          step.id === stepId
            ? { ...step, tool_calls: [...step.tool_calls, { ...toolCallData, timestamp: expect.any(String) }] }
            : step
        )
      })
    })

    // Mock GitHub client
    const mockGitHubClient = {
      getIssue: getIssueMock,
      listIssues: vi.fn().mockReturnValue(Effect.fail(mockError)),
      getIssueComments: vi.fn().mockReturnValue(Effect.fail(mockError)),
      createIssueComment: vi.fn().mockReturnValue(Effect.fail(mockError)),
      getRepository: vi.fn().mockReturnValue(Effect.fail(mockError)),
      updateIssue: vi.fn().mockReturnValue(Effect.fail(mockError)),
      loadAgentState: vi.fn().mockReturnValue(Effect.fail(mockError)),
      createAgentStateForIssue: vi.fn().mockReturnValue(Effect.fail(mockError)),
      saveAgentState: vi.fn().mockReturnValue(Effect.fail(mockError)),
      _tag: "GitHubClient" as const
    }

    const MockGitHubClient = Layer.succeed(
      GitHubClient,
      GitHubClient.of(mockGitHubClient as unknown as GitHubClient)
    )

    // Mock memory manager
    const mockMemoryManager = {
      addConversationMessage: vi.fn().mockImplementation((state, _role, _content) => Effect.succeed(state)),
      addKeyDecision: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      addImportantFinding: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      updateScratchpad: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      addToolInvocationLogEntry: addToolInvocationLogEntryMock
    }

    const MockMemoryManager = Layer.succeed(
      MemoryManager,
      MemoryManager.of(mockMemoryManager)
    )

    // Mock plan manager
    const mockPlanManager = {
      addPlanStep: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      updateStepStatus: vi.fn().mockImplementation((state) => Effect.succeed(state)),
      addToolCallToStep: addToolCallToStepMock,
      getCurrentStep: vi.fn().mockImplementation((state) => Effect.succeed(state.plan[0]))
    }

    const MockPlanManager = Layer.succeed(
      PlanManager,
      PlanManager.of(mockPlanManager)
    )

    // Create StatefulToolContext
    const toolContext = {
      stateRef,
      planManager: mockPlanManager,
      memoryManager: mockMemoryManager
    }

    const StatefulToolContextLayer = Layer.succeed(
      StatefulToolContext,
      StatefulToolContext.of(
        toolContext as unknown as {
          readonly stateRef: Ref.Ref<AgentState>
          readonly planManager: PlanManager
          readonly memoryManager: MemoryManager
        }
      )
    )

    // Create a combined layer with all dependencies
    const TestLayer = Layer.mergeAll(
      MockGitHubClient,
      MockMemoryManager,
      MockPlanManager,
      StatefulToolContextLayer,
      TestEnvLayer
    )

    // Get GitHubTools with test layer
    const GitHubToolsWithDeps = Layer.provide(GitHubToolsLayer, TestLayer)

    // Run the test effect
    const testEffect = Effect.gen(function*() {
      const githubTools = yield* GitHubTools

      // Call the GetGitHubIssue handler - should fail
      const params = { owner: "user", repo: "repo", issueNumber: 123 }

      // Use either to catch the error
      const result = yield* Effect.either(githubTools.handlers.GetGitHubIssue(params))
      expect(result._tag).toBe("Left")

      // Check state was updated
      const finalState = yield* Ref.get(stateRef)

      // Verify the state changes on error
      expect(finalState.tool_invocation_log.length).toBeGreaterThan(0)
      expect(finalState.metrics.tools_called).toBe(1)
      expect(finalState.error_state.last_error).not.toBeNull()
      expect(finalState.error_state.consecutive_error_count).toBe(1)
      expect(addToolInvocationLogEntryMock).toHaveBeenCalled()
      expect(addToolCallToStepMock).toHaveBeenCalled()

      return result
    })

    const providedEffect = Effect.provide(testEffect, GitHubToolsWithDeps) as Effect.Effect<any, unknown, never>
    await Effect.runPromise(providedEffect)
  })
})
