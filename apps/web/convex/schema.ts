import { defineSchema, defineTable } from 'convex/server';
import { v } from 'convex/values';

export default defineSchema({
  numbers: defineTable({
    value: v.number(),
  }),

  // Nostr cache (read-optimized); community = slug from identifier (e.g. c/community)
  nostr_events: defineTable({
    event_id: v.string(),
    kind: v.number(),
    pubkey: v.string(),
    created_at: v.number(),
    content: v.string(),
    tags_json: v.string(),
    identifier: v.optional(v.string()),
    community: v.optional(v.string()),
    parent_id: v.optional(v.string()),
    is_top_level: v.optional(v.boolean()),
    is_ai: v.optional(v.boolean()),
    seen_at: v.number(),
    relay: v.optional(v.string()),
  })
    .index('by_event_id', ['event_id'])
    .index('by_created_at', ['created_at'])
    .index('by_kind_created_at', ['kind', 'created_at'])
    .index('by_kind_parent_id', ['kind', 'parent_id'])
    .index('by_community_created_at', ['community', 'created_at'])
    .index('by_pubkey_created_at', ['pubkey', 'created_at'])
    .index('by_parent_id', ['parent_id']),

  nostr_profiles: defineTable({
    pubkey: v.string(),
    name: v.optional(v.string()),
    picture: v.optional(v.string()),
    about: v.optional(v.string()),
    updated_at: v.number(),
  }).index('by_pubkey', ['pubkey']),
});
