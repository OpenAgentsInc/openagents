import { describe, expect, it } from "@effect/vitest"
import * as Effect from "effect/Effect"
import * as Either from "effect/Either"

import {
  GitHubFileClient,
  FetchFileFromGitHub,
  FileNotFoundError,
  mockGitHubFileClientLayer
} from "../../src/github/FileClient.js"

import {
  RateLimitExceededError,
  GitHubApiError
} from "../../src/github/Errors.js"

describe("GitHubFileClient", () => {
  it("should successfully fetch a file", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "README.md"
      }

      // Get the client from the context
      const client = yield* GitHubFileClient
      const result = yield* client.fetchFile(params)

      // Verify the result
      expect(result.name).toBe("README.md")
      expect(result.path).toBe("README.md")
      expect(result.sha).toBe("abc123")
      expect(result.encoding).toBe("base64")
      expect(result.content).toBe("IyBUaGlzIGlzIGEgdGVzdCBmaWxl")
    }).pipe(
      Effect.provide(mockGitHubFileClientLayer)
    ))

  it("should handle file not found errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "nonexistent.md"
      }

      // Get the client from the context
      const client = yield* GitHubFileClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        if (error instanceof FileNotFoundError) {
          expect(error._tag).toBe("FileNotFoundError")
          expect(error.owner).toBe(params.owner)
          expect(error.repo).toBe(params.repo)
          expect(error.path).toBe(params.path)
        } else {
          throw new Error(`Expected FileNotFoundError but got ${error}`)
        }
      }
    }).pipe(
      Effect.provide(mockGitHubFileClientLayer)
    ))

  it("should handle rate limit errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "rate-limited.md"
      }

      // Get the client from the context
      const client = yield* GitHubFileClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        if (error instanceof RateLimitExceededError) {
          expect(error._tag).toBe("RateLimitExceededError")
          expect(error.resetAt).toBeInstanceOf(Date)
        } else {
          throw new Error(`Expected RateLimitExceededError but got ${error}`)
        }
      }
    }).pipe(
      Effect.provide(mockGitHubFileClientLayer)
    ))

  it("should handle generic API errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "server-error.md"
      }

      // Get the client from the context
      const client = yield* GitHubFileClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        if (error instanceof GitHubApiError) {
          expect(error._tag).toBe("GitHubApiError")
          expect(error.message).toContain("500")
        } else {
          throw new Error(`Expected GitHubApiError but got ${error}`)
        }
      }
    }).pipe(
      Effect.provide(mockGitHubFileClientLayer)
    ))

  it("should handle schema validation errors", () =>
    Effect.gen(function*() {
      const params = {
        owner: "openagentsinc",
        repo: "openagents",
        path: "invalid-response.md"
      }

      // Get the client from the context
      const client = yield* GitHubFileClient
      
      // Run the request and catch the error
      const result = yield* Effect.either(client.fetchFile(params))
      
      // Verify we get the expected error
      expect(Either.isLeft(result)).toBe(true)
      
      if (Either.isLeft(result)) {
        const error = result.left
        
        if (error instanceof GitHubApiError) {
          expect(error._tag).toBe("GitHubApiError")
          expect(error.message).toContain("Invalid response format")
        } else {
          throw new Error(`Expected GitHubApiError but got ${error}`)
        }
      }
    }).pipe(
      Effect.provide(mockGitHubFileClientLayer)
    ))
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