/**
 * Effuse State Service - Live Implementation
 *
 * Production implementation of StateService using Effect primitives.
 */

import { Layer } from "effect"
import { StateServiceTag, type StateService } from "./state.js"
import { makeCell } from "../state/cell.js"

/**
 * Live implementation of StateService
 */
const makeStateService = (): StateService => ({
  cell: makeCell,
})

/**
 * Layer providing the live StateService implementation
 */
export const StateServiceLive = Layer.succeed(
  StateServiceTag,
  makeStateService()
)
