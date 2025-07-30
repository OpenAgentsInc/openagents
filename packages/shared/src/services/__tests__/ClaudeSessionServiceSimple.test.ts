import { Effect, Schedule, Exit } from "effect";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ServiceTestUtils, benchmarkEffect, AdvancedTestPatterns } from "./setup-service-tests";
import { createClaudeSessionService } from "../ClaudeSessionServiceSimple";
import {
  CreateSessionParams,
  SessionData,
  SessionStatus,
  SessionQueryCriteria,
  SessionCreationError,
  SessionNotFoundError,
  SessionPermissionError,
  SessionValidationError,
} from "../../types/session-service-types";

/**
 * ClaudeSessionServiceSimple Testing Suite
 * 
 * Comprehensive testing for Claude session service functionality
 * following Effect-TS v3 patterns and addressing CodeRabbit feedback
 * about missing test coverage for the service layer implementation.
 */

describe("ClaudeSessionServiceSimple", () => {
  let sessionService: ReturnType<typeof createClaudeSessionService>;
  
  beforeEach(() => {
    sessionService = createClaudeSessionService();
  });

  describe("Session Creation", () => {
    const validCreateParams: CreateSessionParams = {
      sessionId: "test-session-1",
      projectPath: "/test/project",
      createdBy: "desktop",
      title: "Test Session",
      initialMessage: "Hello World",
      metadata: {
        workingDirectory: "/test/project",
        model: "claude-3-sonnet",
        systemPrompt: "You are a helpful assistant",
        aiModel: "claude-3-sonnet",
        contextWindow: 200000,
      },
    };

    ServiceTestUtils.runServiceTest(
      "should create session with valid parameters",
      Effect.gen(function* () {
        const result = yield* sessionService.createSession(validCreateParams);

        expect(result).toMatchObject({
          sessionId: validCreateParams.sessionId,
          projectPath: validCreateParams.projectPath,
          title: validCreateParams.title,
          status: "active",
          createdBy: validCreateParams.createdBy,
          metadata: validCreateParams.metadata,
          version: 1,
        });

        expect(result.createdAt).toBeGreaterThan(0);
        expect(result.lastActivity).toBeGreaterThan(0);
        expect(result.userId).toBe("current-user");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should generate default title when not provided",
      Effect.gen(function* () {
        const paramsWithoutTitle: CreateSessionParams = {
          ...validCreateParams,
          title: undefined,
        };

        const result = yield* sessionService.createSession(paramsWithoutTitle);

        expect(result.title).toContain("desktop Session");
        expect(result.title).toContain(new Date().toLocaleString().split(',')[0]); // Contains date
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle empty metadata gracefully",
      Effect.gen(function* () {
        const paramsEmptyMetadata: CreateSessionParams = {
          ...validCreateParams,
          metadata: undefined,
        };

        const result = yield* sessionService.createSession(paramsEmptyMetadata);

        expect(result.metadata).toEqual({});
      })
    );

    describe("Session Creation Validation", () => {
      ServiceTestUtils.runServiceTest(
        "should fail with empty sessionId",
        Effect.gen(function* () {
          const invalidParams = {
            ...validCreateParams,
            sessionId: "",
          };

          const result = yield* Effect.flip(sessionService.createSession(invalidParams));

          expect(result).toBeInstanceOf(SessionCreationError);
          expect(result.reason).toBe("Session ID is required");
          expect(result.sessionId).toBe("");
        })
      );

      ServiceTestUtils.runServiceTest(
        "should fail with whitespace-only sessionId",
        Effect.gen(function* () {
          const invalidParams = {
            ...validCreateParams,
            sessionId: "   ",
          };

          const result = yield* Effect.flip(sessionService.createSession(invalidParams));

          expect(result).toBeInstanceOf(SessionCreationError);
          expect(result.reason).toBe("Session ID is required");
        })
      );

      ServiceTestUtils.runServiceTest(
        "should fail with empty projectPath",
        Effect.gen(function* () {
          const invalidParams = {
            ...validCreateParams,
            projectPath: "",
          };

          const result = yield* Effect.flip(sessionService.createSession(invalidParams));

          expect(result).toBeInstanceOf(SessionCreationError);
          expect(result.reason).toBe("Project path is required");
          expect(result.sessionId).toBe(validCreateParams.sessionId);
        })
      );

      ServiceTestUtils.runServiceTest(
        "should fail with whitespace-only projectPath",
        Effect.gen(function* () {
          const invalidParams = {
            ...validCreateParams,
            projectPath: "   ",
          };

          const result = yield* Effect.flip(sessionService.createSession(invalidParams));

          expect(result).toBeInstanceOf(SessionCreationError);
          expect(result.reason).toBe("Project path is required");
        })
      );
    });

    describe("Performance Benchmarks", () => {
      ServiceTestUtils.runServiceTest(
        "should create session within performance threshold",
        benchmarkEffect(
          "session-creation",
          sessionService.createSession(validCreateParams),
          200 // 200ms threshold
        ).pipe(
          Effect.map(({ result, duration }) => {
            expect(result).toMatchObject({
              sessionId: validCreateParams.sessionId,
              status: "active",
            });
            expect(duration).toBeLessThan(200);
          })
        )
      );
    });
  });

  describe("Session Retrieval", () => {
    ServiceTestUtils.runServiceTest(
      "should retrieve session with sessionId only",
      Effect.gen(function* () {
        const sessionId = "test-session-retrieve";
        const result = yield* sessionService.getSession(sessionId);

        expect(result).toMatchObject({
          sessionId,
          projectPath: "/example/project",
          title: "Example Session",
          status: "active",
          createdBy: "desktop",
          userId: "current-user",
          metadata: {},
          version: 1,
        });

        expect(result.createdAt).toBeGreaterThan(0);
        expect(result.lastActivity).toBeGreaterThan(0);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should retrieve session with specific userId",
      Effect.gen(function* () {
        const sessionId = "test-session-retrieve";
        const userId = "specific-user-123";
        const result = yield* sessionService.getSession(sessionId, userId);

        expect(result.sessionId).toBe(sessionId);
        expect(result.userId).toBe(userId);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle session retrieval performance",
      benchmarkEffect(
        "session-retrieval",
        sessionService.getSession("benchmark-session"),
        100 // 100ms threshold for retrieval
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(result.sessionId).toBe("benchmark-session");
          expect(duration).toBeLessThan(100);
        })
      )
    );
  });

  describe("Session Status Updates", () => {
    const sessionId = "test-session-status";

    ["active", "inactive", "error", "processed", "offline"].forEach((status) => {
      ServiceTestUtils.runServiceTest(
        `should update session status to ${status}`,
        Effect.gen(function* () {
          const result = yield* sessionService.updateSessionStatus(
            sessionId,
            status as SessionStatus
          );

          expect(result).toMatchObject({
            sessionId,
            status,
            version: 1,
          });

          expect(result.lastActivity).toBeGreaterThan(result.createdAt);
        })
      );
    });

    ServiceTestUtils.runServiceTest(
      "should handle status update performance",
      benchmarkEffect(
        "status-update",
        sessionService.updateSessionStatus(sessionId, "active"),
        150 // 150ms threshold
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(result.sessionId).toBe(sessionId);
          expect(result.status).toBe("active");
          expect(duration).toBeLessThan(150);
        })
      )
    );
  });

  describe("Session Deletion", () => {
    ServiceTestUtils.runServiceTest(
      "should delete session successfully",
      Effect.gen(function* () {
        const sessionId = "test-session-delete";
        const userId = "current-user";

        // Should complete without error
        yield* sessionService.deleteSession(sessionId, userId);

        // Test passes if no exception is thrown
        expect(true).toBe(true);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle deletion performance",
      benchmarkEffect(
        "session-deletion",
        sessionService.deleteSession("benchmark-delete", "current-user"),
        100 // 100ms threshold
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(duration).toBeLessThan(100);
        })
      )
    );
  });

  describe("Advanced Session Queries", () => {
    const baseCriteria: SessionQueryCriteria = {
      userId: "test-user",
    };

    ServiceTestUtils.runServiceTest(
      "should query sessions with basic criteria",
      Effect.gen(function* () {
        const result = yield* sessionService.querySessionsAdvanced(baseCriteria);

        expect(result).toMatchObject({
          sessions: expect.arrayContaining([
            expect.objectContaining({
              sessionId: "example-1",
              projectPath: "/example/project1",
              title: "Example Session 1",
              status: "active",
              createdBy: "mobile",
              userId: baseCriteria.userId,
            }),
          ]),
          totalCount: 1,
          hasMore: false,
        });
      })
    );

    ServiceTestUtils.runServiceTest(
      "should query sessions with status filter",
      Effect.gen(function* () {
        const criteria: SessionQueryCriteria = {
          ...baseCriteria,
          status: "active",
        };

        const result = yield* sessionService.querySessionsAdvanced(criteria);

        result.sessions.forEach((session) => {
          expect(session.status).toBe("active");
        });
      })
    );

    ServiceTestUtils.runServiceTest(
      "should query sessions with createdBy filter",
      Effect.gen(function* () {
        const criteria: SessionQueryCriteria = {
          ...baseCriteria,
          createdBy: "mobile",
        };

        const result = yield* sessionService.querySessionsAdvanced(criteria);

        result.sessions.forEach((session) => {
          expect(session.createdBy).toBe("mobile");
        });
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle query performance",
      benchmarkEffect(
        "session-query",
        sessionService.querySessionsAdvanced(baseCriteria),
        250 // 250ms threshold for queries
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(result.sessions).toBeDefined();
          expect(Array.isArray(result.sessions)).toBe(true);
          expect(duration).toBeLessThan(250);
        })
      )
    );
  });

  describe("Concurrency Testing", () => {
    ServiceTestUtils.runServiceTest(
      "should handle concurrent session creation",
      ServiceTestUtils.testConcurrency(
        Array.from({ length: 5 }, (_, i) => 
          sessionService.createSession({
            sessionId: `concurrent-session-${i}`,
            projectPath: `/test/concurrent/${i}`,
            createdBy: "desktop",
            title: `Concurrent Session ${i}`,
          })
        ),
        3 // Max concurrency
      ).pipe(
        Effect.map((results) => {
          expect(results).toHaveLength(5);
          results.forEach((result, index) => {
            expect(result.index).toBe(index);
            expect(result.result.sessionId).toBe(`concurrent-session-${index}`);
          });
        })
      )
    );

    ServiceTestUtils.runServiceTest(
      "should handle concurrent status updates",
      ServiceTestUtils.testConcurrency(
        Array.from({ length: 3 }, (_, i) => 
          sessionService.updateSessionStatus(
            `concurrent-status-${i}`,
            i % 2 === 0 ? "active" : "inactive"
          )
        ),
        2 // Max concurrency
      ).pipe(
        Effect.map((results) => {
          expect(results).toHaveLength(3);
          results.forEach((result, index) => {
            expect(result.result.sessionId).toBe(`concurrent-status-${index}`);
            expect(result.result.status).toBe(index % 2 === 0 ? "active" : "inactive");
          });
        })
      )
    );
  });

  describe("Error Scenarios", () => {
    ServiceTestUtils.runServiceTest(
      "should propagate errors correctly through Effect chain",
      Effect.gen(function* () {
        const invalidParams = {
          sessionId: "",
          projectPath: "/valid/path",
          createdBy: "desktop" as const,
        };

        const exit = yield* Effect.exit(sessionService.createSession(invalidParams));

        expect(Exit.isFailure(exit)).toBe(true);
        
        if (Exit.isFailure(exit)) {
          const cause = exit.cause;
          if (cause._tag === "Fail") {
            const error = cause.error;
            expect(error).toBeInstanceOf(SessionCreationError);
            expect(error.reason).toBe("Session ID is required");
          }
        }
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle error recovery patterns",
      Effect.gen(function* () {
        // Test error then success
        const failingParams = { sessionId: "", projectPath: "/test", createdBy: "desktop" as const };
        const successParams = { sessionId: "recovery-test", projectPath: "/test", createdBy: "desktop" as const };

        const failResult = yield* Effect.either(sessionService.createSession(failingParams));
        const successResult = yield* Effect.either(sessionService.createSession(successParams));

        expect(failResult._tag).toBe("Left");
        expect(successResult._tag).toBe("Right");

        if (successResult._tag === "Right") {
          expect(successResult.right.sessionId).toBe("recovery-test");
        }
      })
    );
  });

  describe("Schema Validation", () => {
    ServiceTestUtils.runServiceTest(
      "should validate CreateSessionParams schema compliance",
      Effect.gen(function* () {
        const validParams: CreateSessionParams = {
          sessionId: "schema-test",
          projectPath: "/test/schema",
          createdBy: "mobile",
          title: "Schema Test Session",
          initialMessage: "Testing schema validation",
          metadata: {
            workingDirectory: "/test/schema",
            model: "claude-3-sonnet",
            systemPrompt: "Test prompt",
            originalMobileSessionId: "mobile-123",
            aiModel: "claude-3-sonnet",
            contextWindow: 100000,
          },
        };

        const result = yield* sessionService.createSession(validParams);

        // Verify all schema fields are correctly handled
        expect(result.sessionId).toBe(validParams.sessionId);
        expect(result.projectPath).toBe(validParams.projectPath);
        expect(result.createdBy).toBe(validParams.createdBy);
        expect(result.title).toBe(validParams.title);
        expect(result.metadata).toEqual(validParams.metadata);
      })
    );
  });

  describe("Edge Cases", () => {
    ServiceTestUtils.runServiceTest(
      "should handle very long session IDs",
      Effect.gen(function* () {
        const longSessionId = "a".repeat(500);
        const params: CreateSessionParams = {
          sessionId: longSessionId,
          projectPath: "/test/long",
          createdBy: "desktop",
        };

        const result = yield* sessionService.createSession(params);
        expect(result.sessionId).toBe(longSessionId);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle very long project paths",
      Effect.gen(function* () {
        const longPath = "/very/long/path/" + "nested/".repeat(100) + "project";
        const params: CreateSessionParams = {
          sessionId: "long-path-test",
          projectPath: longPath,
          createdBy: "desktop",
        };

        const result = yield* sessionService.createSession(params);
        expect(result.projectPath).toBe(longPath.trim());
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle special characters in session data",
      Effect.gen(function* () {
        const specialChars = "!@#$%^&*()_+-=[]{}|;':\",./<>?`~";
        const params: CreateSessionParams = {
          sessionId: "special-chars-test",
          projectPath: "/test/special",
          createdBy: "desktop",
          title: `Special Characters Test: ${specialChars}`,
          initialMessage: `Message with special chars: ${specialChars}`,
        };

        const result = yield* sessionService.createSession(params);
        expect(result.title).toContain(specialChars);
      })
    );
  });
});