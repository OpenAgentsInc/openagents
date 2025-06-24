/**
 * Agent Profile Service - NIP-OA Agent Identity Management
 * Handles creation, updates, and queries for agent profiles
 */
export * as AgentProfileService from "./agent-profile/AgentProfileService.js"

/**
 * Browser entry point for @openagentsinc/nostr
 * Uses browser-compatible crypto implementations
 */
export * as browser from "./browser.js"

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
 * NIP-42: Authentication of clients to relays
 * @module
 */
export * as Nip42Service from "./nip42/Nip42Service.js"

/**
 * NIP-90: Data Vending Machine Service
 * Implements AI service marketplace with job request/result protocol
 * @module
 */
export * as Nip90Service from "./nip90/Nip90Service.js"

/**
 * NIP-02: Contact Lists and Petname System
 * Implements following/contact lists and the petname system for Nostr
 *
 * Contact Lists (kind 3):
 * - Stores a user's contact list as JSON in the content field
 * - Each contact has a public key and optional relay URL
 * - Uses p-tags for efficient querying
 *
 * Petname System:
 * - Maps public keys to human-readable names
 * - Local naming system independent of global usernames
 * - Supports hierarchical name resolution
 */
export * as nip02 from "./nips/nip02.js"

/**
 * NIP-04: Encrypted Direct Messages
 * Implements encrypted direct messages using ECDH and AES-256-CBC
 *
 * Message Events (kind 4):
 * - Encrypts message content using shared secret derived from ECDH
 * - Uses AES-256-CBC with random IV for encryption
 * - Includes recipient in p-tag for efficient querying
 * - Content field contains base64-encoded encrypted payload
 *
 * Security Features:
 * - Forward secrecy through ephemeral keys (optional)
 * - Authenticated encryption to prevent tampering
 * - Message padding to obscure length information
 */
export * as nip04 from "./nips/nip04.js"

/**
 * NIP-05: DNS-based Internet Identifiers
 * Maps human-readable names to Nostr public keys via DNS/HTTP
 *
 * Format: <local-part>@<domain>
 * Resolution: https://<domain>/.well-known/nostr.json?name=<local-part>
 */
export * as nip05 from "./nips/nip05.js"

/**
 * NIP-09: Event Deletion
 * Implements event deletion requests and content moderation
 *
 * Deletion Events (kind 5):
 * - Request deletion of previous events by the same author
 * - References events to delete using e-tags
 * - Optional deletion reason in content field
 * - Clients SHOULD honor deletion requests but MAY choose not to
 *
 * Security Considerations:
 * - Only the original author can request deletion of their events
 * - Relays and clients can implement their own deletion policies
 * - Deletion is a request, not a guarantee
 * - Some clients may preserve deleted content for archival purposes
 */
export * as nip09 from "./nips/nip09.js"

/**
 * NIP-19: bech32-encoded entities
 * Implements encoding and decoding of Nostr entities using bech32 format
 *
 * Supported entity types:
 * - npub (public keys)
 * - nsec (private keys)
 * - note (event IDs)
 * - nprofile (public key + relay hints)
 * - nevent (event ID + relay hints + author)
 * - naddr (parameterized replaceable event coordinates)
 * - nrelay (relay URLs)
 */
export * as nip19 from "./nips/nip19.js"

/**
 * NIP-44: Versioned Encryption
 * Implements versioned encryption as an upgrade to NIP-04
 *
 * Features:
 * - ChaCha20-Poly1305 AEAD encryption (more secure than AES-CBC)
 * - Proper HKDF key derivation from ECDH shared secret
 * - Versioned encryption format for future upgrades
 * - Built-in authentication and integrity checking
 * - Constant-time operations to prevent timing attacks
 *
 * Version 1 Format:
 * - Version byte (0x01)
 * - 32-byte nonce
 * - Variable-length ciphertext with 16-byte auth tag
 * - Base64 encoding for transmission
 */
export * as nip44 from "./nips/nip44.js"

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
