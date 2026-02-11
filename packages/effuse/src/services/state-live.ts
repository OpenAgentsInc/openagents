/**
 * StateService - Effect.Ref implementation
 */

import { makeCell } from "../state/cell.js"
import type { StateService } from "./state.js"

export const StateServiceLive: StateService = {
  cell: <A>(initial: A) => makeCell(initial),
}
