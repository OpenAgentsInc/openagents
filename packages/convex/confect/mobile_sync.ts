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
  GetSessionsArgs,
  GetSessionsResult,
  GetSessionArgs,
  GetSessionResult,
  GetSessionMessagesArgs,
  GetSessionMessagesResult,
  AddClaudeMessageArgs,
  AddClaudeMessageResult,
  BatchAddMessagesArgs,
  BatchAddMessagesResult,
  RequestDesktopSessionArgs,
  RequestDesktopSessionResult,
  UpdateSyncStatusArgs as UpdateSyncStatusSchemaArgs,
  UpdateSyncStatusResult as UpdateSyncStatusSchemaResult,
  GetSyncStatusArgs,
  GetSyncStatusResult,
  SyncSessionFromHookArgs,
  SyncSessionFromHookResult,
  MarkMobileSessionProcessedArgs,
  MarkMobileSessionProcessedResult,
} from "./mobile_sync_schemas";

export const createClaudeSession = mutation({
  args: CreateClaudeSessionArgs,
  returns: CreateClaudeSessionResult,
  handler: ({ sessionId, projectPath, createdBy, title, metadata }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Get authenticated user (optional for backwards compatibility)
      const identity = yield* auth.getUserIdentity();
      let userId: any = undefined;
      
      if (Option.isSome(identity)) {
        // Find user by OpenAuth subject
        const authSubject = identity.value.subject;
        
        console.log(`🔍 [MOBILE_SYNC] Looking for user with OpenAuth subject: ${authSubject}`);
        
        const user = yield* db
          .query("users")
          .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
          .first();
          
        if (Option.isSome(user)) {
          userId = user.value._id;
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
            ...(userId ? { userId } : {}),
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
              ...(userId ? { userId } : {}),
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
        .take(10);

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

// Helper function to get authenticated user with Effect-TS patterns (for mutations)
const getAuthenticatedUserEffectMutation = Effect.gen(function* () {
  const { db, auth } = yield* ConfectMutationCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new Error("Not authenticated"));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  console.log(`🔍 [CONFECT] Looking for user with OpenAuth subject: ${authSubject}`);
  
  const user = yield* db
    .query("users")
    .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
    .first();

  return yield* Option.match(user, {
    onSome: (u) => Effect.succeed(u),
    onNone: () =>
      // Fallback: try looking up by GitHub ID (for backwards compatibility)
      Effect.gen(function* () {
        const fallbackUser = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", authSubject))
          .first();
        
        return yield* Option.match(fallbackUser, {
          onSome: (u) => Effect.succeed(u),
          onNone: () => Effect.fail(new Error("User not found"))
        });
      })
  });
});

// Helper function to get authenticated user with Effect-TS patterns (for queries)
const getAuthenticatedUserEffectQuery = Effect.gen(function* () {
  const { db, auth } = yield* ConfectQueryCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new Error("Not authenticated"));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  console.log(`🔍 [CONFECT] Looking for user with OpenAuth subject: ${authSubject}`);
  
  const user = yield* db
    .query("users")
    .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
    .first();

  return yield* Option.match(user, {
    onSome: (u) => Effect.succeed(u),
    onNone: () =>
      // Fallback: try looking up by GitHub ID (for backwards compatibility)
      Effect.gen(function* () {
        const fallbackUser = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", authSubject))
          .first();
        
        return yield* Option.match(fallbackUser, {
          onSome: (u) => Effect.succeed(u),
          onNone: () => Effect.fail(new Error("User not found"))
        });
      })
  });
});

// Get user sessions
export const getSessions = query({
  args: GetSessionsArgs,
  returns: GetSessionsResult,
  handler: ({ limit, status }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;
      const user = yield* getAuthenticatedUserEffectQuery;

      let queryBuilder = db
        .query("claudeSessions")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id));
      
      if (status) {
        queryBuilder = queryBuilder.filter((q) => q.eq(q.field("status"), status));
      }
      
      const results = yield* queryBuilder.order("desc").take(limit ?? 50);
      
      yield* Effect.logInfo(`🔍 [CONFECT] getSessions returned ${results.length} sessions for user ${user._id}`);
      
      return results;
    }),
});

// Get specific session
export const getSession = query({
  args: GetSessionArgs,
  returns: GetSessionResult,
  handler: ({ sessionId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;
      const user = yield* getAuthenticatedUserEffectQuery;

      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      return yield* Option.match(session, {
        onSome: (s) => 
          s.userId === user._id 
            ? Effect.succeed(s)
            : Effect.fail(new Error("Session not found or access denied")),
        onNone: () => Effect.succeed(null)
      });
    }),
});

// Get session messages
export const getSessionMessages = query({
  args: GetSessionMessagesArgs,
  returns: GetSessionMessagesResult,
  handler: ({ sessionId, limit }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;
      
      // Get authenticated user inline
      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      const authSubject = identity.value.subject;
      const user = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      // First verify session ownership
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isNone(session)) {
        return yield* Effect.fail(new Error("Session not found"));
      }

      if (session.value.userId !== user.value._id) {
        return yield* Effect.fail(new Error("Access denied to session messages"));
      }

      let queryBuilder = db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .order("asc");

      const results = limit 
        ? yield* queryBuilder.take(limit)
        : yield* queryBuilder.take(1000);  // Use a reasonable default limit

      yield* Effect.logInfo(`📋 [CONFECT] getSessionMessages returned ${results.length} messages for session ${sessionId}`);

      return results;
    }),
});

// Add message to session
export const addClaudeMessage = mutation({
  args: AddClaudeMessageArgs,
  returns: AddClaudeMessageResult,
  handler: ({ sessionId, messageId, messageType, content, timestamp, toolInfo, metadata }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      yield* Effect.logInfo(`💾 [CONFECT] addClaudeMessage called:`, {
        sessionId,
        messageId,
        messageType,
        contentPreview: content.substring(0, 50),
        timestamp
      });

      // Verify session ownership
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isNone(session)) {
        return yield* Effect.fail(new Error("Session not found"));
      }

      if (session.value.userId !== user._id) {
        return yield* Effect.fail(new Error("Access denied to add message to this session"));
      }

      // Check if message already exists (prevent duplicates)
      const existingMessage = yield* db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .filter((q) => q.eq(q.field("messageId"), messageId))
        .first();

      return yield* Option.match(existingMessage, {
        onSome: (msg) => 
          Effect.gen(function* () {
            yield* Effect.logInfo(`⚠️ [CONFECT] Message ${messageId} already exists, skipping`);
            return msg._id;
          }),
        onNone: () =>
          Effect.gen(function* () {
            const messageDoc = yield* db.insert("claudeMessages", {
              sessionId,
              messageId,
              messageType,
              content,
              timestamp,
              toolInfo,
              metadata,
            });

            // Update session last activity
            yield* db.patch(session.value._id, {
              lastActivity: Date.now(),
            });

            yield* Effect.logInfo(`✅ [CONFECT] Successfully added message ${messageId} to session ${sessionId}`);

            return messageDoc;
          })
      });
    }),
});

// Batch add messages to session
export const batchAddMessages = mutation({
  args: BatchAddMessagesArgs,
  returns: BatchAddMessagesResult,
  handler: ({ sessionId, messages }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      yield* Effect.logInfo(`📦 [CONFECT] batchAddMessages called:`, {
        sessionId,
        messageCount: messages.length,
      });

      // Verify session ownership
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isNone(session)) {
        return yield* Effect.fail(new Error("Session not found"));
      }

      if (session.value.userId !== user._id) {
        return yield* Effect.fail(new Error("Session not found or access denied"));
      }

      const insertedIds: any[] = [];

      // Process messages in sequence
      for (const message of messages) {
        // Check if message already exists
        const existingMessage = yield* db
          .query("claudeMessages")
          .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
          .filter((q) => q.eq(q.field("messageId"), message.messageId))
          .first();

        if (Option.isNone(existingMessage)) {
          const messageDoc = yield* db.insert("claudeMessages", {
            sessionId,
            messageId: message.messageId,
            messageType: message.messageType,
            content: message.content,
            timestamp: message.timestamp,
            toolInfo: message.toolInfo,
            metadata: message.metadata,
          });

          insertedIds.push(messageDoc);
        }
      }

      // Update session last activity
      yield* db.patch(session.value._id, {
        lastActivity: Date.now(),
      });

      yield* Effect.logInfo(`✅ [CONFECT] batchAddMessages inserted ${insertedIds.length} new messages`);

      return insertedIds;
    }),
});

// Request desktop session from mobile
export const requestDesktopSession = mutation({
  args: RequestDesktopSessionArgs,
  returns: RequestDesktopSessionResult,
  handler: ({ projectPath, initialMessage, title }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      const sessionId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;

      yield* Effect.logInfo(`📱 [CONFECT] requestDesktopSession creating session:`, {
        sessionId,
        projectPath,
        title,
      });

      // Create session
      yield* db.insert("claudeSessions", {
        sessionId,
        projectPath,
        title: title ?? `Mobile Session - ${new Date().toLocaleString()}`,
        status: "active" as const,
        createdBy: "mobile" as const,
        lastActivity: Date.now(),
        userId: user._id,
      });

      // Initialize sync status
      yield* db.insert("syncStatus", {
        sessionId,
      });

      // Add initial message if provided
      if (initialMessage) {
        yield* Effect.logInfo(`📱 [CONFECT] Adding initial message to session ${sessionId}`);
        const messageId = `user-${Date.now()}`;
        
        yield* db.insert("claudeMessages", {
          sessionId,
          messageId,
          messageType: "user" as const,
          content: initialMessage,
          timestamp: new Date().toISOString(),
        });

        yield* Effect.logInfo(`✅ [CONFECT] Initial message created with ID: ${messageId}`);
      }

      return sessionId;
    }),
});

// Update sync status
export const updateSyncStatusConfect = mutation({
  args: UpdateSyncStatusSchemaArgs,
  returns: UpdateSyncStatusSchemaResult,  
  handler: ({ sessionId, desktopLastSeen, mobileLastSeen, syncErrors }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      const syncStatus = yield* db
        .query("syncStatus")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      yield* Option.match(syncStatus, {
        onSome: (status) => {
          const updates: any = {};
          if (desktopLastSeen !== undefined) updates.desktopLastSeen = desktopLastSeen;
          if (mobileLastSeen !== undefined) updates.mobileLastSeen = mobileLastSeen;
          if (syncErrors !== undefined) updates.syncErrors = syncErrors;

          return db.patch(status._id, updates);
        },
        onNone: () => 
          Effect.logInfo(`⚠️ [CONFECT] Sync status not found for session: ${sessionId}`)
      });

      return null;
    }),
});

// Get sync status
export const getSyncStatusConfect = query({
  args: GetSyncStatusArgs,
  returns: GetSyncStatusResult,
  handler: ({ sessionId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;
      const user = yield* getAuthenticatedUserEffectQuery;

      // Verify session belongs to user before showing sync status
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isNone(session)) {
        return yield* Effect.fail(new Error("Session not found"));
      }

      if (session.value.userId !== user._id) {
        return yield* Effect.fail(new Error("Session not found or access denied"));
      }

      const syncStatus = yield* db
        .query("syncStatus")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      return Option.match(syncStatus, {
        onSome: (status) => status,
        onNone: () => null
      });
    }),
});

// Sync session from desktop hook
export const syncSessionFromHook = mutation({
  args: SyncSessionFromHookArgs,
  returns: SyncSessionFromHookResult,
  handler: ({ hookData }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      yield* Effect.logInfo(`🔄 [CONFECT] syncSessionFromHook called:`, {
        sessionId: hookData.sessionId,
        event: hookData.event,
        messageCount: hookData.messages?.length || 0,
      });

      // Ensure session exists
      let session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", hookData.sessionId))
        .first();

      if (Option.isNone(session)) {
        // Create new session
        yield* db.insert("claudeSessions", {
          sessionId: hookData.sessionId,
          projectPath: hookData.projectPath,
          title: `Desktop Session - ${hookData.projectPath}`,
          status: "active" as const,
          createdBy: "desktop" as const,
          lastActivity: Date.now(),
          userId: user._id,
        });

        yield* db.insert("syncStatus", {
          sessionId: hookData.sessionId,
        });
      } else {
        // Verify session belongs to authenticated user
        if (session.value.userId !== user._id) {
          return yield* Effect.fail(new Error("Access denied to sync session"));
        }
      }

      // Sync messages if provided
      if (hookData.messages && hookData.messages.length > 0) {
        for (const message of hookData.messages) {
          yield* db.insert("claudeMessages", {
            sessionId: hookData.sessionId,
            messageId: message.id || `${message.message_type}-${Date.now()}-${Math.random()}`,
            messageType: message.message_type,
            content: message.content,
            timestamp: message.timestamp,
            toolInfo: message.tool_info ? {
              toolName: message.tool_info.tool_name,
              toolUseId: message.tool_info.tool_use_id,
              input: message.tool_info.input,
              output: message.tool_info.output,
            } : undefined,
            metadata: { hookEvent: hookData.event },
          });
        }
      }

      return { success: true };
    }),
});

// Mark mobile session as processed
export const markMobileSessionProcessed = mutation({
  args: MarkMobileSessionProcessedArgs,
  returns: MarkMobileSessionProcessedResult,
  handler: ({ mobileSessionId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", mobileSessionId))
        .first();

      yield* Option.match(session, {
        onSome: (s) =>
          db.patch(s._id, {
            status: "processed" as const,
            lastActivity: Date.now(),
          }),
        onNone: () => 
          Effect.logInfo(`⚠️ [CONFECT] Session not found for processing: ${mobileSessionId}`)
      });

      return null;
    }),
});