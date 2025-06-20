/**
 * NIP-28: Public Chat Channels
 * @module
 */

import { Layer } from "effect"
import { CryptoServiceLive } from "../services/CryptoService.js"
import { EventServiceLive } from "../services/EventService.js"
import { RelayServiceLive } from "../services/RelayService.js"
import { Nip28ServiceLive } from "./Nip28Service.js"

export * from "../core/Errors.js"
export * from "../core/Schema.js"
export * from "./Nip28Service.js"

/**
 * Layer providing all NIP-28 services with dependencies
 */
export const Nip28Live = Layer.mergeAll(
  CryptoServiceLive,
  EventServiceLive,
  RelayServiceLive,
  Nip28ServiceLive
)

/**
 * Convenience layer that provides Nip28Service with its dependencies
 */
export const Nip28ServiceWithDeps = Nip28ServiceLive.pipe(
  Layer.provide(CryptoServiceLive),
  Layer.provide(EventServiceLive),
  Layer.provide(RelayServiceLive)
)
