import * as path from "node:path"
import { describe, expect, it, vi, beforeEach, afterEach, type Mock } from "@effect/vitest"
import { Effect, Layer } from "effect"
import { 
  GitHubClient, 
  StateNotFoundError, 
  StateParseError, 
  StateValidationError,
  GitHubClientLayer
} from "../../src/github/GitHub.js"
import { FileSystem } from "../../src/github/FileSystem.js"
import type { AgentState } from "../../src/github/AgentStateTypes.js"

// Create a valid test state fixture
const createValidTestState = (): AgentState => ({
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

describe("State Storage", () => {
  let existsSyncMock: Mock
  let mkdirSyncMock: Mock
  let writeFileSyncMock: Mock
  let readFileSyncMock: Mock
  let logWarningSpy: Mock
  
  let MockFileSystemLayer: Layer.Layer<FileSystem>

  beforeEach(() => {
    // Initialize mocks
    existsSyncMock = vi.fn()
    mkdirSyncMock = vi.fn()
    writeFileSyncMock = vi.fn()
    readFileSyncMock = vi.fn()
    logWarningSpy = vi.spyOn(Effect, "logWarning").mockReturnValue(Effect.void)

    // Set default mock behavior
    existsSyncMock.mockReturnValue(true)

    // Create mock FileSystem layer
    MockFileSystemLayer = Layer.succeed(
      FileSystem,
      {
        existsSync: existsSyncMock,
        mkdirSync: mkdirSyncMock,
        writeFileSync: writeFileSyncMock,
        readFileSync: readFileSyncMock
      }
    )
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe("Error Classes", () => {
    it("should have error classes defined with proper inheritance", () => {
      // Verify error classes exist
      expect(StateNotFoundError).toBeDefined()
      expect(StateParseError).toBeDefined()
      expect(StateValidationError).toBeDefined()
      
      // Create instances to verify the inheritance
      const notFoundErr = new StateNotFoundError("test-id")
      const parseErr = new StateParseError("parse error")
      const validationErr = new StateValidationError("validation error")
      
      // Check inheritance and properties
      expect(notFoundErr).toBeInstanceOf(Error)
      expect(notFoundErr.name).toBe("StateNotFoundError")
      expect(notFoundErr.message).toContain("test-id")
      
      expect(parseErr).toBeInstanceOf(Error)
      expect(parseErr.name).toBe("StateParseError")
      expect(parseErr.message).toContain("parse error")
      
      expect(validationErr).toBeInstanceOf(Error)
      expect(validationErr.name).toBe("StateValidationError")
      expect(validationErr.message).toContain("validation error")
    })
  })

  describe("saveAgentState", () => {
    it("should save state to the correct path and update last_saved_at", async () => {
      // Arrange
      const initialState = createValidTestState()
      const expectedPath = path.join(process.cwd(), "state", `${initialState.agent_info.instance_id}.json`)
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => client.saveAgentState(initialState)) as Effect.Effect<AgentState, never, never>,
          testLayer
        )
      )
      
      // Assert
      expect(mkdirSyncMock).toHaveBeenCalledWith(path.join(process.cwd(), "state"), { recursive: true })
      expect(writeFileSyncMock).toHaveBeenCalledWith(
        expectedPath,
        expect.any(String) // We'll verify content separately
      )
      
      // Verify immutability and updated timestamp
      expect(result).not.toBe(initialState)
      expect(result.timestamps.last_saved_at).not.toBe(initialState.timestamps.last_saved_at)
      
      // Verify written content
      const writtenContent = JSON.parse(writeFileSyncMock.mock.calls[0][1] as string)
      expect(writtenContent.agent_info.instance_id).toBe(initialState.agent_info.instance_id)
      expect(writtenContent.timestamps.last_saved_at).not.toBe(initialState.timestamps.last_saved_at)
    })
    
    it("should handle filesystem errors", async () => {
      // Arrange
      const initialState = createValidTestState()
      const mockError = new Error("Filesystem error")
      writeFileSyncMock.mockImplementation(() => { throw mockError })
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.saveAgentState(initialState)) as Effect.Effect<AgentState, never, never>,
            testLayer
          )
        )
      ).rejects.toThrow("Failed to save agent state")
    })
  })
  
  describe("loadAgentState", () => {
    it("should load and validate state successfully", async () => {
      // Arrange
      const validState = createValidTestState()
      const instanceId = validState.agent_info.instance_id
      readFileSyncMock.mockReturnValue(JSON.stringify(validState))
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)) as Effect.Effect<AgentState, never, never>,
          testLayer
        )
      )
      
      // Assert
      expect(existsSyncMock).toHaveBeenCalled()
      expect(readFileSyncMock).toHaveBeenCalledWith(
        path.join(process.cwd(), "state", `${instanceId}.json`),
        "utf-8"
      )
      expect(result).toEqual(validState)
    })
    
    it("should throw StateNotFoundError when file doesn't exist", async () => {
      // Arrange
      const instanceId = "non-existent-id"
      existsSyncMock.mockReturnValue(false)
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)) as Effect.Effect<AgentState, never, never>,
            testLayer
          )
        )
      ).rejects.toBeInstanceOf(StateNotFoundError)
    })
    
    it("should throw StateParseError when JSON is invalid", async () => {
      // Arrange
      const instanceId = "test-instance-id"
      readFileSyncMock.mockReturnValue("{ invalid json")
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)) as Effect.Effect<AgentState, never, never>,
            testLayer
          )
        )
      ).rejects.toBeInstanceOf(StateParseError)
    })
    
    it("should throw StateValidationError when state doesn't match schema", async () => {
      // Arrange
      const instanceId = "test-instance-id"
      const invalidState = { not_valid: "missing required fields" }
      readFileSyncMock.mockReturnValue(JSON.stringify(invalidState))
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)) as Effect.Effect<AgentState, never, never>,
            testLayer
          )
        )
      ).rejects.toBeInstanceOf(StateValidationError)
    })
    
    it("should log warning but succeed when schema version differs", async () => {
      // Arrange
      const oldVersionState = {
        ...createValidTestState(),
        agent_info: {
          ...createValidTestState().agent_info,
          state_schema_version: "1.0"
        }
      }
      const instanceId = oldVersionState.agent_info.instance_id
      readFileSyncMock.mockReturnValue(JSON.stringify(oldVersionState))
      
      // Create the test layer with mock FileSystem
      const testLayer = Layer.provide(GitHubClientLayer, MockFileSystemLayer)
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)) as Effect.Effect<AgentState, never, never>,
          testLayer
        )
      )
      
      // Assert
      expect(logWarningSpy).toHaveBeenCalled()
      expect(result).toEqual(oldVersionState)
    })
  })
  
  describe("createAgentStateForIssue", () => {
    it("should create and save initial state for issue", async () => {
      // Arrange - Create a mock GitHubClient that includes our mock FileSystem
      const mockGitHubClient = {
        getIssue: vi.fn().mockReturnValue(Effect.succeed({
          title: "Test Issue Title",
          body: "Test issue description with details",
          state: "open",
          labels: [{ name: "bug" }, { name: "priority" }],
          html_url: "https://github.com/user/repo/issues/123"
        })),
        saveAgentState: vi.fn().mockImplementation(state => Effect.succeed({
          ...state,
          timestamps: {
            ...state.timestamps,
            last_saved_at: "2025-04-22T12:05:00Z"
          }
        })),
        listIssues: vi.fn().mockReturnValue(Effect.succeed({})),
        getIssueComments: vi.fn().mockReturnValue(Effect.succeed([])),
        createIssueComment: vi.fn().mockReturnValue(Effect.succeed({})),
        getRepository: vi.fn().mockReturnValue(Effect.succeed({})),
        updateIssue: vi.fn().mockReturnValue(Effect.succeed({})),
        loadAgentState: vi.fn().mockReturnValue(Effect.succeed({})),
        createAgentStateForIssue: vi.fn().mockReturnValue(Effect.succeed({})),
        _tag: "GitHubClient" as const
      }
      
      const MockGitHubClientLayer = Layer.succeed(
        GitHubClient,
        mockGitHubClient
      )
      
      // Combine with mock FileSystem
      const testLayer = Layer.provide(MockGitHubClientLayer, MockFileSystemLayer)
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => 
            client.createAgentStateForIssue("user", "repo", 123)
          ) as Effect.Effect<AgentState, never, never>,
          testLayer
        )
      )
      
      // Assert
      expect(mockGitHubClient.getIssue).toHaveBeenCalledWith("user", "repo", 123)
      expect(mockGitHubClient.saveAgentState).toHaveBeenCalled()
      expect(result.agent_info.type).toBe("solver")
      expect(result.agent_info.state_schema_version).toBe("1.1")
      expect(result.current_task.repo_owner).toBe("user")
      expect(result.current_task.repo_name).toBe("repo")
      expect(result.current_task.issue_number).toBe(123)
    })
  })
})