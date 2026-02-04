import { defineSchema, defineTable } from 'convex/server';
import { v } from 'convex/values';

export default defineSchema({
  numbers: defineTable({
    value: v.number(),
  }),

  users: defineTable({
    user_id: v.string(),
    kind: v.string(),
    name: v.optional(v.string()),
    email: v.optional(v.string()),
    image: v.optional(v.string()),
    metadata: v.optional(v.any()),
    nostr_pubkey: v.optional(v.string()),
    access_enabled: v.optional(v.boolean()),
    access_updated_at: v.optional(v.number()),
    access_updated_by: v.optional(v.string()),
    created_at: v.number(),
    updated_at: v.number(),
  }).index('by_user_id', ['user_id']),

  api_tokens: defineTable({
    user_id: v.string(),
    token_hash: v.string(),
    name: v.string(),
    created_at: v.number(),
    last_used_at: v.optional(v.number()),
    expires_at: v.optional(v.number()),
  })
    .index('by_user_id', ['user_id'])
    .index('by_token_hash', ['token_hash']),

  openclaw_instances: defineTable({
    user_id: v.string(),
    status: v.string(),
    runtime_url: v.optional(v.string()),
    runtime_name: v.optional(v.string()),
    cf_account_id: v.optional(v.string()),
    cf_worker_name: v.optional(v.string()),
    cf_worker_id: v.optional(v.string()),
    cf_container_app_id: v.optional(v.string()),
    cf_container_app_name: v.optional(v.string()),
    r2_bucket_name: v.optional(v.string()),
    service_token_encrypted: v.optional(v.string()),
    service_token_iv: v.optional(v.string()),
    service_token_alg: v.optional(v.string()),
    provider_keys_encrypted: v.optional(v.string()),
    provider_keys_iv: v.optional(v.string()),
    provider_keys_alg: v.optional(v.string()),
    created_at: v.number(),
    updated_at: v.number(),
    last_ready_at: v.optional(v.number()),
  })
    .index('by_user_id', ['user_id'])
    .index('by_status', ['status']),

  credit_ledger: defineTable({
    user_id: v.string(),
    kind: v.string(),
    amount_usd: v.number(),
    meta: v.optional(v.any()),
    created_at: v.number(),
  })
    .index('by_user_id', ['user_id'])
    .index('by_user_id_created_at', ['user_id', 'created_at'])
    .index('by_user_id_kind', ['user_id', 'kind']),

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

  waitlist: defineTable({
    email: v.string(),
    source: v.optional(v.string()),
    created_at: v.number(),
    approved: v.optional(v.boolean()),
    approved_at: v.optional(v.number()),
    approved_by: v.optional(v.string()),
  })
    .index('by_email', ['email'])
    .index('by_created_at', ['created_at']),

  // Thread index (chats/projects); optional kind for OpenClaw later
  threads: defineTable({
    user_id: v.string(),
    title: v.string(),
    kind: v.optional(v.union(v.literal('chat'), v.literal('project'), v.literal('openclaw'))),
    archived: v.optional(v.boolean()),
    created_at: v.number(),
    updated_at: v.number(),
  })
    .index('by_user_id', ['user_id'])
    .index('by_user_id_updated_at', ['user_id', 'updated_at'])
    .index('by_user_id_archived', ['user_id', 'archived']),

  // Chat messages per thread (for rehydration)
  thread_messages: defineTable({
    thread_id: v.id('threads'),
    message_id: v.string(),
    role: v.string(),
    parts_json: v.string(),
    order: v.number(),
    created_at: v.number(),
  })
    .index('by_thread_id_order', ['thread_id', 'order']),
});
