import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";
import { authTables } from "@convex-dev/auth/server";

// The schema is normally optional, but Convex Auth
// requires indexes defined on `authTables`.
// The schema provides more precise TypeScript types.
export default defineSchema({
  ...authTables,
  numbers: defineTable({
    value: v.number(),
  }),
  threads: defineTable({
    userId: v.id("users"),
    title: v.optional(v.string()),
    // Extended fields from Tinyvex
    projectId: v.optional(v.id("projects")),
    resumeId: v.optional(v.string()),
    rolloutPath: v.optional(v.string()),
    source: v.optional(v.string()), // "codex", "claude-code", etc.
    archived: v.optional(v.boolean()),
    workingDirectory: v.optional(v.string()),
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    .index("by_user", ["userId", "updatedAt"])
    .index("by_updated", ["updatedAt"])
    .index("by_project", ["projectId", "updatedAt"])
    .index("by_archived", ["archived", "updatedAt"]),
  messages: defineTable({
    threadId: v.id("threads"),
    userId: v.id("users"),
    role: v.union(v.literal("user"), v.literal("assistant"), v.literal("system")),
    content: v.string(),
    // Extended fields from Tinyvex
    kind: v.optional(v.union(v.literal("message"), v.literal("reason"))), // "message" or "reason" (for thinking)
    itemId: v.optional(v.string()), // For streaming updates
    partial: v.optional(v.boolean()), // true if still streaming, false when finalized
    seq: v.optional(v.number()), // Sequence number for ordering
    createdAt: v.number(),
  })
    .index("by_thread", ["threadId", "createdAt"])
    .index("by_user", ["userId", "createdAt"])
    .index("by_item", ["itemId"])
    .index("by_partial", ["threadId", "partial"]),
  projects: defineTable({
    userId: v.id("users"),
    name: v.string(),
    path: v.string(), // File system path
    description: v.optional(v.string()),
    icon: v.optional(v.string()),
    color: v.optional(v.string()),
    starred: v.optional(v.boolean()),
    archived: v.optional(v.boolean()),
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    .index("by_user", ["userId", "updatedAt"])
    .index("by_starred", ["userId", "starred", "updatedAt"])
    .index("by_archived", ["userId", "archived", "updatedAt"]),
  toolCalls: defineTable({
    threadId: v.id("threads"),
    userId: v.id("users"),
    toolCallId: v.string(), // ACP tool call ID
    title: v.optional(v.string()),
    kind: v.optional(v.string()), // Tool type (e.g., "bash", "read", "write")
    status: v.optional(v.string()), // "pending", "running", "completed", "failed"
    contentJson: v.optional(v.string()), // JSON blob for tool content
    locationsJson: v.optional(v.string()), // JSON blob for file locations
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    .index("by_thread", ["threadId", "createdAt"])
    .index("by_user", ["userId", "createdAt"])
    .index("by_tool_call_id", ["toolCallId"]),
  planEntries: defineTable({
    threadId: v.id("threads"),
    userId: v.id("users"),
    entriesJson: v.string(), // JSON array of plan entries
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    .index("by_thread", ["threadId"])
    .index("by_user", ["userId"]),
  threadState: defineTable({
    threadId: v.id("threads"),
    userId: v.id("users"),
    currentModeId: v.optional(v.string()), // Current ACP mode (e.g., "plan", "code")
    availableCommandsJson: v.optional(v.string()), // JSON array of available slash commands
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    .index("by_thread", ["threadId"])
    .index("by_user", ["userId"]),
  acpEvents: defineTable({
    userId: v.id("users"),
    sessionId: v.optional(v.string()),
    clientThreadDocId: v.optional(v.string()),
    threadId: v.optional(v.id("threads")),
    updateKind: v.optional(v.string()), // Type of ACP update
    payload: v.string(), // JSON blob of the full event
    timestamp: v.number(),
  })
    .index("by_session", ["sessionId", "timestamp"])
    .index("by_thread", ["threadId", "timestamp"])
    .index("by_user", ["userId", "timestamp"])
    .index("by_timestamp", ["timestamp"]),
});
