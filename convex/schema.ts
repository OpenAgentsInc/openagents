import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  threads: defineTable({
    // threadId is optional to allow migrating existing rows created before this field existed.
    // New inserts should set it; the app upsert does.
    threadId: v.optional(v.string()),
    title: v.string(),
    rolloutPath: v.string(),
    resumeId: v.string(),
    projectId: v.string(),
    source: v.string(),
    createdAt: v.number(),
    updatedAt: v.number(),
  }),
  messages: defineTable({
    threadId: v.string(),
    role: v.optional(v.string()), // 'user' | 'assistant' | 'system' (optional for non-message items)
    kind: v.string(), // 'message' | 'reason' | 'cmd' | 'file' | 'search' | 'mcp' | 'todo' | 'turn' | etc
    text: v.optional(v.string()),
    data: v.optional(v.any()),
    ts: v.number(),
    createdAt: v.number(),
  }),
  runs: defineTable({
    threadDocId: v.string(), // convex threads doc _id (string form)
    projectId: v.optional(v.string()),
    text: v.string(),
    role: v.string(), // usually 'user'
    status: v.string(), // 'pending' | 'processing' | 'done' | 'error'
    error: v.optional(v.string()),
    createdAt: v.number(),
    updatedAt: v.number(),
  }),
});
