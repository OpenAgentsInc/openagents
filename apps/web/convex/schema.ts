import { defineSchema, defineTable } from 'convex/server';
import { v } from 'convex/values';

export default defineSchema({
  numbers: defineTable({
    value: v.number(),
  }),

  /**
   * Canonical identity / ownership plane (Convex).
   *
   * Durable Objects are the canonical execution + workspace plane; Convex stores
   * product identity and index metadata (see MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md).
   */
  users: defineTable({
    userId: v.string(),
    email: v.optional(v.string()),
    createdAtMs: v.number(),
  }).index('by_userId', ['userId']),

  agents: defineTable({
    userSpaceId: v.string(),
    agentId: v.string(),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index('by_userSpaceId', ['userSpaceId'])
    .index('by_agentId', ['agentId']),

  threads: defineTable({
    userSpaceId: v.string(),
    threadId: v.string(),
    agentId: v.optional(v.string()),
    createdAtMs: v.number(),
  })
    .index('by_userSpaceId', ['userSpaceId'])
    .index('by_threadId', ['threadId']),

  /**
   * Append-only event log from the user-space plane (DO SQLite), projected into Convex.
   * Idempotent by `eventId`.
   */
  userSpaceEvents: defineTable({
    userSpaceId: v.string(),
    seq: v.number(),
    eventId: v.string(),
    kind: v.string(),
    json: v.string(),
    createdAtMs: v.number(),
  })
    .index('by_eventId', ['eventId'])
    .index('by_userSpace_seq', ['userSpaceId', 'seq']),
});
