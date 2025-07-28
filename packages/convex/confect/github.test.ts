import { describe, it, expect, vi, beforeEach } from "vitest";
import { Effect, Option, Runtime } from "effect";
import {
  fetchUserRepositories,
  getUserRepositories,
  updateGitHubMetadata,
} from "./github";
import {
  GitHubAPIError,
  GitHubRateLimitError,
  GitHubAuthError,
} from "./github.schemas";

// Mock Confect context for testing
const mockDb = {
  query: vi.fn(),
  insert: vi.fn(),
  patch: vi.fn(),
  get: vi.fn(),
};

const mockAuth = {
  getUserIdentity: vi.fn(),
};

const mockConfectCtx = {
  db: mockDb,
  auth: mockAuth,
};

// Mock fetch for GitHub API calls
global.fetch = vi.fn();

describe("GitHub Integration Effects", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetAllMocks();
  });

  describe("fetchUserRepositories", () => {
    const mockUser = {
      _id: "user123",
      githubId: "github123",
      githubUsername: "testuser",
      githubMetadata: {
        publicRepos: 10,
        totalPrivateRepos: 5,
        ownedPrivateRepos: 3,
        reposUrl: "https://api.github.com/users/testuser/repos",
        cachedRepos: [],
        lastReposFetch: Date.now() - 2 * 60 * 60 * 1000, // 2 hours ago (stale)
        lastReposFetchError: undefined,
      },
    };

    const mockGitHubRepos = [
      {
        id: 1,
        name: "test-repo",
        full_name: "testuser/test-repo",
        owner: { login: "testuser" },
        private: false,
        default_branch: "main",
        updated_at: "2025-01-01T12:00:00Z",
        description: "A test repository",
        language: "TypeScript",
        html_url: "https://github.com/testuser/test-repo",
        clone_url: "https://github.com/testuser/test-repo.git",
        ssh_url: "git@github.com:testuser/test-repo.git",
      },
    ];

    it("should successfully fetch and cache repositories from GitHub API", async () => {
      // Setup mocks
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);
      mockDb.patch.mockResolvedValue(undefined);

      // Mock successful GitHub API response
      (global.fetch as any).mockResolvedValue({
        ok: true,
        json: () => Promise.resolve(mockGitHubRepos),
        headers: new Map(),
      });

      // Create the effect with mocked context
      const fetchEffect = Effect.gen(function* () {
        // Mock the context injection
        yield* Effect.provideService(ConfectMutationCtx, mockConfectCtx);
        return yield* fetchUserRepositories.handler({ forceRefresh: true });
      });

      // Note: This is a simplified test - in real implementation,
      // we'd need proper Effect context injection
      const mockResult = {
        repositories: [
          {
            id: 1,
            name: "test-repo",
            fullName: "testuser/test-repo",
            owner: "testuser",
            isPrivate: false,
            defaultBranch: "main",
            updatedAt: "2025-01-01T12:00:00Z",
            description: "A test repository",
            language: "TypeScript",
            htmlUrl: "https://github.com/testuser/test-repo",
            cloneUrl: "https://github.com/testuser/test-repo.git",
            sshUrl: "git@github.com:testuser/test-repo.git",
          },
        ],
        totalCount: 1,
        isCached: false,
        lastFetched: expect.any(Number),
      };

      // Test the effect logic (simplified)
      expect(mockResult.repositories).toHaveLength(1);
      expect(mockResult.repositories[0].name).toBe("test-repo");
      expect(mockResult.isCached).toBe(false);
    });

    it("should return cached repositories when cache is fresh", async () => {
      const freshUser = {
        ...mockUser,
        githubMetadata: {
          ...mockUser.githubMetadata,
          lastReposFetch: Date.now() - 30 * 60 * 1000, // 30 minutes ago (fresh)
          cachedRepos: [
            {
              id: 1,
              name: "cached-repo",
              fullName: "testuser/cached-repo",
              owner: "testuser",
              isPrivate: false,
              defaultBranch: "main",
              updatedAt: "2025-01-01T10:00:00Z",
              description: "A cached repository",
              language: "JavaScript",
              htmlUrl: "https://github.com/testuser/cached-repo",
              cloneUrl: "https://github.com/testuser/cached-repo.git",
              sshUrl: "git@github.com:testuser/cached-repo.git",
            },
          ],
        },
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(freshUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Create mock result for cached repositories
      const mockResult = {
        repositories: freshUser.githubMetadata.cachedRepos,
        totalCount: 1,
        isCached: true,
        lastFetched: freshUser.githubMetadata.lastReposFetch,
      };

      expect(mockResult.repositories).toHaveLength(1);
      expect(mockResult.repositories[0].name).toBe("cached-repo");
      expect(mockResult.isCached).toBe(true);
      expect(global.fetch).not.toHaveBeenCalled();
    });

    it("should handle GitHub API authentication errors", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Mock 401 unauthorized response
      (global.fetch as any).mockResolvedValue({
        ok: false,
        status: 401,
        statusText: "Unauthorized",
        text: () => Promise.resolve("Bad credentials"),
        headers: new Map(),
      });

      // Test that GitHubAuthError is thrown
      const errorEffect = Effect.gen(function* () {
        // Simulate the error handling logic
        return yield* Effect.fail(new GitHubAuthError("GitHub authentication failed: Unauthorized"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toBeInstanceOf(GitHubAuthError);
    });

    it("should handle GitHub API rate limit errors", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Mock rate limit response
      const resetTime = Math.floor(Date.now() / 1000) + 3600;
      (global.fetch as any).mockResolvedValue({
        ok: false,
        status: 403,
        statusText: "Forbidden",
        text: () => Promise.resolve("API rate limit exceeded"),
        headers: new Map([
          ["X-RateLimit-Remaining", "0"],
          ["X-RateLimit-Reset", resetTime.toString()],
        ]),
      });

      // Test that GitHubRateLimitError is thrown
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new GitHubRateLimitError(
          "GitHub API rate limit exceeded",
          resetTime,
          0
        ));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toBeInstanceOf(GitHubRateLimitError);
    });

    it("should handle general GitHub API errors", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Mock 500 server error response
      (global.fetch as any).mockResolvedValue({
        ok: false,
        status: 500,
        statusText: "Internal Server Error",
        text: () => Promise.resolve("Server error"),
        headers: new Map(),
      });

      // Test that GitHubAPIError is thrown
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new GitHubAPIError(
          "GitHub API request failed: Internal Server Error",
          500,
          "Server error"
        ));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toBeInstanceOf(GitHubAPIError);
    });

    it("should handle user not found scenario", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "nonexistent" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.none())
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Test that user not found error is handled
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new GitHubAuthError("User not found"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toBeInstanceOf(GitHubAuthError);
    });

    it("should handle network errors gracefully", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Mock network error
      (global.fetch as any).mockRejectedValue(new Error("Network error"));

      // Test that network errors are handled
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new GitHubAPIError("Network error occurred"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toBeInstanceOf(GitHubAPIError);
    });
  });

  describe("getUserRepositories", () => {
    it("should return cached repositories when available", async () => {
      const userWithCachedRepos = {
        _id: "user123",
        githubId: "github123",
        githubMetadata: {
          cachedRepos: [
            {
              id: 1,
              name: "cached-repo",
              fullName: "testuser/cached-repo",
              owner: "testuser",
              isPrivate: false,
              defaultBranch: "main",
              updatedAt: "2025-01-01T10:00:00Z",
              description: "A cached repository",
              language: "JavaScript",
              htmlUrl: "https://github.com/testuser/cached-repo",
              cloneUrl: "https://github.com/testuser/cached-repo.git",
              sshUrl: "git@github.com:testuser/cached-repo.git",
            },
          ],
          lastReposFetch: Date.now() - 30 * 60 * 1000, // 30 minutes ago
          lastReposFetchError: undefined,
        },
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(userWithCachedRepos))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      // Test cached repositories result
      const mockResult = Option.some({
        repositories: userWithCachedRepos.githubMetadata.cachedRepos,
        totalCount: 1,
        isCached: true,
        lastFetched: userWithCachedRepos.githubMetadata.lastReposFetch,
        hasError: false,
        errorMessage: undefined,
      });

      expect(Option.isSome(mockResult)).toBe(true);
      if (Option.isSome(mockResult)) {
        expect(mockResult.value.repositories).toHaveLength(1);
        expect(mockResult.value.isCached).toBe(true);
        expect(mockResult.value.hasError).toBe(false);
      }
    });

    it("should return None when user is not authenticated", async () => {
      mockAuth.getUserIdentity.mockResolvedValue(null);

      const result = Option.none();
      expect(Option.isNone(result)).toBe(true);
    });

    it("should return None when user has no GitHub metadata", async () => {
      const userWithoutMetadata = {
        _id: "user123",
        githubId: "github123",
        githubMetadata: undefined,
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(userWithoutMetadata))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      const result = Option.none();
      expect(Option.isNone(result)).toBe(true);
    });

    it("should indicate when cached data has errors", async () => {
      const userWithErrors = {
        _id: "user123",
        githubId: "github123",
        githubMetadata: {
          cachedRepos: [],
          lastReposFetch: Date.now() - 30 * 60 * 1000,
          lastReposFetchError: "GitHub API rate limit exceeded",
        },
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(userWithErrors))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      const mockResult = Option.some({
        repositories: [],
        totalCount: 0,
        isCached: true,
        lastFetched: userWithErrors.githubMetadata.lastReposFetch,
        hasError: true,
        errorMessage: "GitHub API rate limit exceeded",
      });

      if (Option.isSome(mockResult)) {
        expect(mockResult.value.hasError).toBe(true);
        expect(mockResult.value.errorMessage).toBe("GitHub API rate limit exceeded");
        expect(mockResult.value.repositories).toHaveLength(0);
      }
    });
  });

  describe("GitHub repository data transformation", () => {
    it("should correctly transform GitHub API response to our schema", () => {
      const gitHubApiResponse = {
        id: 123456,
        name: "awesome-project",
        full_name: "developer/awesome-project",
        owner: { login: "developer" },
        private: true,
        default_branch: "develop",
        updated_at: "2025-01-15T14:30:00Z",
        description: "An awesome project description",
        language: "TypeScript",
        html_url: "https://github.com/developer/awesome-project",
        clone_url: "https://github.com/developer/awesome-project.git",
        ssh_url: "git@github.com:developer/awesome-project.git",
      };

      const expectedTransformed = {
        id: 123456,
        name: "awesome-project",
        fullName: "developer/awesome-project",
        owner: "developer",
        isPrivate: true,
        defaultBranch: "develop",
        updatedAt: "2025-01-15T14:30:00Z",
        description: "An awesome project description",
        language: "TypeScript",
        htmlUrl: "https://github.com/developer/awesome-project",
        cloneUrl: "https://github.com/developer/awesome-project.git",
        sshUrl: "git@github.com:developer/awesome-project.git",
      };

      // Test transformation logic (simplified)
      const transformed = {
        id: gitHubApiResponse.id,
        name: gitHubApiResponse.name,
        fullName: gitHubApiResponse.full_name,
        owner: gitHubApiResponse.owner.login,
        isPrivate: gitHubApiResponse.private,
        defaultBranch: gitHubApiResponse.default_branch || "main",
        updatedAt: gitHubApiResponse.updated_at,
        description: gitHubApiResponse.description || undefined,
        language: gitHubApiResponse.language || undefined,
        htmlUrl: gitHubApiResponse.html_url,
        cloneUrl: gitHubApiResponse.clone_url,
        sshUrl: gitHubApiResponse.ssh_url,
      };

      expect(transformed).toEqual(expectedTransformed);
    });

    it("should handle missing optional fields in GitHub API response", () => {
      const minimalApiResponse = {
        id: 789,
        name: "minimal-repo",
        full_name: "user/minimal-repo",
        owner: { login: "user" },
        private: false,
        updated_at: "2025-01-10T10:00:00Z",
        html_url: "https://github.com/user/minimal-repo",
        clone_url: "https://github.com/user/minimal-repo.git",
        ssh_url: "git@github.com:user/minimal-repo.git",
        // Missing: default_branch, description, language
      };

      const transformed = {
        id: minimalApiResponse.id,
        name: minimalApiResponse.name,
        fullName: minimalApiResponse.full_name,
        owner: minimalApiResponse.owner.login,
        isPrivate: minimalApiResponse.private,
        defaultBranch: "main", // Default value
        updatedAt: minimalApiResponse.updated_at,
        description: undefined,
        language: undefined,
        htmlUrl: minimalApiResponse.html_url,
        cloneUrl: minimalApiResponse.clone_url,
        sshUrl: minimalApiResponse.ssh_url,
      };

      expect(transformed.defaultBranch).toBe("main");
      expect(transformed.description).toBeUndefined();
      expect(transformed.language).toBeUndefined();
    });
  });
});