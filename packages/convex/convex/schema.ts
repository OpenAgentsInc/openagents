/**
 * Convex database schema for OpenAgents
 * @since 1.0.0
 */

import { defineSchema, defineTable } from "convex/server"
import { v } from "convex/values"

export default defineSchema({
  // Nostr relay tables
  events: defineTable({
    // Core NIP-01 fields
    id: v.string(),
    pubkey: v.string(),
    created_at: v.number(),
    kind: v.number(),
    tags: v.array(v.array(v.string())),
    content: v.string(),
    sig: v.string(),
    
    // Relay metadata
    received_at: v.number(),
    relay_url: v.optional(v.string())
  })
    .index("by_pubkey_created", ["pubkey", "created_at"])
    .index("by_kind_created", ["kind", "created_at"])
    .index("by_created_at", ["created_at"])
    .index("by_kind_pubkey", ["kind", "pubkey"])
    .index("by_received_at", ["received_at"]),

  event_tags: defineTable({
    event_id: v.string(),
    tag_name: v.string(),
    tag_value: v.string(),
    tag_index: v.number()
  })
    .index("by_tag_name_value", ["tag_name", "tag_value"])
    .index("by_event_id", ["event_id"])
    .index("by_tag_name", ["tag_name"])
    .index("by_tag_value", ["tag_value"]),

  agent_profiles: defineTable({
    pubkey: v.string(),
    agent_id: v.string(),
    name: v.optional(v.string()),
    status: v.string(),
    balance: v.optional(v.number()),
    metabolic_rate: v.optional(v.number()),
    capabilities: v.array(v.string()),
    last_activity: v.number(),
    profile_event_id: v.optional(v.string()),
    created_at: v.number(),
    updated_at: v.number()
  })
    .index("by_agent_id", ["agent_id"])
    .index("by_status", ["status"])
    .index("by_last_activity", ["last_activity"])
    .index("by_balance", ["balance"]),

  service_offerings: defineTable({
    id: v.string(),
    agent_pubkey: v.string(),
    service_name: v.string(),
    nip90_kinds: v.array(v.number()),
    pricing: v.object({
      base: v.number(),
      per_unit: v.optional(v.string()),
      currency: v.optional(v.string())
    }),
    capabilities: v.array(v.string()),
    availability: v.string(),
    offering_event_id: v.optional(v.string()),
    created_at: v.number(),
    updated_at: v.number()
  })
    .index("by_agent_pubkey", ["agent_pubkey"])
    .index("by_service_name", ["service_name"])
    .index("by_availability", ["availability"]),

  channels: defineTable({
    id: v.string(),
    name: v.optional(v.string()),
    about: v.optional(v.string()),
    picture: v.optional(v.string()),
    creator_pubkey: v.string(),
    created_by: v.string(),
    message_count: v.number(),
    last_message_at: v.optional(v.number()),
    created_at: v.number(),
    updated_at: v.number()
  })
    .index("by_name", ["name"])
    .index("by_creator", ["creator_pubkey"])
    .index("by_last_message", ["last_message_at"])
    .index("by_message_count", ["message_count"]),

  job_requests: defineTable({
    id: v.string(),
    request_event_id: v.optional(v.string()),
    requester_pubkey: v.string(),
    provider_pubkey: v.optional(v.string()),
    service_type: v.string(),
    status: v.string(),
    description: v.string(),
    payment_amount: v.number(),
    result_data: v.optional(v.any()),
    created_at: v.number(),
    updated_at: v.number()
  })
    .index("by_requester", ["requester_pubkey"])
    .index("by_provider", ["provider_pubkey"])
    .index("by_status", ["status"])
    .index("by_service_type", ["service_type"])
    .index("by_created_at", ["created_at"]),

  relay_stats: defineTable({
    metric_name: v.string(),
    metric_value: v.number(),
    timestamp: v.number(),
    metadata: v.any()
  })
    .index("by_metric_timestamp", ["metric_name", "timestamp"])
    .index("by_timestamp", ["timestamp"]),

  // Chat/Overlord tables
  sessions: defineTable({
    id: v.string(),
    user_id: v.string(),
    project_path: v.string(),
    project_name: v.optional(v.string()),
    status: v.string(),
    started_at: v.number(),
    last_activity: v.number(),
    message_count: v.number(),
    total_cost: v.number()
  })
    .index("by_user_id", ["user_id"])
    .index("by_status", ["status"])
    .index("by_last_activity", ["last_activity"])
    .index("by_project_path", ["project_path"]),

  messages: defineTable({
    session_id: v.string(),
    entry_uuid: v.string(),
    entry_type: v.string(),
    role: v.optional(v.string()),
    content: v.optional(v.string()),
    thinking: v.optional(v.string()),
    summary: v.optional(v.string()),
    model: v.optional(v.string()),
    token_usage: v.optional(v.object({
      input_tokens: v.number(),
      output_tokens: v.number(),
      total_tokens: v.number()
    })),
    cost: v.optional(v.number()),
    timestamp: v.number(),
    turn_count: v.optional(v.number()),
    tool_name: v.optional(v.string()),
    tool_input: v.optional(v.any()),
    tool_use_id: v.optional(v.string()),
    tool_output: v.optional(v.string()),
    tool_is_error: v.optional(v.boolean())
  })
    .index("by_session_id", ["session_id"])
    .index("by_entry_type", ["entry_type"])
    .index("by_timestamp", ["timestamp"])
    .index("by_tool_use_id", ["tool_use_id"]),

  images: defineTable({
    message_id: v.id("messages"),
    image_data: v.string(),
    mime_type: v.string(),
    position: v.number()
  })
    .index("by_message_id", ["message_id"])
    .index("by_position", ["position"])
})