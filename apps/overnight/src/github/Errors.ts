import * as Data from "effect/Data"

/**
 * Common GitHub API errors
 */

/**
 * Generic GitHub API Error
 */
export class GitHubApiError extends Data.TaggedError("GitHubApiError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

/**
 * Rate limit exceeded error
 */
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
 * Generic HTTP error
 */
export class HttpError extends Data.TaggedError("HttpError")<{
  readonly status: number
  readonly statusText: string
  readonly message: string
}> {
  constructor(status: number, statusText: string) {
    super({
      status,
      statusText,
      message: `HTTP Error: ${status} ${statusText}`
    })
  }
}

/**
 * Generic Not Found error
 */
export class NotFoundError extends Data.TaggedError("NotFoundError")<{
  readonly resource: string
  readonly message: string
}> {
  constructor(resource: string) {
    super({
      resource,
      message: `Resource not found: ${resource}`
    })
  }
}
