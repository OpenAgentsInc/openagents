import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

// CreateClaudeSession schemas
export const CreateClaudeSessionArgs = Schema.Struct({
  sessionId: Schema.String,
  projectPath: Schema.String,
  createdBy: Schema.Literal("desktop", "mobile"),
  title: Schema.optional(Schema.String),
  metadata: Schema.optional(
    Schema.Struct({
      workingDirectory: Schema.optional(Schema.String),
      model: Schema.optional(Schema.String),
      systemPrompt: Schema.optional(Schema.String),
      originalMobileSessionId: Schema.optional(Schema.String),
    })
  ),
});

export const CreateClaudeSessionResult = Id.Id("claudeSessions");

// UpdateSessionStatus schemas
export const UpdateSessionStatusArgs = Schema.Struct({
  sessionId: Schema.String,
  status: Schema.Literal("active", "inactive", "error", "processed"),
});

export const UpdateSessionStatusResult = Schema.Null;

// GetPendingMobileSessions schemas  
export const GetPendingMobileSessionsArgs = Schema.Struct({});

export const GetPendingMobileSessionsResult = Schema.Array(
  Schema.Struct({
    _id: Id.Id("claudeSessions"),
    sessionId: Schema.String,
    projectPath: Schema.String,
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
    _creationTime: Schema.Number,
  })
);