import { describe, expect, it } from "vitest"
import { Effect, Stream } from "effect"
import {
  DomServiceLive,
  DomServiceTag,
  StateServiceLive,
  StateServiceTag,
  html,
  mountComponent,
  type Component,
} from "../src/index.ts"

describe("mountComponent (contract)", () => {
  it("renders initially and re-renders on events/state changes", async () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    type E = "inc"

    const component: Component<number, E> = {
      id: "counter",
      initialState: () => 0,
      render: ({ state }) =>
        Effect.gen(function* () {
          const n = yield* state.get
          return html`<div data-count="${String(n)}">count:${String(n)}</div>`
        }),
      handleEvent: (event, { state }) =>
        event === "inc" ? state.update((n) => n + 1) : Effect.void,
    }

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mounted = yield* mountComponent(component, container)

          expect(container.innerHTML).toContain("count:0")

          yield* mounted.emit("inc")
          // give the event + re-render fibers a tick
          yield* Effect.sleep("10 millis")

          expect(container.innerHTML).toContain("count:1")
        }).pipe(
          Effect.provideService(DomServiceTag, DomServiceLive),
          Effect.provideService(StateServiceTag, StateServiceLive)
        )
      )
    )

    container.remove()
  })

  it("cleans up subscriptions on scope close", async () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    let acquired = false
    let released = false

    const component: Component<number, never> = {
      id: "subscribed",
      initialState: () => 0,
      render: () => Effect.succeed(html`<div>ok</div>`),
      subscriptions: () => [
        Stream.acquireRelease(
          Effect.sync(() => {
            acquired = true
          }),
          () =>
            Effect.sync(() => {
              released = true
            })
        ).pipe(Stream.flatMap(() => Stream.never)),
      ],
    }

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          yield* mountComponent(component, container)
          yield* Effect.sleep("10 millis")
          expect(acquired).toBe(true)
          expect(released).toBe(false)
        }).pipe(
          Effect.provideService(DomServiceTag, DomServiceLive),
          Effect.provideService(StateServiceTag, StateServiceLive)
        )
      )
    )

    expect(released).toBe(true)
    container.remove()
  })
})

