/**
 * Browser entry point for @openagentsinc/nostr
 * Uses browser-compatible crypto implementations
 */

// Service exports
export * as AgentProfileService from "./agent-profile/AgentProfileService.js"
export * as Errors from "./core/Errors.js"
export * as Schema from "./core/Schema.js"
export * as Nip06Service from "./nip06/Nip06Service.js"
export * as Nip28Service from "./nip28/Nip28Service.js"
export * as Nip90Service from "./nip90/Nip90Service.js"

// NIP exports
export * as nip02 from "./nips/nip02.js"
export * as nip05 from "./nips/nip05.js"
export * as nip09 from "./nips/nip09.js"
export * as nip19 from "./nips/nip19.js"

// Use browser-compatible versions for crypto-dependent NIPs
export * as nip04 from "./nips/nip04-browser.js"
export * as nip44 from "./nips/nip44-browser.js"

// Standard services
export * as CryptoService from "./services/CryptoService.js"
export * as EventService from "./services/EventService.js"
export * as RelayPoolService from "./services/RelayPoolService.js"
export * as RelayReconnectService from "./services/RelayReconnectService.js"
export * as RelayService from "./services/RelayService.js"
export * as WebSocketService from "./services/WebSocketService.js"
