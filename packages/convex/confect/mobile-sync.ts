import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  CreateClaudeSessionArgs,
  CreateClaudeSessionResult,
  UpdateSessionStatusArgs,
  UpdateSessionStatusResult,
  GetPendingMobileSessionsArgs,
  GetPendingMobileSessionsResult,
} from "./mobile-sync.schemas";

export const createClaudeSession = mutation({
  args: CreateClaudeSessionArgs,
  returns: CreateClaudeSessionResult,
  handler: ({ sessionId, projectPath, createdBy, title, metadata }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Get authenticated user (optional for backwards compatibility)
      const identity = yield* auth.getUserIdentity();
      let userId = Option.none<string>();
      
      if (Option.isSome(identity)) {
        // Find user by OpenAuth subject
        const authSubject = identity.value.subject;
        
        console.log(`🔍 [MOBILE_SYNC] Looking for user with OpenAuth subject: ${authSubject}`);
        
        const user = yield* db
          .query("users")
          .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
          .first();
          
        if (Option.isSome(user)) {
          userId = Option.some(user.value._id);
        }
      }

      // Check if session already exists
      const existingSession = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      return yield* Option.match(existingSession, {
        onSome: (session) =>
          // Update existing session
          db.patch(session._id, {
            title,
            status: "active" as const,
            lastActivity: Date.now(),
            metadata,
            ...(Option.isSome(userId) ? { userId: userId.value } : {}),
          }).pipe(Effect.as(session._id)),
        
        onNone: () =>
          Effect.gen(function* () {
            // Create new session
            const sessionDoc = yield* db.insert("claudeSessions", {
              sessionId,
              projectPath,
              title: title || `${createdBy} Session - ${new Date().toLocaleString()}`,
              status: "active" as const,
              createdBy,
              lastActivity: Date.now(),
              ...(Option.isSome(userId) ? { userId: userId.value } : {}),
              metadata,
            });

            // Initialize sync status
            yield* db.insert("syncStatus", {
              sessionId,
            });

            return sessionDoc;
          })
      });
    }),
});

export const updateSessionStatus = mutation({
  args: UpdateSessionStatusArgs,
  returns: UpdateSessionStatusResult,
  handler: ({ sessionId, status }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      yield* Option.match(session, {
        onSome: (s) =>
          db.patch(s._id, {
            status,
            lastActivity: Date.now(),
          }),
        onNone: () => Effect.void,
      });

      return null;
    }),
});

export const getPendingMobileSessions = query({
  args: GetPendingMobileSessionsArgs,
  returns: GetPendingMobileSessionsResult,
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      const results = yield* db
        .query("claudeSessions")
        .withIndex("by_status", (q) => q.eq("status", "active"))
        .filter((q) => q.eq(q.field("createdBy"), "mobile"))
        .order("desc")
        .take(10)
        .collect();

      yield* Effect.logInfo(`🔍 [CONFECT] getPendingMobileSessions query returned: ${results.length} sessions`);
      
      if (results.length > 0) {
        yield* Effect.logInfo("📋 [CONFECT] Mobile sessions:", results.map(s => ({
          sessionId: s.sessionId,
          status: s.status,
          createdBy: s.createdBy,
          metadata: s.metadata
        })));
      }

      return results;
    }),
});