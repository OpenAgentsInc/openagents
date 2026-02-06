/**
 * Component system types
 */

import type { Effect, Stream } from "effect"
import type { StateCell } from "../state/cell.js"
import type { DomService } from "../services/dom.js"
import type { TemplateResult } from "../template/types.js"

/**
 * Context provided to component methods.
 */
export interface ComponentContext<S, E> {
  /** Reactive state cell */
  readonly state: StateCell<S>

  /** Emit an event from this component */
  readonly emit: (event: E) => Effect.Effect<void, never>

  /** DOM service for queries and rendering */
  readonly dom: DomService

  /** The container element this component is mounted to */
  readonly container: Element
}

/**
 * Component definition interface.
 */
export interface Component<S, E, R = never> {
  /** Unique component identifier (used for debugging) */
  readonly id: string

  /** Factory function for initial state */
  readonly initialState: () => S

  /**
   * Render the component to a TemplateResult.
   *
   * Called on initial mount and whenever state changes.
   * Should be a pure function of state.
   */
  readonly render: (
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<TemplateResult, never, R>

  /**
   * Handle events emitted by this component.
   *
   * Events are typically triggered by user interactions (clicks, inputs).
   * Handlers can update state, make requests, etc.
   */
  readonly handleEvent?: (
    event: E,
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<void, never, R>

  /**
   * Set up event listeners on the container after render.
   *
   * This is called once after mount. Use event delegation for
   * elements that may be re-rendered.
   */
  readonly setupEvents?: (
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<void, never, R>

  /**
   * External streams this component subscribes to.
   *
   * Each stream item is an Effect that updates component state.
   * Subscriptions are automatically cleaned up on unmount.
   */
  readonly subscriptions?: (
    ctx: ComponentContext<S, E>
  ) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}
