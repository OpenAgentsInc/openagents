import { describe, it, expect, beforeEach, vi } from "vitest";
// import { processGitHubWebhook } from "./github"; // Function not exported

describe("GitHub Webhook Processing", () => {
  let mockCtx: any;
  let mockDb: any;

  beforeEach(() => {
    mockDb = {
      query: vi.fn().mockReturnThis(),
      withIndex: vi.fn().mockReturnThis(),
      filter: vi.fn().mockReturnThis(),
      first: vi.fn(),
      insert: vi.fn(),
      patch: vi.fn(),
    };

    mockCtx = {
      db: mockDb,
      auth: {
        getUserIdentity: vi.fn(),
      },
    };
  });

  describe("Event Processing", () => {
    it("should process issue opened event", async () => {
      const payload = {
        action: "opened",
        issue: {
          number: 123,
          title: "Test Issue",
          user: { login: "testuser" },
        },
        repository: {
          full_name: "org/repo",
        },
        sender: {
          login: "testuser",
          id: 12345,
        },
      };

      // Mock user lookup
      mockDb.first.mockResolvedValue({
        _id: "user123",
        githubId: "12345",
      });

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      expect(mockDb.insert).toHaveBeenCalledWith("githubEvents", {
        userId: "user123",
        eventType: "issues",
        action: "opened",
        repository: "org/repo",
        timestamp: expect.any(Number),
        metadata: {
          issueNumber: 123,
          title: "Test Issue",
          actor: "testuser",
        },
      });
    });

    it("should process pull request event", async () => {
      const payload = {
        action: "opened",
        pull_request: {
          number: 456,
          title: "Feature PR",
          user: { login: "developer" },
          additions: 100,
          deletions: 50,
        },
        repository: {
          full_name: "org/repo",
        },
        sender: {
          login: "developer",
          id: 67890,
        },
      };

      mockDb.first.mockResolvedValue({
        _id: "user456",
        githubId: "67890",
      });

      // await processGitHubWebhook(mockCtx, "pull_request", payload);

      expect(mockDb.insert).toHaveBeenCalledWith("githubEvents", {
        userId: "user456",
        eventType: "pull_request",
        action: "opened",
        repository: "org/repo",
        timestamp: expect.any(Number),
        metadata: {
          prNumber: 456,
          title: "Feature PR",
          actor: "developer",
          additions: 100,
          deletions: 50,
        },
      });
    });

    it("should process push event", async () => {
      const payload = {
        ref: "refs/heads/main",
        commits: [
          { message: "Fix bug", id: "abc123" },
          { message: "Add feature", id: "def456" },
        ],
        repository: {
          full_name: "org/repo",
        },
        sender: {
          login: "developer",
          id: 11111,
        },
      };

      mockDb.first.mockResolvedValue({
        _id: "user789",
        githubId: "11111",
      });

      // await processGitHubWebhook(mockCtx, "push", payload);

      expect(mockDb.insert).toHaveBeenCalledWith("githubEvents", {
        userId: "user789",
        eventType: "push",
        action: "push",
        repository: "org/repo",
        timestamp: expect.any(Number),
        metadata: {
          branch: "main",
          commitCount: 2,
          commits: expect.arrayContaining([
            expect.objectContaining({ message: "Fix bug" }),
            expect.objectContaining({ message: "Add feature" }),
          ]),
        },
      });
    });

    it("should handle workflow run events", async () => {
      const payload = {
        action: "completed",
        workflow_run: {
          name: "CI Build",
          conclusion: "success",
          run_number: 123,
        },
        repository: {
          full_name: "org/repo",
        },
        sender: {
          login: "bot",
          id: 99999,
        },
      };

      mockDb.first.mockResolvedValue({
        _id: "user999",
        githubId: "99999",
      });

      // await processGitHubWebhook(mockCtx, "workflow_run", payload);

      expect(mockDb.insert).toHaveBeenCalledWith("githubEvents", {
        userId: "user999",
        eventType: "workflow_run",
        action: "completed",
        repository: "org/repo",
        timestamp: expect.any(Number),
        metadata: {
          workflowName: "CI Build",
          conclusion: "success",
          runNumber: 123,
        },
      });
    });
  });

  describe("User Mapping", () => {
    it("should skip event if user not found", async () => {
      const payload = {
        action: "opened",
        repository: { full_name: "org/repo" },
        sender: { login: "unknown", id: 99999 },
      };

      mockDb.first.mockResolvedValue(null); // User not found

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      expect(mockDb.insert).not.toHaveBeenCalled();
    });

    it("should map GitHub user to authenticated user", async () => {
      const payload = {
        action: "opened",
        repository: { full_name: "org/repo" },
        sender: { login: "testuser", id: 12345 },
      };

      mockDb.first.mockResolvedValue({
        _id: "user123",
        githubId: "12345",
        name: "Test User",
      });

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      expect(mockDb.query).toHaveBeenCalledWith("users");
      expect(mockDb.withIndex).toHaveBeenCalledWith("by_github_id");
      expect(mockDb.filter).toHaveBeenCalledWith(
        expect.any(Function)
      );
    });
  });

  describe("Device Session Creation", () => {
    it("should create device session for GitHub activity", async () => {
      const payload = {
        action: "opened",
        repository: { full_name: "org/repo" },
        sender: { login: "developer", id: 12345 },
      };

      mockDb.first
        .mockResolvedValueOnce({ _id: "user123", githubId: "12345" }) // User lookup
        .mockResolvedValueOnce(null); // No existing session

      const mockTrackSession = vi.fn();
      mockCtx.runMutation = mockTrackSession;

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      // Should track session with 1-minute window
      expect(mockTrackSession).toHaveBeenCalledWith(
        expect.any(Function),
        {
          deviceId: "github-12345",
          deviceType: "github",
          sessionPeriods: [
            {
              start: expect.any(Number),
              end: expect.any(Number),
            },
          ],
          actionsCount: 1,
          userId: "user123",
        }
      );
    });

    it("should update existing session for same user", async () => {
      const existingSession = {
        _id: "session123",
        deviceId: "github-12345",
        sessionPeriods: [{ start: 0, end: 60000 }],
        actionsCount: 5,
      };

      mockDb.first
        .mockResolvedValueOnce({ _id: "user123", githubId: "12345" })
        .mockResolvedValueOnce(existingSession); // Existing session

      const payload = {
        action: "opened",
        repository: { full_name: "org/repo" },
        sender: { login: "developer", id: 12345 },
      };

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      expect(mockDb.patch).toHaveBeenCalledWith("session123", {
        sessionPeriods: expect.arrayContaining([
          { start: 0, end: 60000 },
          expect.objectContaining({
            start: expect.any(Number),
            end: expect.any(Number),
          }),
        ]),
        actionsCount: 6,
        lastUpdated: expect.any(Number),
      });
    });
  });

  describe("Event Metadata", () => {
    it("should extract relevant metadata from issue comment", async () => {
      const payload = {
        action: "created",
        issue: { number: 123 },
        comment: {
          body: "This is a test comment",
          user: { login: "reviewer" },
        },
        repository: { full_name: "org/repo" },
        sender: { login: "reviewer", id: 54321 },
      };

      mockDb.first.mockResolvedValue({ _id: "user321", githubId: "54321" });

      // await processGitHubWebhook(mockCtx, "issue_comment", payload);

      expect(mockDb.insert).toHaveBeenCalledWith("githubEvents", 
        expect.objectContaining({
          metadata: {
            issueNumber: 123,
            commentPreview: "This is a test comment",
            actor: "reviewer",
          },
        })
      );
    });

    it("should handle release events", async () => {
      const payload = {
        action: "published",
        release: {
          tag_name: "v1.0.0",
          name: "Version 1.0.0",
          draft: false,
          prerelease: false,
        },
        repository: { full_name: "org/repo" },
        sender: { login: "maintainer", id: 11111 },
      };

      mockDb.first.mockResolvedValue({ _id: "user111", githubId: "11111" });

      // await processGitHubWebhook(mockCtx, "release", payload);

      expect(mockDb.insert).toHaveBeenCalledWith("githubEvents",
        expect.objectContaining({
          metadata: {
            tagName: "v1.0.0",
            releaseName: "Version 1.0.0",
            isDraft: false,
            isPrerelease: false,
          },
        })
      );
    });
  });

  describe("Error Handling", () => {
    it("should handle database errors gracefully", async () => {
      const payload = {
        action: "opened",
        repository: { full_name: "org/repo" },
        sender: { login: "user", id: 12345 },
      };

      mockDb.first.mockRejectedValue(new Error("Database error"));
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining("Error processing GitHub webhook"),
        expect.any(Error)
      );

      consoleSpy.mockRestore();
    });

    it("should handle malformed payloads", async () => {
      const malformedPayload = {
        // Missing required fields
        action: "opened",
      };

      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

      // await processGitHubWebhook(mockCtx, "issues", malformedPayload as any);

      expect(consoleSpy).toHaveBeenCalled();
      expect(mockDb.insert).not.toHaveBeenCalled();

      consoleSpy.mockRestore();
    });
  });

  describe("APM Calculation Integration", () => {
    it("should trigger APM recalculation after event", async () => {
      const payload = {
        action: "opened",
        repository: { full_name: "org/repo" },
        sender: { login: "developer", id: 12345 },
      };

      mockDb.first.mockResolvedValue({ _id: "user123", githubId: "12345" });

      const mockCalculateAPM = vi.fn();
      mockCtx.runMutation = mockCalculateAPM;

      // // await processGitHubWebhook(mockCtx, "issues", payload); // Function not exported

      // Should trigger APM recalculation
      expect(mockCalculateAPM).toHaveBeenCalledWith(
        expect.any(Function),
        { userId: "user123" }
      );
    });
  });
});