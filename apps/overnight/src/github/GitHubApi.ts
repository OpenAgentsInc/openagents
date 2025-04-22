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
    readonly baseUrl: string,
    private readonly token: Option.Option<string>
  ) {}

  fetchFile(
    params: FetchFilePayload
  ): Effect.Effect<
    GitHubFileContent, 
    FileNotFoundError | RateLimitExceededError | GitHubApiError
  > {
    const { owner, repo, path, ref = "main" } = params
    
    // In a real implementation, we would use the Node fetch API directly
    // For simplicity, we're doing a direct fetch here
    // Ideally, this would be using Effect's HTTP client, but there are import issues
    
    return Effect.tryPromise({
      try: async () => {
        // Create the request URL
        const apiUrl = `${this.baseUrl}/repos/${owner}/${repo}/contents/${path}?ref=${ref}`
        
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
          throw new FileNotFoundError(owner, repo, path)
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
          typeof data.content !== "string" ||
          typeof data.encoding !== "string" ||
          typeof data.sha !== "string" ||
          typeof data.name !== "string" ||
          typeof data.path !== "string" ||
          typeof data.size !== "number"
        ) {
          throw new GitHubApiError({ 
            message: "Invalid response format", 
            cause: new Error("Response doesn't match expected schema") 
          })
        }
        
        return data as GitHubFileContent
      },
      catch: (error) => {
        // Handle errors that weren't caught in the try block
        if (error instanceof FileNotFoundError || 
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