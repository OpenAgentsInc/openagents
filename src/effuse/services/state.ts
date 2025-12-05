/**
 * Effuse State Service
 *
 * Effect service for creating and managing reactive state cells.
 */

import { Context, Effect, Scope } from "effect"
import type { StateCell } from "../state/cell.js"

/**
 * Service interface for state management.
 */
export interface StateService {
  /**
   * Create a new reactive state cell.
   * The cell is automatically cleaned up when the scope closes.
   */
  readonly cell: <A>(initial: A) => Effect.Effect<StateCell<A>, never, Scope.Scope>
}

/**
 * Effect Context.Tag for StateService
 */
export class StateServiceTag extends Context.Tag("effuse/StateService")<
  StateServiceTag,
  StateService
>() {}
