import { Config, Console, Effect } from "effect"
import { HttpClient, HttpClientResponse } from "@effect/platform"
import { NodeHttpClient } from "@effect/platform-node"
import * as fs from "node:fs"
import * as path from "node:path"
import {
  GitHubError,
  GitHubIssue,
  GitHubIssueComment,
  GitHubIssueListResponse,
  GitHubRepository,
  AgentState
} from "./GitHubTypes.js"

/**
 * GitHub API client service implementation
 */
export class GitHubClient extends Effect.Service<GitHubClient>()("GitHubClient", {
  dependencies: [NodeHttpClient.layerUndici],
  effect: Effect.gen(function* () {
    const httpClient = yield* HttpClient.HttpClient
    const githubApiKey = yield* Config.redacted("GITHUB_API_KEY")

    // Configure HTTP client with GitHub API base URL and authentication
    const githubHttpClient = httpClient.pipe(
      HttpClient.filterStatusOk,
      HttpClient.mapRequest((request) => ({ 
        ...request, 
        url: `https://api.github.com${request.url}`,
        headers: {
          ...request.headers,
          "Accept": "application/vnd.github.v3+json",
          "Authorization": `token ${githubApiKey.value}`,
          "User-Agent": "OpenAgents-Engine/1.0"
        }
      }))
    )

    /**
     * Get an issue from a GitHub repository
     */
    const getIssue = Effect.fn("GitHubClient.getIssue")(
      function* (owner: string, repo: string, issueNumber: number) {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}/issues/${issueNumber}`, {
          acceptJson: true
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubIssue)),
          Effect.catchTag("HttpClientResponseError", (error) => {
            if (error.response.status === 404) {
              return Effect.fail(new Error(`Issue #${issueNumber} not found in ${owner}/${repo}`))
            }
            return Effect.fail(error)
          }),
          Effect.catchTag("ParseError", (error) => {
            return Effect.fail(new Error(`Failed to parse GitHub response: ${error.message}`))
          }),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error fetching issue #${issueNumber}: ${error.message}`))
        )
      }
    )

    /**
     * List issues from a GitHub repository
     */
    const listIssues = Effect.fn("GitHubClient.listIssues")(
      function* (owner: string, repo: string, state: "open" | "closed" | "all" = "open") {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}/issues`, {
          acceptJson: true,
          urlParams: { state }
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(Schema.Array(GitHubIssue))),
          Effect.map((issues) => ({ issues })),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error listing issues for ${owner}/${repo}: ${error.message}`))
        )
      }
    )

    /**
     * Get comments for an issue
     */
    const getIssueComments = Effect.fn("GitHubClient.getIssueComments")(
      function* (owner: string, repo: string, issueNumber: number) {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}/issues/${issueNumber}/comments`, {
          acceptJson: true
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(Schema.Array(GitHubIssueComment))),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error fetching comments for issue #${issueNumber}: ${error.message}`))
        )
      }
    )

    /**
     * Create a comment on an issue
     */
    const createIssueComment = Effect.fn("GitHubClient.createIssueComment")(
      function* (owner: string, repo: string, issueNumber: number, body: string) {
        return yield* githubHttpClient.post(`/repos/${owner}/${repo}/issues/${issueNumber}/comments`, {
          acceptJson: true,
          body: JSON.stringify({ body })
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubIssueComment)),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error creating comment on issue #${issueNumber}: ${error.message}`))
        )
      }
    )

    /**
     * Get repository information
     */
    const getRepository = Effect.fn("GitHubClient.getRepository")(
      function* (owner: string, repo: string) {
        return yield* githubHttpClient.get(`/repos/${owner}/${repo}`, {
          acceptJson: true
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubRepository)),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error fetching repository ${owner}/${repo}: ${error.message}`))
        )
      }
    )

    /**
     * Update an issue (status, title, body, labels, etc.)
     */
    const updateIssue = Effect.fn("GitHubClient.updateIssue")(
      function* (owner: string, repo: string, issueNumber: number, updates: {
        title?: string,
        body?: string,
        state?: "open" | "closed",
        labels?: string[],
        assignees?: string[]
      }) {
        return yield* githubHttpClient.patch(`/repos/${owner}/${repo}/issues/${issueNumber}`, {
          acceptJson: true,
          body: JSON.stringify(updates)
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(GitHubIssue)),
          Effect.scoped,
          Effect.tapError((error) => Console.error(`Error updating issue #${issueNumber}: ${error.message}`))
        )
      }
    )

    /**
     * Save agent state to a local JSON file
     */
    const saveAgentState = Effect.fn("GitHubClient.saveAgentState")(
      function* (state: AgentState) {
        const stateDir = path.join(process.cwd(), "state")
        const filePath = path.join(stateDir, `${state.agent_info.instance_id}.json`)
        
        return yield* Effect.try({
          try: () => {
            // Ensure state directory exists
            if (!fs.existsSync(stateDir)) {
              fs.mkdirSync(stateDir, { recursive: true })
            }
            
            // Update the last saved timestamp
            const updatedState = {
              ...state,
              timestamps: {
                ...state.timestamps,
                last_saved_at: new Date().toISOString()
              }
            }
            
            // Write state to file
            fs.writeFileSync(filePath, JSON.stringify(updatedState, null, 2))
            return true
          },
          catch: (error) => new Error(`Failed to save agent state: ${error}`)
        }).pipe(
          Effect.tapError((error) => Console.error(error.message))
        )
      }
    )

    /**
     * Load agent state from a local JSON file
     */
    const loadAgentState = Effect.fn("GitHubClient.loadAgentState")(
      function* (instanceId: string) {
        const filePath = path.join(process.cwd(), "state", `${instanceId}.json`)
        
        return yield* Effect.try({
          try: () => {
            if (!fs.existsSync(filePath)) {
              throw new Error(`State file for ${instanceId} not found`)
            }
            
            const stateJson = fs.readFileSync(filePath, "utf-8")
            const state = JSON.parse(stateJson)
            return state as AgentState
          },
          catch: (error) => new Error(`Failed to load agent state: ${error}`)
        }).pipe(
          Effect.tapError((error) => Console.error(error.message))
        )
      }
    )

    /**
     * Create a new agent state for a GitHub issue
     */
    const createAgentStateForIssue = Effect.fn("GitHubClient.createAgentStateForIssue")(
      function* (owner: string, repo: string, issueNumber: number) {
        // First, fetch the issue to get its details
        const issue = yield* getIssue(owner, repo, issueNumber)
        
        // Generate a unique instance ID
        const instanceId = `solver-${owner}-${repo}-${issueNumber}-${Date.now()}`
        const now = new Date().toISOString()
        
        // Create initial agent state
        const initialState: AgentState = {
          agent_info: {
            type: "solver",
            version: "1.0.0",
            instance_id: instanceId,
            state_schema_version: "1.0"
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
              labels: issue.labels.map(l => l.name),
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
              start_time: undefined,
              end_time: undefined,
              result_summary: undefined,
              tool_calls: []
            }
          ],
          execution_context: {
            current_file_focus: undefined,
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
            last_error: undefined,
            consecutive_error_count: 0,
            retry_count_for_current_action: 0,
            blocked_reason: undefined
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
        
        // Save the initial state
        yield* saveAgentState(initialState)
        
        return initialState
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
}) { }

// Create the default layer for GitHub client
export const GitHubClientLayer = GitHubClient.Default