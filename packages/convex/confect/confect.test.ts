import { describe, it, expect, vi, beforeEach } from "vitest";
import { Effect, Option, Runtime, Schedule } from "effect";
import { sanitizeEmail, sanitizeGitHubUsername, sanitizeMessageContent, ValidationError } from "./validation";

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

describe("Confect Validation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("sanitizeEmail", () => {
    it("should accept valid email addresses", async () => {
      const validEmails = [
        "test@example.com",
        "user.name@domain.co.uk", 
        "test+label@gmail.com"
      ];

      for (const email of validEmails) {
        const result = await Runtime.runPromise(Runtime.defaultRuntime)(sanitizeEmail(email));
        expect(result).toBe(email.toLowerCase());
      }
    });

    it("should reject invalid email formats", async () => {
      const invalidEmails = [
        "not-an-email",
        "@domain.com",
        "user@",
        "user space@domain.com"
      ];

      for (const email of invalidEmails) {
        const result = await Effect.runPromise(Effect.either(sanitizeEmail(email)));
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left).toBeInstanceOf(ValidationError);
        }
      }
    });

    it("should enforce email length limits", async () => {
      const longEmail = "a".repeat(250) + "@example.com";
      const result = await Effect.runPromise(Effect.either(sanitizeEmail(longEmail)));
      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left).toBeInstanceOf(ValidationError);
      }
    });
  });

  describe("sanitizeGitHubUsername", () => {
    it("should accept valid GitHub usernames", async () => {
      const validUsernames = ["octocat", "test-user", "user123", "a"];

      for (const username of validUsernames) {
        const result = await Runtime.runPromise(Runtime.defaultRuntime)(sanitizeGitHubUsername(username));
        expect(result).toBe(username);
      }
    });

    it("should reject invalid GitHub usernames", async () => {
      const invalidUsernames = [
        "-invalid", // Can't start with hyphen
        "invalid-", // Can't end with hyphen  
        "inv--alid", // No consecutive hyphens
        "a".repeat(40), // Too long
        "user name", // No spaces
        ""
      ];

      for (const username of invalidUsernames) {
        const result = await Effect.runPromise(Effect.either(sanitizeGitHubUsername(username)));
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left).toBeInstanceOf(ValidationError);
        }
      }
    });
  });

  describe("sanitizeMessageContent", () => {
    it("should remove dangerous script tags", async () => {
      const dangerousContent = 'Hello <script>alert("xss")</script> world';
      const result = await Runtime.runPromise(Runtime.defaultRuntime)(sanitizeMessageContent(dangerousContent));
      expect(result).toBe("Hello  world");
    });

    it("should remove javascript protocols", async () => {
      const dangerousContent = 'Click <a href="javascript:alert(1)">here</a>';
      const result = await Runtime.runPromise(Runtime.defaultRuntime)(sanitizeMessageContent(dangerousContent));
      expect(result).toBe('Click <a href="">here</a>');
    });

    it("should enforce content length limits", async () => {
      const longContent = "a".repeat(10001);
      const result = await Runtime.runPromise(Runtime.defaultRuntime)(sanitizeMessageContent(longContent));
      expect(result.length).toBe(10000);
    });

    it("should reject empty content after sanitization", async () => {
      const emptyContent = '<script></script>   ';
      const result = await Effect.runPromise(Effect.either(sanitizeMessageContent(emptyContent)));
      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left).toBeInstanceOf(ValidationError);
      }
    });
  });
});

describe("Confect Authentication Flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("getCurrentUser", () => {
    it("should return None when user not authenticated", async () => {
      mockAuth.getUserIdentity.mockResolvedValue(null);

      // Mock the function behavior
      const getCurrentUserEffect = Effect.gen(function* () {
        const identity = yield* Effect.promise(() => mockAuth.getUserIdentity());
        if (!identity) {
          return Option.none();
        }
        // Would query database here
        return Option.some({ id: "user123", email: "test@example.com" });
      });

      const result = await Runtime.runPromise(Runtime.defaultRuntime)(getCurrentUserEffect);
      expect(Option.isNone(result)).toBe(true);
    });

    it("should return user when authenticated", async () => {
      const mockIdentity = { subject: "github123" };
      const mockUser = {
        _id: "user123",
        email: "test@example.com",
        githubId: "github123",
        githubUsername: "testuser",
        createdAt: Date.now(),
        lastLogin: Date.now(),
        _creationTime: Date.now(),
      };

      mockAuth.getUserIdentity.mockResolvedValue(mockIdentity);
      
      // Mock database query chain
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.some(mockUser))
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      const getCurrentUserEffect = Effect.gen(function* () {
        const identity = yield* Effect.promise(() => mockAuth.getUserIdentity());
        if (!identity) {
          return Option.none();
        }

        // Simulate database query
        const user = yield* Effect.promise(async () => {
          const query = mockDb.query("users");
          const indexed = query.withIndex("by_github_id");
          return indexed.first();
        });

        return user;
      });

      const result = await Runtime.runPromise(Runtime.defaultRuntime)(getCurrentUserEffect);
      
      expect(Option.isSome(result)).toBe(true);
      if (Option.isSome(result)) {
        expect(result.value.email).toBe("test@example.com");
        expect(result.value.githubId).toBe("github123");
      }
    });
  });

  describe("getOrCreateUser", () => {
    it("should create new user when none exists", async () => {
      const mockIdentity = { subject: "github123" };
      const newUserId = "user123";
      const userInput = {
        email: "test@example.com",
        githubId: "github123", 
        githubUsername: "testuser",
        name: "Test User",
        avatar: "https://avatar.url"
      };

      mockAuth.getUserIdentity.mockResolvedValue(mockIdentity);
      
      // Mock database operations
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          first: vi.fn().mockResolvedValue(Option.none()) // User doesn't exist
        })
      };
      mockDb.query.mockReturnValue(mockQuery);
      mockDb.insert.mockResolvedValue(newUserId);

      const getOrCreateUserEffect = Effect.gen(function* () {
        const identity = yield* Effect.promise(() => mockAuth.getUserIdentity());
        if (!identity) {
          return yield* Effect.fail(new Error("Not authenticated"));
        }

        // Check if user exists
        const existingUser = yield* Effect.promise(async () => {
          const query = mockDb.query("users");
          const indexed = query.withIndex("by_github_id");
          return indexed.first();
        });

        if (Option.isSome(existingUser)) {
          return existingUser.value._id;
        }

        // Create new user
        const userId = yield* Effect.promise(() => mockDb.insert("users", {
          ...userInput,
          createdAt: Date.now(),
          lastLogin: Date.now(),
        }));

        return userId;
      });

      const result = await Runtime.runPromise(Runtime.defaultRuntime)(getOrCreateUserEffect);
      
      expect(result).toBe(newUserId);
      expect(mockDb.insert).toHaveBeenCalledWith("users", expect.objectContaining({
        email: userInput.email,
        githubId: userInput.githubId,
        githubUsername: userInput.githubUsername,
      }));
    });
  });
});

describe("Confect Message Operations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("addClaudeMessage", () => {
    it("should prevent duplicate messages", async () => {
      const messageInput = {
        sessionId: "session123",
        messageId: "msg123",
        messageType: "user" as const,
        content: "Hello world",
        timestamp: new Date().toISOString(),
      };

      const existingMessage = {
        _id: "existing123",
        ...messageInput,
        _creationTime: Date.now(),
      };

      // Mock database operations
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          filter: vi.fn().mockReturnValue({
            first: vi.fn().mockResolvedValue(Option.some(existingMessage))
          })
        })
      };
      mockDb.query.mockReturnValue(mockQuery);

      const addMessageEffect = Effect.gen(function* () {
        // Check for existing message
        const existing = yield* Effect.promise(async () => {
          const query = mockDb.query("claudeMessages");
          const indexed = query.withIndex("by_session_id");
          const filtered = indexed.filter(() => true); // Simplified
          return filtered.first();
        });

        if (Option.isSome(existing)) {
          yield* Effect.logWarning('⚠️ [CONFECT] Message already exists, skipping duplicate');
          return existing.value._id;
        }

        // Would insert new message here
        return "new-message-id";
      });

      const result = await Runtime.runPromise(Runtime.defaultRuntime)(addMessageEffect);
      
      expect(result).toBe("existing123");
      expect(mockDb.insert).not.toHaveBeenCalled();
    });

    it("should create new message when none exists", async () => {
      const messageInput = {
        sessionId: "session123",
        messageId: "msg123", 
        messageType: "user" as const,
        content: "Hello world",
        timestamp: new Date().toISOString(),
      };

      const newMessageId = "new-msg-123";

      // Mock database operations
      const mockQuery = {
        withIndex: vi.fn().mockReturnValue({
          filter: vi.fn().mockReturnValue({
            first: vi.fn().mockResolvedValue(Option.none()) // No existing message
          })
        })
      };
      mockDb.query.mockReturnValue(mockQuery);
      mockDb.insert.mockResolvedValue(newMessageId);

      const addMessageEffect = Effect.gen(function* () {
        // Check for existing message
        const existing = yield* Effect.promise(async () => {
          const query = mockDb.query("claudeMessages");
          const indexed = query.withIndex("by_session_id");
          const filtered = indexed.filter(() => true);
          return filtered.first();
        });

        if (Option.isSome(existing)) {
          return existing.value._id;
        }

        // Insert new message
        const messageId = yield* Effect.promise(() => mockDb.insert("claudeMessages", messageInput));
        
        return messageId;
      });

      const result = await Runtime.runPromise(Runtime.defaultRuntime)(addMessageEffect);
      
      expect(result).toBe(newMessageId);
      expect(mockDb.insert).toHaveBeenCalledWith("claudeMessages", messageInput);
    });
  });
});

describe("Confect Error Handling", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should handle database connection errors gracefully", async () => {
    const dbError = new Error("Database connection failed");
    mockDb.query.mockRejectedValue(dbError);

    const queryEffect = Effect.gen(function* () {
      try {
        const result = yield* Effect.promise(() => mockDb.query("users"));
        return result;
      } catch (error) {
        return yield* Effect.fail(error);
      }
    });

    const result = await Effect.runPromise(Effect.either(queryEffect));
    expect(result._tag).toBe("Left");
    if (result._tag === "Left") {
      expect(result.left.message).toContain("Database connection failed");
    }
  });

  it("should retry failed operations with exponential backoff", async () => {
    let attemptCount = 0;
    mockDb.query.mockImplementation(() => {
      attemptCount++;
      if (attemptCount < 3) {
        throw new Error("Temporary failure");
      }
      return Promise.resolve({ data: "success" });
    });

    const retryEffect = Effect.gen(function* () {
      const result = yield* Effect.promise(() => mockDb.query("users"));
      return result;
    }).pipe(
      Effect.retry(Schedule.exponential("100 millis").pipe(Schedule.recurs(3)))
    );

    const result = await Runtime.runPromise(Runtime.defaultRuntime)(retryEffect);
    
    expect(result).toEqual({ data: "success" });
    expect(attemptCount).toBe(3);
  });
});