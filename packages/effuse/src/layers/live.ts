/**
 * Production layers
 */

import { Layer } from "effect"
import { DomServiceTag } from "../services/dom.js"
import { DomServiceLive } from "../services/dom-live.js"
import { StateServiceTag } from "../services/state.js"
import { StateServiceLive } from "../services/state-live.js"

/**
 * Production layer with all services (browser)
 */
export const EffuseLive = Layer.mergeAll(
  Layer.succeed(DomServiceTag, DomServiceLive),
  Layer.succeed(StateServiceTag, StateServiceLive)
)
