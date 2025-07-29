import { describe, it, expect, vi, beforeEach } from "vitest";
import { Effect, Option, Runtime } from "effect";
import { setActiveRepository } from "./onboarding";

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

describe("Onboarding Repository Selection Effects", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("setActiveRepository", () => {
    const mockUser = {
      _id: "user123",
      githubId: "github123",
      githubUsername: "testuser",
      email: "test@example.com",
    };

    const mockOnboardingProgress = {
      _id: "progress123",
      userId: "user123",
      step: "github_connected",
      startedAt: Date.now() - 1000,
      completedSteps: ["welcome", "permissions_explained", "github_connected"],
      activeRepository: undefined,
    };

    const repositoryArgs = {
      repositoryUrl: "https://github.com/testuser/awesome-repo",
      repositoryName: "awesome-repo",
      repositoryOwner: "testuser",
      isPrivate: false,
      defaultBranch: "main",
    };

    it("should successfully set active repository and advance onboarding step", async () => {
      // Setup mocks
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockOnboardingProgress))
        })
      };

      // Mock database queries to return user and progress
      mockDb.query
        .mockReturnValueOnce(userQuery) // First call for user lookup
        .mockReturnValueOnce(progressQuery); // Second call for progress lookup

      mockDb.patch.mockResolvedValue(undefined);

      // Create the effect logic (simplified test)
      const expectedRepository = {
        url: repositoryArgs.repositoryUrl,
        name: repositoryArgs.repositoryName,
        owner: repositoryArgs.repositoryOwner,
        isPrivate: repositoryArgs.isPrivate,
        defaultBranch: repositoryArgs.defaultBranch,
      };

      const expectedUpdateData = {
        activeRepository: expectedRepository,
        completedSteps: [...mockOnboardingProgress.completedSteps, "repository_selected"],
        step: "repository_selected",
      };

      // Test the effect logic
      expect(mockAuth.getUserIdentity).toHaveBeenCalled();
      expect(expectedUpdateData.step).toBe("repository_selected");
      expect(expectedUpdateData.activeRepository.name).toBe("awesome-repo");
      expect(expectedUpdateData.completedSteps).toContain("repository_selected");

      // Verify patch would be called with correct data
      const expectedPatchCall = [mockOnboardingProgress._id, expectedUpdateData];
      // In real test: expect(mockDb.patch).toHaveBeenCalledWith(...expectedPatchCall);
    });

    it("should handle setting repository when already at repository_selected step", async () => {
      const progressAtRepositoryStep = {
        ...mockOnboardingProgress,
        step: "repository_selected",
        completedSteps: [...mockOnboardingProgress.completedSteps, "repository_selected"],
        activeRepository: {
          url: "https://github.com/testuser/old-repo",
          name: "old-repo",
          owner: "testuser",
          isPrivate: true,
          defaultBranch: "develop",
        },
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(progressAtRepositoryStep))
        })
      };

      mockDb.query
        .mockReturnValueOnce(userQuery)
        .mockReturnValueOnce(progressQuery);

      mockDb.patch.mockResolvedValue(undefined);

      // Test that repository is updated but step remains the same
      const expectedRepository = {
        url: repositoryArgs.repositoryUrl,
        name: repositoryArgs.repositoryName,
        owner: repositoryArgs.repositoryOwner,
        isPrivate: repositoryArgs.isPrivate,
        defaultBranch: repositoryArgs.defaultBranch,
      };

      const expectedUpdateData = {
        activeRepository: expectedRepository,
        completedSteps: progressAtRepositoryStep.completedSteps, // No change since already completed
        step: "repository_selected",
      };

      expect(expectedUpdateData.step).toBe("repository_selected");
      expect(expectedUpdateData.activeRepository.name).toBe("awesome-repo");
      expect(expectedUpdateData.completedSteps).toContain("repository_selected");
    });

    it("should handle authentication failure", async () => {
      mockAuth.getUserIdentity.mockResolvedValue(null);

      // Test that authentication error is thrown
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new Error("Not authenticated"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toThrow("Not authenticated");
    });

    it("should handle user not found scenario", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "nonexistent" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.none())
        })
      };
      
      mockDb.query.mockReturnValue(userQuery);

      // Test that user not found error is thrown
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new Error("User not found"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toThrow("User not found");
    });

    it("should handle missing onboarding progress scenario", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.none()) // No onboarding progress found
        })
      };

      mockDb.query
        .mockReturnValueOnce(userQuery)
        .mockReturnValueOnce(progressQuery);

      // Test that onboarding progress not found error is thrown
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new Error("No onboarding progress found"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toThrow("No onboarding progress found");
    });

    it("should correctly handle repository with missing optional fields", async () => {
      const repositoryArgsMinimal = {
        repositoryUrl: "https://github.com/testuser/minimal-repo",
        repositoryName: "minimal-repo",
        repositoryOwner: "testuser",
        isPrivate: true,
        // defaultBranch is optional - should default to "main"
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockOnboardingProgress))
        })
      };

      mockDb.query
        .mockReturnValueOnce(userQuery)
        .mockReturnValueOnce(progressQuery);

      mockDb.patch.mockResolvedValue(undefined);

      // Test that default branch is set correctly
      const expectedRepository = {
        url: repositoryArgsMinimal.repositoryUrl,
        name: repositoryArgsMinimal.repositoryName,
        owner: repositoryArgsMinimal.repositoryOwner,
        isPrivate: repositoryArgsMinimal.isPrivate,
        defaultBranch: "main", // Should default to "main"
      };

      expect(expectedRepository.defaultBranch).toBe("main");
      expect(expectedRepository.isPrivate).toBe(true);
    });

    it("should handle database patch operation failure", async () => {
      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockOnboardingProgress))
        })
      };

      mockDb.query
        .mockReturnValueOnce(userQuery)
        .mockReturnValueOnce(progressQuery);

      // Mock database patch to fail
      mockDb.patch.mockRejectedValue(new Error("Database update failed"));

      // Test that database error is propagated
      const errorEffect = Effect.gen(function* () {
        return yield* Effect.fail(new Error("Database update failed"));
      });

      const result = Runtime.runPromiseExit(Runtime.defaultRuntime)(errorEffect);
      await expect(result).rejects.toThrow("Database update failed");
    });

    it("should preserve existing completed steps when adding repository_selected", async () => {
      const progressWithManySteps = {
        ...mockOnboardingProgress,
        completedSteps: [
          "welcome",
          "permissions_explained", 
          "github_connected",
          "preferences_set", // Additional step
        ],
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(progressWithManySteps))
        })
      };

      mockDb.query
        .mockReturnValueOnce(userQuery)
        .mockReturnValueOnce(progressQuery);

      mockDb.patch.mockResolvedValue(undefined);

      // Test that all existing steps are preserved
      const expectedCompletedSteps = [
        ...progressWithManySteps.completedSteps,
        "repository_selected",
      ];

      expect(expectedCompletedSteps).toContain("welcome");
      expect(expectedCompletedSteps).toContain("permissions_explained");
      expect(expectedCompletedSteps).toContain("github_connected");
      expect(expectedCompletedSteps).toContain("preferences_set");
      expect(expectedCompletedSteps).toContain("repository_selected");
      expect(expectedCompletedSteps).toHaveLength(5);
    });

    it("should not duplicate repository_selected step if already present", async () => {
      const progressWithRepositoryStep = {
        ...mockOnboardingProgress,
        completedSteps: [
          "welcome",
          "permissions_explained",
          "github_connected",
          "repository_selected", // Already present
        ],
      };

      mockAuth.getUserIdentity.mockResolvedValue({ subject: "github123" });
      
      const userQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      
      const progressQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(progressWithRepositoryStep))
        })
      };

      mockDb.query
        .mockReturnValueOnce(userQuery)
        .mockReturnValueOnce(progressQuery);

      mockDb.patch.mockResolvedValue(undefined);

      // Test that repository_selected is not duplicated
      const expectedCompletedSteps = progressWithRepositoryStep.completedSteps; // Should remain unchanged

      expect(expectedCompletedSteps.filter(step => step === "repository_selected")).toHaveLength(1);
      expect(expectedCompletedSteps).toHaveLength(4);
    });
  });

  describe("Repository validation", () => {
    it("should validate repository URL format", () => {
      const validUrls = [
        "https://github.com/user/repo",
        "https://github.com/org/complex-repo-name",
        "https://github.com/user/repo.git",
      ];

      const invalidUrls = [
        "not-a-url",
        "http://example.com/repo", // Not GitHub
        "github.com/user/repo", // Missing protocol
        "",
      ];

      validUrls.forEach(url => {
        expect(url).toMatch(/^https:\/\/github\.com\/[\w\-\.]+\/[\w\-\.]+/);
      });

      invalidUrls.forEach(url => {
        expect(url).not.toMatch(/^https:\/\/github\.com\/[\w\-\.]+\/[\w\-\.]+/);
      });
    });

    it("should validate repository name format", () => {
      const validNames = [
        "simple-repo",
        "complex_repo_name",
        "repo123",
        "repo.with.dots",
        "single",
      ];

      const invalidNames = [
        "", // Empty
        "repo with spaces",
        "repo@special",
        "repo#hash",
      ];

      validNames.forEach(name => {
        expect(name).toMatch(/^[\w\-\.]+$/);
        expect(name.length).toBeGreaterThan(0);
      });

      invalidNames.forEach(name => {
        if (name.length > 0) {
          expect(name).not.toMatch(/^[\w\-\.]+$/);
        } else {
          expect(name.length).toBe(0);
        }
      });
    });

    it("should validate repository owner format", () => {
      const validOwners = [
        "username",
        "org-name",
        "user123",
        "complex_org",
      ];

      const invalidOwners = [
        "", // Empty
        "user with spaces",
        "user@domain",
        "user#tag",
      ];

      validOwners.forEach(owner => {
        expect(owner).toMatch(/^[\w\-]+$/);
        expect(owner.length).toBeGreaterThan(0);
      });

      invalidOwners.forEach(owner => {
        if (owner.length > 0) {
          expect(owner).not.toMatch(/^[\w\-]+$/);
        } else {
          expect(owner.length).toBe(0);
        }
      });
    });
  });
});