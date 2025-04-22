import * as Schema from "@effect/schema/Schema"
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Option from "effect/Option"
import * as Data from "effect/Data"

/**
 * GitHub issue response schema
 */
export const GitHubIssueSchema = Schema.Struct({
  id: Schema.Number,
  number: Schema.Number,
  title: Schema.String,
  state: Schema.Union(
    Schema.Literal("open"),
    Schema.Literal("closed")
  ),
  body: Schema.String,
  user: Schema.Struct({
    login: Schema.String
  }),
  created_at: Schema.String,
  updated_at: Schema.String,
  labels: Schema.Array(Schema.Struct({
    name: Schema.String
  }))
})

// Define the type directly
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
 * GitHub API errors
 */
export class GitHubApiError extends Data.TaggedError("GitHubApiError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

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

export class RateLimitExceededError extends Data.TaggedError("RateLimitExceededError")<{
  readonly resetAt: Date
  readonly message: string
}> {
  constructor(resetAt: Date) {
    super({
      resetAt,
      message: `GitHub API rate limit exceeded. Resets at ${resetAt.toISOString()}`
    })
  }
}

/**
 * Schema for the issue fetch payload
 */
export const FetchIssuePayloadSchema = Schema.Struct({
  owner: Schema.String.annotations({
    description: "The GitHub repository owner (user or organization)"
  }),
  repo: Schema.String.annotations({
    description: "The GitHub repository name"
  }),
  issueNumber: Schema.Number.annotations({
    description: "The issue number"
  })
})

// Define the type directly
export interface FetchIssuePayload {
  owner: string
  repo: string
  issueNumber: number
}

/**
 * The FetchIssueFromGitHub class
 */
export class FetchIssueFromGitHub {
  readonly _tag = "FetchIssueFromGitHub"
  
  constructor(readonly payload: FetchIssuePayload) {}
}

/**
 * GitHub Issue API Client interface
 */
export interface GitHubIssueClient {
  fetchIssue(params: FetchIssuePayload): Effect.Effect<
    GitHubIssue,
    IssueNotFoundError | RateLimitExceededError | GitHubApiError
  >
}

/**
 * GitHub Issue API Client service
 */
export const GitHubIssueClient = Context.GenericTag<GitHubIssueClient>("GitHubIssueClient")

/**
 * Real implementation of the GitHub Issue API Client
 */
export class GitHubIssueClientLive implements GitHubIssueClient {
  constructor(
    readonly baseUrl: string,
    private readonly token: Option.Option<string>
  ) {}

  fetchIssue(
    params: FetchIssuePayload
  ): Effect.Effect<
    GitHubIssue, 
    IssueNotFoundError | RateLimitExceededError | GitHubApiError
  > {
    const { owner, repo, issueNumber } = params
    
    return Effect.tryPromise({
      try: async () => {
        // Create the request URL
        const apiUrl = `${this.baseUrl}/repos/${owner}/${repo}/issues/${issueNumber}`
        
        // Set up headers
        const headers: Record<string, string> = {
          "Accept": "application/vnd.github.v3+json"
        }
        
        // Add auth token if available
        if (Option.isSome(this.token)) {
          headers["Authorization"] = `token ${this.token.value}`
        }
        
        // Make the request
        const response = await fetch(apiUrl, { headers })
        
        // Handle rate limiting
        const rateLimitRemaining = response.headers.get("x-ratelimit-remaining")
        const rateLimitReset = response.headers.get("x-ratelimit-reset")
        
        if (rateLimitRemaining === "0" && rateLimitReset) {
          const resetTime = new Date(parseInt(rateLimitReset) * 1000)
          throw new RateLimitExceededError(resetTime)
        }
        
        // Handle 404
        if (response.status === 404) {
          throw new IssueNotFoundError(owner, repo, issueNumber)
        }
        
        // Handle other errors
        if (!response.ok) {
          throw new GitHubApiError({ 
            message: `GitHub API error: ${response.status} ${response.statusText}`
          })
        }
        
        // Parse the response
        const data = await response.json()
        
        // Validate the response shape
        if (
          typeof data.id !== "number" ||
          typeof data.number !== "number" ||
          typeof data.title !== "string" ||
          typeof data.body !== "string" ||
          !data.user || typeof data.user.login !== "string" ||
          typeof data.created_at !== "string" ||
          typeof data.updated_at !== "string" ||
          !Array.isArray(data.labels)
        ) {
          throw new GitHubApiError({ 
            message: "Invalid response format", 
            cause: new Error("Response doesn't match expected schema") 
          })
        }
        
        return data as GitHubIssue
      },
      catch: (error) => {
        // Handle errors that weren't caught in the try block
        if (error instanceof IssueNotFoundError || 
            error instanceof RateLimitExceededError || 
            error instanceof GitHubApiError) {
          return error
        }
        
        // For any other errors, wrap them in a GitHubApiError
        return new GitHubApiError({ 
          message: `GitHub API error: ${error instanceof Error ? error.message : String(error)}`, 
          cause: error 
        })
      }
    })
  }
}

/**
 * Mock implementation of the GitHub Issue API Client for testing
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
 * Create a GitHub Issue API Client Layer with real API
 */
export const createGitHubIssueClient = (baseUrl = "https://api.github.com", token?: string) =>
  Layer.succeed(
    GitHubIssueClient,
    new GitHubIssueClientLive(
      baseUrl,
      Option.fromNullable(token)
    )
  )

/**
 * Create a GitHub Issue API Client Layer with mock implementation for testing
 */
export const createMockGitHubIssueClient = Layer.succeed(
  GitHubIssueClient,
  new GitHubIssueClientMock()
)