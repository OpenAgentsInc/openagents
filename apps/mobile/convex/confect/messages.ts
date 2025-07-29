import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  ConfectDoc,
  mutation,
  query,
} from "./confect";
import { UserIdentity } from "@rjdellecese/confect/server";
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
  GetSessionMessagesPaginatedArgs,
  GetSessionMessagesPaginatedResult,
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

      console.log('💾 [CONFECT] addClaudeMessage called:', {
        sessionId,
        messageId,
        messageType,
        contentPreview: content.substring(0, 50),
        timestamp
      });

      // Get authenticated user (optional for backwards compatibility)
      const identity = yield* auth.getUserIdentity();
      let userId = Option.none<string>();
      
      if (Option.isSome(identity)) {
        // Find user by OpenAuth subject
        const authSubject = (identity.value as UserIdentity).subject;
        
        console.log(`🔍 [MESSAGES] Looking for user with OpenAuth subject: ${authSubject}`);
        
        const user = yield* db
          .query("users")
          .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
          .first();
          
        if (Option.isSome(user)) {
          userId = Option.some((user.value as ConfectDoc<"users">)._id);
        }
      }

      // Check if message already exists to avoid duplicates
      const existingMessage = yield* db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .filter((q) => q.eq(q.field("messageId"), messageId))
        .first();

      if (Option.isSome(existingMessage)) {
        console.warn('⚠️ [CONFECT] Message already exists, skipping duplicate:', messageId);
        return (existingMessage.value as ConfectDoc<"claudeMessages">)._id;
      }

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
      
      console.log('✅ [CONFECT] Message added successfully:', messageId);

      // Update session last activity
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isSome(session)) {
        yield* db.patch((session.value as ConfectDoc<"claudeSessions">)._id, {
          lastActivity: Date.now(),
        });
      }

      // Update sync status
      const syncStatus = yield* db
        .query("syncStatus")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isSome(syncStatus)) {
        yield* db.patch((syncStatus.value as ConfectDoc<"syncStatus">)._id, {
          lastSyncedMessageId: messageId,
        });
      }

      return messageDoc;
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

// Get session messages with pagination
export const getSessionMessagesPaginated = query({
  args: GetSessionMessagesPaginatedArgs,
  returns: GetSessionMessagesPaginatedResult,
  handler: ({ sessionId, paginationOpts }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      console.log(`📄 [CONFECT] Fetching paginated messages for session: ${sessionId} (${paginationOpts.numItems} items)`);

      const result = yield* db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .order("asc")
        .paginate(paginationOpts);

      console.log(`✅ [CONFECT] Paginated messages fetched: ${result.page.length} items, isDone: ${result.isDone}`);

      return result;
    }),
});