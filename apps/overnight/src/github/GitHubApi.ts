import * as Schema from "@effect/schema/Schema"
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Option from "effect/Option"
import * as Data from "effect/Data"

/**
 * GitHub file content response schema
 */
export const GitHubFileContentSchema = Schema.Struct({
  content: Schema.String,
  encoding: Schema.String,
  sha: Schema.String,
  name: Schema.String,
  path: Schema.String,
  size: Schema.Number
})

// Define the type directly since Schema.Type isn't available
export interface GitHubFileContent {
  content: string
  encoding: string
  sha: string
  name: string
  path: string
  size: number
}

/**
 * GitHub API errors
 */
export class GitHubApiError extends Data.TaggedError("GitHubApiError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

export class FileNotFoundError extends Data.TaggedError("FileNotFoundError")<{
  readonly owner: string
  readonly repo: string
  readonly path: string
  readonly message: string
}> {
  constructor(owner: string, repo: string, path: string) {
    super({
      owner,
      repo,
      path,
      message: `File not found: ${owner}/${repo}/${path}`
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
 * Schema for the file fetch payload
 */
export const FetchFilePayloadSchema = Schema.Struct({
  owner: Schema.String.annotations({
    description: "The GitHub repository owner (user or organization)"
  }),
  repo: Schema.String.annotations({
    description: "The GitHub repository name"
  }),
  path: Schema.String.annotations({
    description: "The path to the file within the repository"
  }),
  ref: Schema.optional(Schema.String).annotations({
    description: "The branch, tag, or commit SHA (defaults to the default branch)"
  })
})

// Define the type directly
export interface FetchFilePayload {
  owner: string
  repo: string
  path: string
  ref?: string
}

/**
 * The FetchFileFromGitHub class
 */
export class FetchFileFromGitHub {
  readonly _tag = "FetchFileFromGitHub"
  
  constructor(readonly payload: FetchFilePayload) {}
}

/**
 * GitHub API Client interface
 */
export interface GitHubApiClient {
  fetchFile(params: FetchFilePayload): Effect.Effect<
    GitHubFileContent,
    FileNotFoundError | RateLimitExceededError | GitHubApiError
  >
}

/**
 * GitHub API Client service
 */
export const GitHubApiClient = Context.GenericTag<GitHubApiClient>("GitHubApiClient")

/**
 * Mock implementation of the GitHub API Client for testing
 */
export class GitHubApiClientLive implements GitHubApiClient {
  constructor(
    // We'll need baseUrl in the real implementation 
    // but for the mock we just store it
    readonly baseUrl: string,
    private readonly token: Option.Option<string>
  ) {}

  fetchFile(
    params: FetchFilePayload
  ): Effect.Effect<
    GitHubFileContent, 
    FileNotFoundError | RateLimitExceededError | GitHubApiError
  > {
    const { owner, repo, path } = params
    
    // Capture these values so they can be used in the generator function
    const token = this.token
    
    return Effect.gen(function*() {
      // Simulate HTTP request
      const headers: Record<string, string> = {}
      
      // Add auth token if available
      if (Option.isSome(token)) {
        headers["Authorization"] = `token ${token.value}`
      }

      try {
        // In a real implementation, we would make the actual HTTP request
        // For now, we'll simulate the GitHub API response
        
        // Simulate error conditions for testing
        if (path === "nonexistent.md") {
          return yield* Effect.fail(new FileNotFoundError(owner, repo, path))
        }
        
        if (path === "rate-limited.md") {
          return yield* Effect.fail(new RateLimitExceededError(new Date(Date.now() + 3600000)))
        }
        
        if (path === "server-error.md") {
          return yield* Effect.fail(new GitHubApiError({ message: "GitHub API error: 500 Internal Server Error" }))
        }
        
        if (path === "invalid-response.md") {
          // We'd decode the actual response with Schema.decode(GitHubFileContentSchema)
          return yield* Effect.fail(new GitHubApiError({ 
            message: "Invalid response format", 
            cause: new Error("Missing required fields") 
          }))
        }

        // Return a mock successful response
        return {
          name: path.split("/").pop() || "",
          path,
          sha: "abc123",
          size: 5432,
          content: "IyBUaGlzIGlzIGEgdGVzdCBmaWxl", // Base64 encoded "# This is a test file"
          encoding: "base64"
        }
      } catch (error) {
        return yield* Effect.fail(new GitHubApiError({ 
          message: "Failed to fetch file from GitHub", 
          cause: error 
        }))
      }
    })
  }
}

/**
 * Create a GitHub API Client Layer
 */
export const createGitHubApiClient = (baseUrl = "https://api.github.com", token?: string) =>
  Layer.succeed(
    GitHubApiClient,
    new GitHubApiClientLive(
      baseUrl,
      Option.fromNullable(token)
    )
  )