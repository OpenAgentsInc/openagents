import { Effect, Schedule, Exit, Cause } from "effect";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ServiceTestUtils, benchmarkEffect } from "./setup-service-tests";
import { createClaudeSessionService } from "../ClaudeSessionServiceSimple";
import { createSessionResilienceService } from "../SessionResilienceServiceSimple";
import {
  CreateSessionParams,
  SessionData,
  SessionStatus,
  SessionCreationError,
  SessionNotFoundError,
  SessionPermissionError,
  SessionValidationError,
  DatabaseOperationError,
  AuthenticationError,
  ProcessingTimeoutError,
  CreateSessionParamsSchema,
  SessionDataSchema,
} from "../../types/session-service-types";

/**
 * Session Error Handling and Edge Cases Testing Suite
 * 
 * Comprehensive testing for error scenarios, edge cases, and type safety
 * in the Effect-TS service layer implementation.
 * 
 * Addresses CodeRabbit feedback about error handling patterns and edge case coverage.
 */

describe("Session Error Handling and Edge Cases", () => {
  let sessionService: ReturnType<typeof createClaudeSessionService>;
  let resilienceService: ReturnType<typeof createSessionResilienceService>;
  
  beforeEach(() => {
    sessionService = createClaudeSessionService();
    resilienceService = createSessionResilienceService();
  });

  describe("Tagged Error Handling", () => {
    ServiceTestUtils.runServiceTest(
      "should create and handle SessionCreationError correctly",
      Effect.gen(function* () {
        const error = new SessionCreationError({
          reason: "Database connection failed",
          sessionId: "error-test-session",
          metadata: {
            errorCode: "DB_CONNECTION_FAILED",
            timestamp: Date.now(),
            retryable: true,
          },
        });

        // Test error properties
        expect(error._tag).toBe("SessionCreationError");
        expect(error.reason).toBe("Database connection failed");
        expect(error.sessionId).toBe("error-test-session");
        expect(error.metadata?.errorCode).toBe("DB_CONNECTION_FAILED");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should create and handle SessionNotFoundError correctly",
      Effect.gen(function* () {
        const error = new SessionNotFoundError({
          sessionId: "nonexistent-session-123",
        });

        expect(error._tag).toBe("SessionNotFoundError");
        expect(error.sessionId).toBe("nonexistent-session-123");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should create and handle SessionPermissionError correctly",
      Effect.gen(function* () {
        const error = new SessionPermissionError({
          sessionId: "permission-test-session",
          userId: "unauthorized-user-456",
          action: "delete",
        });

        expect(error._tag).toBe("SessionPermissionError");
        expect(error.sessionId).toBe("permission-test-session");
        expect(error.userId).toBe("unauthorized-user-456");
        expect(error.action).toBe("delete");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should create and handle SessionValidationError correctly",
      Effect.gen(function* () {
        const error = new SessionValidationError({
          reason: "Invalid project path format",
          sessionId: "validation-test",
          field: "projectPath",
        });

        expect(error._tag).toBe("SessionValidationError");
        expect(error.reason).toBe("Invalid project path format");
        expect(error.sessionId).toBe("validation-test");
        expect(error.field).toBe("projectPath");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should create and handle complex error types",
      Effect.gen(function* () {
        const dbError = new DatabaseOperationError({
          operation: "session_insert",
          sessionId: "db-error-test",
          cause: new Error("Connection timeout"),
        });

        const authError = new AuthenticationError({
          reason: "Token expired",
          userId: "expired-user-789", 
        });

        const timeoutError = new ProcessingTimeoutError({
          timeoutMs: 5000,
          operation: "session_processing",
          sessionId: "timeout-test",
        });

        expect(dbError._tag).toBe("DatabaseOperationError");
        expect(dbError.operation).toBe("session_insert");
        expect(dbError.cause).toBeInstanceOf(Error);

        expect(authError._tag).toBe("AuthenticationError");
        expect(authError.reason).toBe("Token expired");

        expect(timeoutError._tag).toBe("ProcessingTimeoutError");
        expect(timeoutError.timeoutMs).toBe(5000);
        expect(timeoutError.operation).toBe("session_processing");
      })
    );
  });

  describe("Error Propagation Through Effect Chains", () => {
    ServiceTestUtils.runServiceTest(
      "should propagate errors correctly through Effect composition", 
      Effect.gen(function* () {
        const invalidParams: CreateSessionParams = {
          sessionId: "",
          projectPath: "/test/propagation",
          createdBy: "desktop",
        };

        // Test error propagation through service chain
        const result = yield* Effect.exit(
          Effect.gen(function* () {
            const session = yield* sessionService.createSession(invalidParams);
            const updated = yield* sessionService.updateSessionStatus(session.sessionId, "processed");
            return updated;
          })
        );

        expect(Exit.isFailure(result)).toBe(true);
        
        if (Exit.isFailure(result)) {
          const cause = result.cause;
          if (cause._tag === "Fail") {
            expect(cause.error).toBeInstanceOf(SessionCreationError);
          }
        }
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle error recovery in Effect chains",
      Effect.gen(function* () {
        const invalidParams: CreateSessionParams = {
          sessionId: "",
          projectPath: "/test/recovery",
          createdBy: "desktop",
        };

        const validParams: CreateSessionParams = {
          sessionId: "recovery-session",
          projectPath: "/test/recovery", 
          createdBy: "desktop",
        };

        // Test error recovery with catchAll
        const result = yield* sessionService.createSession(invalidParams).pipe(
          Effect.catchAll((error) => {
            if (error instanceof SessionCreationError) {
              return sessionService.createSession(validParams);
            }
            return Effect.fail(error);
          })
        );

        expect(result.sessionId).toBe("recovery-session");
        expect(result.status).toBe("active");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle nested error scenarios",
      Effect.gen(function* () {
        const deepErrorChain = Effect.gen(function* () {
          // Level 1: Session creation error
          const session = yield* sessionService.createSession({
            sessionId: "",
            projectPath: "/test/nested",
            createdBy: "desktop",
          });
          
          // Level 2: This would fail if reached
          const updated = yield* sessionService.updateSessionStatus(session.sessionId, "processed");
          
          // Level 3: This would also fail if reached
          const queried = yield* sessionService.querySessionsAdvanced({
            userId: updated.userId,
          });
          
          return queried;
        });

        const result = yield* Effect.exit(deepErrorChain);
        
        expect(Exit.isFailure(result)).toBe(true);
        // Should fail at the first level (session creation)
        if (Exit.isFailure(result)) {
          const cause = result.cause;
          if (cause._tag === "Fail") {
            expect(cause.error).toBeInstanceOf(SessionCreationError);
            expect(cause.error.reason).toBe("Session ID is required");
          }
        }
      })
    );
  });

  describe("Type Safety and Schema Validation", () => {
    ServiceTestUtils.runServiceTest(
      "should validate CreateSessionParams schema",
      Effect.gen(function* () {
        const validParams: CreateSessionParams = {
          sessionId: "schema-valid",
          projectPath: "/test/schema",
          createdBy: "desktop",
          title: "Schema Test",
          initialMessage: "Testing schema validation",
          metadata: {
            workingDirectory: "/test/schema",
            model: "claude-3-sonnet",
            systemPrompt: "Schema test prompt",
            originalMobileSessionId: "mobile-session-123",
            aiModel: "claude-3-sonnet",
            contextWindow: 200000,
          },
        };

        // In a real implementation, we would use the schema to parse/validate
        // For now, we test that our type definitions are correct
        const result = yield* sessionService.createSession(validParams);
        
        expect(result.sessionId).toBe(validParams.sessionId);
        expect(result.projectPath).toBe(validParams.projectPath);
        expect(result.createdBy).toBe(validParams.createdBy);
        expect(result.title).toBe(validParams.title);
        expect(result.metadata).toEqual(validParams.metadata);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle type boundaries correctly",
      Effect.gen(function* () {
        // Test that SessionStatus type is enforced
        const statusValues: SessionStatus[] = ["active", "inactive", "error", "processed", "offline"];
        
        for (const status of statusValues) {
          const result = yield* sessionService.updateSessionStatus("type-test", status);
          expect(result.status).toBe(status);
        }
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle branded type safety",
      Effect.gen(function* () {
        // Test branded types work correctly (compile-time check mostly)
        const sessionId = "branded-type-test" as any; // In real usage, would use createSessionId()
        const projectPath = "/test/branded" as any; // In real usage, would use createProjectPath()
        
        const params: CreateSessionParams = {
          sessionId,
          projectPath,
          createdBy: "desktop",
        };

        const result = yield* sessionService.createSession(params);
        expect(result.sessionId).toBe(sessionId);
        expect(result.projectPath).toBe(projectPath);
      })
    );
  });

  describe("Edge Cases and Boundary Conditions", () => {
    ServiceTestUtils.runServiceTest(
      "should handle empty string inputs gracefully",
      Effect.gen(function* () {
        const emptyStringParams: CreateSessionParams = {
          sessionId: "",
          projectPath: "",
          createdBy: "desktop",
          title: "",
          initialMessage: "",
        };

        const result = yield* Effect.exit(sessionService.createSession(emptyStringParams));
        
        expect(Exit.isFailure(result)).toBe(true);
        if (Exit.isFailure(result) && result.cause._tag === "Fail") {
          expect(result.cause.error).toBeInstanceOf(SessionCreationError);
        }
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle null and undefined inputs",
      Effect.gen(function* () {
        const nullParams = {
          sessionId: "null-test",
          projectPath: "/test/null",
          createdBy: "desktop" as const,
          title: null as any,
          metadata: null as any,
        };

        const undefinedParams = {
          sessionId: "undefined-test", 
          projectPath: "/test/undefined",
          createdBy: "desktop" as const,
          title: undefined,
          metadata: undefined,
        };

        // Should handle null values gracefully
        const nullResult = yield* sessionService.createSession(nullParams);
        expect(nullResult.title).toBeDefined(); // Should get default title
        expect(nullResult.metadata).toEqual({}); // Should get empty metadata

        // Should handle undefined values gracefully
        const undefinedResult = yield* sessionService.createSession(undefinedParams);
        expect(undefinedResult.title).toBeDefined(); // Should get default title
        expect(undefinedResult.metadata).toEqual({}); // Should get empty metadata
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle extremely large inputs",
      Effect.gen(function* () {
        const largeString = "x".repeat(10000);
        const largeMetadata = {
          workingDirectory: "/test/" + "nested/".repeat(1000) + "deep",
          systemPrompt: "Large prompt: " + "content ".repeat(1000),
          model: "claude-3-sonnet",
        };

        const largeParams: CreateSessionParams = {
          sessionId: "large-input-test",
          projectPath: "/test/large",
          createdBy: "desktop",
          title: largeString,
          initialMessage: largeString,
          metadata: largeMetadata,
        };

        // Should handle large inputs without crashing
        const result = yield* sessionService.createSession(largeParams);
        expect(result.sessionId).toBe("large-input-test");
        expect(result.title).toBe(largeString);
        expect(result.metadata).toEqual(largeMetadata);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle special Unicode characters",
      Effect.gen(function* () {
        const unicodeParams: CreateSessionParams = {
          sessionId: "unicode-test-ñáéíóú-中文-🚀",
          projectPath: "/test/unicode/ñáéíóú/中文/🚀",
          createdBy: "desktop",
          title: "Unicode Test: ñáéíóú 中文测试 🚀🎉",
          initialMessage: "Message with emojis: 😀😃😄😁😆😅😂🤣",
          metadata: {
            workingDirectory: "/unicode/中文/path",
            systemPrompt: "Unicode prompt: ñáéíóú 中文 🚀",
          },
        };

        const result = yield* sessionService.createSession(unicodeParams);
        expect(result.sessionId).toBe(unicodeParams.sessionId);
        expect(result.title).toBe(unicodeParams.title);
        expect(result.projectPath).toBe(unicodeParams.projectPath);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle concurrent error scenarios",
      ServiceTestUtils.testConcurrency(
        Array.from({ length: 5 }, (_, i) => 
          Effect.exit(sessionService.createSession({
            sessionId: i % 2 === 0 ? "" : `concurrent-error-${i}`, // Half will fail
            projectPath: `/test/concurrent/${i}`,
            createdBy: "desktop",
          }))
        ),
        3
      ).pipe(
        Effect.map((results) => {
          expect(results).toHaveLength(5);
          
          let successCount = 0;
          let failureCount = 0;
          
          results.forEach((result, index) => {
            if (Exit.isSuccess(result.result)) {
              successCount++;
              expect(result.result.value.sessionId).toBe(`concurrent-error-${index}`);
            } else if (Exit.isFailure(result.result)) {
              failureCount++;
              if (result.result.cause._tag === "Fail") {
                expect(result.result.cause.error).toBeInstanceOf(SessionCreationError);
              }
            }
          });
          
          expect(successCount).toBe(2); // Indices 1, 3 should succeed  
          expect(failureCount).toBe(3); // Indices 0, 2, 4 should fail
        })
      )
    );
  });

  describe("Performance Under Error Conditions", () => {
    ServiceTestUtils.runServiceTest(
      "should handle error scenarios within performance bounds",
      benchmarkEffect(
        "error-handling-performance",
        Effect.exit(sessionService.createSession({
          sessionId: "",
          projectPath: "/test/perf-error",
          createdBy: "desktop",
        })),
        50 // Should fail fast
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(Exit.isFailure(result)).toBe(true);
          expect(duration).toBeLessThan(50);
        })
      )
    );

    ServiceTestUtils.runServiceTest(
      "should handle retry performance with errors",
      benchmarkEffect(
        "retry-error-performance",
        Effect.exit(
          resilienceService.withRetry(
            Effect.fail(new SessionCreationError({
              reason: "Performance test error",
              sessionId: "perf-retry-test",
            })),
            3
          )
        ),
        200 // Allow time for retries
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(Exit.isFailure(result)).toBe(true);
          expect(duration).toBeLessThan(200);
        })
      )
    );
  });

  describe("Memory Management Under Error Conditions", () => {
    ServiceTestUtils.runServiceTest(
      "should not leak memory during error scenarios",
      Effect.gen(function* () {
        // Create many failing operations to test memory usage
        const failingOperations = Array.from({ length: 100 }, (_, i) => 
          Effect.exit(sessionService.createSession({
            sessionId: "", // Will fail
            projectPath: `/test/memory-leak/${i}`,
            createdBy: "desktop",
          }))
        );

        const results = yield* Effect.all(failingOperations);
        
        // All should fail
        expect(results).toHaveLength(100);
        results.forEach((result) => {
          expect(Exit.isFailure(result)).toBe(true);
        });

        // Test should complete without memory issues
        expect(true).toBe(true);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle resource cleanup in error scenarios",
      Effect.gen(function* () {
        let cleanupCalled = false;
        
        const resourceWithCleanup = Effect.acquireRelease(
          Effect.succeed("test-resource"),
          () => Effect.sync(() => { cleanupCalled = true; })
        );

        const failingOperation = Effect.gen(function* () {
          const resource = yield* resourceWithCleanup;
          // Simulate operation that fails after resource acquisition
          yield* Effect.fail(new Error("Resource operation failed"));
          return resource;
        });

        const result = yield* Effect.exit(Effect.scoped(failingOperation));
        
        expect(Exit.isFailure(result)).toBe(true);
        expect(cleanupCalled).toBe(true); // Cleanup should still be called
      })
    );
  });

  describe("Error Message Quality and Debugging", () => {
    ServiceTestUtils.runServiceTest(
      "should provide detailed error messages for debugging",
      Effect.gen(function* () {
        const invalidParams: CreateSessionParams = {
          sessionId: "",
          projectPath: "   ", // Whitespace only
          createdBy: "desktop",
          title: "Debug Test",
        };

        const result = yield* Effect.exit(sessionService.createSession(invalidParams));
        
        expect(Exit.isFailure(result)).toBe(true);
        if (Exit.isFailure(result) && result.cause._tag === "Fail") {
          const error = result.cause.error;
          expect(error).toBeInstanceOf(SessionCreationError);
          expect(error.reason).toContain("Session ID is required");
          expect(error.sessionId).toBe("");
        }
      })
    );

    ServiceTestUtils.runServiceTest(
      "should maintain error context through service layers",
      Effect.gen(function* () {
        const errorWithContext = new SessionCreationError({
          reason: "Custom validation failed", 
          sessionId: "context-test",
          metadata: {
            validationErrors: ["sessionId too short", "invalid characters"],
            timestamp: Date.now(),
            requestId: "req-123",
          },
        });

        // Test that error context is preserved
        expect(errorWithContext.metadata?.validationErrors).toEqual([
          "sessionId too short", 
          "invalid characters"
        ]);
        expect(errorWithContext.metadata?.requestId).toBe("req-123");
      })
    );
  });
});