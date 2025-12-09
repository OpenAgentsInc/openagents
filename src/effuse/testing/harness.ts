/**
 * Effuse TestHarness Service
 *
 * Effect-native component testing harness.
 * Provides component mounting with direct access to state and events.
 */

import { Context, Effect, Stream, type Scope } from "effect"
import type { Component } from "../component/types.js"
import type { HudMessage } from "../../hud/protocol.js"
import { StateServiceTag } from "../services/state.js"
import { TestError, type WaitOptions } from "./errors.js"

/**
 * Handle to a mounted component for testing.
 *
 * Provides direct Effect-based access to component internals:
 * - State (get, set, update, observe)
 * - Events (emit directly)
 * - DOM container
 *
 * @template S - Component state type
 * @template E - Component event type
 */
export interface ComponentHandle<S, E> {
  /** The DOM container element */
  readonly container: Element

  // ─────────────────────────────────────────────────────────────────
  // Direct State Access
  // ─────────────────────────────────────────────────────────────────

  /**
   * Get current component state.
   */
  readonly getState: Effect.Effect<S, never>

  /**
   * Set component state directly.
   */
  readonly setState: (state: S) => Effect.Effect<void, never>

  /**
   * Update component state with a function.
   */
  readonly updateState: (f: (current: S) => S) => Effect.Effect<void, never>

  // ─────────────────────────────────────────────────────────────────
  // Event Emission
  // ─────────────────────────────────────────────────────────────────

  /**
   * Emit an event to the component's event handler.
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
   * Get current rendered HTML of the component container.
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
 * - Component mounting with state/event access
 * - HUD message injection (for socket subscriptions)
 * - Cleanup
 */
export interface TestHarness {
  /**
   * Mount a component for testing.
   *
   * Returns a ComponentHandle with direct access to state and events.
   *
   * @example
   * ```typescript
   * const handle = yield* harness.mount(TBControlsComponent)
   * yield* handle.emit({ type: "loadSuite", path: "/test.json" })
   * yield* handle.waitForState(s => s.loading === true)
   * ```
   */
  readonly mount: <S, E, R>(
    component: Component<S, E, R>,
    options?: {
      /** Custom container ID (default: auto-generated) */
      containerId?: string
      /** Initial state override */
      initialState?: S
    }
  ) => Effect.Effect<ComponentHandle<S, E>, TestError, R | StateServiceTag | Scope.Scope>

  // ─────────────────────────────────────────────────────────────────
  // Socket Message Injection
  // ─────────────────────────────────────────────────────────────────

  /**
   * Inject a HUD message into component subscriptions.
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
   * Clean up all mounted components and resources.
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
