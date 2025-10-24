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
    role: v.string(), // 'user' | 'assistant' | 'system'
    text: v.string(),
    ts: v.number(),
    createdAt: v.number(),
  }),
});
