import { Effect, Exit } from "effect"
import { describe, expect, it, vi } from "vitest"
import { GitHubClient } from "../../src/github/GitHub.js"

// Mock the HTTP client
vi.mock("@effect/platform", async () => {
  const actual = await vi.importActual("@effect/platform")
  return {
    ...actual,
    HttpClient: {
      HttpClient: {
        pipe: () => ({
          get: vi.fn(),
          post: vi.fn(),
          patch: vi.fn(),
        })
      }
    }
  }
})

// Mock the NodeHttpClient
vi.mock("@effect/platform-node", () => ({
  NodeHttpClient: {
    layerUndici: {
      // mock Layer implementation
    }
  }
}))

// Mock the filesystem
vi.mock("node:fs", () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  writeFileSync: vi.fn(),
  readFileSync: vi.fn(),
}))

// Sample test data
const mockIssue = {
  id: 123,
  number: 1,
  title: "Test Issue",
  body: "This is a test issue",
  state: "open",
  html_url: "https://github.com/test/repo/issues/1",
  created_at: "2025-04-22T10:00:00Z",
  updated_at: "2025-04-22T10:00:00Z",
  user: {
    id: 456,
    login: "testuser",
    avatar_url: "https://avatars.githubusercontent.com/u/456",
    html_url: "https://github.com/testuser",
    type: "User"
  },
  labels: [],
  assignees: [],
  comments: 0
}

describe("GitHubClient", () => {
  // Create a mock layer for testing
  const TestLayer = GitHubClient.provide([])

  it("should define GitHubClient service", () => {
    expect(GitHubClient).toBeDefined()
  })

  it("should define GitHubClientLayer", () => {
    expect(GitHubClient.Default).toBeDefined()
  })

  it("should save agent state successfully", async () => {
    const mockFs = await import("node:fs")
    // @ts-ignore - mock implementation
    mockFs.existsSync.mockReturnValue(true)
    
    const mockState = {
      agent_info: {
        type: "solver",
        version: "1.0.0",
        instance_id: "test-instance-id",
        state_schema_version: "1.0"
      },
      timestamps: {
        created_at: "2025-04-22T10:00:00Z",
        last_saved_at: "2025-04-22T10:00:00Z",
        last_action_at: "2025-04-22T10:00:00Z"
      },
      current_task: {
        repo_owner: "test",
        repo_name: "repo",
        repo_branch: "main",
        issue_number: 1,
        status: "planning",
        current_step_index: 0
      },
      plan: [],
      execution_context: {
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
        total_steps_in_plan: 0,
        session_start_time: "2025-04-22T10:00:00Z",
        total_time_spent_seconds: 0,
        llm_calls_made: 0,
        llm_tokens_used: {},
        tools_called: 0,
        commits_made: 0
      },
      error_state: {
        consecutive_error_count: 0,
        retry_count_for_current_action: 0
      },
      configuration: {
        agent_goal: "Resolve issue #1",
        llm_config: {
          model: "claude-3-5-sonnet-latest",
          temperature: 0.7,
          max_tokens: 1024
        },
        max_retries_per_action: 3,
        allowed_actions: [],
        restricted_paths: [],
        action_timeout_seconds: 300,
        session_timeout_minutes: 120,
        github_token_available: true
      }
    }

    // Test saving agent state
    const program = Effect.gen(function* () {
      const github = yield* GitHubClient
      return yield* github.saveAgentState(mockState as any)
    })

    // Run the program
    const result = await Effect.runPromiseExit(Effect.provide(program, TestLayer))

    // Check the result
    if (Exit.isSuccess(result)) {
      expect(result.value).toBe(true)
      expect(mockFs.writeFileSync).toHaveBeenCalled()
    } else {
      fail("Expected success but got failure")
    }
  })
})