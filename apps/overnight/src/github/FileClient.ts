import * as Context from "effect/Context"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"

import { GitHubHttpExecutor } from "./Client.js"
import { GitHubApiError, RateLimitExceededError, HttpError } from "./Errors.js"

/**
 * GitHub file content response schema
 */
export interface GitHubFileContent {
  content: string
  encoding: string
  sha: string
  name: string
  path: string
  size: number
}

/**
 * File-specific not found error
 */
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

/**
 * Schema for the file fetch payload
 */
export interface FetchFilePayload {
  owner: string
  repo: string
  path: string
  ref?: string
}

/**
 * The FetchFileFromGitHub class for tool usage
 */
export class FetchFileFromGitHub {
  readonly _tag = "FetchFileFromGitHub"
  
  constructor(readonly payload: FetchFilePayload) {}
}

/**
 * GitHub File Client interface
 */
export interface GitHubFileClient {
  fetchFile(params: FetchFilePayload): Effect.Effect<
    GitHubFileContent,
    FileNotFoundError | RateLimitExceededError | GitHubApiError | HttpError
  >
}

/**
 * GitHub File Client service tag
 */
export const GitHubFileClient = Context.GenericTag<GitHubFileClient>("GitHubFileClient")

/**
 * Real implementation of the GitHub File Client
 */
export class GitHubFileClientLive implements GitHubFileClient {
  constructor(
    private readonly executor: GitHubHttpExecutor
  ) {}

  fetchFile(
    params: FetchFilePayload
  ): Effect.Effect<
    GitHubFileContent, 
    FileNotFoundError | RateLimitExceededError | GitHubApiError | HttpError
  > {
    const { owner, repo, path, ref } = params
    
    // Build the request URL path
    const apiPath = `/repos/${owner}/${repo}/contents/${path}`
    
    // Create the request with query params
    const request = HttpClientRequest.get(apiPath).pipe(
      ref ? HttpClientRequest.setUrlParam("ref", ref) : (req) => req
    )
    
    return this.executor.execute<GitHubFileContent>(request)
      .pipe(
        // Map NotFoundError to FileNotFoundError
        Effect.catchTag("NotFoundError", () => 
          Effect.fail(new FileNotFoundError(owner, repo, path))
        )
      )
  }
}

/**
 * Mock implementation of the GitHub File Client for testing
 */
export class GitHubFileClientMock implements GitHubFileClient {
  private readonly mockFiles: Map<string, GitHubFileContent> = new Map()
  
  constructor() {
    // Set up mock files
    this.mockFiles.set("openagentsinc/openagents/README.md", {
      name: "README.md",
      path: "README.md",
      sha: "abc123",
      size: 5432,
      content: "IyBUaGlzIGlzIGEgdGVzdCBmaWxl", // Base64 encoded "# This is a test file"
      encoding: "base64"
    })
  }
  
  fetchFile(params: FetchFilePayload) {
    const { owner, repo, path } = params
    const key = `${owner}/${repo}/${path}`
    const mockFiles = this.mockFiles
    
    return Effect.gen(function*() {
      // Special testing paths
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
        return yield* Effect.fail(new GitHubApiError({ 
          message: "Invalid response format", 
          cause: new Error("Missing required fields") 
        }))
      }
      
      // Look up file in mock DB
      const file = mockFiles.get(key)
      if (!file) {
        return yield* Effect.fail(new FileNotFoundError(owner, repo, path))
      }
      
      return file
    })
  }
}

/**
 * Create GitHub File Client Layer with the real implementation
 */
export const githubFileClientLayer = Layer.effect(
  GitHubFileClient,
  Effect.gen(function*() {
    const executor = yield* GitHubHttpExecutor
    return new GitHubFileClientLive(executor)
  })
)

/**
 * Create GitHub File Client Layer with mock implementation for testing
 */
export const mockGitHubFileClientLayer = Layer.succeed(
  GitHubFileClient,
  new GitHubFileClientMock()
)