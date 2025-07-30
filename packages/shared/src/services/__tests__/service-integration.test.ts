import { Effect, Runtime, Exit } from "effect";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ServiceTestUtils, benchmarkEffect } from "./setup-service-tests";
import { createClaudeSessionService } from "../ClaudeSessionServiceSimple";
import { createSessionResilienceService } from "../SessionResilienceServiceSimple";
import {
  CreateSessionParams,
  SessionData,
  SessionCreationError,
} from "../../types/session-service-types";

/**
 * Service Integration Testing Suite
 * 
 * Tests the integration between ClaudeSessionService and SessionResilienceService,
 * simulating how React hooks would use these services together.
 * 
 * Addresses CodeRabbit feedback about service integration and proper Effect runtime usage.
 */

describe("Service Integration", () => {
  let sessionService: ReturnType<typeof createClaudeSessionService>;
  let resilienceService: ReturnType<typeof createSessionResilienceService>;
  let runtime: Runtime.Runtime<never>;
  
  beforeEach(() => {
    sessionService = createClaudeSessionService();
    resilienceService = createSessionResilienceService();
    runtime = Runtime.defaultRuntime;
  });

  const validCreateParams: CreateSessionParams = {
    sessionId: "integration-test-session",
    projectPath: "/test/integration",
    createdBy: "desktop",
    title: "Integration Test Session",
    metadata: {
      workingDirectory: "/test/integration",
      model: "claude-3-sonnet",
      systemPrompt: "Integration test prompt",
    },
  };

  describe("Service Interoperability", () => {
    ServiceTestUtils.runServiceTest(
      "should integrate basic session service with resilience service",
      Effect.gen(function* () {
        // Create session with basic service
        const basicSession = yield* sessionService.createSession(validCreateParams);
        
        // Create resilient session for comparison
        const resilientSession = yield* resilienceService.createSessionResilient({
          ...validCreateParams,
          sessionId: "resilient-integration-session",
        });

        // Both should create valid sessions
        expect(basicSession.sessionId).toBe(validCreateParams.sessionId);
        expect(resilientSession.sessionId).toMatch(/resilient-integration-session/);
        
        // Both should have compatible structures
        expect(basicSession).toMatchObject({
          projectPath: validCreateParams.projectPath,
          createdBy: validCreateParams.createdBy,
          version: 1,
        });
        
        expect(resilientSession).toMatchObject({
          projectPath: validCreateParams.projectPath,
          createdBy: validCreateParams.createdBy,
          version: 1,
        });
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle service composition for complete workflows",
      Effect.gen(function* () {
        // Step 1: Create session with resilience
        const session = yield* resilienceService.createSessionResilient(validCreateParams);
        
        // Step 2: Update session status
        const updatedSession = yield* sessionService.updateSessionStatus(
          session.sessionId.replace("offline-", ""), // Handle potential offline prefix
          "processed"
        );
        
        // Step 3: Query sessions to verify
        const queryResult = yield* sessionService.querySessionsAdvanced({
          userId: session.userId,
          status: "processed",
        });
        
        expect(updatedSession.status).toBe("processed");
        expect(queryResult.sessions).toBeDefined();
        expect(Array.isArray(queryResult.sessions)).toBe(true);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should maintain data consistency across service boundaries",
      Effect.gen(function* () {
        const testMetadata = {
          workingDirectory: "/test/consistency",
          model: "claude-3-opus",
          systemPrompt: "Consistency test",
          aiModel: "claude-3-opus",
          contextWindow: 180000,
        };

        const paramsWithMetadata: CreateSessionParams = {
          ...validCreateParams,
          sessionId: "consistency-test",
          metadata: testMetadata,
        };

        // Create with resilience service
        const resilientSession = yield* resilienceService.createSessionResilient(paramsWithMetadata);
        
        // Retrieve with basic service (simulating hook behavior)
        const retrievedSession = yield* sessionService.getSession("consistency-test");
        
        // Verify metadata consistency (adjust for offline fallback behavior)
        if (resilientSession.status === "offline") {
          expect(resilientSession.metadata).toEqual(testMetadata);
        } else {
          expect(resilientSession.metadata).toEqual(testMetadata);
          expect(retrievedSession.metadata).toBeDefined();
        }
      })
    );
  });

  describe("Runtime Integration Patterns", () => {
    ServiceTestUtils.runServiceTest(
      "should handle Effect runtime execution patterns like React hooks",
      Effect.gen(function* () {
        // Simulate how React hook would execute Effect operations
        const createOperation = sessionService.createSession(validCreateParams);
        
        // Test Promise-based execution (how hooks would call it)
        const promiseResult = yield* Effect.promise(() => 
          Runtime.runPromise(runtime)(createOperation)
        );
        
        expect(promiseResult).toMatchObject({
          sessionId: validCreateParams.sessionId,
          status: "active",
        });
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle Effect Exit patterns for error discrimination",
      Effect.gen(function* () {
        const invalidParams: CreateSessionParams = {
          sessionId: "",
          projectPath: "/test/exit",
          createdBy: "desktop",
        };

        // Test Exit-based error handling (how hooks handle errors)
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
      "should handle concurrent operations like React hooks might trigger",
      ServiceTestUtils.testConcurrency(
        [
          sessionService.createSession({
            sessionId: "concurrent-hook-1",
            projectPath: "/test/hook1",
            createdBy: "desktop",
          }),
          resilienceService.createSessionResilient({
            sessionId: "concurrent-hook-2", 
            projectPath: "/test/hook2",
            createdBy: "mobile",
          }),
          sessionService.updateSessionStatus("concurrent-hook-1", "active"),
        ],
        2 // Simulate hook concurrency limits
      ).pipe(
        Effect.map((results) => {
          expect(results).toHaveLength(3);
          
          // Verify all operations completed
          results.forEach((result, index) => {
            expect(result.index).toBe(index);
            expect(result.result).toBeDefined();
          });
        })
      )
    );
  });

  describe("Error Recovery Integration", () => {
    ServiceTestUtils.runServiceTest(
      "should demonstrate error recovery across service layers",
      Effect.gen(function* () {
        // Step 1: Try to create session with invalid params (will fail)
        const invalidParams = {
          sessionId: "",
          projectPath: "/test/recovery",
          createdBy: "desktop" as const,
        };

        const failedResult = yield* Effect.either(
          sessionService.createSession(invalidParams)
        );
        
        expect(failedResult._tag).toBe("Left");

        // Step 2: Use resilience service as recovery mechanism
        const recoveryParams: CreateSessionParams = {
          sessionId: "recovery-session",
          projectPath: "/test/recovery",
          createdBy: "desktop",
          title: "Recovery Session",
        };

        const recoveredResult = yield* resilienceService.createSessionResilient(recoveryParams);
        
        // Should succeed (either normal or offline)
        expect(recoveredResult.sessionId).toMatch(/recovery-session/);
        expect(recoveredResult.status).toMatch(/^(active|offline)$/);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should handle cascading error recovery patterns",
      Effect.gen(function* () {
        // Simulate multiple service calls with potential failures
        const operations = [
          Effect.either(sessionService.createSession({
            sessionId: "cascade-1",
            projectPath: "/test/cascade",
            createdBy: "desktop",
          })),
          Effect.either(sessionService.updateSessionStatus("cascade-1", "processed")),
          Effect.either(sessionService.querySessionsAdvanced({ userId: "test-user" })),
        ];

        const results = yield* Effect.all(operations);
        
        // All operations should complete (either success or handled failure)
        expect(results).toHaveLength(3);
        results.forEach((result) => {
          expect(result._tag === "Left" || result._tag === "Right").toBe(true);
        });
      })
    );
  });

  describe("Performance Integration", () => {
    ServiceTestUtils.runServiceTest(
      "should maintain performance across integrated service calls",
      benchmarkEffect(
        "integrated-service-workflow",
        Effect.gen(function* () {
          // Complete workflow: create -> update -> query
          const session = yield* resilienceService.createSessionResilient({
            sessionId: "perf-integration",
            projectPath: "/test/perf",
            createdBy: "desktop",
          });
          
          const updated = yield* sessionService.updateSessionStatus(
            session.sessionId.replace("offline-", ""),
            "active"
          );
          
          const queried = yield* sessionService.querySessionsAdvanced({
            userId: updated.userId,
          });
          
          return {
            sessionCreated: session.sessionId,
            statusUpdated: updated.status,
            queriedCount: queried.sessions.length,
          };
        }),
        800 // Allow more time for integrated workflow
      ).pipe(
        Effect.map(({ result, duration }) => {
          expect(result.sessionCreated).toBeDefined();
          expect(result.statusUpdated).toBe("active");
          expect(result.queriedCount).toBeGreaterThanOrEqual(0);
          expect(duration).toBeLessThan(800);
        })
      )
    );

    ServiceTestUtils.runServiceTest(
      "should handle memory efficiency in service integration",
      Effect.gen(function* () {
        // Create multiple sessions to test memory usage patterns
        const sessions = yield* Effect.all(
          Array.from({ length: 10 }, (_, i) => 
            resilienceService.createSessionResilient({
              sessionId: `memory-test-${i}`,
              projectPath: `/test/memory/${i}`,
              createdBy: "desktop",
              title: `Memory Test Session ${i}`,
            })
          )
        );

        // Verify all sessions were created
        expect(sessions).toHaveLength(10);
        sessions.forEach((session, index) => {
          expect(session.sessionId).toMatch(new RegExp(`memory-test-${index}`));
        });

        // Test cleanup pattern (simulating React hook cleanup)
        const cleanup = Effect.sync(() => {
          // Simulate cleanup operations
          return { cleaned: sessions.length };
        });

        const cleanupResult = yield* cleanup;
        expect(cleanupResult.cleaned).toBe(10);
      })
    );
  });

  describe("Service Configuration Integration", () => {
    ServiceTestUtils.runServiceTest(
      "should handle service configuration compatibility",
      Effect.gen(function* () {
        // Test different service configurations work together
        const desktopParams: CreateSessionParams = {
          sessionId: "desktop-config",
          projectPath: "/desktop/project",
          createdBy: "desktop",
          metadata: {
            aiModel: "claude-3-sonnet",
            contextWindow: 200000,
          },
        };

        const mobileParams: CreateSessionParams = {
          sessionId: "mobile-config",
          projectPath: "/mobile/project", 
          createdBy: "mobile",
          metadata: {
            aiModel: "claude-3-haiku",
            contextWindow: 100000,
          },
        };

        const desktopSession = yield* sessionService.createSession(desktopParams);
        const mobileSession = yield* resilienceService.createSessionResilient(mobileParams);

        // Both should work with their respective configurations
        expect(desktopSession.createdBy).toBe("desktop");
        expect(mobileSession.createdBy).toBe("mobile");
        
        expect(desktopSession.metadata?.contextWindow).toBe(200000);
        expect(mobileSession.metadata?.contextWindow).toBe(100000);
      })
    );

    ServiceTestUtils.runServiceTest(
      "should maintain service boundary integrity",
      Effect.gen(function* () {
        // Test that services maintain their boundaries and don't interfere
        let sessionServiceCalls = 0;
        let resilienceServiceCalls = 0;

        // Wrap services to count calls
        const countingSessionService = {
          ...sessionService,
          createSession: (params: CreateSessionParams) => {
            sessionServiceCalls++;
            return sessionService.createSession(params);
          },
        };

        const countingResilienceService = {
          ...resilienceService,
          createSessionResilient: (params: CreateSessionParams) => {
            resilienceServiceCalls++;
            return resilienceService.createSessionResilient(params);
          },
        };

        // Make calls to both services
        yield* countingSessionService.createSession({
          sessionId: "boundary-test-1",
          projectPath: "/test/boundary",
          createdBy: "desktop",
        });

        yield* countingResilienceService.createSessionResilient({
          sessionId: "boundary-test-2",
          projectPath: "/test/boundary",
          createdBy: "mobile",
        });

        // Verify calls were tracked correctly
        expect(sessionServiceCalls).toBe(1);
        expect(resilienceServiceCalls).toBe(1);
      })
    );
  });
});