import * as path from "node:path"
import { describe, expect, it, vi, beforeEach, afterEach } from "@effect/vitest"
import { Effect } from "effect"
import { 
  GitHubClient, 
  StateNotFoundError, 
  StateParseError, 
  StateValidationError,
  GitHubClientLayer
} from "../../src/github/GitHub.js"
import type { AgentState } from "../../src/github/AgentStateTypes.js"
import * as fs from "node:fs"

// Mock the filesystem module
vi.mock("node:fs", () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  writeFileSync: vi.fn(),
  readFileSync: vi.fn()
}))

// Mock the Effect.logWarning function
const logWarningSpy = vi.spyOn(Effect, 'logWarning').mockImplementation(() => Effect.void)

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
  // Helper to get test state path - removed unused helper
  // const getTestStatePath = (instanceId: string) => path.join(process.cwd(), "state", `${instanceId}.json`)

  beforeEach(() => {
    vi.clearAllMocks()
    // Default mock for existsSync
    vi.mocked(fs.existsSync).mockReturnValue(true)
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  it("should have error classes defined", () => {
    expect(StateNotFoundError).toBeDefined()
    expect(StateParseError).toBeDefined()
    expect(StateValidationError).toBeDefined()
  })

  describe("saveAgentState", () => {
    it("should save state to the correct path and update last_saved_at", async () => {
      // Arrange
      const initialState = createValidTestState()
      const expectedPath = path.join(process.cwd(), "state", `${initialState.agent_info.instance_id}.json`)
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => client.saveAgentState(initialState)),
          GitHubClientLayer
        )
      )
      
      // Assert
      expect(fs.mkdirSync).toHaveBeenCalledWith(path.join(process.cwd(), "state"), { recursive: true })
      expect(fs.writeFileSync).toHaveBeenCalledWith(
        expectedPath,
        expect.any(String) // We'll verify content separately
      )
      
      // Verify immutability and updated timestamp
      expect(result).not.toBe(initialState)
      expect(result.timestamps.last_saved_at).not.toBe(initialState.timestamps.last_saved_at)
      
      // Verify written content
      const writtenContent = JSON.parse(vi.mocked(fs.writeFileSync).mock.calls[0][1] as string)
      expect(writtenContent.agent_info.instance_id).toBe(initialState.agent_info.instance_id)
      expect(writtenContent.timestamps.last_saved_at).not.toBe(initialState.timestamps.last_saved_at)
    })
    
    it("should handle filesystem errors", async () => {
      // Arrange
      const initialState = createValidTestState()
      const mockError = new Error("Filesystem error")
      vi.mocked(fs.writeFileSync).mockImplementation(() => { throw mockError })
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.saveAgentState(initialState)),
            GitHubClientLayer
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
      vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(validState))
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)),
          GitHubClientLayer
        )
      )
      
      // Assert
      expect(fs.existsSync).toHaveBeenCalled()
      expect(fs.readFileSync).toHaveBeenCalledWith(
        path.join(process.cwd(), "state", `${instanceId}.json`),
        "utf-8"
      )
      expect(result).toEqual(validState)
    })
    
    it("should throw StateNotFoundError when file doesn't exist", async () => {
      // Arrange
      const instanceId = "non-existent-id"
      vi.mocked(fs.existsSync).mockReturnValue(false)
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)),
            GitHubClientLayer
          )
        )
      ).rejects.toBeInstanceOf(StateNotFoundError)
    })
    
    it("should throw StateParseError when JSON is invalid", async () => {
      // Arrange
      const instanceId = "test-instance-id"
      vi.mocked(fs.readFileSync).mockReturnValue("{ invalid json")
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)),
            GitHubClientLayer
          )
        )
      ).rejects.toBeInstanceOf(StateParseError)
    })
    
    it("should throw StateValidationError when state doesn't match schema", async () => {
      // Arrange
      const instanceId = "test-instance-id"
      const invalidState = { not_valid: "missing required fields" }
      vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(invalidState))
      
      // Act & Assert
      await expect(
        Effect.runPromise(
          Effect.provide(
            Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)),
            GitHubClientLayer
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
      vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(oldVersionState))
      
      // Act
      const result = await Effect.runPromise(
        Effect.provide(
          Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)),
          GitHubClientLayer
        )
      )
      
      // Assert
      expect(logWarningSpy).toHaveBeenCalled()
      expect(result).toEqual(oldVersionState)
    })
  })
})