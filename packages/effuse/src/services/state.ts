/**
 * StateService - Creates StateCell instances
 */

import { Context, Effect, Scope } from "effect"
import type { StateCell } from "../state/cell.js"

export interface StateService {
  /**
   * Create a new StateCell with initial value
   */
  readonly cell: <A>(initial: A) => Effect.Effect<StateCell<A>, never, Scope.Scope>
}

export class StateServiceTag extends Context.Tag("@openagentsinc/effuse/StateService")<
  StateServiceTag,
  StateService
>() {}
