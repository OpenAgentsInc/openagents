/**
 * Effuse Live Layer
 *
 * Production layer providing all Effuse services for browser use.
 */

import { Layer } from "effect"
import { DomServiceLive } from "../services/dom-live.js"
import { StateServiceLive } from "../services/state-live.js"
import { SocketServiceDefault } from "../services/socket-live.js"

/**
 * Full production layer with all services.
 *
 * Includes:
 * - DomService (browser DOM operations)
 * - StateService (reactive state cells)
 * - SocketService (desktop server communication)
 */
export const EffuseLive = Layer.mergeAll(
  DomServiceLive,
  StateServiceLive,
  SocketServiceDefault
)

/**
 * Production layer without socket (for standalone UI testing).
 */
export const EffuseLiveNoSocket = Layer.mergeAll(DomServiceLive, StateServiceLive)

/**
 * Type helper for the full live layer.
 */
export type EffuseLiveLayer = typeof EffuseLive
