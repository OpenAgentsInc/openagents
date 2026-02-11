/**
 * Component mount system
 */

import { Effect, Queue, Scope, Stream, pipe } from "effect"
import type { Component, ComponentContext } from "./types.js"
import { DomServiceTag } from "../services/dom.js"
import { StateServiceTag } from "../services/state.js"

const formatUnknown = (value: unknown): string =>
  value instanceof Error ? value.message : String(value)

export interface MountedComponent<E> {
  readonly emit: (event: E) => Effect.Effect<void, never>
}

/**
 * Mount a component to a DOM container.
 */
export const mountComponent = <S, E, R>(
  component: Component<S, E, R>,
  container: Element
): Effect.Effect<MountedComponent<E>, never, R | DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const stateService = yield* StateServiceTag

    // Create state cell with initial state
    const state = yield* stateService.cell(component.initialState())

    // Create event queue
    const eventQueue = yield* Effect.acquireRelease(
      Queue.unbounded<E>(),
      (queue) => Queue.shutdown(queue)
    )

    // Create emit function
    const emit = (event: E) =>
      Queue.offer(eventQueue, event).pipe(
        Effect.asVoid // unbounded queue never fails; offer may return false when shutdown
      )

    // Build context
    const ctx: ComponentContext<S, E> = {
      state,
      emit,
      dom,
      container,
    }

    // Initial render
    const initialContent = yield* component.render(ctx)
    yield* dom.render(container, initialContent).pipe(
      Effect.catchAll((error) =>
        Effect.logError(
          `[Effuse] Initial render error for "${component.id}": ${formatUnknown(error)}`
        )
      )
    )

    // Set up events after initial render
    if (component.setupEvents) {
      yield* component.setupEvents(ctx)
    }

    // Re-render on state changes
    yield* pipe(
      state.changes,
      Stream.tap(() =>
        Effect.gen(function* () {
          const content = yield* component.render(ctx)
          yield* dom.render(container, content).pipe(
            Effect.catchAll((error) =>
              Effect.logError(
                `[Effuse] Re-render error for "${component.id}": ${formatUnknown(error)}`
              )
            )
          )
        })
      ),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Handle events
    if (component.handleEvent) {
      yield* pipe(
        Stream.fromQueue(eventQueue),
        Stream.tap((event) => component.handleEvent!(event, ctx)),
        Stream.runDrain,
        Effect.forkScoped
      )
    }

    // Subscriptions
    if (component.subscriptions) {
      for (const sub of component.subscriptions(ctx)) {
        yield* pipe(
          sub,
          Stream.tap((effect) => effect),
          Stream.runDrain,
          Effect.forkScoped
        )
      }
    }

    return { emit }
  })
