import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  // Existing basic messages (keep for demo compatibility)
  messages: defineTable({
    body: v.string(),
    user: v.string(),
    timestamp: v.number(),
  }),
  
  // Claude Code sessions
  claudeSessions: defineTable({
    sessionId: v.string(),          // From desktop Claude Code
    projectPath: v.string(),        // Project being worked on
    title: v.optional(v.string()),  // Session title/description  
    status: v.union(                // Session state
      v.literal("active"),
      v.literal("inactive"), 
      v.literal("error"),
      v.literal("processed")
    ),
    createdBy: v.union(             // Which platform created it
      v.literal("desktop"),
      v.literal("mobile")
    ),
    lastActivity: v.number(),       // Timestamp of last message
    metadata: v.optional(v.object({ // Additional session data
      workingDirectory: v.optional(v.string()),
      model: v.optional(v.string()),
      systemPrompt: v.optional(v.string()),
      originalMobileSessionId: v.optional(v.string()), // Track original mobile session
    })),
  }).index("by_session_id", ["sessionId"])
    .index("by_status", ["status"])
    .index("by_last_activity", ["lastActivity"]),
    
  // Claude Code messages within sessions  
  claudeMessages: defineTable({
    sessionId: v.string(),         // References claudeSessions
    messageId: v.string(),         // Unique message ID
    messageType: v.union(          // Message type from Claude Code
      v.literal("user"),
      v.literal("assistant"), 
      v.literal("tool_use"),
      v.literal("tool_result"),
      v.literal("thinking")
    ),
    content: v.string(),           // Message content
    timestamp: v.string(),         // ISO timestamp
    toolInfo: v.optional(v.object({ // Tool information if applicable
      toolName: v.string(),
      toolUseId: v.string(), 
      input: v.any(),             // Tool input parameters
      output: v.optional(v.string()), // Tool output if available
    })),
    metadata: v.optional(v.any()), // Additional message metadata
  }).index("by_session_id", ["sessionId"])
    .index("by_timestamp", ["timestamp"]),
    
  // Sync status tracking
  syncStatus: defineTable({
    sessionId: v.string(),
    lastSyncedMessageId: v.optional(v.string()),
    desktopLastSeen: v.optional(v.number()),
    mobileLastSeen: v.optional(v.number()),
    syncErrors: v.optional(v.array(v.string())),
  }).index("by_session_id", ["sessionId"]),
});