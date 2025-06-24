/**
 * Convex database schema definitions for OpenAgents
 * Maps from existing PlanetScale MySQL schemas to Convex validators
 * @since 1.0.0
 */

import { defineSchema, defineTable } from "convex/server"
import { v } from "convex/values"

/**
 * Core Nostr events table - maps from relay/src/schema.ts
 */
const events = defineTable({
  // Core NIP-01 fields
  id: v.string(), // Event ID (64 chars)
  pubkey: v.string(), // Public key (64 chars)
  created_at: v.number(), // Unix timestamp
  kind: v.number(), // Event kind
  tags: v.array(v.array(v.string())), // Event tags
  content: v.string(), // Event content
  sig: v.string(), // Signature (128 chars)
  
  // Relay metadata
  received_at: v.number(), // When relay received the event
  relay_url: v.optional(v.string()) // Relay URL
})
  .index("by_pubkey_created", ["pubkey", "created_at"])
  .index("by_kind_created", ["kind", "created_at"])
  .index("by_created_at", ["created_at"])
  .index("by_kind_pubkey", ["kind", "pubkey"])
  .index("by_received_at", ["received_at"])

/**
 * Event tags table for efficient tag-based filtering
 */
const event_tags = defineTable({
  event_id: v.string(), // Reference to events.id
  tag_name: v.string(), // Tag name (e, p, t, etc.)
  tag_value: v.string(), // Tag value
  tag_index: v.number() // Position in tag array
})
  .index("by_tag_name_value", ["tag_name", "tag_value"])
  .index("by_event_id", ["event_id"])
  .index("by_tag_name", ["tag_name"])
  .index("by_tag_value", ["tag_value"])

/**
 * Agent profiles cache for NIP-OA optimization
 */
const agent_profiles = defineTable({
  pubkey: v.string(), // Primary key - agent pubkey
  agent_id: v.string(), // Agent identifier
  name: v.optional(v.string()), // Agent display name
  status: v.string(), // active, hibernating, etc.
  balance: v.optional(v.number()), // Balance in satoshis
  metabolic_rate: v.optional(v.number()), // Metabolic rate
  capabilities: v.array(v.string()), // Agent capabilities
  last_activity: v.number(), // Last activity timestamp
  profile_event_id: v.optional(v.string()), // Reference to profile event
  created_at: v.number(),
  updated_at: v.number()
})
  .index("by_agent_id", ["agent_id"])
  .index("by_status", ["status"])
  .index("by_last_activity", ["last_activity"])
  .index("by_balance", ["balance"])

/**
 * Service offerings cache for NIP-90 marketplace
 */
const service_offerings = defineTable({
  id: v.string(), // Composite key: agent_id:service_id
  agent_pubkey: v.string(), // Reference to agent_profiles.pubkey
  service_name: v.string(), // Service name
  nip90_kinds: v.array(v.number()), // Supported NIP-90 kinds
  pricing: v.object({
    base: v.number(),
    per_unit: v.optional(v.string()),
    currency: v.optional(v.string())
  }),
  capabilities: v.array(v.string()),
  availability: v.string(), // available, busy, offline
  offering_event_id: v.optional(v.string()),
  created_at: v.number(),
  updated_at: v.number()
})
  .index("by_agent_pubkey", ["agent_pubkey"])
  .index("by_service_name", ["service_name"])
  .index("by_availability", ["availability"])

/**
 * Channel state for NIP-28 public channels
 */
const channels = defineTable({
  id: v.string(), // Channel event ID
  name: v.optional(v.string()),
  about: v.optional(v.string()),
  picture: v.optional(v.string()),
  creator_pubkey: v.string(),
  created_by: v.string(), // Alternative name for creator_pubkey
  message_count: v.number(),
  last_message_at: v.optional(v.number()),
  created_at: v.number(),
  updated_at: v.number()
})
  .index("by_name", ["name"])
  .index("by_creator", ["creator_pubkey"])
  .index("by_last_message", ["last_message_at"])
  .index("by_message_count", ["message_count"])

/**
 * Job requests for NIP-90 marketplace
 */
const job_requests = defineTable({
  id: v.string(),
  request_event_id: v.optional(v.string()),
  requester_pubkey: v.string(),
  provider_pubkey: v.optional(v.string()),
  service_type: v.string(),
  status: v.string(), // pending, processing, completed, failed, cancelled
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
  .index("by_created_at", ["created_at"])

/**
 * Relay statistics for monitoring
 */
const relay_stats = defineTable({
  metric_name: v.string(),
  metric_value: v.number(),
  timestamp: v.number(),
  metadata: v.any()
})
  .index("by_metric_timestamp", ["metric_name", "timestamp"])
  .index("by_timestamp", ["timestamp"])

/**
 * Chat sessions for Overlord integration
 * Maps from overlord/src/services/DatabaseMapper.ts
 */
const sessions = defineTable({
  id: v.string(), // Session ID
  user_id: v.string(), // User identifier
  project_path: v.string(), // Hashed project path
  project_name: v.optional(v.string()), // Display name
  status: v.string(), // active, inactive, archived
  started_at: v.number(), // Start timestamp
  last_activity: v.number(), // Last activity timestamp
  message_count: v.number(), // Number of messages
  total_cost: v.number() // Total cost in USD
})
  .index("by_user_id", ["user_id"])
  .index("by_status", ["status"])
  .index("by_last_activity", ["last_activity"])
  .index("by_project_path", ["project_path"])

/**
 * Chat messages for conversation history
 */
const messages = defineTable({
  session_id: v.string(), // Reference to sessions.id
  entry_uuid: v.string(), // Unique entry identifier
  entry_type: v.string(), // user, assistant, summary, tool_use, tool_result
  role: v.optional(v.string()), // user, assistant, system
  content: v.optional(v.string()), // Message content
  thinking: v.optional(v.string()), // Assistant thinking
  summary: v.optional(v.string()), // Summary content
  model: v.optional(v.string()), // Model used
  token_usage: v.optional(v.object({
    input_tokens: v.number(),
    output_tokens: v.number(),
    total_tokens: v.number()
  })),
  cost: v.optional(v.number()), // Cost in USD
  timestamp: v.number(), // Message timestamp
  turn_count: v.optional(v.number()), // Turn count for summaries
  tool_name: v.optional(v.string()), // Tool name for tool_use
  tool_input: v.optional(v.any()), // Tool input
  tool_use_id: v.optional(v.string()), // Tool use ID
  tool_output: v.optional(v.string()), // Tool output
  tool_is_error: v.optional(v.boolean()) // Tool error flag
})
  .index("by_session_id", ["session_id"])
  .index("by_entry_type", ["entry_type"])
  .index("by_timestamp", ["timestamp"])
  .index("by_tool_use_id", ["tool_use_id"])

/**
 * Images associated with messages
 */
const images = defineTable({
  message_id: v.id("messages"), // Reference to messages table
  image_data: v.string(), // Base64 image data
  mime_type: v.string(), // Image MIME type
  position: v.number() // Position in message
})
  .index("by_message_id", ["message_id"])
  .index("by_position", ["position"])

export default defineSchema({
  // Nostr relay tables
  events,
  event_tags,
  agent_profiles,
  service_offerings,
  channels,
  job_requests,
  relay_stats,
  
  // Chat/Overlord tables
  sessions,
  messages,
  images
})