/**
 * Agent Profile Service - NIP-OA Agent Identity Management (Stub Implementation)
 * Handles creation, updates, and queries for agent profiles
 */
export * as AgentProfileService from "./agent-profile/AgentProfileService.js"

/**
 * Core error types for Nostr operations
 * @module
 */
export * as Errors from "./core/Errors.js"

/**
 * Core Nostr schemas - aggregates all schemas from primitives and NIPs
 * @module
 */
export * as Schema from "./core/Schema.js"

/**
 * NIP-06: Basic key derivation from mnemonic seed phrase
 * @module
 */
export * as Nip06Service from "./nip06/Nip06Service.js"

/**
 * NIP-28: Public Chat Channel Service
 * Implements channel creation, messaging, and subscription functionality
 * @module
 */
export * as Nip28Service from "./nip28/Nip28Service.js"

/**
 * NIP-90: Data Vending Machine Service
 * Implements AI service marketplace with job request/result protocol
 * @module
 */
export * as Nip90Service from "./nip90/Nip90Service.js"

/**
 * Cryptographic operations for Nostr
 * @module
 */
export * as CryptoService from "./services/CryptoService.js"

/**
 * Service for creating and validating Nostr events
 * @module
 */
export * as EventService from "./services/EventService.js"

/**
 * Relay connection pool for managing multiple Nostr relays
 * @module
 */
export * as RelayPoolService from "./services/RelayPoolService.js"

/**
 * Automatic reconnection service for Nostr relays
 * @module
 */
export * as RelayReconnectService from "./services/RelayReconnectService.js"

/**
 * Nostr relay connection and subscription management
 * @module
 */
export * as RelayService from "./services/RelayService.js"

/**
 * WebSocket connection management service
 * @module
 */
export * as WebSocketService from "./services/WebSocketService.js"
