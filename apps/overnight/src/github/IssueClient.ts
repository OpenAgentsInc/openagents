import * as Context from "effect/Context"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"

import { GitHubHttpExecutor } from "./Client.js"
import { GitHubApiError, RateLimitExceededError, HttpError } from "./Errors.js"

/**
 * GitHub issue response schema
 */
export interface GitHubIssue {
  id: number
  number: number
  title: string
  state: "open" | "closed"
  body: string
  user: {
    login: string
  }
  created_at: string
  updated_at: string
  labels: Array<{
    name: string
  }>
}

/**
 * Issue-specific not found error
 */
export class IssueNotFoundError extends Data.TaggedError("IssueNotFoundError")<{
  readonly owner: string
  readonly repo: string
  readonly issueNumber: number
  readonly message: string
}> {
  constructor(owner: string, repo: string, issueNumber: number) {
    super({
      owner,
      repo,
      issueNumber,
      message: `Issue not found: ${owner}/${repo}#${issueNumber}`
    })
  }
}

/**
 * Schema for the issue fetch payload
 */
export interface FetchIssuePayload {
  owner: string
  repo: string
  issueNumber: number
}

/**
 * The FetchIssueFromGitHub class for tool usage
 */
export class FetchIssueFromGitHub {
  readonly _tag = "FetchIssueFromGitHub"
  
  constructor(readonly payload: FetchIssuePayload) {}
}

/**
 * GitHub Issue Client interface
 */
export interface GitHubIssueClient {
  fetchIssue(params: FetchIssuePayload): Effect.Effect<
    GitHubIssue,
    IssueNotFoundError | RateLimitExceededError | GitHubApiError | HttpError
  >
}

/**
 * GitHub Issue Client service tag
 */
export const GitHubIssueClient = Context.GenericTag<GitHubIssueClient>("GitHubIssueClient")

/**
 * Real implementation of the GitHub Issue Client
 */
export class GitHubIssueClientLive implements GitHubIssueClient {
  constructor(
    private readonly executor: GitHubHttpExecutor
  ) {}

  fetchIssue(
    params: FetchIssuePayload
  ): Effect.Effect<
    GitHubIssue, 
    IssueNotFoundError | RateLimitExceededError | GitHubApiError | HttpError
  > {
    const { owner, repo, issueNumber } = params
    
    // Build the request URL path
    const apiPath = `/repos/${owner}/${repo}/issues/${issueNumber}`
    
    // Create the request
    const request = HttpClientRequest.get(apiPath)
    
    return this.executor.execute<GitHubIssue>(request)
      .pipe(
        // Map NotFoundError to IssueNotFoundError
        Effect.catchTag("NotFoundError", () => 
          Effect.fail(new IssueNotFoundError(owner, repo, issueNumber))
        )
      )
  }
}

/**
 * Mock implementation of the GitHub Issue Client for testing
 */
export class GitHubIssueClientMock implements GitHubIssueClient {
  private readonly mockIssues: Map<string, GitHubIssue> = new Map()

  constructor() {
    // Set up some mock issues for testing
    this.mockIssues.set("openagentsinc/openagents#1", {
      id: 1,
      number: 1,
      title: "Mock Issue #1",
      state: "open",
      body: "This is a mock issue for testing",
      user: {
        login: "mockuser"
      },
      created_at: "2025-04-21T12:00:00Z",
      updated_at: "2025-04-21T12:00:00Z",
      labels: [
        { name: "bug" },
        { name: "high-priority" }
      ]
    })

    this.mockIssues.set("openagentsinc/openagents#2", {
      id: 2,
      number: 2,
      title: "Mock Issue #2",
      state: "closed",
      body: "This is another mock issue for testing",
      user: {
        login: "mockuser2"
      },
      created_at: "2025-04-20T12:00:00Z",
      updated_at: "2025-04-21T15:00:00Z",
      labels: [
        { name: "enhancement" }
      ]
    })
  }

  fetchIssue(
    params: FetchIssuePayload
  ): Effect.Effect<
    GitHubIssue, 
    IssueNotFoundError | RateLimitExceededError | GitHubApiError
  > {
    const { owner, repo, issueNumber } = params
    const key = `${owner}/${repo}#${issueNumber}`
    // Capture the mockIssues reference to use in the generator
    const mockIssues = this.mockIssues

    return Effect.gen(function*() {
      // Special case for rate limiting test
      if (issueNumber === 9999) {
        return yield* Effect.fail(new RateLimitExceededError(new Date(Date.now() + 3600000)))
      }

      // Simulate a server error for testing
      if (issueNumber === 8888) {
        return yield* Effect.fail(new GitHubApiError({ 
          message: "GitHub API error: 500 Internal Server Error" 
        }))
      }

      // Look up the issue in our mock database
      const issue = mockIssues.get(key)
      
      if (!issue) {
        return yield* Effect.fail(new IssueNotFoundError(owner, repo, issueNumber))
      }
      
      return issue
    })
  }
}

/**
 * Create GitHub Issue Client Layer with real implementation
 */
export const githubIssueClientLayer = Layer.effect(
  GitHubIssueClient,
  Effect.gen(function*() {
    const executor = yield* GitHubHttpExecutor
    return new GitHubIssueClientLive(executor)
  })
)

/**
 * Create GitHub Issue Client Layer with mock implementation for testing
 */
export const mockGitHubIssueClientLayer = Layer.succeed(
  GitHubIssueClient,
  new GitHubIssueClientMock()
)