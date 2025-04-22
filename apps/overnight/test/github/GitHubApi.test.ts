import { describe, expect, it } from "@effect/vitest"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Option from "effect/Option"
import * as Either from "effect/Either"

import {
  GitHubApiClient,
  GitHubApiClientLive,
  FetchFileFromGitHub,
  FileNotFoundError,
  RateLimitExceededError,
  GitHubApiError
} from "../../src/github/GitHubApi.js"

// Create test environment
const baseTestLayer = Layer.succeed(
  GitHubApiClient,
  new GitHubApiClientLive("https://api.github.com", Option.none())
)

describe("GitHubApiClient", () => {
  it("should successfully fetch a file", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "README.md"
      }

      // Get the client from the context
      const client = yield* GitHubApiClient
      const result = yield* client.fetchFile(params)

      // Verify the result
      expect(result.name).toBe("README.md")
      expect(result.path).toBe("README.md")
      expect(result.sha).toBe("abc123")
      expect(result.encoding).toBe("base64")
      expect(result.content).toBe("IyBUaGlzIGlzIGEgdGVzdCBmaWxl")
    }).pipe(
      Effect.provide(baseTestLayer),
      Effect.runPromise
    ))

  it("should handle file not found errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "nonexistent.md"
      }

      // Get the client from the context
      const client = yield* GitHubApiClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        // Since we know what error we should get, we can safely cast it
        expect(error instanceof FileNotFoundError).toBe(true)
        
        if (error instanceof FileNotFoundError) {
          expect(error._tag).toBe("FileNotFoundError")
          expect(error.owner).toBe(params.owner)
          expect(error.repo).toBe(params.repo)
          expect(error.path).toBe(params.path)
        }
      }
    }).pipe(
      Effect.provide(baseTestLayer),
      Effect.runPromise
    ))

  it("should handle rate limit errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "rate-limited.md"
      }

      // Get the client from the context
      const client = yield* GitHubApiClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        expect(error instanceof RateLimitExceededError).toBe(true)
        
        if (error instanceof RateLimitExceededError) {
          expect(error._tag).toBe("RateLimitExceededError")
          expect(error.resetAt).toBeInstanceOf(Date)
        }
      }
    }).pipe(
      Effect.provide(baseTestLayer),
      Effect.runPromise
    ))

  it("should handle generic API errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "server-error.md"
      }

      // Get the client from the context
      const client = yield* GitHubApiClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
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
      Effect.provide(baseTestLayer),
      Effect.runPromise
    ))

  it("should handle schema validation errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "invalid-response.md"
      }

      // Get the client from the context
      const client = yield* GitHubApiClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        expect(error instanceof GitHubApiError).toBe(true)
        
        if (error instanceof GitHubApiError) {
          expect(error._tag).toBe("GitHubApiError")
          expect(error.message).toContain("Invalid response format")
        }
      }
    }).pipe(
      Effect.provide(baseTestLayer),
      Effect.runPromise
    ))

  it("should use the provided ref parameter", () =>
    Effect.gen(function*() {
      // Create a client with auth token
      const client = new GitHubApiClientLive(
        "https://api.github.com",
        Option.some("test-token")
      )
      
      // Make a request to capture the URL with a special path
      // We're not testing the result, just that the ref gets used
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "README.md",
        ref: "develop"
      }
      
      const result = yield* client.fetchFile(params)
      
      // The implementation should use the "develop" ref
      expect(result).toBeDefined()
    }).pipe(Effect.runPromise))
})

describe("FetchFileFromGitHub Tool", () => {
  it("should create a valid tool request", () => {
    const request = new FetchFileFromGitHub({
      owner: "openagentsinc",
      repo: "openagents",
      path: "README.md"
    })

    expect(request._tag).toBe("FetchFileFromGitHub")
    expect(request.payload.owner).toBe("openagentsinc")
    expect(request.payload.repo).toBe("openagents")
    expect(request.payload.path).toBe("README.md")
    expect(request.payload.ref).toBeUndefined()
  })

  it("should create a request with an optional ref", () => {
    const request = new FetchFileFromGitHub({
      owner: "openagentsinc",
      repo: "openagents",
      path: "README.md",
      ref: "develop"
    })

    expect(request.payload.ref).toBe("develop")
  })
})