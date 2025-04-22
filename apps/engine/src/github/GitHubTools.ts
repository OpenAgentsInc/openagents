import { AiToolkit } from "@effect/ai"
import { Console, Effect, Layer } from "effect"
import { GitHubClient } from "./GitHub.js"
import { AgentState } from "./GitHubTypes.js"

// Define GitHub tools
const getIssueSchema = {
  name: "GetGitHubIssue",
  description: "Get a GitHub issue by number",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" },
    issueNumber: { type: "number", description: "The issue number to retrieve" }
  }
}

const listIssuesSchema = {
  name: "ListGitHubIssues",
  description: "List GitHub issues for a repository",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" },
    state: { type: "string", enum: ["open", "closed", "all"], description: "The state of issues to retrieve" }
  }
}

const createCommentSchema = {
  name: "CreateGitHubComment",
  description: "Create a comment on a GitHub issue",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" },
    issueNumber: { type: "number", description: "The issue number to comment on" },
    body: { type: "string", description: "The comment text (supports GitHub-flavored markdown)" }
  }
}

const updateIssueSchema = {
  name: "UpdateGitHubIssue",
  description: "Update a GitHub issue (title, body, state, labels, or assignees)",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" },
    issueNumber: { type: "number", description: "The issue number to update" },
    title: { type: "string", description: "The updated title for the issue" },
    body: { type: "string", description: "The updated body text for the issue" },
    state: { type: "string", enum: ["open", "closed"], description: "The updated state for the issue" },
    labels: { type: "array", items: { type: "string" }, description: "The updated labels for the issue" },
    assignees: { type: "array", items: { type: "string" }, description: "The updated assignees for the issue" }
  }
}

const getRepositorySchema = {
  name: "GetGitHubRepository",
  description: "Get information about a GitHub repository",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" }
  }
}

const getIssueCommentsSchema = {
  name: "GetGitHubIssueComments",
  description: "Get comments on a GitHub issue",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" },
    issueNumber: { type: "number", description: "The issue number to get comments for" }
  }
}

const createAgentStateSchema = {
  name: "CreateAgentStateForIssue",
  description: "Create a new agent state for processing a GitHub issue",
  parameters: {
    owner: { type: "string", description: "The owner of the repository" },
    repo: { type: "string", description: "The name of the repository" },
    issueNumber: { type: "number", description: "The issue number to create an agent state for" }
  }
}

const loadAgentStateSchema = {
  name: "LoadAgentState",
  description: "Load an existing agent state by instance ID",
  parameters: {
    instanceId: { type: "string", description: "The instance ID of the agent state to load" }
  }
}

const saveAgentStateSchema = {
  name: "SaveAgentState",
  description: "Save the current agent state",
  parameters: {
    state: { type: "object", description: "The agent state to save" }
  }
}

// Define the GitHub tools service
export class GitHubTools extends Effect.Tag("GitHubTools")<
  GitHubTools,
  {
    tools: AiToolkit.AiToolkit,
    handlers: Record<string, Function>
  }
>() {}

// Create the layer for the tools
export const GitHubToolsLayer = Layer.effect(
  GitHubTools,
  Effect.gen(function* () {
    const github = yield* GitHubClient
    
    const handlers = {
      GetGitHubIssue: (params: { owner: string; repo: string; issueNumber: number }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getIssue(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to get GitHub issue"))
          }
        }),

      ListGitHubIssues: (params: { owner: string; repo: string; state?: "open" | "closed" | "all" }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: ListGitHubIssues")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.listIssues(params.owner, params.repo, params.state || "open")
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to list GitHub issues"))
          }
        }),

      CreateGitHubComment: (params: { owner: string; repo: string; issueNumber: number; body: string }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: CreateGitHubComment")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.createIssueComment(params.owner, params.repo, params.issueNumber, params.body)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to create GitHub issue comment"))
          }
        }),

      UpdateGitHubIssue: (params: { owner: string; repo: string; issueNumber: number; title?: string; body?: string; state?: "open" | "closed"; labels?: string[]; assignees?: string[] }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: UpdateGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const { owner, repo, issueNumber, ...updates } = params
            const result = yield* github.updateIssue(owner, repo, issueNumber, updates)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to update GitHub issue"))
          }
        }),

      GetGitHubRepository: (params: { owner: string; repo: string }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetGitHubRepository")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getRepository(params.owner, params.repo)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to get GitHub repository info"))
          }
        }),

      GetGitHubIssueComments: (params: { owner: string; repo: string; issueNumber: number }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetGitHubIssueComments")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.getIssueComments(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to get GitHub issue comments"))
          }
        }),

      CreateAgentStateForIssue: (params: { owner: string; repo: string; issueNumber: number }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: CreateAgentStateForIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.createAgentStateForIssue(params.owner, params.repo, params.issueNumber)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to create agent state for issue"))
          }
        }),

      LoadAgentState: (params: { instanceId: string }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: LoadAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.loadAgentState(params.instanceId)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to load agent state"))
          }
        }),

      SaveAgentState: (params: { state: AgentState }) => 
        Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: SaveAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          try {
            const result = yield* github.saveAgentState(params.state)
            yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
            return result
          } catch (error) {
            return yield* Effect.fail(new Error("Failed to save agent state"))
          }
        })
    }

    // Create the AiToolkit instance with GitHub tools
    const tools = AiToolkit.empty
      .add("GetGitHubIssue")
      .add("ListGitHubIssues")
      .add("CreateGitHubComment")
      .add("UpdateGitHubIssue")
      .add("GetGitHubRepository")
      .add("GetGitHubIssueComments")
      .add("CreateAgentStateForIssue")
      .add("LoadAgentState")
      .add("SaveAgentState")
      
    return {
      tools,
      handlers
    }
  })
).pipe(Layer.provide(GitHubClient.Default))