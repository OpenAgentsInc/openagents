import type { AiToolkit } from "@effect/ai"
import { Console, Effect, Layer } from "effect"
import { GitHubClient } from "./GitHub.js"
import type { AgentState } from "./GitHubTypes.js"

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

// Create the layer for the tools
export const GitHubToolsLayer = Layer.effect(
  GitHubTools,
  Effect.gen(function*() {
    const github = yield* GitHubClient

    const handlers = {
      GetGitHubIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: GetGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getIssue(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result

            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to get GitHub issue"))
          }
        }),

      ListGitHubIssues: (params: { owner: string; repo: string; state?: "open" | "closed" | "all" }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: ListGitHubIssues")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.listIssues(params.owner, params.repo, params.state || "open")
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to list GitHub issues"))
          }
        }),

      CreateGitHubComment: (params: { owner: string; repo: string; issueNumber: number; body: string }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: CreateGitHubComment")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.createIssueComment(params.owner, params.repo, params.issueNumber, params.body)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to create GitHub issue comment"))
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
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to update GitHub issue"))
          }
        }),

      GetGitHubRepository: (params: { owner: string; repo: string }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: GetGitHubRepository")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getRepository(params.owner, params.repo)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to get GitHub repository info"))
          }
        }),

      GetGitHubIssueComments: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: GetGitHubIssueComments")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getIssueComments(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to get GitHub issue comments"))
          }
        }),

      CreateAgentStateForIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: CreateAgentStateForIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.createAgentStateForIssue(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to create agent state for issue"))
          }
        }),

      LoadAgentState: (params: { instanceId: string }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: LoadAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.loadAgentState(params.instanceId)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to load agent state"))
          }
        }),

      SaveAgentState: (params: { state: AgentState }) =>
        Effect.gen(function*() {
          yield* Console.log("🛠️ Tool called: SaveAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.saveAgentState(params.state)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
            /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
          } catch (_error) {
            return yield* Effect.fail(new Error("Failed to save agent state"))
          }
        })
    }

    // Create a simple toolkit object with the handlers
    const mockToolkit = {
      tools: {
        [TOOL_NAMES.GET_ISSUE]: handlers.GetGitHubIssue,
        [TOOL_NAMES.LIST_ISSUES]: handlers.ListGitHubIssues,
        [TOOL_NAMES.CREATE_COMMENT]: handlers.CreateGitHubComment,
        [TOOL_NAMES.UPDATE_ISSUE]: handlers.UpdateGitHubIssue,
        [TOOL_NAMES.GET_REPOSITORY]: handlers.GetGitHubRepository,
        [TOOL_NAMES.GET_ISSUE_COMMENTS]: handlers.GetGitHubIssueComments,
        [TOOL_NAMES.CREATE_AGENT_STATE]: handlers.CreateAgentStateForIssue,
        [TOOL_NAMES.LOAD_AGENT_STATE]: handlers.LoadAgentState,
        [TOOL_NAMES.SAVE_AGENT_STATE]: handlers.SaveAgentState
      }
    }

    return {
      tools: mockToolkit as unknown as AiToolkit.AiToolkit<any>,
      handlers
    }
  })
).pipe(Layer.provide(GitHubClient.Default))
