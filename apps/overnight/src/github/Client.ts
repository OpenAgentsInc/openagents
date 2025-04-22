import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"

import { GitHubApiError, RateLimitExceededError, HttpError, NotFoundError } from "./Errors.js"

/**
 * GitHub Config interface
 */
export interface GitHubConfig {
  readonly baseUrl: string
  readonly token?: string | undefined
}

/**
 * GitHub Config Tag
 */
export const GitHubConfigTag = Context.GenericTag<GitHubConfig>("GitHubConfig")

/**
 * GitHub HTTP Executor interface
 */
export interface GitHubHttpExecutor {
  execute<A>(
    request: HttpClientRequest.HttpClientRequest
  ): Effect.Effect<A, NotFoundError | RateLimitExceededError | GitHubApiError | HttpError>
}

/**
 * GitHub HTTP Executor Tag
 */
export const GitHubHttpExecutor = Context.GenericTag<GitHubHttpExecutor>("GitHubHttpExecutor")

/**
 * Live implementation of GitHub HTTP Executor
 */
export class GitHubHttpExecutorLive implements GitHubHttpExecutor {
  constructor(
    private readonly config: GitHubConfig
  ) {}

  execute<A>(
    request: HttpClientRequest.HttpClientRequest
  ): Effect.Effect<A, NotFoundError | RateLimitExceededError | GitHubApiError | HttpError> {
    const { baseUrl, token } = this.config

    // Prepare the request
    const preparedRequest = request.pipe(
      // Use full URL if provided, or prepend baseUrl if relative
      (req) => {
        const url = new URL(req.url, baseUrl)
        return HttpClientRequest.setUrl(url.toString())(req)
      },
      // Add standard headers
      HttpClientRequest.setHeader("Accept", "application/vnd.github.v3+json"),
      // Add auth token if available
      (req) => token ? HttpClientRequest.setHeader("Authorization", `token ${token}`)(req) : req
    )

    // Execute the request and handle the response
    return Effect.tryPromise({
      try: async () => {
        // Execute HTTP request
        const response = await fetch(preparedRequest.url, {
          method: preparedRequest.method,
          headers: Object.fromEntries(
            Object.entries(preparedRequest.headers).map(([k, v]) => [k, Array.isArray(v) ? v.join(", ") : String(v)])
          )
          // Don't include body as it's not needed for GET requests
        })

        // Check rate limit headers
        const rateLimitRemaining = response.headers.get("x-ratelimit-remaining")
        const rateLimitReset = response.headers.get("x-ratelimit-reset")

        if (rateLimitRemaining === "0" && rateLimitReset) {
          const resetTime = new Date(parseInt(rateLimitReset) * 1000)
          throw new RateLimitExceededError(resetTime)
        }

        // Check for common status codes
        if (response.status === 404) {
          throw new NotFoundError(preparedRequest.url)
        }

        if (response.status === 403) {
          throw new GitHubApiError({ 
            message: `GitHub API authorization failed: ${response.status}`
          })
        }

        if (response.status >= 400) {
          throw new HttpError(response.status, "Error")
        }

        // Parse the JSON response
        const data = await response.json()
        return data as A
      },
      catch: (error): NotFoundError | RateLimitExceededError | GitHubApiError | HttpError => {
        // Pass through our defined errors
        if (
          error instanceof NotFoundError || 
          error instanceof RateLimitExceededError || 
          error instanceof GitHubApiError ||
          error instanceof HttpError
        ) {
          return error
        }

        // Wrap any other errors
        return new GitHubApiError({ 
          message: `GitHub API error: ${error instanceof Error ? error.message : String(error)}`,
          cause: error
        })
      }
    })
  }
}

/**
 * Create GitHub HTTP Executor Layer
 */
export const createGitHubHttpExecutor = Layer.effect(
  GitHubHttpExecutor,
  Effect.gen(function*() {
    const config = yield* GitHubConfigTag
    return new GitHubHttpExecutorLive(config)
  })
)

/**
 * Create GitHub Config Layer
 */
export const createGitHubConfig = (baseUrl = "https://api.github.com", token?: string) =>
  Layer.succeed(
    GitHubConfigTag,
    { baseUrl, token }
  )

/**
 * Default GitHub Layer (combines config and executor)
 */
export const defaultGitHubLayer = Layer.provide(
  createGitHubHttpExecutor,
  createGitHubConfig()
)