/**
 * Database schema for Nostr relay event storage
 * Optimized for NIP-01 filter queries and agent coordination
 */
import { relations } from "drizzle-orm"
import { bigint, index, int, json, mysqlTable, text, timestamp, varchar } from "drizzle-orm/mysql-core"

/**
 * Core Nostr events table
 * Stores all events according to NIP-01 specification
 */
export const events = mysqlTable("events", {
  // Core NIP-01 fields
  id: varchar("id", { length: 64 }).primaryKey(),
  pubkey: varchar("pubkey", { length: 64 }).notNull(),
  created_at: bigint("created_at", { mode: "number" }).notNull(),
  kind: int("kind").notNull(),
  tags: json("tags").$type<Array<Array<string>>>().notNull(),
  content: text("content").notNull(),
  sig: varchar("sig", { length: 128 }).notNull(),

  // Relay metadata
  received_at: timestamp("received_at").defaultNow().notNull(),
  relay_url: varchar("relay_url", { length: 255 }).default("ws://localhost:3000/relay")
}, (table) => ({
  // Optimized indexes for common Nostr filter patterns
  pubkeyCreatedIdx: index("idx_pubkey_created").on(table.pubkey, table.created_at),
  kindCreatedIdx: index("idx_kind_created").on(table.kind, table.created_at),
  createdAtIdx: index("idx_created_at").on(table.created_at),
  kindPubkeyIdx: index("idx_kind_pubkey").on(table.kind, table.pubkey),
  receivedAtIdx: index("idx_received_at").on(table.received_at)
}))

/**
 * Event tags table for efficient tag-based filtering
 * Denormalized for fast #e, #p, #t queries
 */
export const event_tags = mysqlTable("event_tags", {
  id: bigint("id", { mode: "number" }).primaryKey().autoincrement(),
  event_id: varchar("event_id", { length: 64 }).notNull().references(() => events.id, { onDelete: "cascade" }),
  tag_name: varchar("tag_name", { length: 64 }).notNull(), // e, p, t, etc.
  tag_value: varchar("tag_value", { length: 255 }).notNull(),
  tag_index: bigint("tag_index", { mode: "number" }).notNull() // Position in tag array
}, (table) => ({
  // Tag filtering indexes
  tagNameValueIdx: index("idx_tag_name_value").on(table.tag_name, table.tag_value),
  eventIdIdx: index("idx_event_id").on(table.event_id),
  tagNameIdx: index("idx_tag_name").on(table.tag_name),
  tagValueIdx: index("idx_tag_value").on(table.tag_value)
}))

/**
 * Agent profiles cache for NIP-OA optimization
 * Denormalized agent data for fast lookup
 */
export const agent_profiles = mysqlTable("agent_profiles", {
  pubkey: varchar("pubkey", { length: 64 }).primaryKey(),
  agent_id: varchar("agent_id", { length: 255 }).notNull(),
  name: varchar("name", { length: 255 }),
  status: varchar("status", { length: 32 }).notNull().default("active"), // active, hibernating, etc.
  balance: bigint("balance", { mode: "number" }).default(0),
  metabolic_rate: bigint("metabolic_rate", { mode: "number" }).default(100),
  capabilities: json("capabilities").$type<Array<string>>().default([]),
  last_activity: timestamp("last_activity").defaultNow(),
  profile_event_id: varchar("profile_event_id", { length: 64 }).references(() => events.id),
  created_at: timestamp("created_at").defaultNow().notNull(),
  updated_at: timestamp("updated_at").defaultNow().onUpdateNow().notNull()
}, (table) => ({
  agentIdIdx: index("idx_agent_id").on(table.agent_id),
  statusIdx: index("idx_status").on(table.status),
  lastActivityIdx: index("idx_last_activity").on(table.last_activity),
  balanceIdx: index("idx_balance").on(table.balance)
}))

/**
 * Service offerings cache for NIP-90 marketplace
 * Optimized for service discovery queries
 */
export const service_offerings = mysqlTable("service_offerings", {
  id: varchar("id", { length: 255 }).primaryKey(), // agent_id:service_id
  agent_pubkey: varchar("agent_pubkey", { length: 64 }).notNull().references(() => agent_profiles.pubkey),
  service_name: varchar("service_name", { length: 255 }).notNull(),
  nip90_kinds: json("nip90_kinds").$type<Array<number>>().notNull(),
  pricing: json("pricing").$type<{
    base: number
    per_unit?: string
    currency?: string
  }>().notNull(),
  capabilities: json("capabilities").$type<Array<string>>().default([]),
  availability: varchar("availability", { length: 32 }).default("available"),
  offering_event_id: varchar("offering_event_id", { length: 64 }).references(() => events.id),
  created_at: timestamp("created_at").defaultNow().notNull(),
  updated_at: timestamp("updated_at").defaultNow().onUpdateNow().notNull()
}, (table) => ({
  agentPubkeyIdx: index("idx_agent_pubkey").on(table.agent_pubkey),
  serviceNameIdx: index("idx_service_name").on(table.service_name),
  availabilityIdx: index("idx_availability").on(table.availability)
}))

/**
 * Channel state for NIP-28 public channels
 * Tracks channel metadata and recent activity
 */
export const channels = mysqlTable("channels", {
  id: varchar("id", { length: 64 }).primaryKey(), // Channel event ID
  name: varchar("name", { length: 255 }),
  about: text("about"),
  picture: varchar("picture", { length: 500 }),
  creator_pubkey: varchar("creator_pubkey", { length: 64 }).notNull(),
  created_by: varchar("created_by", { length: 64 }).notNull(), // Alternative name for creator_pubkey
  message_count: bigint("message_count", { mode: "number" }).default(0),
  last_message_at: timestamp("last_message_at"),
  created_at: timestamp("created_at").defaultNow().notNull(),
  updated_at: timestamp("updated_at").defaultNow().onUpdateNow().notNull()
}, (table) => ({
  nameIdx: index("idx_name").on(table.name),
  creatorIdx: index("idx_creator").on(table.creator_pubkey),
  lastMessageIdx: index("idx_last_message").on(table.last_message_at),
  messageCountIdx: index("idx_message_count").on(table.message_count)
}))

/**
 * Job requests for NIP-90 marketplace
 * Tracks service requests and their status
 */
export const job_requests = mysqlTable("job_requests", {
  id: varchar("id", { length: 255 }).primaryKey(),
  request_event_id: varchar("request_event_id", { length: 64 }).references(() => events.id),
  requester_pubkey: varchar("requester_pubkey", { length: 64 }).notNull(),
  provider_pubkey: varchar("provider_pubkey", { length: 64 }),
  service_type: varchar("service_type", { length: 255 }).notNull(),
  status: varchar("status", { length: 32 }).notNull().default("pending"), // pending, processing, completed, failed, cancelled
  description: text("description").notNull(),
  payment_amount: bigint("payment_amount", { mode: "number" }).notNull(),
  result_data: json("result_data").$type<Record<string, unknown>>(),
  created_at: timestamp("created_at").defaultNow().notNull(),
  updated_at: timestamp("updated_at").defaultNow().onUpdateNow().notNull()
}, (table) => ({
  requesterIdx: index("idx_requester").on(table.requester_pubkey),
  providerIdx: index("idx_provider").on(table.provider_pubkey),
  statusIdx: index("idx_status").on(table.status),
  serviceTypeIdx: index("idx_service_type").on(table.service_type),
  createdAtIdx: index("idx_created_at").on(table.created_at)
}))

/**
 * Relay statistics for monitoring and optimization
 */
export const relay_stats = mysqlTable("relay_stats", {
  id: bigint("id", { mode: "number" }).primaryKey().autoincrement(),
  metric_name: varchar("metric_name", { length: 64 }).notNull(),
  metric_value: bigint("metric_value", { mode: "number" }).notNull(),
  timestamp: timestamp("timestamp").defaultNow().notNull(),
  metadata: json("metadata").$type<Record<string, unknown>>().default({})
}, (table) => ({
  metricNameTimestampIdx: index("idx_metric_timestamp").on(table.metric_name, table.timestamp),
  timestampIdx: index("idx_timestamp").on(table.timestamp)
}))

// Relations
export const eventsRelations = relations(events, ({ many }) => ({
  tags: many(event_tags)
}))

export const eventTagsRelations = relations(event_tags, ({ one }) => ({
  event: one(events, {
    fields: [event_tags.event_id],
    references: [events.id]
  })
}))

export const agentProfilesRelations = relations(agent_profiles, ({ many, one }) => ({
  services: many(service_offerings),
  profileEvent: one(events, {
    fields: [agent_profiles.profile_event_id],
    references: [events.id]
  })
}))

export const serviceOfferingsRelations = relations(service_offerings, ({ one }) => ({
  agent: one(agent_profiles, {
    fields: [service_offerings.agent_pubkey],
    references: [agent_profiles.pubkey]
  }),
  offeringEvent: one(events, {
    fields: [service_offerings.offering_event_id],
    references: [events.id]
  })
}))

// Type exports
export type Event = typeof events.$inferSelect
export type NewEvent = typeof events.$inferInsert
export type EventTag = typeof event_tags.$inferSelect
export type NewEventTag = typeof event_tags.$inferInsert
export type AgentProfile = typeof agent_profiles.$inferSelect
export type NewAgentProfile = typeof agent_profiles.$inferInsert
export type ServiceOffering = typeof service_offerings.$inferSelect
export type NewServiceOffering = typeof service_offerings.$inferInsert
export type Channel = typeof channels.$inferSelect
export type NewChannel = typeof channels.$inferInsert
export type JobRequest = typeof job_requests.$inferSelect
export type NewJobRequest = typeof job_requests.$inferInsert
export type RelayStat = typeof relay_stats.$inferSelect
export type NewRelayStat = typeof relay_stats.$inferInsert
