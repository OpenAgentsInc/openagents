import type { AiToolkit } from "@effect/ai"
import { Console, Effect, Layer } from "effect"
import type { AgentState } from "./AgentStateTypes.js"
import { GitHubClient } from "./GitHub.js"

// GitHub tool names for reference
const TOOL_NAMES = {
  GET_ISSUE: "GetGitHubIssue",
  LIST_ISSUES: "ListGitHubIssues",
  CREATE_COMMENT: "CreateGitHubComment",
  UPDATE_ISSUE: "UpdateGitHubIssue",
  GET_REPOSITORY: "GetGitHubRepository",
  GET_ISSUE_COMMENTS: "GetGitHubIssueComments",
  CREATE_AGENT_STATE: "CreateAgentStateForIssue",
  LOAD_AGENT_STATE: "LoadAgentState",
  SAVE_AGENT_STATE: "SaveAgentState"
}

// Define the GitHub tools service
export class GitHubTools extends Effect.Tag("GitHubTools")<
  GitHubTools,
  {
    tools: AiToolkit.AiToolkit<any>
    handlers: Record<string, (...args: Array<any>) => any>
  }
>() {
  static readonly fullName = "GitHubTools"
}

// Create a class for tool execution errors
export class ToolExecutionError extends Error {
  readonly _tag = "ToolExecutionError"
  readonly toolName: string
  readonly params: any

  constructor(message: string, toolName: string, params: any) {
    super(message)
    this.name = "ToolExecutionError"
    this.toolName = toolName
    this.params = params
  }
}

// Create the layer for the tools
export const GitHubToolsLayer = Layer.effect(
  GitHubTools,
  Effect.gen(function*() {
    // Get dependencies from the context
    const github = yield* GitHubClient

    const handlers = {
      GetGitHubIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: GetGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getIssue(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "GetGitHubIssue", params))
          }
        }),

      ListGitHubIssues: (params: { owner: string; repo: string; state?: "open" | "closed" | "all" }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: ListGitHubIssues")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.listIssues(params.owner, params.repo, params.state || "open")
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "ListGitHubIssues", params))
          }
        }),

      CreateGitHubComment: (params: { owner: string; repo: string; issueNumber: number; body: string }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: CreateGitHubComment")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.createIssueComment(params.owner, params.repo, params.issueNumber, params.body)
            yield* Console.log("✅ Tool result obtained")

            // Don't update context directly here to avoid passing currentState
            // This will be handled by TaskExecutor instead

            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "CreateGitHubComment", params))
          }
        }),

      UpdateGitHubIssue: (
        params: {
          owner: string
          repo: string
          issueNumber: number
          title?: string
          body?: string
          state?: "open" | "closed"
          labels?: Array<string>
          assignees?: Array<string>
        }
      ) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: UpdateGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const { issueNumber, owner, repo, ...updates } = params
            const result = yield* github.updateIssue(owner, repo, issueNumber, updates)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "UpdateGitHubIssue", params))
          }
        }),

      GetGitHubRepository: (params: { owner: string; repo: string }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: GetGitHubRepository")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getRepository(params.owner, params.repo)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "GetGitHubRepository", params))
          }
        }),

      GetGitHubIssueComments: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: GetGitHubIssueComments")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getIssueComments(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "GetGitHubIssueComments", params))
          }
        }),

      CreateAgentStateForIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: CreateAgentStateForIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.createAgentStateForIssue(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "CreateAgentStateForIssue", params))
          }
        }),

      LoadAgentState: (params: { instanceId: string }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: LoadAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.loadAgentState(params.instanceId)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "LoadAgentState", params))
          }
        }),

      SaveAgentState: (params: { state: AgentState }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: SaveAgentState")
          yield* Console.log("📝 Parameters: [state object]")
          try {
            const result = yield* github.saveAgentState(params.state)
            yield* Console.log("✅ Tool result obtained")
            return result
          } catch (error) {
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`❌ Tool error: ${err.message}`)
            return yield* Effect.fail(new ToolExecutionError(err.message, "SaveAgentState", params))
          }
        })
    }

    // Define schemas for the tools
    const toolSchemas = {
      [TOOL_NAMES.GET_ISSUE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.GET_ISSUE,
          description: "Fetch a GitHub issue by its number",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.LIST_ISSUES]: {
        type: "function",
        function: {
          name: TOOL_NAMES.LIST_ISSUES,
          description: "List GitHub issues in a repository",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              state: { type: "string", enum: ["open", "closed", "all"], description: "Filter issues by state" }
            },
            required: ["owner", "repo"]
          }
        }
      },
      [TOOL_NAMES.CREATE_COMMENT]: {
        type: "function",
        function: {
          name: TOOL_NAMES.CREATE_COMMENT,
          description: "Create a comment on a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" },
              body: { type: "string", description: "The comment text" }
            },
            required: ["owner", "repo", "issueNumber", "body"]
          }
        }
      },
      [TOOL_NAMES.UPDATE_ISSUE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.UPDATE_ISSUE,
          description: "Update a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" },
              title: { type: "string", description: "The new title for the issue" },
              body: { type: "string", description: "The new body for the issue" },
              state: { type: "string", enum: ["open", "closed"], description: "The new state for the issue" },
              labels: { type: "array", items: { type: "string" }, description: "Labels to set on the issue" },
              assignees: { type: "array", items: { type: "string" }, description: "Users to assign to the issue" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.GET_REPOSITORY]: {
        type: "function",
        function: {
          name: TOOL_NAMES.GET_REPOSITORY,
          description: "Get information about a GitHub repository",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" }
            },
            required: ["owner", "repo"]
          }
        }
      },
      [TOOL_NAMES.GET_ISSUE_COMMENTS]: {
        type: "function",
        function: {
          name: TOOL_NAMES.GET_ISSUE_COMMENTS,
          description: "Get comments on a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.CREATE_AGENT_STATE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.CREATE_AGENT_STATE,
          description: "Create an agent state for a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.LOAD_AGENT_STATE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.LOAD_AGENT_STATE,
          description: "Load an agent state by instance ID",
          parameters: {
            type: "object",
            properties: {
              instanceId: { type: "string", description: "The instance ID of the state to load" }
            },
            required: ["instanceId"]
          }
        }
      },
      [TOOL_NAMES.SAVE_AGENT_STATE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.SAVE_AGENT_STATE,
          description: "Save an agent state",
          parameters: {
            type: "object",
            properties: {
              state: { type: "object", description: "The agent state to save" }
            },
            required: ["state"]
          }
        }
      }
    }

    // Create the toolkit object with schema definitions
    const toolkit = {
      tools: toolSchemas
    }

    return {
      tools: toolkit as unknown as AiToolkit.AiToolkit<any>,
      handlers
    }
  })
).pipe(Layer.provide(GitHubClient.Default))

// Default implementation
export const GitHubToolsDefault = GitHubToolsLayer
