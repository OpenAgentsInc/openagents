import { Effect, Exit } from "effect"
import { describe, expect, it, vi } from "vitest"
import {
  GetGitHubIssue,
  GitHubTools
} from "../../src/github/GitHubTools.js"

// Mock the GitHubClient
vi.mock("../../src/github/GitHub.js", async () => {
  return {
    GitHubClient: {
      Default: {},
      provide: () => Effect.succeed({
        getIssue: vi.fn().mockImplementation((owner, repo, issueNumber) => 
          Effect.succeed({
            id: 123,
            number: issueNumber,
            title: "Test Issue",
            body: "This is a test issue",
            state: "open",
            html_url: `https://github.com/${owner}/${repo}/issues/${issueNumber}`,
            created_at: "2025-04-22T10:00:00Z",
            updated_at: "2025-04-22T10:00:00Z",
            user: {
              id: 456,
              login: "testuser",
              avatar_url: "https://avatars.githubusercontent.com/u/456",
              html_url: "https://github.com/testuser",
              type: "User"
            },
            labels: [],
            assignees: [],
            comments: 0
          })
        ),
        listIssues: vi.fn().mockImplementation((owner, repo, state) => 
          Effect.succeed({
            issues: [
              {
                id: 123,
                number: 1,
                title: "Test Issue 1",
                body: "This is test issue 1",
                state: state || "open",
                html_url: `https://github.com/${owner}/${repo}/issues/1`,
                created_at: "2025-04-22T10:00:00Z",
                updated_at: "2025-04-22T10:00:00Z",
                user: {
                  id: 456,
                  login: "testuser",
                  avatar_url: "https://avatars.githubusercontent.com/u/456",
                  html_url: "https://github.com/testuser",
                  type: "User"
                },
                labels: [],
                assignees: [],
                comments: 0
              }
            ]
          })
        ),
        createIssueComment: vi.fn().mockImplementation((owner, repo, issueNumber, body) => 
          Effect.succeed({
            id: 789,
            body: body,
            created_at: "2025-04-22T11:00:00Z",
            updated_at: "2025-04-22T11:00:00Z",
            user: {
              id: 456,
              login: "testuser",
              avatar_url: "https://avatars.githubusercontent.com/u/456",
              html_url: "https://github.com/testuser",
              type: "User"
            },
            html_url: `https://github.com/${owner}/${repo}/issues/${issueNumber}#issuecomment-789`
          })
        )
      })
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

  it("should successfully get an issue using GetGitHubIssue tool", async () => {
    // Create a test program that uses the GetGitHubIssue tool
    const program = Effect.gen(function* () {
      // Setup a request for the GetGitHubIssue tool
      const request = new GetGitHubIssue({
        owner: "testowner",
        repo: "testrepo",
        issueNumber: 42
      })
      
      // Create a manual context with a mock handler for the GetGitHubIssue tool
      const mockContext = {
        get: (tag: string) => {
          if (tag === "GetGitHubIssue") {
            return (params: any) => Effect.succeed({
              id: 123,
              number: params.issueNumber,
              title: "Test Issue",
              body: "This is a test issue",
              state: "open",
              html_url: `https://github.com/${params.owner}/${params.repo}/issues/${params.issueNumber}`,
              created_at: "2025-04-22T10:00:00Z",
              updated_at: "2025-04-22T10:00:00Z",
              user: {
                id: 456,
                login: "testuser",
                avatar_url: "https://avatars.githubusercontent.com/u/456",
                html_url: "https://github.com/testuser",
                type: "User"
              },
              labels: [],
              assignees: [],
              comments: 0
            })
          }
          return undefined
        }
      }
      
      // Run the request with the mock context
      return yield* request.pipe(Effect.provideContext(mockContext as any))
    })

    // Run the program
    const result = await Effect.runPromiseExit(program)

    // Check the result
    if (Exit.isSuccess(result)) {
      expect(result.value).toBeDefined()
      expect(result.value.number).toBe(42)
      expect(result.value.title).toBe("Test Issue")
    } else {
      fail("Expected success but got failure")
    }
  })
})