/**
 * OpenAgents Nostr Relay
 * Full-featured Nostr relay with NIP-01 support, Effect.js architecture,
 * and PlanetScale database backend
 */

// Core relay exports
export {
  NostrRelay,
  NostrRelayLive,
  type ConnectionHandler,
  type RelayStats,
  RelayError,
  MessageError
} from "./relay.js"

// Database layer exports
export {
  RelayDatabase,
  RelayDatabaseLive,
  DatabaseError,
  ValidationError
} from "./database.js"

// Schema exports
export * from "./schema.js"

// Psionic integration exports
export {
  createRelayPlugin,
  // mountRelay, // TODO: Update to work with Effect-based Psionic
  type RelayPluginConfig
} from "./psionic-plugin.js"

// Re-export some common types from nostr package
import type { Schema } from "@openagentsinc/nostr"
export type NostrEvent = Schema.NostrEvent
export type Filter = Schema.Filter  
export type ClientMessage = Schema.ClientMessage
