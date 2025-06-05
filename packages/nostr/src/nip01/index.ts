/**
 * NIP-01: Basic protocol flow
 * @module
 */

import { Layer } from "effect"
import { CryptoServiceLive } from "../services/CryptoService.js"
import { EventServiceLive } from "../services/EventService.js"
import { RelayServiceLive } from "../services/RelayService.js"
import { WebSocketServiceLive } from "../services/WebSocketService.js"

export * from "../core/Errors.js"
export * from "../core/Schema.js"
export * from "../services/CryptoService.js"
export * from "../services/EventService.js"
export * from "../services/RelayService.js"
export * from "../services/WebSocketService.js"

/**
 * Layer providing all NIP-01 services
 */
export const Nip01Live = Layer.mergeAll(
  CryptoServiceLive,
  EventServiceLive,
  WebSocketServiceLive,
  RelayServiceLive
)

/**
 * Convenience layer that provides EventService with its dependencies
 */
export const EventServiceWithDeps = EventServiceLive.pipe(
  Layer.provide(CryptoServiceLive)
)

/**
 * Convenience layer that provides RelayService with its dependencies
 */
export const RelayServiceWithDeps = RelayServiceLive.pipe(
  Layer.provide(WebSocketServiceLive)
)
