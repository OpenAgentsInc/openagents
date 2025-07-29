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

// GetSessions schemas
export const GetSessionsArgs = Schema.Struct({
  limit: Schema.optional(Schema.Number),
  status: Schema.optional(Schema.Literal("active", "inactive", "error", "processed")),
});

export const GetSessionsResult = Schema.Array(
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

// GetSession schemas
export const GetSessionArgs = Schema.Struct({
  sessionId: Schema.String,
});

export const GetSessionResult = Schema.Union(
  Schema.Null,
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

// GetSessionMessages schemas
export const GetSessionMessagesArgs = Schema.Struct({
  sessionId: Schema.String,
  limit: Schema.optional(Schema.Number),
});

export const GetSessionMessagesResult = Schema.Array(
  Schema.Struct({
    _id: Id.Id("claudeMessages"),
    sessionId: Schema.String,
    messageId: Schema.String,
    messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
    content: Schema.String,
    timestamp: Schema.String,
    toolInfo: Schema.optional(
      Schema.Struct({
        toolName: Schema.String,
        toolUseId: Schema.String,
        input: Schema.Any,
        output: Schema.optional(Schema.String),
      })
    ),
    metadata: Schema.optional(Schema.Any),
    _creationTime: Schema.Number,
  })
);

// AddClaudeMessage schemas
export const AddClaudeMessageArgs = Schema.Struct({
  sessionId: Schema.String,
  messageId: Schema.String,
  messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
  content: Schema.String,
  timestamp: Schema.String,
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

// BatchAddMessages schemas
export const BatchAddMessagesArgs = Schema.Struct({
  sessionId: Schema.String,
  messages: Schema.Array(
    Schema.Struct({
      messageId: Schema.String,
      messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
      content: Schema.String,
      timestamp: Schema.String,
      toolInfo: Schema.optional(
        Schema.Struct({
          toolName: Schema.String,
          toolUseId: Schema.String,
          input: Schema.Any,
          output: Schema.optional(Schema.String),
        })
      ),
      metadata: Schema.optional(Schema.Any),
    })
  ),
});

export const BatchAddMessagesResult = Schema.Array(Id.Id("claudeMessages"));

// RequestDesktopSession schemas
export const RequestDesktopSessionArgs = Schema.Struct({
  projectPath: Schema.String,
  initialMessage: Schema.optional(Schema.String),
  title: Schema.optional(Schema.String),
});

export const RequestDesktopSessionResult = Schema.String; // sessionId

// UpdateSyncStatus schemas
export const UpdateSyncStatusArgs = Schema.Struct({
  sessionId: Schema.String,
  desktopLastSeen: Schema.optional(Schema.Number),
  mobileLastSeen: Schema.optional(Schema.Number),
  syncErrors: Schema.optional(Schema.Array(Schema.String)),
});

export const UpdateSyncStatusResult = Schema.Null;

// GetSyncStatus schemas
export const GetSyncStatusArgs = Schema.Struct({
  sessionId: Schema.String,
});

export const GetSyncStatusResult = Schema.Union(
  Schema.Null,
  Schema.Struct({
    _id: Id.Id("syncStatus"),
    sessionId: Schema.String,
    desktopLastSeen: Schema.optional(Schema.Number),
    mobileLastSeen: Schema.optional(Schema.Number),
    syncErrors: Schema.optional(Schema.Array(Schema.String)),
    _creationTime: Schema.Number,
  })
);

// SyncSessionFromHook schemas
export const SyncSessionFromHookArgs = Schema.Struct({
  hookData: Schema.Struct({
    sessionId: Schema.String,
    projectPath: Schema.String,
    messages: Schema.optional(Schema.Array(Schema.Any)),
    event: Schema.String,
    timestamp: Schema.String,
  }),
});

export const SyncSessionFromHookResult = Schema.Struct({
  success: Schema.Boolean,
});

// MarkMobileSessionProcessed schemas
export const MarkMobileSessionProcessedArgs = Schema.Struct({
  mobileSessionId: Schema.String,
});

export const MarkMobileSessionProcessedResult = Schema.Null;