/**
 * Effuse Component Mount System
 *
 * Connects components to the DOM and manages their lifecycle.
 */

import { Effect, Queue, Stream, Scope, pipe } from "effect"
import type { Component, ComponentContext, MountedComponent } from "./types.js"
import { DomServiceTag } from "../services/dom.js"
import { StateServiceTag } from "../services/state.js"
import { loadComponentState, saveComponentState } from "../hmr/registry.js"

/**
 * Mount a component to a DOM container.
 *
 * This:
 * 1. Creates the component's state cell
 * 2. Performs initial render
 * 3. Sets up re-rendering on state changes
 * 4. Sets up event handling
 * 5. Starts any subscriptions
 *
 * The component is automatically cleaned up when the scope closes.
 *
 * @example
 * ```typescript
 * const program = Effect.gen(function* () {
 *   const container = document.getElementById("my-component")!
 *   yield* mountComponent(MyComponent, container)
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
export const mountComponent = <S, E, R>(
  component: Component<S, E, R>,
  container: Element
): Effect.Effect<MountedComponent<E>, never, R | DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const stateService = yield* StateServiceTag

    // Check for HMR preserved state, otherwise use initialState
    // Special handling for tbcc-testgen component migration
    let preservedState = loadComponentState<S>(component.id)

    // Migrate old state format for tbcc-testgen component
    if (component.id === "tbcc-testgen" && preservedState) {
      const oldState = preservedState as any
      // Check if it has old format (tests/reflections arrays) but not threadItems
      if (!oldState.threadItems &&
          ((oldState.tests && Array.isArray(oldState.tests)) ||
           (oldState.reflections && Array.isArray(oldState.reflections)))) {
        console.log("[Effuse] Migrating tbcc-testgen state from old format to thread format")
        const threadItems: any[] = []
        const now = Date.now()

        // Convert old tests array to thread items
        if (Array.isArray(oldState.tests)) {
          oldState.tests.forEach((test: any, index: number) => {
            threadItems.push({
              type: "test",
              timestamp: now - (oldState.tests.length - index) * 1000,
              data: test,
            })
          })
        }

        // Convert old reflections array to thread items
        if (Array.isArray(oldState.reflections)) {
          oldState.reflections.forEach((reflection: any, index: number) => {
            threadItems.push({
              type: "reflection",
              timestamp: now - (oldState.reflections.length - index) * 1000,
              data: reflection,
            })
          })
        }

        // Sort by timestamp
        threadItems.sort((a, b) => a.timestamp - b.timestamp)

        // Create migrated state
        const migratedState = {
          ...oldState,
          threadItems,
          expandedItemId: oldState.expandedItemId ?? null,
        }
        delete (migratedState as any).tests
        delete (migratedState as any).reflections

        preservedState = migratedState as S
      }
    }

    const initialState = preservedState ?? component.initialState()

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
    const ctx: ComponentContext<S, E> = {
      state,
      emit,
      dom,
      container,
    }

    // Initial render (catch errors and log)
    const initialContent = yield* component.render(ctx)
    yield* dom.render(container, initialContent).pipe(
      Effect.catchAll((error) => {
        console.error(`[Effuse] Initial render error for "${component.id}":`, error)
        return Effect.void
      })
    )

    // Set up events after initial render
    if (component.setupEvents) {
      yield* component.setupEvents(ctx)
    }

    // HMR: Continuously snapshot state for preservation across reloads
    yield* pipe(
      state.changes,
      Stream.tap((s) => Effect.sync(() => saveComponentState(component.id, s))),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Re-render on state changes
    yield* pipe(
      state.changes,
      Stream.tap(() =>
        Effect.gen(function* () {
          const content = yield* component.render(ctx)
          yield* dom.render(container, content).pipe(
            Effect.catchAll((error) => {
              console.error(`[Effuse] Re-render error for "${component.id}":`, error)
              return Effect.void
            })
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

    // Start subscriptions
    if (component.subscriptions) {
      const subs = component.subscriptions(ctx)
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
    const mounted: MountedComponent<E> = {
      unmount: Effect.void, // Unmount happens via scope
      events: Stream.fromQueue(eventQueue),
      emit,
    }

    return mounted
  })

/**
 * Mount a component to an element by ID.
 *
 * Convenience wrapper that queries for the element first.
 */
export const mountComponentById = <S, E, R>(
  component: Component<S, E, R>,
  containerId: string
): Effect.Effect<MountedComponent<E>, never, R | DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const container = yield* dom.queryId(containerId)
    return yield* mountComponent(component, container)
  }).pipe(
    Effect.catchAll((error) => {
      console.error(`[Effuse] Failed to mount component "${component.id}":`, error)
      // Return a dummy mounted component with empty streams
      return Effect.succeed({
        unmount: Effect.void,
        events: Stream.empty,
        emit: () => Effect.void,
      })
    })
  )

/**
 * Mount multiple components in parallel.
 *
 * @example
 * ```typescript
 * yield* mountComponents([
 *   { component: Component1, container: el1 },
 *   { component: Component2, container: el2 },
 * ])
 * ```
 */
export const mountComponents = (
  components: Array<{ component: Component<unknown, unknown, unknown>; container: Element }>
): Effect.Effect<MountedComponent[], never, DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.all(
    components.map(({ component, container }) => mountComponent(component, container))
  ) as unknown as Effect.Effect<MountedComponent[], never, DomServiceTag | StateServiceTag | Scope.Scope>
