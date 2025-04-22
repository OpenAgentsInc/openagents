import { describe, expect, it } from "@effect/vitest"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Option from "effect/Option"
import * as Either from "effect/Either"

import {
  GitHubIssueClient,
  IssueNotFoundError,
  RateLimitExceededError,
  GitHubApiError,
  GitHubIssue,
  GitHubIssueClientLive,
  createMockGitHubIssueClient
} from "../../src/github/GitHubIssueApi.js"

describe("GitHubIssueClient Mock Tests", () => {
  // Use the mock implementation for testing
  const MockLayer = createMockGitHubIssueClient

  it("should successfully fetch an existing issue", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        issueNumber: 1
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      const result = yield* client.fetchIssue(params)

      // Verify the result
      expect(result.number).toBe(1)
      expect(result.title).toBe("Mock Issue #1")
      expect(result.state).toBe("open")
      expect(result.user.login).toBe("mockuser")
      expect(result.labels.length).toBe(2)
      expect(result.labels[0].name).toBe("bug")
      expect(result.labels[1].name).toBe("high-priority")
    }).pipe(
      Effect.provide(MockLayer),
      Effect.runPromise
    ))

  it("should successfully fetch another existing issue", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        issueNumber: 2
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      const result = yield* client.fetchIssue(params)

      // Verify the result
      expect(result.number).toBe(2)
      expect(result.title).toBe("Mock Issue #2")
      expect(result.state).toBe("closed")
      expect(result.user.login).toBe("mockuser2")
      expect(result.labels.length).toBe(1)
      expect(result.labels[0].name).toBe("enhancement")
    }).pipe(
      Effect.provide(MockLayer),
      Effect.runPromise
    ))

  it("should handle issue not found errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        issueNumber: 404
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchIssue(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        expect(error instanceof IssueNotFoundError).toBe(true)
        
        if (error instanceof IssueNotFoundError) {
          expect(error._tag).toBe("IssueNotFoundError")
          expect(error.owner).toBe(params.owner)
          expect(error.repo).toBe(params.repo)
          expect(error.issueNumber).toBe(params.issueNumber)
          expect(error.message).toContain("Issue not found")
        }
      }
    }).pipe(
      Effect.provide(MockLayer),
      Effect.runPromise
    ))

  it("should handle rate limit errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        issueNumber: 9999 // Special case in mock for rate limit
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchIssue(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        expect(error instanceof RateLimitExceededError).toBe(true)
        
        if (error instanceof RateLimitExceededError) {
          expect(error._tag).toBe("RateLimitExceededError")
          expect(error.resetAt).toBeInstanceOf(Date)
          expect(error.message).toContain("GitHub API rate limit exceeded")
        }
      }
    }).pipe(
      Effect.provide(MockLayer),
      Effect.runPromise
    ))

  it("should handle generic API errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        issueNumber: 8888 // Special case in mock for server error
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchIssue(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        expect(error instanceof GitHubApiError).toBe(true)
        
        if (error instanceof GitHubApiError) {
          expect(error._tag).toBe("GitHubApiError")
          expect(error.message).toContain("500")
        }
      }
    }).pipe(
      Effect.provide(MockLayer),
      Effect.runPromise
    ))
})

// These tests use the real GitHub API and are skipped by default
// To run them, you need to:
// 1. Remove the '.skip' to enable the test
// 2. Replace the issue number with a valid one from a real repo
// 3. Optionally add a GitHub token to avoid rate limits
describe.skip("GitHubIssueClient Real API Tests", () => {
  // Replace with real repos and issues for testing
  const realOwner = "nodejs"
  const realRepo = "node"
  const realIssueNumber = 50000 // A real issue number
  
  // Replace with your token or leave undefined for public access
  const githubToken = undefined
  
  // Use the real API for these tests
  const RealApiLayer = Layer.succeed(
    GitHubIssueClient,
    new GitHubIssueClientLive("https://api.github.com", Option.fromNullable(githubToken))
  )

  it("should fetch a real issue from GitHub", () =>
    Effect.gen(function*() {
      const params = {
        owner: realOwner,
        repo: realRepo,
        issueNumber: realIssueNumber
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      
      // Run the request
      const result = yield* client.fetchIssue(params)
      
      // Verify basic structure without asserting specific content
      expect(result.number).toBe(realIssueNumber)
      expect(typeof result.title).toBe("string")
      expect(typeof result.body).toBe("string")
      expect(result.user).toBeDefined()
      expect(typeof result.user.login).toBe("string")
      expect(Array.isArray(result.labels)).toBe(true)
    }).pipe(
      Effect.provide(RealApiLayer),
      Effect.runPromise
    ))

  it("should handle real not found errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: realOwner,
        repo: realRepo,
        issueNumber: 9999999 // An issue that doesn't exist
      }

      // Get the client from the context
      const client = yield* GitHubIssueClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchIssue(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        expect(error instanceof IssueNotFoundError).toBe(true)
      }
    }).pipe(
      Effect.provide(RealApiLayer),
      Effect.runPromise
    ))
})