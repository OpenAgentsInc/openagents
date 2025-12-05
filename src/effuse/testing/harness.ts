/**
 * Effuse TestHarness Service
 *
 * Effect-native widget testing harness.
 * Provides widget mounting with direct access to state and events.
 */

import { Context, Effect, Stream, type Scope } from "effect"
import type { Widget } from "../widget/types.js"
import type { HudMessage } from "../../hud/protocol.js"
import { StateServiceTag } from "../services/state.js"
import { TestError, type WaitOptions } from "./errors.js"

/**
 * Handle to a mounted widget for testing.
 *
 * Provides direct Effect-based access to widget internals:
 * - State (get, set, update, observe)
 * - Events (emit directly)
 * - DOM container
 *
 * @template S - Widget state type
 * @template E - Widget event type
 */
export interface WidgetHandle<S, E> {
  /** The DOM container element */
  readonly container: Element

  // ─────────────────────────────────────────────────────────────────
  // Direct State Access
  // ─────────────────────────────────────────────────────────────────

  /**
   * Get current widget state.
   */
  readonly getState: Effect.Effect<S, never>

  /**
   * Set widget state directly.
   */
  readonly setState: (state: S) => Effect.Effect<void, never>

  /**
   * Update widget state with a function.
   */
  readonly updateState: (f: (current: S) => S) => Effect.Effect<void, never>

  // ─────────────────────────────────────────────────────────────────
  // Event Emission
  // ─────────────────────────────────────────────────────────────────

  /**
   * Emit an event to the widget's event handler.
   * This bypasses DOM - directly triggers handleEvent.
   */
  readonly emit: (event: E) => Effect.Effect<void, never>

  // ─────────────────────────────────────────────────────────────────
  // Observable State
  // ─────────────────────────────────────────────────────────────────

  /**
   * Stream of state changes.
   * Use with Stream utilities for reactive testing.
   */
  readonly stateChanges: Stream.Stream<S, never>

  /**
   * Wait for state to match a predicate.
   * Polls state changes until predicate returns true or timeout.
   */
  readonly waitForState: (
    predicate: (state: S) => boolean,
    options?: WaitOptions
  ) => Effect.Effect<S, TestError>

  // ─────────────────────────────────────────────────────────────────
  // DOM Access
  // ─────────────────────────────────────────────────────────────────

  /**
   * Get current rendered HTML of the widget container.
   */
  readonly getHTML: Effect.Effect<string, never>

  /**
   * Wait for a re-render to complete.
   * Useful after state changes to ensure DOM is updated.
   */
  readonly waitForRender: Effect.Effect<void, never>
}

/**
 * Service interface for test harness operations.
 *
 * Provides:
 * - Widget mounting with state/event access
 * - HUD message injection (for socket subscriptions)
 * - Cleanup
 */
export interface TestHarness {
  /**
   * Mount a widget for testing.
   *
   * Returns a WidgetHandle with direct access to state and events.
   *
   * @example
   * ```typescript
   * const handle = yield* harness.mount(TBControlsWidget)
   * yield* handle.emit({ type: "loadSuite", path: "/test.json" })
   * yield* handle.waitForState(s => s.loading === true)
   * ```
   */
  readonly mount: <S, E, R>(
    widget: Widget<S, E, R>,
    options?: {
      /** Custom container ID (default: auto-generated) */
      containerId?: string
      /** Initial state override */
      initialState?: S
    }
  ) => Effect.Effect<WidgetHandle<S, E>, TestError, R | StateServiceTag | Scope.Scope>

  // ─────────────────────────────────────────────────────────────────
  // Socket Message Injection
  // ─────────────────────────────────────────────────────────────────

  /**
   * Inject a HUD message into widget subscriptions.
   * Simulates messages from the socket server.
   */
  readonly injectMessage: (msg: HudMessage) => Effect.Effect<void, never>

  /**
   * Inject a sequence of messages with delay between each.
   */
  readonly injectSequence: (
    messages: readonly HudMessage[],
    delayMs?: number
  ) => Effect.Effect<void, never>

  // ─────────────────────────────────────────────────────────────────
  // Lifecycle
  // ─────────────────────────────────────────────────────────────────

  /**
   * Clean up all mounted widgets and resources.
   * Called automatically when scope closes.
   */
  readonly cleanup: Effect.Effect<void, never>
}

/**
 * Effect Context.Tag for TestHarness service.
 */
export class TestHarnessTag extends Context.Tag("effuse/testing/TestHarness")<
  TestHarnessTag,
  TestHarness
>() {}
