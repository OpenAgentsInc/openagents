import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

/**
 * Convex-first MVP schema for Autopilot chat (threads/messages/messageParts).
 *
 * Canonical store:
 * - threads/messages/messageParts/receipts/blueprints live in Convex.
 * - Cloudflare Worker is compute/enforcement and writes chunked deltas.
 */
export default defineSchema({
  users: defineTable({
    userId: v.string(),
    email: v.optional(v.string()),
    createdAtMs: v.number(),
    defaultThreadId: v.optional(v.string()),
  }).index("by_userId", ["userId"]),

  threads: defineTable({
    threadId: v.string(),
    ownerId: v.optional(v.string()),
    anonKey: v.optional(v.string()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_threadId", ["threadId"])
    .index("by_ownerId", ["ownerId"]),

  messages: defineTable({
    threadId: v.string(),
    messageId: v.string(),
    role: v.union(v.literal("user"), v.literal("assistant"), v.literal("system")),
    status: v.union(
      v.literal("draft"),
      v.literal("streaming"),
      v.literal("final"),
      v.literal("error"),
      v.literal("canceled"),
    ),
    text: v.optional(v.string()),
    runId: v.optional(v.string()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_threadId_createdAtMs", ["threadId", "createdAtMs"])
    .index("by_threadId_messageId", ["threadId", "messageId"])
    .index("by_runId", ["runId"]),

  /**
   * Chunked streaming parts (idempotent by (runId, seq)).
   *
   * We store `@effect/ai/Response` StreamPartEncoded objects under `part`.
   */
  messageParts: defineTable({
    threadId: v.string(),
    runId: v.string(),
    messageId: v.string(),
    seq: v.number(),
    part: v.any(),
    createdAtMs: v.number(),
  })
    .index("by_runId_seq", ["runId", "seq"])
    .index("by_messageId_seq", ["messageId", "seq"])
    .index("by_threadId_createdAtMs", ["threadId", "createdAtMs"]),

  runs: defineTable({
    threadId: v.string(),
    runId: v.string(),
    assistantMessageId: v.string(),
    status: v.union(
      v.literal("streaming"),
      v.literal("final"),
      v.literal("error"),
      v.literal("canceled"),
    ),
    cancelRequested: v.boolean(),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_runId", ["runId"])
    .index("by_threadId_updatedAtMs", ["threadId", "updatedAtMs"]),

  blueprints: defineTable({
    threadId: v.string(),
    blueprint: v.any(),
    updatedAtMs: v.number(),
  }).index("by_threadId", ["threadId"]),

  receipts: defineTable({
    threadId: v.string(),
    runId: v.string(),
    kind: v.union(v.literal("model"), v.literal("tool")),
    json: v.any(),
    createdAtMs: v.number(),
  })
    .index("by_runId_createdAtMs", ["runId", "createdAtMs"])
    .index("by_threadId_createdAtMs", ["threadId", "createdAtMs"]),
});
