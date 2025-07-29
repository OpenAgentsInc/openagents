import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";
import { confectSchema } from "./schema";

// GetMessages schemas
export const GetMessagesArgs = Schema.Struct({});

export const GetMessagesResult = Schema.Array(
  confectSchema.tableSchemas.messages.withSystemFields
);

// AddMessage schemas
export const AddMessageArgs = Schema.Struct({
  body: Schema.String,
  user: Schema.String,
});

export const AddMessageResult = Id.Id("messages");

// GetMessageCount schemas
export const GetMessageCountArgs = Schema.Struct({});

export const GetMessageCountResult = Schema.Number;

// AddClaudeMessage schemas
export const AddClaudeMessageArgs = Schema.Struct({
  sessionId: Schema.String,
  messageId: Schema.String,
  messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
  content: Schema.String,
  timestamp: Schema.String, // ISO timestamp
  toolInfo: Schema.optional(
    Schema.Struct({
      toolName: Schema.String,
      toolUseId: Schema.String,
      input: Schema.Any,
      output: Schema.optional(Schema.String),
    })
  ),
  metadata: Schema.optional(Schema.Any),
});

export const AddClaudeMessageResult = Id.Id("claudeMessages");

// GetSessionMessages schemas
export const GetSessionMessagesArgs = Schema.Struct({
  sessionId: Schema.String,
  limit: Schema.optional(Schema.Number),
});

export const GetSessionMessagesResult = Schema.Array(
  confectSchema.tableSchemas.claudeMessages.withSystemFields
);

// Paginated GetSessionMessages schemas  
export const GetSessionMessagesPaginatedArgs = Schema.Struct({
  sessionId: Schema.String,
  paginationOpts: Schema.Struct({
    numItems: Schema.Number, // Max 100 items per page
    cursor: Schema.optional(Schema.Union(Schema.String, Schema.Null)),
  }),
});

export const GetSessionMessagesPaginatedResult = Schema.Struct({
  page: Schema.Array(
    confectSchema.tableSchemas.claudeMessages.withSystemFields
  ),
  isDone: Schema.Boolean,
  continueCursor: Schema.optional(Schema.String),
});