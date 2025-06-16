/**
 * NIP-06: Basic key derivation from mnemonic seed phrase
 * @module
 */

import { Layer } from "effect"
import { CryptoServiceLive } from "../services/CryptoService.js"
import { Nip06ServiceLive } from "./Nip06Service.js"

export * from "../core/Errors.js"
export * from "../core/Schema.js"
export * from "./Nip06Service.js"

/**
 * Layer providing all NIP-06 services
 */
export const Nip06Live = Layer.mergeAll(
  CryptoServiceLive,
  Nip06ServiceLive
)

/**
 * Convenience layer that provides Nip06Service with its dependencies
 */
export const Nip06ServiceWithDeps = Nip06ServiceLive.pipe(
  Layer.provide(CryptoServiceLive)
)