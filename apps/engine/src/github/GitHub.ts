import { HttpBody, HttpClient, HttpClientError, HttpClientResponse } from "@effect/platform"
import { NodeHttpClient } from "@effect/platform-node"
import { Config, Console, Effect, Schema } from "effect"
import * as fs from "node:fs"
import * as path from "node:path"
import { AgentState as AgentStateSchema } from "./AgentStateTypes.js"
import type { AgentState } from "./AgentStateTypes.js"
import { GitHubIssue, GitHubIssueComment, GitHubRepository } from "./GitHubTypes.js"

/**
 * Error class for state storage operations
 */
export class StateStorageError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "StateStorageError"
  }
}

/**
 * Error class for state not found
 */
export class StateNotFoundError extends StateStorageError {
  constructor(instanceId: string) {
    super(`State file for ${instanceId} not found`)
    this.name = "StateNotFoundError"
  }
}

/**
 * Error class for state parsing errors
 */
export class StateParseError extends StateStorageError {
  constructor(message: string) {
    super(`Failed to parse state: ${message}`)
    this.name = "StateParseError"
  }
}

/**
 * Error class for state validation errors
 */
export class StateValidationError extends StateStorageError {
  constructor(message: string) {
    super(`State validation failed: ${message}`)
    this.name = "StateValidationError"
  }
}

/**
 * GitHub API client service implementation
 */
export class GitHubClient extends Effect.Service<GitHubClient>()("GitHubClient", {
  dependencies: [NodeHttpClient.layerUndici],
  effect: Effect.gen(function*() {
    const httpClient = yield* HttpClient.HttpClient
    const githubApiKey = yield* Config.secret("GITHUB_API_KEY")

    // Configure HTTP client with GitHub API base URL and authentication
    const githubHttpClient = httpClient.pipe(
      HttpClient.filterStatusOk,
      HttpClient.mapRequest((request) => ({
        ...request,
        url: `https://api.github.com${request.url}`,
        headers: {
          ...request.headers,
          "Accept": "application/vnd.github.v3+json",
          "Authorization": `token ${githubApiKey}`,
          "User-Agent": "OpenAgents-Engine/1.0"
        }
      }))
    )

    /**
     * Get an issue from a GitHub repository
     */
    const getIssue = Effect.fn("GitHubClient.getIssue")(
      function*(owner: string, repo: string, issueNumber: number) {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}/issues/${issueNumber}`, {
          acceptJson: true
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubIssue)),
          Effect.catchAll((error) => {
            if (
              HttpClientError.isHttpClientError(error) &&
              "status" in error && error.status === 404
            ) {
              return Effect.fail(new Error(`Issue #${issueNumber} not found in ${owner}/${repo}`))
            }
            return Effect.fail(new Error(`GitHub API error: ${String(error)}`))
          }),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error fetching issue #${issueNumber}: ${error}`))
        )
      }
    )

    /**
     * List issues from a GitHub repository
     */
    const listIssues = Effect.fn("GitHubClient.listIssues")(
      function*(owner: string, repo: string, state: "open" | "closed" | "all" = "open") {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}/issues`, {
          acceptJson: true,
          urlParams: { state }
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(Schema.Array(GitHubIssue))),
          Effect.map((issues) => ({ issues })),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error listing issues for ${owner}/${repo}: ${error}`))
        )
      }
    )

    /**
     * Get comments for an issue
     */
    const getIssueComments = Effect.fn("GitHubClient.getIssueComments")(
      function*(owner: string, repo: string, issueNumber: number) {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}/issues/${issueNumber}/comments`, {
          acceptJson: true
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(Schema.Array(GitHubIssueComment))),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error fetching comments for issue #${issueNumber}: ${error}`))
        )
      }
    )

    /**
     * Create a comment on an issue
     */
    const createIssueComment = Effect.fn("GitHubClient.createIssueComment")(
      function*(owner: string, repo: string, issueNumber: number, body: string) {
        return yield* githubHttpClient.post(`/repos/${owner}/${repo}/issues/${issueNumber}/comments`, {
          acceptJson: true,
          body: yield* HttpBody.json({ body })
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubIssueComment)),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error creating comment on issue #${issueNumber}: ${error}`))
        )
      }
    )

    /**
     * Get repository information
     */
    const getRepository = Effect.fn("GitHubClient.getRepository")(
      function*(owner: string, repo: string) {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}`, {
          acceptJson: true
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubRepository)),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error fetching repository ${owner}/${repo}: ${error}`))
        )
      }
    )

    /**
     * Update an issue (status, title, body, labels, etc.)
     */
    const updateIssue = Effect.fn("GitHubClient.updateIssue")(
      function*(owner: string, repo: string, issueNumber: number, updates: {
        title?: string
        body?: string
        state?: "open" | "closed"
        labels?: Array<string>
        assignees?: Array<string>
      }) {
        return yield* githubHttpClient.patch(`/repos/${owner}/${repo}/issues/${issueNumber}`, {
          acceptJson: true,
          body: yield* HttpBody.json(updates)
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubIssue)),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error updating issue #${issueNumber}: ${error}`))
        )
      }
    )

    /**
     * Ensure the state directory exists
     */
    const ensureStateDirectory = Effect.fn("GitHubClient.ensureStateDirectory")(
      function*() {
        const stateDir = path.join(process.cwd(), "state")

        return yield* Effect.try({
          try: () => {
            if (!fs.existsSync(stateDir)) {
              fs.mkdirSync(stateDir, { recursive: true })
              Effect.logInfo("Created state directory")
            }
            return stateDir
          },
          catch: (error) => new StateStorageError(`Failed to create state directory: ${error}`)
        })
      }
    )

    /**
     * Save agent state to a local JSON file
     */
    const saveAgentState = Effect.fn("GitHubClient.saveAgentState")(
      function*(state: AgentState) {
        // First ensure state directory exists
        const stateDir = yield* ensureStateDirectory()
        const filePath = path.join(stateDir, `${state.agent_info.instance_id}.json`)

        // Create a new state object with updated timestamp (immutability)
        const updatedState = {
          ...state,
          timestamps: {
            ...state.timestamps,
            last_saved_at: new Date().toISOString()
          }
        }

        return yield* Effect.try({
          try: () => {
            // Write state to file
            fs.writeFileSync(filePath, JSON.stringify(updatedState, null, 2))
            Effect.logInfo(`Saved agent state to ${filePath}`)
            return updatedState
          },
          catch: (error) => new StateStorageError(`Failed to save agent state: ${error}`)
        }).pipe(
          Effect.tapError((error) => Console.error(error.message))
        )
      }
    )

    /**
     * Load agent state from a local JSON file
     */
    const loadAgentState = Effect.fn("GitHubClient.loadAgentState")(
      function*(instanceId: string) {
        const filePath = path.join(process.cwd(), "state", `${instanceId}.json`)

        // Check if file exists
        const fileExists = yield* Effect.try({
          try: () => fs.existsSync(filePath),
          catch: (error) => new StateStorageError(`Error checking if state file exists: ${error}`)
        })

        if (!fileExists) {
          return yield* Effect.fail(new StateNotFoundError(instanceId))
        }

        // Read file and parse JSON
        const parsedJson = yield* Effect.try({
          try: () => {
            const stateJson = fs.readFileSync(filePath, "utf-8")
            return JSON.parse(stateJson)
          },
          catch: (error) => new StateParseError(String(error))
        }).pipe(
          Effect.tapError((error) => Console.error(error.message))
        )

        // Validate against schema
        const validatedState = yield* Schema.decodeUnknown(AgentStateSchema)(parsedJson).pipe(
          Effect.catchAll((error) => {
            return Effect.fail(new StateValidationError(String(error)))
          }),
          Effect.tapError((error) => Console.error(error.message))
        )

        // Check schema version
        if (validatedState.agent_info.state_schema_version !== "1.1") {
          yield* Effect.logWarning(
            `Loaded state with schema version ${validatedState.agent_info.state_schema_version}, expected 1.1. Migration might be needed.`
          )
        }

        yield* Effect.logInfo(`Loaded agent state for ${instanceId}`)
        return validatedState
      }
    )

    /**
     * Create a new agent state for a GitHub issue
     */
    const createAgentStateForIssue = Effect.fn("GitHubClient.createAgentStateForIssue")(
      function*(owner: string, repo: string, issueNumber: number) {
        // First, fetch the issue to get its details
        const issue: GitHubIssue = yield* getIssue(owner, repo, issueNumber)

        // Generate a unique instance ID
        const instanceId = `solver-${owner}-${repo}-${issueNumber}-${Date.now()}`
        const now = new Date().toISOString()

        // Create initial agent state
        const initialState: AgentState = {
          agent_info: {
            type: "solver",
            version: "1.0.0",
            instance_id: instanceId,
            state_schema_version: "1.1" // Updated to match expected version
          },
          timestamps: {
            created_at: now,
            last_saved_at: now,
            last_action_at: now
          },
          current_task: {
            repo_owner: owner,
            repo_name: repo,
            repo_branch: "main", // Default to main
            issue_number: issueNumber,
            issue_details_cache: {
              title: issue.title,
              description_snippet: issue.body.slice(0, 200) + (issue.body.length > 200 ? "..." : ""),
              status: issue.state,
              labels: issue.labels.map((l) => l.name),
              source_url: issue.html_url
            },
            status: "planning",
            current_step_index: 0
          },
          plan: [
            {
              id: `step-${Date.now()}-1`,
              step_number: 1,
              description: "Analyze issue requirements and context",
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
            session_start_time: now,
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
            agent_goal: `Resolve issue #${issueNumber}: ${issue.title}`,
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
        }

        // Validate the initial state against the schema
        yield* Schema.decodeUnknown(AgentStateSchema)(initialState).pipe(
          Effect.catchAll((error) => {
            return Effect.fail(new StateValidationError(`Invalid initial state: ${error}`))
          }),
          Effect.tapError((error) => Console.error(error.message))
        )

        // Save the initial state
        const savedState = yield* saveAgentState(initialState)

        yield* Effect.logInfo(`Created new agent state for issue #${issueNumber} with ID ${instanceId}`)
        return savedState
      }
    )

    return {
      getIssue,
      listIssues,
      getIssueComments,
      createIssueComment,
      getRepository,
      updateIssue,
      saveAgentState,
      loadAgentState,
      createAgentStateForIssue
    } as const
  })
}) {}

// Create the default layer for GitHub client
export const GitHubClientLayer = GitHubClient.Default
