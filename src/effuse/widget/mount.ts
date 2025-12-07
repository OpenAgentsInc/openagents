/**
 * Effuse Widget Mount System
 *
 * Connects widgets to the DOM and manages their lifecycle.
 */

import { Effect, Queue, Stream, Scope, pipe } from "effect"
import type { Widget, WidgetContext, MountedWidget } from "./types.js"
import { DomServiceTag } from "../services/dom.js"
import { StateServiceTag } from "../services/state.js"
import { loadWidgetState, saveWidgetState } from "../hmr/registry.js"

/**
 * Mount a widget to a DOM container.
 *
 * This:
 * 1. Creates the widget's state cell
 * 2. Performs initial render
 * 3. Sets up re-rendering on state changes
 * 4. Sets up event handling
 * 5. Starts any subscriptions
 *
 * The widget is automatically cleaned up when the scope closes.
 *
 * @example
 * ```typescript
 * const program = Effect.gen(function* () {
 *   const container = document.getElementById("my-widget")!
 *   yield* mountWidget(MyWidget, container)
 * })
 *
 * Effect.runPromise(
 *   program.pipe(
 *     Effect.provide(EffuseLive),
 *     Effect.scoped
 *   )
 * )
 * ```
 */
export const mountWidget = <S, E, R>(
  widget: Widget<S, E, R>,
  container: Element
): Effect.Effect<MountedWidget<E>, never, R | DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const stateService = yield* StateServiceTag

    // Check for HMR preserved state, otherwise use initialState
    const preservedState = loadWidgetState<S>(widget.id)
    const initialState = preservedState ?? widget.initialState()

    // Create state cell with initial or preserved state
    const state = yield* stateService.cell(initialState)

    // Create event queue
    const eventQueue = yield* Effect.acquireRelease(
      Queue.unbounded<E>(),
      (queue) => Queue.shutdown(queue)
    )

    // Create emit function
    const emit = (event: E) =>
      Queue.offer(eventQueue, event).pipe(
        Effect.catchAll(() => Effect.void) // Silently ignore queue full/shutdown
      )

    // Build context
    const ctx: WidgetContext<S, E> = {
      state,
      emit,
      dom,
      container,
    }

    // Initial render (catch errors and log)
    const initialContent = yield* widget.render(ctx)
    yield* dom.render(container, initialContent).pipe(
      Effect.catchAll((error) => {
        console.error(`[Effuse] Initial render error for "${widget.id}":`, error)
        return Effect.void
      })
    )

    // Set up events after initial render
    if (widget.setupEvents) {
      yield* widget.setupEvents(ctx)
    }

    // HMR: Continuously snapshot state for preservation across reloads
    yield* pipe(
      state.changes,
      Stream.tap((s) => Effect.sync(() => saveWidgetState(widget.id, s))),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Re-render on state changes
    yield* pipe(
      state.changes,
      Stream.tap(() =>
        Effect.gen(function* () {
          const content = yield* widget.render(ctx)
          yield* dom.render(container, content).pipe(
            Effect.catchAll((error) => {
              console.error(`[Effuse] Re-render error for "${widget.id}":`, error)
              return Effect.void
            })
          )
        })
      ),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Handle events
    if (widget.handleEvent) {
      yield* pipe(
        Stream.fromQueue(eventQueue),
        Stream.tap((event) => widget.handleEvent!(event, ctx)),
        Stream.runDrain,
        Effect.forkScoped
      )
    }

    // Start subscriptions
    if (widget.subscriptions) {
      const subs = widget.subscriptions(ctx)
      for (const sub of subs) {
        yield* pipe(
          sub,
          Stream.tap((effect) => effect),
          Stream.runDrain,
          Effect.forkScoped
        )
      }
    }

    // Return handle for manual unmount and event access
    const mounted: MountedWidget<E> = {
      unmount: Effect.void, // Unmount happens via scope
      events: Stream.fromQueue(eventQueue),
      emit,
    }

    return mounted
  })

/**
 * Mount a widget to an element by ID.
 *
 * Convenience wrapper that queries for the element first.
 */
export const mountWidgetById = <S, E, R>(
  widget: Widget<S, E, R>,
  containerId: string
): Effect.Effect<MountedWidget<E>, never, R | DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const container = yield* dom.queryId(containerId)
    return yield* mountWidget(widget, container)
  }).pipe(
    Effect.catchAll((error) => {
      console.error(`[Effuse] Failed to mount widget "${widget.id}":`, error)
      // Return a dummy mounted widget with empty streams
      return Effect.succeed({
        unmount: Effect.void,
        events: Stream.empty,
        emit: () => Effect.void,
      })
    })
  )

/**
 * Mount multiple widgets in parallel.
 *
 * @example
 * ```typescript
 * yield* mountWidgets([
 *   { widget: Widget1, container: el1 },
 *   { widget: Widget2, container: el2 },
 * ])
 * ```
 */
export const mountWidgets = (
  widgets: Array<{ widget: Widget<unknown, unknown, unknown>; container: Element }>
): Effect.Effect<MountedWidget[], never, DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.all(
    widgets.map(({ widget, container }) => mountWidget(widget, container))
  ) as Effect.Effect<MountedWidget[], never, DomServiceTag | StateServiceTag | Scope.Scope>
