// Import vitest utilities
import { describe, expect, it, vi } from "vitest"
import { GitHubClient } from "../../src/github/GitHub.js"

// Mock the HTTP client
vi.mock("@effect/platform", async () => {
  const actual = await vi.importActual("@effect/platform") as object
  return {
    ...actual,
    HttpClient: {
      HttpClient: {
        pipe: () => ({
          get: vi.fn(),
          post: vi.fn(),
          patch: vi.fn(),
        })
      }
    },
    HttpClientError: {
      isHttpClientError: vi.fn()
    }
  }
})

// Mock the NodeHttpClient
vi.mock("@effect/platform-node", () => ({
  NodeHttpClient: {
    layerUndici: {
      // mock Layer implementation
    }
  }
}))

// Mock the filesystem
vi.mock("node:fs", () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  writeFileSync: vi.fn(),
  readFileSync: vi.fn(),
}))

describe("GitHubClient", () => {
  it("should define GitHubClient service", () => {
    expect(GitHubClient).toBeDefined()
  })

  it("should define GitHubClientLayer", () => {
    expect(GitHubClient.Default).toBeDefined()
  })

  it("should have a layer structure", () => {
    expect(typeof GitHubClient.Default).toBe("object")
  })
})