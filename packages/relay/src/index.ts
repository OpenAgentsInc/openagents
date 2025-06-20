/**
 * Database service combining Effect SQL with Drizzle ORM
 * Provides type-safe database operations for Nostr relay
 */
export * as database from "./database.js"

/**
 * Psionic framework integration for Nostr relay
 * Mounts relay as WebSocket endpoint at /relay
 */
export * as psionic-plugin from "./psionic-plugin.js"

/**
 * Core Nostr relay implementation
 * Handles WebSocket connections, subscriptions, and NIP-01 protocol
 */
export * as relay from "./relay.js"

/**
 * Database schema for Nostr relay event storage
 * Optimized for NIP-01 filter queries and agent coordination
 */
export * as schema from "./schema.js"
