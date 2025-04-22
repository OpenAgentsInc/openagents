// Import vitest utilities
import { describe, expect, it, vi } from "vitest"
import { GitHubTools } from "../../src/github/GitHubTools.js"

// Mock the GitHubClient
vi.mock("../../src/github/GitHub.js", async () => {
  return {
    GitHubClient: {
      Default: {}
    },
    GitHubClientLayer: {}
  }
})

describe("GitHubTools", () => {
  it("should define GitHubTools", () => {
    expect(GitHubTools).toBeDefined()
  })

  it("should have the right class name", () => {
    expect(GitHubTools.fullName).toBe("GitHubTools")
  })
})
