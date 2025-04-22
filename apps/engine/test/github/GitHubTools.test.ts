import { Effect } from "effect"
import { describe, expect, it, vi } from "vitest"
import { GetGitHubIssue, GitHubTools } from "../../src/github/GitHubTools.js"

// Mock the GitHubClient
vi.mock("../../src/github/GitHub.js", async () => {
  return {
    GitHubClient: {
      Default: {},
    },
    GitHubClientLayer: {}
  }
})

describe("GitHubTools", () => {
  it("should define GitHubTools", () => {
    expect(GitHubTools).toBeDefined()
  })

  it("should define GetGitHubIssue tool", () => {
    expect(GetGitHubIssue).toBeDefined()
  })

  it("should create tools for GitHub API operations", () => {
    expect(GitHubTools.tools).toBeDefined()
  })
})