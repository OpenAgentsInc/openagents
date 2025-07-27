import { Effect, Schedule, Duration, Data } from "effect";

// Tagged error types for mobile sync operations
export class MobileSyncError extends Data.TaggedError("MobileSyncError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class SessionValidationError extends Data.TaggedError("SessionValidationError")<{
  sessionId: string;
  reason: string;
}> {}

export class ProcessingTimeoutError extends Data.TaggedError("ProcessingTimeoutError")<{
  sessionId: string;
  timeoutMs: number;
}> {}

// Mobile session type
export interface MobileSession {
  sessionId: string;
  projectPath: string;
  title?: string;
}

// Service interface for mobile sync operations
export interface MobileSyncService {
  readonly processMobileSession: (session: MobileSession) => Effect.Effect<string, MobileSyncError | SessionValidationError | ProcessingTimeoutError>;
  readonly getPendingMobileSessions: () => Effect.Effect<MobileSession[], MobileSyncError>;
  readonly updateSessionStatus: (sessionId: string, status: "active" | "inactive" | "error" | "processed") => Effect.Effect<void, MobileSyncError>;
}

// Effect service implementation
export class MobileSyncServiceLive extends Effect.Service<MobileSyncService>()("MobileSyncService", {
  sync: () => {
    let confectMutations: any = null;
    let confectQueries: any = null;

    return {
      processMobileSession: (session: MobileSession) =>
        pipe(
          validateMobileSession(session),
          Effect.flatMap(() => createOrUpdateSession(session)),
          Effect.retry(
            Schedule.exponential(Duration.seconds(1)).pipe(
              Schedule.intersect(Schedule.recurs(3))
            )
          ),
          Effect.timeout(Duration.seconds(30)),
          Effect.catchTags({
            SessionValidationError: (error) => Effect.fail(error),
            TimeoutError: () => Effect.fail(new ProcessingTimeoutError({
              sessionId: session.sessionId,
              timeoutMs: 30000
            }))
          }),
          Effect.catchAll((error) => Effect.fail(new MobileSyncError({
            operation: "processMobileSession",
            message: String(error),
            cause: error
          })))
        ),

      getPendingMobileSessions: () =>
        pipe(
          Effect.tryPromise({
            try: () => confectQueries.getPendingMobileSessions({}),
            catch: (error) => new MobileSyncError({
              operation: "getPendingMobileSessions",
              message: String(error),
              cause: error
            })
          }),
          Effect.retry(Schedule.exponential(Duration.millis(500)).pipe(Schedule.recurs(2))),
          Effect.timeout(Duration.seconds(10)),
          Effect.map((sessions) => sessions || [])
        ),

      updateSessionStatus: (sessionId: string, status: "active" | "inactive" | "error" | "processed") =>
        pipe(
          Effect.tryPromise({
            try: () => confectMutations.updateSessionStatus({ sessionId, status }),
            catch: (error) => new MobileSyncError({
              operation: "updateSessionStatus",
              message: String(error),
              cause: error
            })
          }),
          Effect.retry(Schedule.exponential(Duration.seconds(2)).pipe(Schedule.recurs(2))),
          Effect.timeout(Duration.seconds(15)),
          Effect.asVoid
        )
    };

    function validateMobileSession(session: MobileSession): Effect.Effect<void, SessionValidationError> {
      if (!session.sessionId || !session.projectPath) {
        return Effect.fail(new SessionValidationError({
          sessionId: session.sessionId || "unknown",
          reason: "Missing required fields: sessionId or projectPath"
        }));
      }
      return Effect.void;
    }

    function createOrUpdateSession(session: MobileSession): Effect.Effect<string> {
      return Effect.tryPromise({
        try: () => confectMutations.createClaudeSession({
          sessionId: session.sessionId,
          projectPath: session.projectPath,
          createdBy: "mobile" as const,
          title: session.title
        }),
        catch: (error) => new MobileSyncError({
          operation: "createOrUpdateSession",
          message: String(error),
          cause: error
        })
      });
    }
  }
}) {}

// Helper to pipe operations with Effect
function pipe<A, B>(a: A, ab: (a: A) => B): B;
function pipe<A, B, C>(a: A, ab: (a: A) => B, bc: (b: B) => C): C;
function pipe<A, B, C, D>(a: A, ab: (a: A) => B, bc: (b: B) => C, cd: (c: C) => D): D;
function pipe(value: any, ...fns: any[]): any {
  return fns.reduce((acc, fn) => fn(acc), value);
}