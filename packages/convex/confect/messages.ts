import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  GetMessagesArgs,
  GetMessagesResult,
  AddMessageArgs,
  AddMessageResult,
  GetMessageCountArgs,
  GetMessageCountResult,
  AddClaudeMessageArgs,
  AddClaudeMessageResult,
  GetSessionMessagesArgs,
  GetSessionMessagesResult,
} from "./messages.schemas";

// Query to get all messages
export const getMessages = query({
  args: GetMessagesArgs,
  returns: GetMessagesResult,
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      return yield* db.query("messages").order("asc").take(100).collect();
    }),
});

// Mutation to add a new message
export const addMessage = mutation({
  args: AddMessageArgs,
  returns: AddMessageResult,
  handler: ({ body, user }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      return yield* db.insert("messages", {
        body,
        user,
        timestamp: Date.now(),
      });
    }),
});

// Query to get message count
export const getMessageCount = query({
  args: GetMessageCountArgs,
  returns: GetMessageCountResult,
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      const messages = yield* db.query("messages").collect();
      return messages.length;
    }),
});

// Add Claude Code message
export const addClaudeMessage = mutation({
  args: AddClaudeMessageArgs,
  returns: AddClaudeMessageResult,
  handler: ({ sessionId, messageId, messageType, content, timestamp, toolInfo, metadata }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      yield* Effect.logInfo('💾 [CONFECT] addClaudeMessage called:', {
        sessionId,
        messageId,
        messageType,
        contentPreview: content.substring(0, 50),
        timestamp
      });

      // Get authenticated user (optional for backwards compatibility)
      const identity = yield* Effect.promise(() => auth.getUserIdentity());
      let userId = Option.none<string>();
      
      if (identity) {
        const user = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
          .first();
          
        if (Option.isSome(user)) {
          userId = Option.some(user.value._id);
        }
      }

      // Check if message already exists to avoid duplicates
      const existingMessage = yield* db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .filter((q) => q.eq(q.field("messageId"), messageId))
        .first();

      return yield* Option.match(existingMessage, {
        onSome: (message) => {
          yield* Effect.logWarning('⚠️ [CONFECT] Message already exists, skipping duplicate:', messageId);
          return Effect.succeed(message._id);
        },
        
        onNone: () =>
          Effect.gen(function* () {
            // Add message with user ID
            const messageDoc = yield* db.insert("claudeMessages", {
              sessionId,
              messageId,
              messageType,
              content,
              timestamp,
              toolInfo,
              metadata,
              ...(Option.isSome(userId) ? { userId: userId.value } : {}),
            });
            
            yield* Effect.logInfo('✅ [CONFECT] Message added successfully:', messageId);

            // Update session last activity
            const session = yield* db
              .query("claudeSessions")
              .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
              .first();

            yield* Option.match(session, {
              onSome: (s) =>
                db.patch(s._id, {
                  lastActivity: Date.now(),
                }),
              onNone: () => Effect.void,
            });

            // Update sync status
            const syncStatus = yield* db
              .query("syncStatus")
              .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
              .first();

            yield* Option.match(syncStatus, {
              onSome: (sync) =>
                db.patch(sync._id, {
                  lastSyncedMessageId: messageId,
                }),
              onNone: () => Effect.void,
            });

            return messageDoc;
          })
      });
    }),
});

// Get session messages
export const getSessionMessages = query({
  args: GetSessionMessagesArgs,
  returns: GetSessionMessagesResult,
  handler: ({ sessionId, limit }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      const query = db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .order("asc");

      if (limit) {
        return yield* query.take(limit).collect();
      }

      return yield* query.collect();
    }),
});