import { defineSchema, defineTable, Id } from "@rjdellecese/confect/server";
import { Schema } from "effect";

export const confectSchema = defineSchema({
  // Users table for authenticated users
  users: defineTable(
    Schema.Struct({
      email: Schema.String.pipe(Schema.nonEmpty()),
      name: Schema.optional(Schema.String),
      avatar: Schema.optional(Schema.String),
      githubId: Schema.String.pipe(Schema.nonEmpty()),
      githubUsername: Schema.String.pipe(Schema.nonEmpty()),
      createdAt: Schema.Number,
      lastLogin: Schema.Number,
    })
  ).index("by_email", ["email"])
   .index("by_github_id", ["githubId"]),

  // Claude Code sessions
  claudeSessions: defineTable(
    Schema.Struct({
      sessionId: Schema.String.pipe(Schema.nonEmpty()),
      projectPath: Schema.String.pipe(Schema.nonEmpty()),
      title: Schema.optional(Schema.String),
      status: Schema.Literal("active", "inactive", "error", "processed"),
      createdBy: Schema.Literal("desktop", "mobile"),
      lastActivity: Schema.Number,
      userId: Schema.optional(Id.Id("users")),
      metadata: Schema.optional(
        Schema.Struct({
          workingDirectory: Schema.optional(Schema.String),
          model: Schema.optional(Schema.String),
          systemPrompt: Schema.optional(Schema.String),
          originalMobileSessionId: Schema.optional(Schema.String),
        })
      ),
    })
  ).index("by_session_id", ["sessionId"])
   .index("by_status", ["status"])
   .index("by_last_activity", ["lastActivity"])
   .index("by_user_id", ["userId"]),

  // Sync status tracking
  syncStatus: defineTable(
    Schema.Struct({
      sessionId: Schema.String.pipe(Schema.nonEmpty()),
      lastSyncedMessageId: Schema.optional(Schema.String),
      desktopLastSeen: Schema.optional(Schema.Number),
      mobileLastSeen: Schema.optional(Schema.Number),
      syncErrors: Schema.optional(Schema.Array(Schema.String)),
    })
  ).index("by_session_id", ["sessionId"]),
});

// Export the traditional Convex schema for compatibility
export default confectSchema.convexSchemaDefinition;