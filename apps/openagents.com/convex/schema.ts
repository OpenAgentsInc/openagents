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
  
  // Conversations table for chat sessions
  conversations: defineTable({
    userId: v.id("users"),
    title: v.string(),
    lastMessageAt: v.optional(v.number()),
    messageCount: v.number(),
    metadata: v.optional(v.object({
      model: v.optional(v.string()),
      context: v.optional(v.any()),
    })),
  })
    .index("by_user", ["userId"])
    .index("by_user_last_message", ["userId", "lastMessageAt"]),
  
  // Messages table for individual chat messages
  messages: defineTable({
    conversationId: v.id("conversations"),
    role: v.union(v.literal("user"), v.literal("assistant"), v.literal("system")),
    content: v.string(),
    metadata: v.optional(v.object({
      model: v.optional(v.string()),
      usage: v.optional(v.object({
        promptTokens: v.optional(v.number()),
        completionTokens: v.optional(v.number()),
        totalTokens: v.optional(v.number()),
      })),
      toolCalls: v.optional(v.array(v.any())),
    })),
  })
    .index("by_conversation", ["conversationId"]),
});
