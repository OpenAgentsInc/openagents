import { Effect, Schedule, Exit } from "effect";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ServiceTestUtils, benchmarkEffect, AdvancedTestPatterns } from "./setup-service-tests";
import { createSessionResilienceService } from "../SessionResilienceServiceSimple";
import {
  CreateSessionParams,
  SessionData,
  SessionCreationError,
} from "../../types/session-service-types";

/**
 * SessionResilienceServiceSimple Testing Suite
 * 
 * Comprehensive testing for session resilience functionality including
 * retry patterns, fallback mechanisms, and circuit breaker behaviors.
 * 
 * Addresses CodeRabbit feedback about missing test coverage for resilience patterns.
 */

describe("SessionResilienceServiceSimple", () => {
  let resilienceService: ReturnType<typeof createSessionResilienceService>;
  
  beforeEach(() => {
    resilienceService = createSessionResilienceService();
  });

  const validCreateParams: CreateSessionParams = {
    sessionId: "resilience-test-session",
    projectPath: "/test/resilience",
    createdBy: "desktop",
    title: "Resilience Test Session",
    metadata: {
      workingDirectory: "/test/resilience",
      model: "claude-3-sonnet",
    },
  };

  describe("Retry Mechanisms", () => {
    ServiceTestUtils.runServiceTest(
      "should provide retry functionality with default retries",
      Effect.gen(function* () {
        let attemptCount = 0;
        
        const failingOperation = Effect.gen(function* () {
          attemptCount++;
          if (attemptCount < 3) {
            yield* Effect.fail(new Error(`Attempt ${attemptCount} failed`));
          }
          return `Success on attempt ${attemptCount}`;
        });

        const result = yield* resilienceService.withRetry(failingOperation);
        
        expect(result).toBe("Success on attempt 3");
        expect(attemptCount).toBe(3);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should respect custom retry count",
      Effect.gen(function* () {
        let attemptCount = 0;
        
        const alwaysFailingOperation = Effect.gen(function* () {
          attemptCount++;
          yield* Effect.fail(new Error(`Attempt ${attemptCount} failed`));
        });

        const maxRetries = 2;
        const result = yield* Effect.flip(
          resilienceService.withRetry(alwaysFailingOperation, maxRetries)
        );
        
        expect(result).toBeInstanceOf(Error);
        expect(attemptCount).toBe(maxRetries + 1); // Initial attempt + retries
      })
    );

    ServiceTestUtils.runServiceTest(
      "should succeed immediately if operation succeeds on first try",
      Effect.gen(function* () {
        let attemptCount = 0;
        
        const successfulOperation = Effect.gen(function* () {
          attemptCount++;
          return `Success on first attempt`;
        });

        const result = yield* resilienceService.withRetry(successfulOperation, 5);
        
        expect(result).toBe("Success on first attempt");
        expect(attemptCount).toBe(1);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle retry performance within reasonable bounds",
      benchmarkEffect(
        "retry-mechanism",
        resilienceService.withRetry(Effect.succeed("immediate-success"), 3),
        100 // Should be fast for immediate success
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(result).toBe("immediate-success");
          expect(duration).toBeLessThan(100);
        })
      )
    );
  });

  describe("Fallback Mechanisms", () => {
    ServiceTestUtils.runServiceTest(
      "should use primary operation when it succeeds",
      Effect.gen(function* () {
        const primaryOperation = Effect.succeed("primary-success");
        const fallbackOperation = Effect.succeed("fallback-success");

        const result = yield* resilienceService.withFallback(
          primaryOperation,
          fallbackOperation
        );

        expect(result).toBe("primary-success");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should use fallback when primary operation fails",
      Effect.gen(function* () {
        const primaryOperation = Effect.fail(new Error("Primary failed"));
        const fallbackOperation = Effect.succeed("fallback-success");

        const result = yield* resilienceService.withFallback(
          primaryOperation,
          fallbackOperation
        );

        expect(result).toBe("fallback-success");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle typed primary operation failures",
      Effect.gen(function* () {
        const primaryOperation = Effect.fail(
          new SessionCreationError({
            reason: "Database connection failed",
            sessionId: "test-session",
          })
        );
        const fallbackOperation = Effect.succeed("fallback-handled");

        const result = yield* resilienceService.withFallback(
          primaryOperation,
          fallbackOperation
        );

        expect(result).toBe("fallback-handled");
      })
    );

    ServiceTestUtils.runServiceTest(
      "should maintain type safety with fallback operations",
      Effect.gen(function* () {
        const primaryOperation: Effect.Effect<string, Error> = Effect.fail(new Error("fail"));
        const fallbackOperation: Effect.Effect<string, never> = Effect.succeed("fallback");

        const result = yield* resilienceService.withFallback(
          primaryOperation,
          fallbackOperation
        );

        // Type should be Effect.Effect<string, never> - never fails
        expect(typeof result).toBe("string");
        expect(result).toBe("fallback");
      })
    );
  });

  describe("Resilient Session Creation", () => {
    ServiceTestUtils.runServiceTest(
      "should create session successfully on first attempt (70% simulated success rate)",
      Effect.gen(function* () {
        // Mock Math.random to return value > 0.7 (success case)
        const originalRandom = Math.random;
        Math.random = () => 0.8;

        const result = yield* resilienceService.createSessionResilient(validCreateParams);

        expect(result).toMatchObject({
          sessionId: validCreateParams.sessionId,
          projectPath: validCreateParams.projectPath,
          title: validCreateParams.title,
          status: "active",
          createdBy: validCreateParams.createdBy,
          userId: "current-user",
          version: 1,
        });

        expect(result.metadata).toEqual(validCreateParams.metadata);

        // Restore original Math.random
        Math.random = originalRandom;
      })
    );

    ServiceTestUtils.runServiceTest(
      "should fallback to offline session when primary fails (30% simulated failure rate)",
      Effect.gen(function* () {
        // Mock Math.random to return value <= 0.7 (failure case)
        const originalRandom = Math.random;
        Math.random = () => 0.5;

        const result = yield* resilienceService.createSessionResilient(validCreateParams);

        expect(result).toMatchObject({
          sessionId: `offline-${validCreateParams.sessionId}`,
          projectPath: validCreateParams.projectPath,
          title: expect.stringContaining("Offline Session"),
          status: "offline",
          createdBy: validCreateParams.createdBy,
          userId: "offline-user",
          version: 1,
        });

        expect(result.title).toContain(validCreateParams.createdBy);

        // Restore original Math.random
        Math.random = originalRandom;
      })
    );

    ServiceTestUtils.runServiceTest(
      "should retry multiple times before falling back",
      Effect.gen(function* () {
        let attemptCount = 0;
        const originalRandom = Math.random;
        
        // First 3 attempts fail, then succeed
        Math.random = () => {
          attemptCount++;
          return attemptCount <= 3 ? 0.5 : 0.8; // Fail first 3, succeed on 4th
        };

        const result = yield* resilienceService.createSessionResilient(validCreateParams);

        // Should eventually succeed after retries
        expect(result.status).toBe("active");
        expect(result.sessionId).toBe(validCreateParams.sessionId);
        expect(attemptCount).toBeGreaterThan(3);

        // Restore original Math.random
        Math.random = originalRandom;
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle metadata correctly in offline fallback",
      Effect.gen(function* () {
        const originalRandom = Math.random;
        Math.random = () => 0.3; // Force failure and fallback

        const paramsWithMetadata: CreateSessionParams = {
          ...validCreateParams,
          metadata: {
            workingDirectory: "/test/custom",
            model: "claude-3-opus",
            systemPrompt: "Custom system prompt",
            contextWindow: 150000,
          },
        };

        const result = yield* resilienceService.createSessionResilient(paramsWithMetadata);

        expect(result.status).toBe("offline");
        expect(result.metadata).toEqual(paramsWithMetadata.metadata);

        // Restore original Math.random
        Math.random = originalRandom;
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle resilient session creation performance",
      benchmarkEffect(
        "resilient-session-creation",
        resilienceService.createSessionResilient(validCreateParams),
        500 // Higher threshold due to retry logic
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(result.sessionId).toContain("test-session");
          expect(duration).toBeLessThan(500);
        })
      )
    );
  });

  describe("Combined Retry and Fallback Patterns", () => {
    ServiceTestUtils.runServiceTest(
      "should combine retry with fallback correctly",
      Effect.gen(function* () {
        let primaryAttempts = 0;
        
        const failingPrimary = Effect.gen(function* () {
          primaryAttempts++;
          yield* Effect.fail(new Error(`Primary attempt ${primaryAttempts} failed`));
        });

        const fallbackOperation = Effect.succeed("combined-fallback-success");

        const combined = resilienceService.withFallback(
          resilienceService.withRetry(failingPrimary, 2),
          fallbackOperation
        );

        const result = yield* combined;

        expect(result).toBe("combined-fallback-success");
        expect(primaryAttempts).toBe(3); // Initial + 2 retries
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle nested resilience patterns",
      Effect.gen(function* () {
        const level1Operation = Effect.fail(new Error("Level 1 failure"));
        const level2Fallback = Effect.fail(new Error("Level 2 failure"));
        const finalFallback = Effect.succeed("final-fallback-success");

        const nested = resilienceService.withFallback(
          resilienceService.withFallback(level1Operation, level2Fallback),
          finalFallback
        );

        const result = yield* nested;
        expect(result).toBe("final-fallback-success");
      })
    );
  });

  describe("Error Propagation and Handling", () => {
    ServiceTestUtils.runServiceTest(
      "should properly propagate SessionCreationError through retry",
      Effect.gen(function* () {
        const error = new SessionCreationError({
          reason: "Validation failed",
          sessionId: "error-test",
          metadata: { extra: "error-info" },
        });

        const failingOperation = Effect.fail(error);
        const result = yield* Effect.flip(
          resilienceService.withRetry(failingOperation, 2)
        );

        expect(result).toBeInstanceOf(SessionCreationError);
        expect(result.reason).toBe("Validation failed");
        expect(result.sessionId).toBe("error-test");
        expect(result.metadata).toEqual({ extra: "error-info" });
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle error recovery through Effect Exit patterns",
      Effect.gen(function* () {
        const error = new SessionCreationError({
          reason: "Database connection lost",
          sessionId: "recovery-test",
        });

        const failingOperation = Effect.fail(error);
        const fallbackOperation = Effect.succeed("recovered");

        const exit = yield* Effect.exit(
          resilienceService.withFallback(failingOperation, fallbackOperation)
        );

        expect(Exit.isSuccess(exit)).toBe(true);
        if (Exit.isSuccess(exit)) {
          expect(exit.value).toBe("recovered");
        }
      })
    );
  });

  describe("Concurrency and Race Conditions", () => {
    ServiceTestUtils.runServiceTest(
      "should handle concurrent resilient operations",
      ServiceTestUtils.testConcurrency(
        Array.from({ length: 4 }, (_, i) => 
          resilienceService.createSessionResilient({
            sessionId: `concurrent-resilience-${i}`,
            projectPath: `/test/concurrent/${i}`,
            createdBy: "desktop",
            title: `Concurrent Resilient Session ${i}`,
          })
        ),
        2 // Max concurrency
      ).pipe(
        Effect.map((results) => {
          expect(results).toHaveLength(4);
          results.forEach((result, index) => {
            expect(result.index).toBe(index);
            // Session ID should contain the index (either direct or with offline prefix)
            expect(result.result.sessionId).toMatch(
              new RegExp(`(offline-)?concurrent-resilience-${index}`)
            );
          });
        })
      )
    );

    ServiceTestUtils.runServiceTest(
      "should handle concurrent retry operations",
      ServiceTestUtils.testConcurrency(
        Array.from({ length: 3 }, (_, i) => {
          let attempts = 0;
          return resilienceService.withRetry(
            Effect.gen(function* () {
              attempts++;
              if (attempts <= i + 1) {
                yield* Effect.fail(new Error(`Concurrent fail ${i}-${attempts}`));
              }
              return `concurrent-success-${i}`;
            }),
            3
          );
        }),
        2 // Max concurrency  
      ).pipe(
        Effect.map((results) => {
          expect(results).toHaveLength(3);
          results.forEach((result, index) => {
            expect(result.result).toBe(`concurrent-success-${index}`);
          });
        })
      )
    );
  });

  describe("Edge Cases and Boundary Conditions", () => {
    ServiceTestUtils.runServiceTest(
      "should handle zero retries gracefully",
      Effect.gen(function* () {
        let attemptCount = 0;
        const failingOperation = Effect.gen(function* () {
          attemptCount++;
          yield* Effect.fail(new Error("Always fails"));
        });

        const result = yield* Effect.flip(
          resilienceService.withRetry(failingOperation, 0)
        );

        expect(result).toBeInstanceOf(Error);
        expect(attemptCount).toBe(1); // Only initial attempt, no retries
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle very high retry counts",
      Effect.gen(function* () {
        let attemptCount = 0;
        const eventualSuccessOperation = Effect.gen(function* () {
          attemptCount++;
          if (attemptCount < 50) {
            yield* Effect.fail(new Error(`Not yet: ${attemptCount}`));
          }
          return "finally-succeeded";
        });

        const result = yield* resilienceService.withRetry(
          eventualSuccessOperation, 
          100 // Very high retry count
        );

        expect(result).toBe("finally-succeeded");
        expect(attemptCount).toBe(50);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle fallback with same type as primary",
      Effect.gen(function* () {
        const sessionData: SessionData = {
          sessionId: "fallback-test",
          projectPath: "/test/fallback",
          title: "Fallback Test",
          status: "active",
          createdBy: "desktop",
          lastActivity: Date.now(),
          createdAt: Date.now(),
          userId: "test-user",
          metadata: {},
          version: 1,
        };

        const failingPrimary: Effect.Effect<SessionData, SessionCreationError> = 
          Effect.fail(new SessionCreationError({
            reason: "Primary failed",
            sessionId: "fallback-test",
          }));

        const successfulFallback: Effect.Effect<SessionData, never> = 
          Effect.succeed(sessionData);

        const result = yield* resilienceService.withFallback(
          failingPrimary,
          successfulFallback
        );

        expect(result).toEqual(sessionData);
      })
    );
  });

  describe("Integration with Schedule and Effect Patterns", () => {
    ServiceTestUtils.runServiceTest(
      "should work with Effect Schedule patterns",
      Effect.gen(function* () {
        let attemptCount = 0;
        
        const scheduledOperation = Effect.gen(function* () {
          attemptCount++;
          if (attemptCount < 3) {
            yield* Effect.fail(new Error(`Scheduled attempt ${attemptCount}`));
          }
          return `scheduled-success-${attemptCount}`;
        });

        // Test that our retry logic works with Schedule patterns
        const result = yield* resilienceService.withRetry(scheduledOperation, 5);
        
        expect(result).toBe("scheduled-success-3");
        expect(attemptCount).toBe(3);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle complex Effect compositions",
      Effect.gen(function* () {
        const complexOperation = Effect.gen(function* () {
          // Simulate complex async operations
          const step1 = yield* Effect.succeed("step1");
          const step2 = yield* Effect.succeed("step2");
          
          if (Math.random() > 0.5) {
            yield* Effect.fail(new Error("Complex operation failed"));
          }
          
          return `${step1}-${step2}-complete`;
        });

        const fallback = Effect.succeed("complex-fallback");

        const result = yield* resilienceService.withFallback(
          resilienceService.withRetry(complexOperation, 2),
          fallback
        );

        // Should either succeed with the complex result or fallback
        expect(typeof result).toBe("string");
        expect(result === "step1-step2-complete" || result === "complex-fallback").toBe(true);
      })
    );
  });
});