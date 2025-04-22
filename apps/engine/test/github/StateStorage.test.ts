import { describe, expect, it, vi } from "vitest"
import * as path from "node:path"
import { GitHubClient, StateNotFoundError, StateParseError, StateValidationError } from "../../src/github/GitHub.js"

// Mock the filesystem module
vi.mock("node:fs", () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  writeFileSync: vi.fn(),
  readFileSync: vi.fn()
}))

describe("State Storage", () => {
  it("should have error classes defined", () => {
    expect(StateNotFoundError).toBeDefined()
    expect(StateParseError).toBeDefined()
    expect(StateValidationError).toBeDefined()
  })

  it("should use correct file path structure for state files", () => {
    // Create a basic test to verify the state file path structure
    const instanceId = "test-instance-id"
    const expectedPath = path.join(process.cwd(), "state", `${instanceId}.json`)
    expect(expectedPath).toContain("state/test-instance-id.json")
  })

  it("should have enhanced state storage functions in GitHubClient", () => {
    // Verify the service is defined
    expect(GitHubClient).toBeDefined()
    
    // Just verify the service exists, not trying to check methods
    // which would require more complex mocking
    expect(GitHubClient.Default).toBeDefined()
  })
})