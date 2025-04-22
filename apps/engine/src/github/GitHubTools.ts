import { AiToolkit } from "@effect/ai"
import { Console, Effect, Layer, Schema } from "effect"
import { GitHubClient } from "./GitHub.js"

/**
 * Tool for getting a GitHub issue
 */
export class GetGitHubIssue extends Schema.TaggedRequest<GetGitHubIssue>()("GetGitHubIssue", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    }),
    issueNumber: Schema.Number.annotations({
      description: "The issue number to retrieve"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Get a GitHub issue by number"
}) { }

/**
 * Tool for listing GitHub issues
 */
export class ListGitHubIssues extends Schema.TaggedRequest<ListGitHubIssues>()("ListGitHubIssues", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    }),
    state: Schema.Union(
      Schema.Literal("open"),
      Schema.Literal("closed"),
      Schema.Literal("all")
    ).optional().annotations({
      description: "The state of issues to retrieve (open, closed, or all). Defaults to open."
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "List GitHub issues for a repository"
}) { }

/**
 * Tool for creating a comment on a GitHub issue
 */
export class CreateGitHubComment extends Schema.TaggedRequest<CreateGitHubComment>()("CreateGitHubComment", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    }),
    issueNumber: Schema.Number.annotations({
      description: "The issue number to comment on"
    }),
    body: Schema.String.annotations({
      description: "The comment text (supports GitHub-flavored markdown)"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Create a comment on a GitHub issue"
}) { }

/**
 * Tool for updating a GitHub issue
 */
export class UpdateGitHubIssue extends Schema.TaggedRequest<UpdateGitHubIssue>()("UpdateGitHubIssue", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    }),
    issueNumber: Schema.Number.annotations({
      description: "The issue number to update"
    }),
    title: Schema.String.optional().annotations({
      description: "The updated title for the issue"
    }),
    body: Schema.String.optional().annotations({
      description: "The updated body text for the issue"
    }),
    state: Schema.Union(
      Schema.Literal("open"),
      Schema.Literal("closed")
    ).optional().annotations({
      description: "The updated state for the issue (open or closed)"
    }),
    labels: Schema.Array(Schema.String).optional().annotations({
      description: "The updated labels for the issue"
    }),
    assignees: Schema.Array(Schema.String).optional().annotations({
      description: "The updated assignees for the issue"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Update a GitHub issue (title, body, state, labels, or assignees)"
}) { }

/**
 * Tool for getting a GitHub repository
 */
export class GetGitHubRepository extends Schema.TaggedRequest<GetGitHubRepository>()("GetGitHubRepository", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Get information about a GitHub repository"
}) { }

/**
 * Tool for getting comments on a GitHub issue
 */
export class GetGitHubIssueComments extends Schema.TaggedRequest<GetGitHubIssueComments>()("GetGitHubIssueComments", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    }),
    issueNumber: Schema.Number.annotations({
      description: "The issue number to get comments for"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Get comments on a GitHub issue"
}) { }

/**
 * Tool for creating an agent state for a GitHub issue
 */
export class CreateAgentStateForIssue extends Schema.TaggedRequest<CreateAgentStateForIssue>()("CreateAgentStateForIssue", {
  payload: {
    owner: Schema.String.annotations({
      description: "The owner of the repository"
    }),
    repo: Schema.String.annotations({
      description: "The name of the repository"
    }),
    issueNumber: Schema.Number.annotations({
      description: "The issue number to create an agent state for"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Create a new agent state for processing a GitHub issue"
}) { }

/**
 * Tool for loading an existing agent state
 */
export class LoadAgentState extends Schema.TaggedRequest<LoadAgentState>()("LoadAgentState", {
  payload: {
    instanceId: Schema.String.annotations({
      description: "The instance ID of the agent state to load"
    })
  },
  success: Schema.Any,
  failure: Schema.Never
}, {
  description: "Load an existing agent state by instance ID"
}) { }

/**
 * Tool for saving the current agent state
 */
export class SaveAgentState extends Schema.TaggedRequest<SaveAgentState>()("SaveAgentState", {
  payload: {
    state: Schema.Any.annotations({
      description: "The agent state to save"
    })
  },
  success: Schema.Boolean,
  failure: Schema.Never
}, {
  description: "Save the current agent state"
}) { }

// Create a toolkit with all GitHub tools
export const GitHubTools = AiToolkit.empty
  .add(GetGitHubIssue)
  .add(ListGitHubIssues)
  .add(CreateGitHubComment)
  .add(UpdateGitHubIssue)
  .add(GetGitHubRepository)
  .add(GetGitHubIssueComments)
  .add(CreateAgentStateForIssue)
  .add(LoadAgentState)
  .add(SaveAgentState)

// Implement the GitHub tools layer
export const GitHubToolsLayer = GitHubTools.implement((handlers) =>
  Effect.gen(function* () {
    const github = yield* GitHubClient
    
    return handlers
      .handle("GetGitHubIssue", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.getIssue(params.owner, params.repo, params.issueNumber)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("ListGitHubIssues", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: ListGitHubIssues")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.listIssues(params.owner, params.repo, params.state)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("CreateGitHubComment", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: CreateGitHubComment")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.createIssueComment(params.owner, params.repo, params.issueNumber, params.body)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("UpdateGitHubIssue", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: UpdateGitHubIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const { owner, repo, issueNumber, ...updates } = params
          const result = yield* github.updateIssue(owner, repo, issueNumber, updates)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("GetGitHubRepository", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetGitHubRepository")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.getRepository(params.owner, params.repo)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("GetGitHubIssueComments", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetGitHubIssueComments")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.getIssueComments(params.owner, params.repo, params.issueNumber)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("CreateAgentStateForIssue", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: CreateAgentStateForIssue")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.createAgentStateForIssue(params.owner, params.repo, params.issueNumber)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("LoadAgentState", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: LoadAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.loadAgentState(params.instanceId)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
      .handle("SaveAgentState", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: SaveAgentState")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* github.saveAgentState(params.state)
          yield* Console.log("✅ Tool result:", JSON.stringify(result, null, 2))
          return result
        })
      })
  })
).pipe(Layer.provide(GitHubClient.Default))