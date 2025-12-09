/**
 * Effuse Component System Types
 *
 * Defines the Component interface and related types for building
 * Effect-native UI components.
 */

import type { Effect, Stream } from "effect"
import type { StateCell } from "../state/cell.js"
import type { DomService } from "../services/dom.js"
import type { TemplateResult } from "../template/types.js"

/**
 * Context provided to component methods.
 *
 * @template S - Component state type
 * @template E - Component event type
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
 *
 * Components are Effect-native UI elements that:
 * - Have typed state and events
 * - Render to TemplateResult
 * - Can subscribe to external streams (like HUD messages)
 * - Are fully testable via service mocking
 *
 * @template S - Component state type
 * @template E - Component event type (discriminated union recommended)
 * @template R - Effect requirements (services this component needs)
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
   *
   * Return a cleanup Effect that removes listeners.
   */
  readonly setupEvents?: (
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<void, never, R>

  /**
   * External streams this component subscribes to.
   *
   * Each stream item is an Effect that updates component state.
   * Subscriptions are automatically cleaned up on unmount.
   *
   * @example
   * ```typescript
   * subscriptions: (ctx) => [
   *   pipe(
   *     socketService.messages,
   *     Stream.filter(isTBRunStart),
   *     Stream.map(msg => ctx.state.update(s => ({ ...s, runId: msg.runId })))
   *   )
   * ]
   * ```
   */
  readonly subscriptions?: (
    ctx: ComponentContext<S, E>
  ) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}

/**
 * Mounted component handle.
 *
 * Returned by mountComponent, allows unmounting and accessing component events.
 */
export interface MountedComponent<E = never> {
  /** Unmount the component and clean up resources */
  readonly unmount: Effect.Effect<void, never>

  /** Stream of events emitted by this component */
  readonly events: Stream.Stream<E, never>

  /** Emit an event to this component */
  readonly emit: (event: E) => Effect.Effect<void, never>
}

/**
 * Helper type to extract state type from a Component.
 */
export type ComponentState<W> = W extends Component<infer S, unknown, unknown> ? S : never

/**
 * Helper type to extract event type from a Component.
 */
export type ComponentEvent<W> = W extends Component<unknown, infer E, unknown> ? E : never

/**
 * Helper type to extract requirements from a Component.
 */
export type ComponentRequirements<W> = W extends Component<unknown, unknown, infer R> ? R : never
