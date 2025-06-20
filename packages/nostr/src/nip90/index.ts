/**
 * NIP-90: Data Vending Machine (AI Service Marketplace)
 * @module
 */

import { Layer } from "effect"
import { EventServiceLive } from "../services/EventService.js"
import { RelayServiceLive } from "../services/RelayService.js"
import { Nip90ServiceLive } from "./Nip90Service.js"

export * from "../core/Errors.js"
export * from "../core/Schema.js"
export * from "./Nip90Service.js"

/**
 * Layer providing all NIP-90 services with dependencies
 */
export const Nip90Live = Layer.mergeAll(
  EventServiceLive,
  RelayServiceLive,
  Nip90ServiceLive
)

/**
 * Convenience layer that provides Nip90Service with its dependencies
 */
export const Nip90ServiceWithDeps = Nip90ServiceLive.pipe(
  Layer.provide(EventServiceLive),
  Layer.provide(RelayServiceLive)
)
