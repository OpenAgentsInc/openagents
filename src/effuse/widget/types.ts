/**
 * Effuse Widget System Types
 *
 * Defines the Widget interface and related types for building
 * Effect-native UI components.
 */

import type { Effect, Stream } from "effect"
import type { StateCell } from "../state/cell.js"
import type { DomService } from "../services/dom.js"
import type { TemplateResult } from "../template/types.js"

/**
 * Context provided to widget methods.
 *
 * @template S - Widget state type
 * @template E - Widget event type
 */
export interface WidgetContext<S, E> {
  /** Reactive state cell */
  readonly state: StateCell<S>

  /** Emit an event from this widget */
  readonly emit: (event: E) => Effect.Effect<void, never>

  /** DOM service for queries and rendering */
  readonly dom: DomService

  /** The container element this widget is mounted to */
  readonly container: Element
}

/**
 * Widget definition interface.
 *
 * Widgets are Effect-native UI components that:
 * - Have typed state and events
 * - Render to TemplateResult
 * - Can subscribe to external streams (like HUD messages)
 * - Are fully testable via service mocking
 *
 * @template S - Widget state type
 * @template E - Widget event type (discriminated union recommended)
 * @template R - Effect requirements (services this widget needs)
 */
export interface Widget<S, E, R = never> {
  /** Unique widget identifier (used for debugging) */
  readonly id: string

  /** Factory function for initial state */
  readonly initialState: () => S

  /**
   * Render the widget to a TemplateResult.
   *
   * Called on initial mount and whenever state changes.
   * Should be a pure function of state.
   */
  readonly render: (
    ctx: WidgetContext<S, E>
  ) => Effect.Effect<TemplateResult, never, R>

  /**
   * Handle events emitted by this widget.
   *
   * Events are typically triggered by user interactions (clicks, inputs).
   * Handlers can update state, make requests, etc.
   */
  readonly handleEvent?: (
    event: E,
    ctx: WidgetContext<S, E>
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
    ctx: WidgetContext<S, E>
  ) => Effect.Effect<void, never, R>

  /**
   * External streams this widget subscribes to.
   *
   * Each stream item is an Effect that updates widget state.
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
    ctx: WidgetContext<S, E>
  ) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}

/**
 * Mounted widget handle.
 *
 * Returned by mountWidget, allows unmounting and accessing widget events.
 */
export interface MountedWidget<E = never> {
  /** Unmount the widget and clean up resources */
  readonly unmount: Effect.Effect<void, never>

  /** Stream of events emitted by this widget */
  readonly events: Stream.Stream<E, never>

  /** Emit an event to this widget */
  readonly emit: (event: E) => Effect.Effect<void, never>
}

/**
 * Helper type to extract state type from a Widget.
 */
export type WidgetState<W> = W extends Widget<infer S, unknown, unknown> ? S : never

/**
 * Helper type to extract event type from a Widget.
 */
export type WidgetEvent<W> = W extends Widget<unknown, infer E, unknown> ? E : never

/**
 * Helper type to extract requirements from a Widget.
 */
export type WidgetRequirements<W> = W extends Widget<unknown, unknown, infer R> ? R : never
